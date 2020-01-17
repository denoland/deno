// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::any_error::ErrBox;
use crate::es_isolate::DynImportId;
use crate::es_isolate::ModuleId;
use crate::es_isolate::SourceCodeInfo;
use crate::module_specifier::ModuleSpecifier;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

pub type SourceCodeInfoFuture =
  dyn Future<Output = Result<SourceCodeInfo, ErrBox>> + Send;

pub trait Loader: Send + Sync {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
    is_dyn_import: bool,
  ) -> Result<ModuleSpecifier, ErrBox>;

  /// Given ModuleSpecifier, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Pin<Box<SourceCodeInfoFuture>>;
}

#[derive(Debug, Eq, PartialEq)]
enum Kind {
  Main,
  DynamicImport,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LoadState {
  ResolveMain(String, Option<String>),
  ResolveImport(String, String),
  LoadingRoot,
  LoadingImports,
  Done,
}

/// This future is used to implement parallel async module loading without
/// that is consumed by the isolate.
pub struct RecursiveModuleLoad {
  kind: Kind,
  // Kind::Main
  pub root_module_id: Option<ModuleId>,
  // Kind::Main
  pub dyn_import_id: Option<DynImportId>,
  pub state: LoadState,
  pub loader: Arc<Box<dyn Loader + Unpin>>,
  pub pending: FuturesUnordered<Pin<Box<SourceCodeInfoFuture>>>,
  pub is_pending: HashSet<ModuleSpecifier>,
}

impl RecursiveModuleLoad {
  /// Starts a new parallel load of the given URL of the main module.
  pub fn main(
    specifier: &str,
    code: Option<String>,
    loader: Arc<Box<dyn Loader + Unpin>>,
  ) -> Self {
    let kind = Kind::Main;
    let state = LoadState::ResolveMain(specifier.to_owned(), code);
    Self::new(kind, state, loader, None)
  }

  pub fn dynamic_import(
    id: DynImportId,
    specifier: &str,
    referrer: &str,
    loader: Arc<Box<dyn Loader + Unpin>>,
  ) -> Self {
    let kind = Kind::DynamicImport;
    let state =
      LoadState::ResolveImport(specifier.to_owned(), referrer.to_owned());
    Self::new(kind, state, loader, Some(id))
  }

  pub fn is_dynamic_import(&self) -> bool {
    self.kind != Kind::Main
  }

  fn new(
    kind: Kind,
    state: LoadState,
    loader: Arc<Box<dyn Loader + Unpin>>,
    dyn_import_id: Option<DynImportId>,
  ) -> Self {
    Self {
      root_module_id: None,
      dyn_import_id,
      kind,
      state,
      loader,
      pending: FuturesUnordered::new(),
      is_pending: HashSet::new(),
    }
  }

  fn add_root(&mut self) -> Result<(), ErrBox> {
    let module_specifier = match self.state {
      LoadState::ResolveMain(ref specifier, _) => {
        self.loader.resolve(specifier, ".", true, false)?
      }
      LoadState::ResolveImport(ref specifier, ref referrer) => {
        self.loader.resolve(specifier, referrer, false, true)?
      }

      _ => unreachable!(),
    };

    let load_fut = match &self.state {
      LoadState::ResolveMain(_, Some(code)) => {
        futures::future::ok(SourceCodeInfo {
          code: code.to_owned(),
          module_url_specified: module_specifier.to_string(),
          module_url_found: module_specifier.to_string(),
        })
        .boxed()
      }
      _ => self.loader.load(&module_specifier, None).boxed(),
    };

    self.pending.push(load_fut);

    self.state = LoadState::LoadingRoot;
    Ok(())
  }

  pub fn add_import(
    &mut self,
    specifier: ModuleSpecifier,
    referrer: ModuleSpecifier,
  ) {
    if !self.is_pending.contains(&specifier) {
      let fut = self.loader.load(&specifier, Some(referrer));
      self.pending.push(fut.boxed());
      self.is_pending.insert(specifier);
    }
  }
}

impl Stream for RecursiveModuleLoad {
  type Item = Result<SourceCodeInfo, ErrBox>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    match inner.state {
      LoadState::ResolveMain(..) | LoadState::ResolveImport(..) => {
        if let Err(e) = inner.add_root() {
          return Poll::Ready(Some(Err(e)));
        }
        inner.try_poll_next_unpin(cx)
      }
      LoadState::LoadingRoot | LoadState::LoadingImports => {
        match inner.pending.try_poll_next_unpin(cx)? {
          Poll::Ready(None) => unreachable!(),
          Poll::Ready(Some(info)) => Poll::Ready(Some(Ok(info))),
          Poll::Pending => Poll::Pending,
        }
      }
      LoadState::Done => Poll::Ready(None),
    }
  }
}

pub struct ModuleInfo {
  pub main: bool,
  pub name: String,
  pub handle: v8::Global<v8::Module>,
  pub import_specifiers: Vec<String>,
}

/// A symbolic module entity.
enum SymbolicModule {
  /// This module is an alias to another module.
  /// This is useful such that multiple names could point to
  /// the same underlying module (particularly due to redirects).
  Alias(String),
  /// This module associates with a V8 module by id.
  Mod(ModuleId),
}

#[derive(Default)]
/// Alias-able module name map
struct ModuleNameMap {
  inner: HashMap<String, SymbolicModule>,
}

impl ModuleNameMap {
  pub fn new() -> Self {
    ModuleNameMap {
      inner: HashMap::new(),
    }
  }

  /// Get the id of a module.
  /// If this module is internally represented as an alias,
  /// follow the alias chain to get the final module id.
  pub fn get(&self, name: &str) -> Option<ModuleId> {
    let mut mod_name = name;
    loop {
      let cond = self.inner.get(mod_name);
      match cond {
        Some(SymbolicModule::Alias(target)) => {
          mod_name = target;
        }
        Some(SymbolicModule::Mod(mod_id)) => {
          return Some(*mod_id);
        }
        _ => {
          return None;
        }
      }
    }
  }

  /// Insert a name assocated module id.
  pub fn insert(&mut self, name: String, id: ModuleId) {
    self.inner.insert(name, SymbolicModule::Mod(id));
  }

  /// Create an alias to another module.
  pub fn alias(&mut self, name: String, target: String) {
    self.inner.insert(name, SymbolicModule::Alias(target));
  }

  /// Check if a name is an alias to another module.
  pub fn is_alias(&self, name: &str) -> bool {
    let cond = self.inner.get(name);
    match cond {
      Some(SymbolicModule::Alias(_)) => true,
      _ => false,
    }
  }
}

/// A collection of JS modules.
#[derive(Default)]
pub struct Modules {
  pub(crate) info: HashMap<ModuleId, ModuleInfo>,
  by_name: ModuleNameMap,
  pub(crate) specifier_cache: HashMap<(String, String), ModuleSpecifier>,
}

impl Modules {
  pub fn new() -> Modules {
    Self {
      info: HashMap::new(),
      by_name: ModuleNameMap::new(),
      specifier_cache: HashMap::new(),
    }
  }

  pub fn get_id(&self, name: &str) -> Option<ModuleId> {
    self.by_name.get(name)
  }

  pub fn get_children(&self, id: ModuleId) -> Option<&Vec<String>> {
    self.info.get(&id).map(|i| &i.import_specifiers)
  }

  pub fn get_children2(&self, name: &str) -> Option<&Vec<String>> {
    self.get_id(name).and_then(|id| self.get_children(id))
  }

  pub fn get_name(&self, id: ModuleId) -> Option<&String> {
    self.info.get(&id).map(|i| &i.name)
  }

  pub fn is_registered(&self, name: &str) -> bool {
    self.by_name.get(name).is_some()
  }

  pub fn register(
    &mut self,
    id: ModuleId,
    name: &str,
    main: bool,
    handle: v8::Global<v8::Module>,
    import_specifiers: Vec<String>,
  ) {
    let name = String::from(name);
    debug!("register_complete {}", name);

    self.by_name.insert(name.clone(), id);
    self.info.insert(
      id,
      ModuleInfo {
        main,
        name,
        import_specifiers,
        handle,
      },
    );
  }

  pub fn alias(&mut self, name: &str, target: &str) {
    self.by_name.alias(name.to_owned(), target.to_owned());
  }

  pub fn is_alias(&self, name: &str) -> bool {
    self.by_name.is_alias(name)
  }

  pub fn get_info(&self, id: ModuleId) -> Option<&ModuleInfo> {
    if id == 0 {
      return None;
    }
    self.info.get(&id)
  }

  pub fn cache_specifier(
    &mut self,
    specifier: &str,
    referrer: &str,
    resolved_specifier: &ModuleSpecifier,
  ) {
    self.specifier_cache.insert(
      (specifier.to_string(), referrer.to_string()),
      resolved_specifier.to_owned(),
    );
  }

  pub fn get_cached_specifier(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Option<&ModuleSpecifier> {
    self
      .specifier_cache
      .get(&(specifier.to_string(), referrer.to_string()))
  }

  pub fn deps(&self, url: &str) -> Option<Deps> {
    Deps::new(self, url)
  }
}

/// This is a tree structure representing the dependencies of a given module.
/// Use Modules::deps to construct it. The 'deps' member is None if this module
/// was already seen elsewhere in the tree.
#[derive(Debug, PartialEq)]
pub struct Deps {
  pub name: String,
  pub deps: Option<Vec<Deps>>,
  prefix: String,
  is_last: bool,
}

impl Deps {
  fn new(modules: &Modules, module_name: &str) -> Option<Deps> {
    let mut seen = HashSet::new();
    Self::helper(&mut seen, "".to_string(), true, modules, module_name)
  }

  fn helper(
    seen: &mut HashSet<String>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    name: &str, // TODO(ry) rename url
  ) -> Option<Deps> {
    if seen.contains(name) {
      Some(Deps {
        name: name.to_string(),
        prefix,
        deps: None,
        is_last,
      })
    } else {
      let children = modules.get_children2(name)?;
      seen.insert(name.to_string());
      let child_count = children.len();
      let deps: Vec<Deps> = children
        .iter()
        .enumerate()
        .map(|(index, dep_name)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');

          Self::helper(seen, new_prefix, new_is_last, modules, dep_name)
        })
        // If any of the children are missing, return None.
        .collect::<Option<_>>()?;

      Some(Deps {
        name: name.to_string(),
        prefix,
        deps: Some(deps),
        is_last,
      })
    }
  }

  pub fn to_json(&self) -> String {
    let mut children = "[".to_string();

    if let Some(ref deps) = self.deps {
      for d in deps {
        children.push_str(&d.to_json());
        if !d.is_last {
          children.push_str(",");
        }
      }
    }
    children.push_str("]");

    format!("[\"{}\",{}]", self.name, children)
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
  use crate::es_isolate::EsIsolate;
  use crate::isolate::js_check;
  use futures::future::FutureExt;
  use std::error::Error;
  use std::fmt;
  use std::future::Future;
  use std::sync::Mutex;

  struct MockLoader {
    pub loads: Arc<Mutex<Vec<String>>>,
  }

  impl MockLoader {
    fn new() -> Self {
      Self {
        loads: Arc::new(Mutex::new(Vec::new())),
      }
    }
  }

  fn mock_source_code(url: &str) -> Option<(&'static str, &'static str)> {
    // (code, real_module_name)
    let spec: Vec<&str> = url.split("file://").collect();
    match spec[1] {
      "/a.js" => Some((A_SRC, "file:///a.js")),
      "/b.js" => Some((B_SRC, "file:///b.js")),
      "/c.js" => Some((C_SRC, "file:///c.js")),
      "/d.js" => Some((D_SRC, "file:///d.js")),
      "/circular1.js" => Some((CIRCULAR1_SRC, "file:///circular1.js")),
      "/circular2.js" => Some((CIRCULAR2_SRC, "file:///circular2.js")),
      "/circular3.js" => Some((CIRCULAR3_SRC, "file:///circular3.js")),
      "/redirect1.js" => Some((REDIRECT1_SRC, "file:///redirect1.js")),
      // pretend redirect - real module name is different than one requested
      "/redirect2.js" => Some((REDIRECT2_SRC, "file:///dir/redirect2.js")),
      "/dir/redirect3.js" => Some((REDIRECT3_SRC, "file:///redirect3.js")),
      "/slow.js" => Some((SLOW_SRC, "file:///slow.js")),
      "/never_ready.js" => {
        Some(("should never be Ready", "file:///never_ready.js"))
      }
      "/main.js" => Some((MAIN_SRC, "file:///main.js")),
      "/bad_import.js" => Some((BAD_IMPORT_SRC, "file:///bad_import.js")),
      // deliberately empty code.
      "/main_with_code.js" => Some(("", "file:///main_with_code.js")),
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
    fn cause(&self) -> Option<&dyn Error> {
      unimplemented!()
    }
  }

  struct DelayedSourceCodeFuture {
    url: String,
    counter: u32,
  }

  impl Future for DelayedSourceCodeFuture {
    type Output = Result<SourceCodeInfo, ErrBox>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
      let inner = self.get_mut();
      inner.counter += 1;
      if inner.url == "file:///never_ready.js" {
        return Poll::Pending;
      }
      if inner.url == "file:///slow.js" && inner.counter < 2 {
        // TODO(ry) Hopefully in the future we can remove current task
        // notification. See comment above run_in_task.
        cx.waker().wake_by_ref();
        return Poll::Pending;
      }
      match mock_source_code(&inner.url) {
        Some(src) => Poll::Ready(Ok(SourceCodeInfo {
          code: src.0.to_owned(),
          module_url_specified: inner.url.clone(),
          module_url_found: src.1.to_owned(),
        })),
        None => Poll::Ready(Err(MockError::LoadErr.into())),
      }
    }
  }

  impl Loader for MockLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _is_root: bool,
      _is_dyn_import: bool,
    ) -> Result<ModuleSpecifier, ErrBox> {
      let referrer = if referrer == "." {
        "file:///"
      } else {
        referrer
      };

      eprintln!(">> RESOLVING, S: {}, R: {}", specifier, referrer);

      let output_specifier =
        match ModuleSpecifier::resolve_import(specifier, referrer) {
          Ok(specifier) => specifier,
          Err(..) => return Err(MockError::ResolveErr.into()),
        };

      if mock_source_code(&output_specifier.to_string()).is_some() {
        Ok(output_specifier)
      } else {
        Err(MockError::ResolveErr.into())
      }
    }

    fn load(
      &self,
      module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<ModuleSpecifier>,
    ) -> Pin<Box<SourceCodeInfoFuture>> {
      let mut loads = self.loads.lock().unwrap();
      loads.push(module_specifier.to_string());
      let url = module_specifier.to_string();
      DelayedSourceCodeFuture { url, counter: 0 }.boxed()
    }
  }

  const A_SRC: &str = r#"
    import { b } from "/b.js";
    import { c } from "/c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'file:///a.js') throw Error();
  "#;

  const B_SRC: &str = r#"
    import { c } from "/c.js";
    if (c() != 'c') throw Error();
    export function b() { return 'b'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///b.js') throw Error();
  "#;

  const C_SRC: &str = r#"
    import { d } from "/d.js";
    export function c() { return 'c'; }
    if (d() != 'd') throw Error();
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///c.js') throw Error();
  "#;

  const D_SRC: &str = r#"
    export function d() { return 'd'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///d.js') throw Error();
  "#;

  // TODO(ry) Sadly FuturesUnordered requires the current task to be set. So
  // even though we are only using poll() in these tests and not Tokio, we must
  // nevertheless run it in the tokio executor. Ideally run_in_task can be
  // removed in the future.
  use crate::isolate::tests::run_in_task;
  use crate::isolate::StartupData;

  #[test]
  fn test_recursive_load() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut isolate =
      EsIsolate::new(Box::new(loader), StartupData::None, false);
    let a_id_fut = isolate.load_module("/a.js", None);
    let a_id = futures::executor::block_on(a_id_fut).expect("Failed to load");

    js_check(isolate.mod_evaluate(a_id));
    let l = loads.lock().unwrap();
    assert_eq!(
      l.to_vec(),
      vec![
        "file:///a.js",
        "file:///b.js",
        "file:///c.js",
        "file:///d.js"
      ]
    );

    let modules = &isolate.modules;
    assert_eq!(modules.get_id("file:///a.js"), Some(a_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();
    assert_eq!(
      modules.get_children(a_id),
      Some(&vec!["/b.js".to_string(), "/c.js".to_string()])
    );
    assert_eq!(modules.get_children(b_id), Some(&vec!["/c.js".to_string()]));
    assert_eq!(modules.get_children(c_id), Some(&vec!["/d.js".to_string()]));
    assert_eq!(modules.get_children(d_id), Some(&vec![]));
  }

  const CIRCULAR1_SRC: &str = r#"
    import "/circular2.js";
    Deno.core.print("circular1");
  "#;

  const CIRCULAR2_SRC: &str = r#"
    import "/circular3.js";
    Deno.core.print("circular2");
  "#;

  const CIRCULAR3_SRC: &str = r#"
    import "/circular1.js";
    import "/circular2.js";
    Deno.core.print("circular3");
  "#;

  #[test]
  fn test_circular_load() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut isolate =
      EsIsolate::new(Box::new(loader), StartupData::None, false);

    let fut = async move {
      let result = isolate.load_module("/circular1.js", None).await;
      assert!(result.is_ok());
      let circular1_id = result.unwrap();
      js_check(isolate.mod_evaluate(circular1_id));

      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///circular1.js",
          "file:///circular2.js",
          "file:///circular3.js"
        ]
      );

      let modules = &isolate.modules;

      assert_eq!(modules.get_id("file:///circular1.js"), Some(circular1_id));
      let circular2_id = modules.get_id("file:///circular2.js").unwrap();

      assert_eq!(
        modules.get_children(circular1_id),
        Some(&vec!["/circular2.js".to_string()])
      );

      assert_eq!(
        modules.get_children(circular2_id),
        Some(&vec!["/circular3.js".to_string()])
      );

      assert!(modules.get_id("file:///circular3.js").is_some());
      let circular3_id = modules.get_id("file:///circular3.js").unwrap();
      assert_eq!(
        modules.get_children(circular3_id),
        Some(&vec![
          "/circular1.js".to_string(),
          "/circular2.js".to_string()
        ])
      );
    }
    .boxed();

    futures::executor::block_on(fut);
  }

  const REDIRECT1_SRC: &str = r#"
    import "./redirect2.js";
    Deno.core.print("redirect1");
  "#;

  const REDIRECT2_SRC: &str = r#"
    import "./redirect3.js";
    Deno.core.print("redirect2");
  "#;

  const REDIRECT3_SRC: &str = r#"
    Deno.core.print("redirect3");
  "#;

  #[test]
  fn test_redirect_load() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut isolate =
      EsIsolate::new(Box::new(loader), StartupData::None, false);

    let fut = async move {
      let result = isolate.load_module("/redirect1.js", None).await;
      println!(">> result {:?}", result);
      assert!(result.is_ok());
      let redirect1_id = result.unwrap();
      js_check(isolate.mod_evaluate(redirect1_id));
      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///redirect1.js",
          "file:///redirect2.js",
          "file:///dir/redirect3.js"
        ]
      );

      let modules = &isolate.modules;

      assert_eq!(modules.get_id("file:///redirect1.js"), Some(redirect1_id));

      let redirect2_id = modules.get_id("file:///dir/redirect2.js").unwrap();
      assert!(modules.is_alias("file:///redirect2.js"));
      assert!(!modules.is_alias("file:///dir/redirect2.js"));
      assert_eq!(modules.get_id("file:///redirect2.js"), Some(redirect2_id));

      let redirect3_id = modules.get_id("file:///redirect3.js").unwrap();
      assert!(modules.is_alias("file:///dir/redirect3.js"));
      assert!(!modules.is_alias("file:///redirect3.js"));
      assert_eq!(
        modules.get_id("file:///dir/redirect3.js"),
        Some(redirect3_id)
      );
    }
    .boxed();

    futures::executor::block_on(fut);
  }

  // main.js
  const MAIN_SRC: &str = r#"
    // never_ready.js never loads.
    import "/never_ready.js";
    // slow.js resolves after one tick.
    import "/slow.js";
  "#;

  // slow.js
  const SLOW_SRC: &str = r#"
    // Circular import of never_ready.js
    // Does this trigger two Loader calls? It shouldn't.
    import "/never_ready.js";
    import "/a.js";
  "#;

  #[test]
  fn slow_never_ready_modules() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let loads = loader.loads.clone();
      let mut isolate =
        EsIsolate::new(Box::new(loader), StartupData::None, false);
      let mut recursive_load = isolate.load_module("/main.js", None).boxed();

      let result = recursive_load.poll_unpin(&mut cx);
      assert!(result.is_pending());

      // TODO(ry) Arguably the first time we poll only the following modules
      // should be loaded:
      //      "file:///main.js",
      //      "file:///never_ready.js",
      //      "file:///slow.js"
      // But due to current task notification in DelayedSourceCodeFuture they
      // all get loaded in a single poll. Also see the comment above
      // run_in_task.

      for _ in 0..10 {
        let result = recursive_load.poll_unpin(&mut cx);
        assert!(result.is_pending());
        let l = loads.lock().unwrap();
        assert_eq!(
          l.to_vec(),
          vec![
            "file:///main.js",
            "file:///never_ready.js",
            "file:///slow.js",
            "file:///a.js",
            "file:///b.js",
            "file:///c.js",
            "file:///d.js"
          ]
        );
      }
    })
  }

  // bad_import.js
  const BAD_IMPORT_SRC: &str = r#"
    import "foo";
  "#;

  #[test]
  fn loader_disappears_after_error() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let mut isolate =
        EsIsolate::new(Box::new(loader), StartupData::None, false);
      let mut load_fut = isolate.load_module("/bad_import.js", None).boxed();
      let result = load_fut.poll_unpin(&mut cx);
      if let Poll::Ready(Err(err)) = result {
        assert_eq!(
          err.downcast_ref::<MockError>().unwrap(),
          &MockError::ResolveErr
        );
      } else {
        unreachable!();
      }
    })
  }

  const MAIN_WITH_CODE_SRC: &str = r#"
    import { b } from "/b.js";
    import { c } from "/c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'file:///main_with_code.js') throw Error();
  "#;

  #[test]
  fn recursive_load_main_with_code() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut isolate =
      EsIsolate::new(Box::new(loader), StartupData::None, false);
    // In default resolution code should be empty.
    // Instead we explicitly pass in our own code.
    // The behavior should be very similar to /a.js.
    let main_id_fut = isolate
      .load_module("/main_with_code.js", Some(MAIN_WITH_CODE_SRC.to_owned()))
      .boxed();
    let main_id =
      futures::executor::block_on(main_id_fut).expect("Failed to load");

    js_check(isolate.mod_evaluate(main_id));

    let l = loads.lock().unwrap();
    assert_eq!(
      l.to_vec(),
      vec!["file:///b.js", "file:///c.js", "file:///d.js"]
    );

    let modules = &isolate.modules;

    assert_eq!(modules.get_id("file:///main_with_code.js"), Some(main_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();

    assert_eq!(
      modules.get_children(main_id),
      Some(&vec!["/b.js".to_string(), "/c.js".to_string()])
    );
    assert_eq!(modules.get_children(b_id), Some(&vec!["/c.js".to_string()]));
    assert_eq!(modules.get_children(c_id), Some(&vec!["/d.js".to_string()]));
    assert_eq!(modules.get_children(d_id), Some(&vec![]));
  }

  #[test]
  fn empty_deps() {
    let modules = Modules::new();
    assert!(modules.deps("foo").is_none());
  }

  /* TODO(bartlomieju): reenable
  #[test]
  fn deps() {
    // "foo" -> "bar"
    let mut modules = Modules::new();
    modules.register(1, "foo");
    modules.register(2, "bar");
    modules.add_child(1, "bar");
    let maybe_deps = modules.deps("foo");
    assert!(maybe_deps.is_some());
    let mut foo_deps = maybe_deps.unwrap();
    assert_eq!(foo_deps.name, "foo");
    assert!(foo_deps.deps.is_some());
    let foo_children = foo_deps.deps.take().unwrap();
    assert_eq!(foo_children.len(), 1);
    let bar_deps = &foo_children[0];
    assert_eq!(bar_deps.name, "bar");
    assert_eq!(bar_deps.deps, Some(vec![]));
  }

  #[test]
  fn test_deps_to_json() {
    let mut modules = Modules::new();
    modules.register(1, "foo");
    modules.register(2, "bar");
    modules.register(3, "baz");
    modules.register(4, "zuh");
    modules.add_child(1, "bar");
    modules.add_child(1, "baz");
    modules.add_child(3, "zuh");
    let maybe_deps = modules.deps("foo");
    assert!(maybe_deps.is_some());
    assert_eq!(
      "[\"foo\",[[\"bar\",[]],[\"baz\",[[\"zuh\",[]]]]]]",
      maybe_deps.unwrap().to_json()
    );
  }
  */
}
