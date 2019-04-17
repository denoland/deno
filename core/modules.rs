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
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;

pub type SourceCodeFuture<E> = dyn Future<Item = String, Error = E> + Send;

pub trait Loader {
  type Dispatch: crate::isolate::Dispatch;
  type Error: std::error::Error + 'static;

  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(specifier: &str, referrer: &str) -> Result<String, Self::Error>;

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

struct PendingLoad<E: Error> {
  url: String,
  is_root: bool,
  source_code_future: Box<SourceCodeFuture<E>>,
}

/// This future is used to implement parallel async module loading without
/// complicating the Isolate API. Note that RecursiveLoad will take ownership of
/// an Isolate during load.
pub struct RecursiveLoad<L: Loader> {
  loader: Option<L>,
  pending: Vec<PendingLoad<L::Error>>,
  is_pending: HashSet<String>,
  phantom: PhantomData<L>,
  // TODO(ry) The following can all be combined into a single enum State type.
  root: Option<String>,           // Empty before polled.
  root_specifier: Option<String>, // Empty after first poll
  root_id: Option<deno_mod>,
}

impl<L: Loader> RecursiveLoad<L> {
  /// Starts a new parallel load of the given URL.
  pub fn new(url: &str, loader: L) -> Self {
    Self {
      loader: Some(loader),
      root: None,
      root_specifier: Some(url.to_string()),
      root_id: None,
      pending: Vec::new(),
      is_pending: HashSet::new(),
      phantom: PhantomData,
    }
  }

  fn take_loader(&mut self) -> L {
    self.loader.take().unwrap()
  }

  fn add(
    &mut self,
    specifier: &str,
    referrer: &str,
    parent_id: Option<deno_mod>,
  ) -> Result<String, L::Error> {
    let url = L::resolve(specifier, referrer)?;

    let is_root = if let Some(parent_id) = parent_id {
      let loader = self.loader.as_mut().unwrap();
      let modules = loader.modules();
      modules.add_child(parent_id, &url);
      false
    } else {
      true
    };

    if !self.is_pending.contains(&url) {
      self.is_pending.insert(url.clone());
      let source_code_future = {
        let loader = self.loader.as_mut().unwrap();
        loader.load(&url)
      };
      self.pending.push(PendingLoad {
        url: url.clone(),
        source_code_future,
        is_root,
      });
    }

    Ok(url)
  }
}

// TODO(ry) This is basically the same thing as RustOrJsError. They should be
// combined into one type.
#[derive(Debug, PartialEq)]
pub enum JSErrorOr<E> {
  JSError(JSError),
  Other(E),
}

impl<L: Loader> Future for RecursiveLoad<L> {
  type Item = (deno_mod, L);
  type Error = (JSErrorOr<L::Error>, L);

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    if self.root.is_none() && self.root_specifier.is_some() {
      let s = self.root_specifier.take().unwrap();
      match self.add(&s, ".", None) {
        Err(err) => {
          return Err((JSErrorOr::Other(err), self.take_loader()));
        }
        Ok(root) => {
          self.root = Some(root);
        }
      }
    }
    assert!(self.root_specifier.is_none());
    assert!(self.root.is_some());

    let mut i = 0;
    while i < self.pending.len() {
      let pending = &mut self.pending[i];
      match pending.source_code_future.poll() {
        Err(err) => {
          return Err((JSErrorOr::Other(err), self.take_loader()));
        }
        Ok(Async::NotReady) => {
          i += 1;
        }
        Ok(Async::Ready(source_code)) => {
          // We have completed loaded one of the modules.
          let completed = self.pending.remove(i);

          let result = {
            let loader = self.loader.as_mut().unwrap();
            let isolate = loader.isolate();
            isolate.mod_new(completed.is_root, &completed.url, &source_code)
          };
          if let Err(err) = result {
            return Err((JSErrorOr::JSError(err), self.take_loader()));
          }
          let mod_id = result.unwrap();
          if completed.is_root {
            assert!(self.root_id.is_none());
            self.root_id = Some(mod_id);
          }

          let referrer = &completed.url.clone();

          {
            let loader = self.loader.as_mut().unwrap();
            let modules = loader.modules();
            modules.register(mod_id, &completed.url);
          }

          // Now we must iterate over all imports of the module and load them.
          let imports = {
            let loader = self.loader.as_mut().unwrap();
            let isolate = loader.isolate();
            isolate.mod_get_imports(mod_id)
          };
          for specifier in imports {
            self
              .add(&specifier, referrer, Some(mod_id))
              .map_err(|e| (JSErrorOr::Other(e), self.take_loader()))?;
          }
        }
      }
    }

    if !self.pending.is_empty() {
      return Ok(Async::NotReady);
    }

    let root_id = self.root_id.unwrap();
    let mut loader = self.take_loader();
    let (isolate, modules) = loader.isolate_and_modules();
    let result = {
      let mut resolve_cb =
        |specifier: &str, referrer_id: deno_mod| -> deno_mod {
          let referrer = modules.get_name(referrer_id).unwrap();
          match L::resolve(specifier, &referrer) {
            Ok(url) => match modules.get_id(&url) {
              Some(id) => id,
              None => 0,
            },
            // We should have already resolved and loaded this module, so
            // resolve() will not fail this time.
            Err(_err) => unreachable!(),
          }
        };

      isolate.mod_instantiate(root_id, &mut resolve_cb)
    };

    match result {
      Err(err) => Err((JSErrorOr::JSError(err), loader)),
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

  pub fn deps(&self, url: &str) -> Deps {
    Deps::new(self, url)
  }
}

pub struct Deps {
  pub name: String,
  pub deps: Option<Vec<Deps>>,
  prefix: String,
  is_last: bool,
}

impl Deps {
  pub fn new(modules: &Modules, module_name: &str) -> Deps {
    let mut seen = HashSet::new();
    Self::helper(&mut seen, "".to_string(), true, modules, module_name)
  }

  fn helper(
    seen: &mut HashSet<String>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    name: &str, // TODO(ry) rename url
  ) -> Deps {
    if seen.contains(name) {
      Deps {
        name: name.to_string(),
        prefix,
        deps: None,
        is_last,
      }
    } else {
      seen.insert(name.to_string());
      let children = modules.get_children2(name).unwrap();
      let child_count = children.iter().count();
      let deps = children
        .iter()
        .enumerate()
        .map(|(index, dep_name)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');

          Self::helper(seen, new_prefix, new_is_last, modules, dep_name)
        }).collect();
      Deps {
        name: name.to_string(),
        prefix,
        deps: Some(deps),
        is_last,
      }
    }
  }
}

impl fmt::Display for Deps {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut has_children = false;
    if let Some(ref deps) = self.deps {
      has_children = !deps.is_empty();
    }
    write!(
      f,
      "{}{}─{} {}",
      self.prefix,
      if self.is_last { "└" } else { "├" },
      if has_children { "┬" } else { "─" },
      self.name
    )?;

    if let Some(ref deps) = self.deps {
      for d in deps {
        write!(f, "\n{}", d)?;
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate::js_check;
  use crate::isolate::tests::*;
  use std::fmt;

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

  fn mock_source_code(url: &str) -> Option<&'static str> {
    match url {
      "a.js" => Some(A_SRC),
      "b.js" => Some(B_SRC),
      "c.js" => Some(C_SRC),
      "d.js" => Some(D_SRC),
      "circular1.js" => Some(CIRCULAR1_SRC),
      "circular2.js" => Some(CIRCULAR2_SRC),
      "circular3.js" => Some(CIRCULAR3_SRC),
      "slow.js" => Some(SLOW_SRC),
      "never_ready.js" => Some("should never be loaded"),
      "main.js" => Some(MAIN_SRC),
      "bad_import.js" => Some(BAD_IMPORT_SRC),
      _ => None,
    }
  }

  #[derive(Debug, PartialEq)]
  enum MockError {
    ResolveErr,
    LoadErr,
  }

  impl fmt::Display for MockError {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
      unimplemented!()
    }
  }

  impl Error for MockError {
    fn cause(&self) -> Option<&Error> {
      unimplemented!()
    }
  }

  struct DelayedSourceCodeFuture {
    url: String,
    counter: u32,
  }

  impl Future for DelayedSourceCodeFuture {
    type Item = String;
    type Error = MockError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
      self.counter += 1;
      if self.url == "never_ready.js"
        || (self.url == "slow.js" && self.counter < 2)
      {
        return Ok(Async::NotReady);
      }
      match mock_source_code(&self.url) {
        Some(src) => Ok(Async::Ready(src.to_string())),
        None => Err(MockError::LoadErr),
      }
    }
  }

  impl Loader for MockLoader {
    type Dispatch = TestDispatch;
    type Error = MockError;

    fn resolve(
      specifier: &str,
      _referrer: &str,
    ) -> Result<String, Self::Error> {
      if mock_source_code(specifier).is_some() {
        Ok(specifier.to_string())
      } else {
        Err(MockError::ResolveErr)
      }
    }

    fn load(&mut self, url: &str) -> Box<SourceCodeFuture<Self::Error>> {
      self.loads.push(url.to_string());
      let url = url.to_string();
      Box::new(DelayedSourceCodeFuture { url, counter: 0 })
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
      panic!("this shouldn't happen");
    }
  }

  const CIRCULAR1_SRC: &str = r#"
    import "circular2.js";
    Deno.core.print("circular1");
  "#;

  const CIRCULAR2_SRC: &str = r#"
    import "circular3.js";
    Deno.core.print("circular2");
  "#;

  const CIRCULAR3_SRC: &str = r#"
    import "circular1.js";
    import "circular2.js";
    Deno.core.print("circular3");
  "#;

  #[test]
  fn test_circular_load() {
    let loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("circular1.js", loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    if let Async::Ready((circular1_id, mut loader)) = result.ok().unwrap() {
      js_check(loader.isolate.mod_evaluate(circular1_id));
      assert_eq!(
        loader.loads,
        vec!["circular1.js", "circular2.js", "circular3.js"]
      );

      let modules = &loader.modules;

      assert_eq!(modules.get_id("circular1.js"), Some(circular1_id));
      let circular2_id = modules.get_id("circular2.js").unwrap();

      assert_eq!(
        modules.get_children(circular1_id),
        Some(&vec!["circular2.js".to_string()])
      );

      assert_eq!(
        modules.get_children(circular2_id),
        Some(&vec!["circular3.js".to_string()])
      );

      assert!(modules.get_id("circular3.js").is_some());
      let circular3_id = modules.get_id("circular3.js").unwrap();
      assert_eq!(
        modules.get_children(circular3_id),
        Some(&vec![
          "circular1.js".to_string(),
          "circular2.js".to_string()
        ])
      );
    } else {
      panic!("this shouldn't happen");
    }
  }

  // main.js
  const MAIN_SRC: &str = r#"
    // never_ready.js never loads.
    import "never_ready.js";
    // slow.js resolves after one tick.
    import "slow.js";
  "#;

  // slow.js
  const SLOW_SRC: &str = r#"
    // Circular import of never_ready.js
    // Does this trigger two Loader calls? It shouldn't.
    import "never_ready.js";
    import "a.js";
  "#;

  #[test]
  fn slow_never_ready_modules() {
    let loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("main.js", loader);

    let result = recursive_load.poll();
    assert!(result.is_ok());
    assert!(result.ok().unwrap().is_not_ready());

    {
      let loader = recursive_load.loader.as_ref().unwrap();
      assert_eq!(loader.loads, vec!["main.js", "never_ready.js", "slow.js"]);
    }

    let result = recursive_load.poll();
    assert!(result.is_ok());
    assert!(result.ok().unwrap().is_not_ready());

    {
      let loader = recursive_load.loader.as_ref().unwrap();
      assert_eq!(
        loader.loads,
        vec![
          "main.js",
          "never_ready.js",
          "slow.js",
          "a.js",
          "b.js",
          "c.js",
          "d.js"
        ]
      );
    }

    let result = recursive_load.poll();
    assert!(result.is_ok());
    assert!(result.ok().unwrap().is_not_ready());

    {
      let loader = recursive_load.loader.as_ref().unwrap();
      assert_eq!(
        loader.loads,
        vec![
          "main.js",
          "never_ready.js",
          "slow.js",
          "a.js",
          "b.js",
          "c.js",
          "d.js"
        ]
      );
    }

    let result = recursive_load.poll();
    assert!(result.is_ok());
    assert!(result.ok().unwrap().is_not_ready());

    {
      let loader = recursive_load.loader.as_ref().unwrap();
      assert_eq!(
        loader.loads,
        vec![
          "main.js",
          "never_ready.js",
          "slow.js",
          "a.js",
          "b.js",
          "c.js",
          "d.js"
        ]
      );
    }
  }

  // bad_import.js
  const BAD_IMPORT_SRC: &str = r#"
    import "foo";
  "#;

  #[test]
  fn loader_disappears_after_error() {
    let loader = MockLoader::new();
    let mut recursive_load = RecursiveLoad::new("bad_import.js", loader);
    let result = recursive_load.poll();
    assert!(result.is_err());
    let (either_err, _loader) = result.err().unwrap();
    assert_eq!(either_err, JSErrorOr::Other(MockError::ResolveErr));
    assert!(recursive_load.loader.is_none());
  }
}
