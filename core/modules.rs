// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Implementation note: one could imagine combining this module with Isolate to
// provide a more intuitive high-level API. However, due to the complexity
// inherent in asynchronous module loading, we would like the Isolate to remain
// small and simple for users who do not use modules or if they do can load them
// synchronously. The isolate.rs module should never depend on this module.

use crate::isolate::Behavior;
use crate::isolate::Isolate;
use crate::js_errors::JSError;
use crate::libdeno::deno_mod;
use futures::Async;
use futures::Future;
use futures::Poll;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::marker::PhantomData;

pub type BoxError = Box<dyn Error + Send>;
pub type SourceCodeFuture<E> = dyn Future<Item = String, Error = E> + Send;

pub trait Loader<E: Error, B: Behavior> {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(&mut self, specifier: &str, referrer: &str) -> String;

  /// Given an absolute url, load its source code.
  fn load(&mut self, url: &str) -> Box<SourceCodeFuture<E>>;

  fn use_isolate<R, F: FnMut(&mut Isolate<B>) -> R>(&mut self, cb: F) -> R;

  fn use_modules<R, F: FnMut(&mut Modules) -> R>(&mut self, cb: F) -> R;
}

struct PendingLoad<E: Error> {
  url: String,
  source_code_future: Box<SourceCodeFuture<E>>,
}

/// This future is used to implement parallel async module loading without
/// complicating the Isolate API. Note that RecursiveLoad will take ownership of
/// an Isolate during load.
pub struct RecursiveLoad<E: Error, B: Behavior, L: Loader<E, B>> {
  loader: Option<L>,
  pending: Vec<PendingLoad<E>>,
  is_pending: HashSet<String>,
  root: String,
  phantom: PhantomData<(B, L)>,
}

impl<E: 'static + Error, B: Behavior, L: Loader<E, B>> RecursiveLoad<E, B, L> {
  /// Starts a new parallel load of the given URL.
  pub fn new(url: &str, mut loader: L) -> Self {
    let root = loader.resolve(url, ".");
    let mut recursive_load = Self {
      loader: Some(loader),
      root,
      pending: Vec::new(),
      is_pending: HashSet::new(),
      phantom: PhantomData,
    };
    recursive_load.add(url, ".", None);
    recursive_load
  }

  fn take_loader(&mut self) -> L {
    self.loader.take().unwrap()
  }

  fn add(
    &mut self,
    specifier: &str,
    referrer: &str,
    parent_id: Option<deno_mod>,
  ) {
    let url = {
      let loader = self.loader.as_mut().unwrap();
      loader.resolve(specifier, referrer)
    };

    if let Some(parent_id) = parent_id {
      let loader = self.loader.as_mut().unwrap();
      loader.use_modules(|modules| modules.add_child(parent_id, &url));
    }

    if !self.is_pending.contains(&url) {
      self.is_pending.insert(url.clone());
      let source_code_future = {
        let loader = self.loader.as_mut().unwrap();
        Box::new(loader.load(&url))
      };
      self.pending.push(PendingLoad {
        url,
        source_code_future,
      });
    }
  }
}

// TODO(ry) This is basically the same thing as RustOrJsError. They should be
// combined into one type.
pub enum Either<E> {
  JSError(JSError),
  Other(E),
}

// TODO remove 'static below.
impl<E: 'static + Error, B: Behavior, L: 'static + Loader<E, B>> Future
  for RecursiveLoad<E, B, L>
{
  type Item = (deno_mod, L);
  type Error = (Either<E>, L);

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut i = 0;
    while i < self.pending.len() {
      let pending = &mut self.pending[i];
      match pending.source_code_future.poll() {
        Err(err) => {
          return Err((Either::Other(err), self.take_loader()));
        }
        Ok(Async::NotReady) => {
          i += 1;
        }
        Ok(Async::Ready(source_code)) => {
          // We have completed loaded one of the modules.
          let completed = self.pending.remove(i);
          let main = completed.url == self.root;

          let result = {
            let loader = self.loader.as_mut().unwrap();
            loader.use_isolate(|isolate: &mut Isolate<B>| {
              isolate.mod_new(main, &completed.url, &source_code)
            })
          };
          if let Err(err) = result {
            return Err((Either::JSError(err), self.take_loader()));
          }
          let mod_id = result.unwrap();
          let referrer = &completed.url.clone();

          {
            let loader = self.loader.as_mut().unwrap();
            loader.use_modules(|modules: &mut Modules| {
              modules.register(mod_id, &completed.url)
            });
          }

          // Now we must iterate over all imports of the module and load them.
          let imports = {
            let loader = self.loader.as_mut().unwrap();
            loader.use_isolate(|isolate| isolate.mod_get_imports(mod_id))
          };
          for specifier in imports {
            self.add(&specifier, referrer, Some(mod_id));
          }
        }
      }
    }

    if self.pending.len() > 0 {
      return Ok(Async::NotReady);
    }

    let mut loader = self.take_loader();

    // TODO Fix this resolve callback weirdness.
    let loader_ =
      unsafe { std::mem::transmute::<&mut L, &'static mut L>(&mut loader) };

    let mut resolve = move |specifier: &str,
                            referrer_id: deno_mod|
          -> deno_mod {
      let referrer = loader_
        .use_modules(|modules| modules.get_name(referrer_id).unwrap().clone());
      let url = loader_.resolve(specifier, &referrer);
      loader_.use_modules(|modules| match modules.get_id(&url) {
        Some(id) => id,
        None => 0,
      })
    };

    let root_id =
      loader.use_modules(|modules| modules.get_id(&self.root).unwrap());

    let result = loader
      .use_isolate(|isolate| isolate.mod_instantiate(root_id, &mut resolve));

    match result {
      Err(err) => Err((Either::JSError(err), loader)),
      Ok(()) => Ok(Async::Ready((root_id, loader))),
    }
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

  pub fn get_name(&self, id: deno_mod) -> Option<&String> {
    self.info.get(&id).map(|i| &i.name)
  }

  pub fn is_registered(&self, name: &str) -> bool {
    self.by_name.get(name).is_some()
  }

  pub fn add_child(&mut self, parent_id: deno_mod, child_name: &str) -> bool {
    self
      .info
      .get_mut(&parent_id)
      .map(move |i| {
        if !i.has_child(&child_name) {
          i.children.push(child_name.to_string());
        }
      }).is_some()
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
    pub isolate: Isolate<TestBehavior>,
    pub modules: Modules,
  }

  impl MockLoader {
    fn new() -> Self {
      let modules = Modules::new();
      let isolate = TestBehavior::setup(TestBehaviorMode::AsyncImmediate);
      Self {
        loads: Vec::new(),
        isolate,
        modules,
      }
    }
  }

  impl Loader<std::io::Error, TestBehavior> for MockLoader {
    fn resolve(&mut self, specifier: &str, _referrer: &str) -> String {
      specifier.to_string()
    }

    fn load(&mut self, url: &str) -> Box<SourceCodeFuture<std::io::Error>> {
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

    fn use_isolate<R, F: FnMut(&mut Isolate<TestBehavior>) -> R>(
      &mut self,
      mut cb: F,
    ) -> R {
      cb(&mut self.isolate)
    }

    fn use_modules<R, F: FnMut(&mut Modules) -> R>(&mut self, mut cb: F) -> R {
      cb(&mut self.modules)
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
    let loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("a.js", loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    if let Async::Ready((a_id, mut loader)) = result.ok().unwrap() {
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
      assert!(false);
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
    let loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("circular1.js", loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    if let Async::Ready((circular1_id, mut loader)) = result.ok().unwrap() {
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
      assert!(false);
    }
  }
}
