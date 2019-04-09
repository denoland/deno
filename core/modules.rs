// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Implementation note: one could imagine combining this module with Isolate to
// provide a more intuitive high-level API. However, due to the complexity
// inherent in asynchronous module loading, we would like the Isolate to remain
// small and simple for users who do not use modules or if they do can load them
// synchronously. The isolate.rs module should never depend on this module.

use crate::isolate::Isolate;
use crate::js_errors::JSError;
use crate::libdeno::deno_mod;
use futures::Async;
use futures::Future;
use futures::Poll;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub type SourceCodeFuture<E> = dyn Future<Item = String, Error = E> + Send;

pub trait Loader {
  type Dispatch: crate::isolate::Dispatch;
  type Error: std::error::Error + 'static;

  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(specifier: &str, referrer: &str) -> String;

  /// Given an absolute url, load its source code.
  fn load(&mut self, url: &str) -> Box<SourceCodeFuture<Self::Error>>;

  fn isolate_and_modules<'a: 'b + 'c, 'b, 'c>(
    &'a mut self,
  ) -> (&'b mut Isolate<Self::Dispatch>, &'c mut Modules);

  fn isolate<'a: 'b, 'b>(&'a mut self) -> &'b mut Isolate<Self::Dispatch> {
    let (isolate, _) = self.isolate_and_modules();
    isolate
  }

  fn modules<'a: 'b, 'b>(&'a mut self) -> &'b mut Modules {
    let (_, modules) = self.isolate_and_modules();
    modules
  }
}

// TODO(ry) This is basically the same thing as RustOrJsError. They should be
// combined into one type.
pub enum Either<E> {
  JSError(JSError),
  Other(E),
}

/// This future is used to implement parallel async module loading without
/// complicating the Isolate API.
pub struct RecursiveLoad<'l, L: Loader> {
  loader: &'l mut L,
  pending: HashMap<String, Box<SourceCodeFuture<<L as Loader>::Error>>>,
  root: String,
}

impl<'l, L: Loader> RecursiveLoad<'l, L> {
  /// Starts a new parallel load of the given URL.
  pub fn new(url: &str, loader: &'l mut L) -> Self {
    let root = L::resolve(url, ".");
    let mut recursive_load = Self {
      loader,
      root: root.clone(),
      pending: HashMap::new(),
    };
    recursive_load
      .pending
      .insert(root.clone(), recursive_load.loader.load(&root));
    recursive_load
  }
}

impl<'l, L: Loader> Future for RecursiveLoad<'l, L> {
  type Item = deno_mod;
  type Error = Either<L::Error>;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let loader = &mut self.loader;
    let pending = &mut self.pending;
    let root = self.root.as_str();

    // Find all finished futures (those that are ready or that have errored).
    // Turn it into a list of (url, source_code) tuples.
    let mut finished_loads: Vec<(String, String)> = pending
      .iter_mut()
      .filter_map(|(url, fut)| match fut.poll() {
        Ok(Async::NotReady) => None,
        Ok(Async::Ready(source_code)) => Some(Ok((url.clone(), source_code))),
        Err(err) => Some(Err(Either::Other(err))),
      }).collect::<Result<_, _>>()?;

    while !finished_loads.is_empty() {
      // Instantiate and register the loaded modules, and discover new imports.
      // Build a list of (parent_url, Vec<child_url>) tuples.
      let parent_and_child_urls: Vec<(&str, Vec<String>)> = finished_loads
        .iter()
        .map(|(url, source_code)| {
          // Instantiate and register the module.
          let mod_id = loader
            .isolate()
            .mod_new(url == root, &url, &source_code)
            .map_err(Either::JSError)?;
          loader.modules().register(mod_id, &url);

          // Find child modules imported by the newly registered module.
          // Resolve all child import specifiers to URLs. Register all
          // imports as a children; however any modules that are already
          // known to the modules registry won't be stored in `child_urls`.
          let child_urls: Vec<String> = loader
            .isolate()
            .mod_get_imports(mod_id)
            .into_iter()
            .map(|specifier| L::resolve(&specifier, &url))
            .filter(|child_url| !loader.modules().add_child(mod_id, &child_url))
            .collect();
          Ok((url.as_str(), child_urls))
        }).collect::<Result<_, _>>()?;

      // Make updates to the `pending` hash map. If we find any more finished
      // futures, we'll loop and process `finished_loads` again.
      finished_loads = parent_and_child_urls
        .into_iter()
        .flat_map(|(url, child_urls)| {
          // Remove the parent module url that is done loading from `pending`.
          pending.remove(url);

          // Look for newly discovered child module imports.
          child_urls
            .into_iter()
            .filter_map(|child_url| {
              // If the url isn't present in the pending load table, create a
              // load future and associate it with the url in the hash map.
              match pending.entry(child_url.clone()) {
                Entry::Occupied(_) => None,
                Entry::Vacant(entry) => {
                  Some(entry.insert(Box::new(loader.load(&child_url))).poll())
                }
              }
              // Immediately poll any newly created futures and gather the
              // ones that are immediately ready or errored.
              .and_then(|poll_result| match poll_result {
                Ok(Async::NotReady) => None,
                Ok(Async::Ready(source_code)) => {
                  Some(Ok((child_url.clone(), source_code)))
                }
                Err(err) => Some(Err(Either::Other(err))),
              })
            }).collect::<Vec<_>>()
        }).collect::<Result<_, _>>()?;
    }

    if !self.pending.is_empty() {
      return Ok(Async::NotReady);
    }

    let (isolate, modules) = loader.isolate_and_modules();
    let root_id = modules.get_id(root).unwrap();
    let mut resolve = |specifier: &str, referrer_id: deno_mod| -> deno_mod {
      let referrer = modules.get_name(referrer_id).unwrap();
      let url = L::resolve(specifier, referrer);
      match modules.get_id(&url) {
        Some(id) => id,
        None => 0,
      }
    };
    isolate
      .mod_instantiate(root_id, &mut resolve)
      .map_err(Either::JSError)?;

    Ok(Async::Ready(root_id))
  }
}

struct ModuleInfo {
  name: String,
  children: Vec<String>,
}

impl ModuleInfo {
  fn has_child(&self, child_name: &str) -> bool {
    for c in self.children.iter() {
      if c == child_name {
        return true;
      }
    }
    false
  }
}

/// A collection of JS modules.
#[derive(Default)]
pub struct Modules {
  info: HashMap<deno_mod, ModuleInfo>,
  by_name: HashMap<String, deno_mod>,
}

impl Modules {
  pub fn new() -> Modules {
    Self {
      info: HashMap::new(),
      by_name: HashMap::new(),
    }
  }

  pub fn get_id(&self, name: &str) -> Option<deno_mod> {
    self.by_name.get(name).cloned()
  }

  pub fn get_children(&self, id: deno_mod) -> Option<&Vec<String>> {
    self.info.get(&id).map(|i| &i.children)
  }

  pub fn get_children2(&self, name: &str) -> Option<&Vec<String>> {
    self.get_id(name).and_then(|id| self.get_children(id))
  }

  pub fn get_name(&self, id: deno_mod) -> Option<&str> {
    self.info.get(&id).map(|i| i.name.as_str())
  }

  pub fn is_registered(&self, name: &str) -> bool {
    self.by_name.get(name).is_some()
  }

  // Returns true if the child name is a registered module, false otherwise.
  pub fn add_child(&mut self, parent_id: deno_mod, child_name: &str) -> bool {
    let parent = self.info.get_mut(&parent_id).unwrap();
    if !parent.has_child(&child_name) {
      parent.children.push(child_name.to_string());
    }
    self.is_registered(child_name)
  }

  pub fn register(&mut self, id: deno_mod, name: &str) {
    let name = String::from(name);
    debug!("register_complete {}", name);

    let _r = self.by_name.insert(name.clone(), id);
    // TODO should this be an assert or not ? assert!(r.is_none());

    self.info.insert(
      id,
      ModuleInfo {
        name,
        children: Vec::new(),
      },
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate::js_check;
  use crate::isolate::tests::*;

  struct MockLoader {
    pub loads: Vec<String>,
    pub isolate: Isolate<TestDispatch>,
    pub modules: Modules,
  }

  impl MockLoader {
    fn new() -> Self {
      let modules = Modules::new();
      let isolate = TestDispatch::setup(TestDispatchMode::AsyncImmediate);
      Self {
        loads: Vec::new(),
        isolate,
        modules,
      }
    }
  }

  impl Loader for MockLoader {
    type Dispatch = TestDispatch;
    type Error = std::io::Error;

    fn resolve(specifier: &str, _referrer: &str) -> String {
      specifier.to_string()
    }

    fn load(&mut self, url: &str) -> Box<SourceCodeFuture<Self::Error>> {
      use std::io::{Error, ErrorKind};
      self.loads.push(url.to_string());
      let result = match url {
        "a.js" => Ok(A_SRC),
        "b.js" => Ok(B_SRC),
        "c.js" => Ok(C_SRC),
        "d.js" => Ok(D_SRC),
        "circular1.js" => Ok(CIRCULAR1_SRC),
        "circular2.js" => Ok(CIRCULAR2_SRC),
        _ => Err(Error::new(ErrorKind::Other, "oh no!")),
      };
      let result = result.map(|src| src.to_string());
      Box::new(futures::future::result(result))
    }

    fn isolate_and_modules<'a: 'b + 'c, 'b, 'c>(
      &'a mut self,
    ) -> (&'b mut Isolate<Self::Dispatch>, &'c mut Modules) {
      (&mut self.isolate, &mut self.modules)
    }
  }

  const A_SRC: &str = r#"
    import { b } from "b.js";
    import { c } from "c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'a.js') throw Error();
  "#;

  const B_SRC: &str = r#"
    import { c } from "c.js";
    if (c() != 'c') throw Error();
    export function b() { return 'b'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'b.js') throw Error();
  "#;

  const C_SRC: &str = r#"
    import { d } from "d.js";
    export function c() { return 'c'; }
    if (d() != 'd') throw Error();
    if (import.meta.main) throw Error();
    if (import.meta.url != 'c.js') throw Error();
  "#;

  const D_SRC: &str = r#"
    export function d() { return 'd'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'd.js') throw Error();
  "#;

  #[test]
  fn test_recursive_load() {
    let mut loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("a.js", &mut loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    if let Async::Ready(a_id) = result.ok().unwrap() {
      js_check(loader.isolate.mod_evaluate(a_id));
      assert_eq!(loader.loads, vec!["a.js", "b.js", "c.js", "d.js"]);

      let modules = &loader.modules;

      assert_eq!(modules.get_id("a.js"), Some(a_id));
      let b_id = modules.get_id("b.js").unwrap();
      let c_id = modules.get_id("c.js").unwrap();
      let d_id = modules.get_id("d.js").unwrap();

      assert_eq!(
        modules.get_children(a_id),
        Some(&vec!["b.js".to_string(), "c.js".to_string()])
      );
      assert_eq!(modules.get_children(b_id), Some(&vec!["c.js".to_string()]));
      assert_eq!(modules.get_children(c_id), Some(&vec!["d.js".to_string()]));
      assert_eq!(modules.get_children(d_id), Some(&vec![]));
    } else {
      panic!("Future should be ready")
    }
  }

  const CIRCULAR1_SRC: &str = r#"
    import "circular2.js";
    Deno.core.print("circular1");
  "#;

  const CIRCULAR2_SRC: &str = r#"
    import "circular1.js";
    Deno.core.print("circular2");
  "#;

  #[test]
  fn test_circular_load() {
    let mut loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("circular1.js", &mut loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    if let Async::Ready(circular1_id) = result.ok().unwrap() {
      js_check(loader.isolate.mod_evaluate(circular1_id));
      assert_eq!(loader.loads, vec!["circular1.js", "circular2.js"]);

      let modules = &loader.modules;

      assert_eq!(modules.get_id("circular1.js"), Some(circular1_id));
      let circular2_id = modules.get_id("circular2.js").unwrap();

      assert_eq!(
        modules.get_children(circular1_id),
        Some(&vec!["circular2.js".to_string()])
      );

      assert_eq!(
        modules.get_children(circular2_id),
        Some(&vec!["circular1.js".to_string()])
      );
    } else {
      panic!("Future should be ready")
    }
  }
}
