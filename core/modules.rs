// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::module_specifier::ModuleSpecifier;
use crate::ErrBox;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

lazy_static! {
  pub static ref NEXT_LOAD_ID: AtomicI32 = AtomicI32::new(0);
}

pub type ModuleId = i32;
pub type ModuleLoadId = i32;

/// EsModule source code that will be loaded into V8.
///
/// Users can implement `Into<ModuleInfo>` for different file types that
/// can be transpiled to valid EsModule.
///
/// Found module URL might be different from specified URL
/// used for loading due to redirections (like HTTP 303).
/// Eg. Both "https://example.com/a.ts" and
/// "https://example.com/b.ts" may point to "https://example.com/c.ts"
/// By keeping track of specified and found URL we can alias modules and avoid
/// recompiling the same code 3 times.
// TODO(bartlomieju): I have a strong opinion we should store all redirects
// that happened; not only first and final target. It would simplify a lot
// of things throughout the codebase otherwise we may end up requesting
// intermediate redirects from file loader.
#[derive(Debug, Eq, PartialEq)]
pub struct ModuleSource {
  pub code: String,
  pub module_url_specified: String,
  pub module_url_found: String,
}

pub type PrepareLoadFuture =
  dyn Future<Output = (ModuleLoadId, Result<RecursiveModuleLoad, ErrBox>)>;
pub type ModuleSourceFuture = dyn Future<Output = Result<ModuleSource, ErrBox>>;

pub trait ModuleLoader {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  ///
  /// `is_main` can be used to resolve from current working directory or
  /// apply import map for child imports.
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
  ) -> Result<ModuleSpecifier, ErrBox>;

  /// Given ModuleSpecifier, load its source code.
  ///
  /// `is_dyn_import` can be used to check permissions or deny
  /// dynamic imports altogether.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>>;

  /// This hook can be used by implementors to do some preparation
  /// work before starting loading of modules.
  ///
  /// For example implementor might download multiple modules in
  /// parallel and transpile them to final JS sources before
  /// yielding control back to Isolate.
  ///
  /// It's not required to implement this method.
  fn prepare_load(
    &self,
    _load_id: ModuleLoadId,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), ErrBox>>>> {
    async { Ok(()) }.boxed_local()
  }
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
  // TODO(bartlomieju): in future this value should
  // be randomized
  pub id: ModuleLoadId,
  pub root_module_id: Option<ModuleId>,
  pub state: LoadState,
  pub loader: Rc<dyn ModuleLoader>,
  pub pending: FuturesUnordered<Pin<Box<ModuleSourceFuture>>>,
  pub is_pending: HashSet<ModuleSpecifier>,
}

impl RecursiveModuleLoad {
  /// Starts a new parallel load of the given URL of the main module.
  pub fn main(
    specifier: &str,
    code: Option<String>,
    loader: Rc<dyn ModuleLoader>,
  ) -> Self {
    let kind = Kind::Main;
    let state = LoadState::ResolveMain(specifier.to_owned(), code);
    Self::new(kind, state, loader)
  }

  pub fn dynamic_import(
    specifier: &str,
    referrer: &str,
    loader: Rc<dyn ModuleLoader>,
  ) -> Self {
    let kind = Kind::DynamicImport;
    let state =
      LoadState::ResolveImport(specifier.to_owned(), referrer.to_owned());
    Self::new(kind, state, loader)
  }

  pub fn is_dynamic_import(&self) -> bool {
    self.kind != Kind::Main
  }

  fn new(kind: Kind, state: LoadState, loader: Rc<dyn ModuleLoader>) -> Self {
    Self {
      id: NEXT_LOAD_ID.fetch_add(1, Ordering::SeqCst),
      root_module_id: None,
      kind,
      state,
      loader,
      pending: FuturesUnordered::new(),
      is_pending: HashSet::new(),
    }
  }

  pub async fn prepare(self) -> (ModuleLoadId, Result<Self, ErrBox>) {
    let (module_specifier, maybe_referrer) = match self.state {
      LoadState::ResolveMain(ref specifier, _) => {
        let spec = match self.loader.resolve(specifier, ".", true) {
          Ok(spec) => spec,
          Err(e) => return (self.id, Err(e)),
        };
        (spec, None)
      }
      LoadState::ResolveImport(ref specifier, ref referrer) => {
        let spec = match self.loader.resolve(specifier, referrer, false) {
          Ok(spec) => spec,
          Err(e) => return (self.id, Err(e)),
        };
        (spec, Some(referrer.to_string()))
      }
      _ => unreachable!(),
    };

    let prepare_result = self
      .loader
      .prepare_load(
        self.id,
        &module_specifier,
        maybe_referrer,
        self.is_dynamic_import(),
      )
      .await;

    match prepare_result {
      Ok(()) => (self.id, Ok(self)),
      Err(e) => (self.id, Err(e)),
    }
  }

  fn add_root(&mut self) -> Result<(), ErrBox> {
    let module_specifier = match self.state {
      LoadState::ResolveMain(ref specifier, _) => {
        self.loader.resolve(specifier, ".", true)?
      }
      LoadState::ResolveImport(ref specifier, ref referrer) => {
        self.loader.resolve(specifier, referrer, false)?
      }

      _ => unreachable!(),
    };

    let load_fut = match &self.state {
      LoadState::ResolveMain(_, Some(code)) => {
        futures::future::ok(ModuleSource {
          code: code.to_owned(),
          module_url_specified: module_specifier.to_string(),
          module_url_found: module_specifier.to_string(),
        })
        .boxed()
      }
      _ => self
        .loader
        .load(&module_specifier, None, self.is_dynamic_import())
        .boxed_local(),
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
      let fut =
        self
          .loader
          .load(&specifier, Some(referrer), self.is_dynamic_import());
      self.pending.push(fut.boxed_local());
      self.is_pending.insert(specifier);
    }
  }
}

impl Stream for RecursiveModuleLoad {
  type Item = Result<ModuleSource, ErrBox>;

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
  pub import_specifiers: Vec<ModuleSpecifier>,
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
    matches!(cond, Some(SymbolicModule::Alias(_)))
  }
}

/// A collection of JS modules.
#[derive(Default)]
pub struct Modules {
  pub(crate) info: HashMap<ModuleId, ModuleInfo>,
  by_name: ModuleNameMap,
}

impl Modules {
  pub fn new() -> Modules {
    Self {
      info: HashMap::new(),
      by_name: ModuleNameMap::new(),
    }
  }

  pub fn get_id(&self, name: &str) -> Option<ModuleId> {
    self.by_name.get(name)
  }

  pub fn get_children(&self, id: ModuleId) -> Option<&Vec<ModuleSpecifier>> {
    self.info.get(&id).map(|i| &i.import_specifiers)
  }

  pub fn get_name(&self, id: ModuleId) -> Option<&String> {
    self.info.get(&id).map(|i| &i.name)
  }

  pub fn is_registered(&self, specifier: &ModuleSpecifier) -> bool {
    self.by_name.get(&specifier.to_string()).is_some()
  }

  pub fn register(
    &mut self,
    id: ModuleId,
    name: &str,
    main: bool,
    handle: v8::Global<v8::Module>,
    import_specifiers: Vec<ModuleSpecifier>,
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

  pub fn deps(&self, module_specifier: &ModuleSpecifier) -> Option<Deps> {
    Deps::new(self, module_specifier)
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
  fn new(
    modules: &Modules,
    module_specifier: &ModuleSpecifier,
  ) -> Option<Deps> {
    let mut seen = HashSet::new();
    Self::helper(&mut seen, "".to_string(), true, modules, module_specifier)
  }

  fn helper(
    seen: &mut HashSet<String>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    module_specifier: &ModuleSpecifier,
  ) -> Option<Deps> {
    let name = module_specifier.to_string();
    if seen.contains(&name) {
      Some(Deps {
        name,
        prefix,
        deps: None,
        is_last,
      })
    } else {
      let mod_id = modules.get_id(&name)?;
      let children = modules.get_children(mod_id).unwrap();
      seen.insert(name.to_string());
      let child_count = children.len();
      let deps: Vec<Deps> = children
        .iter()
        .enumerate()
        .map(|(index, dep_specifier)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');

          Self::helper(seen, new_prefix, new_is_last, modules, dep_specifier)
        })
        // If any of the children are missing, return None.
        .collect::<Option<_>>()?;

      Some(Deps {
        name,
        prefix,
        deps: Some(deps),
        is_last,
      })
    }
  }

  pub fn to_json(&self) -> serde_json::Value {
    let children;
    if let Some(deps) = &self.deps {
      children = deps.iter().map(|c| c.to_json()).collect();
    } else {
      children = Vec::new()
    }
    serde_json::json!([&self.name, children])
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

#[macro_export]
macro_rules! crate_modules {
  () => {
    pub const DENO_CRATE_PATH: &'static str = env!("CARGO_MANIFEST_DIR");
  };
}

#[macro_export]
macro_rules! include_crate_modules {
  ( $( $x:ident ),* ) => {
    {
      let mut temp: HashMap<String, String> = HashMap::new();
      $(
        temp.insert(stringify!($x).to_string(), $x::DENO_CRATE_PATH.to_string());
      )*
      temp
    }
  };
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::es_isolate::EsIsolate;
  use crate::js_check;
  use crate::StartupData;
  use futures::future::FutureExt;
  use std::error::Error;
  use std::fmt;
  use std::future::Future;
  use std::sync::Arc;
  use std::sync::Mutex;

  // TODO(ry) Sadly FuturesUnordered requires the current task to be set. So
  // even though we are only using poll() in these tests and not Tokio, we must
  // nevertheless run it in the tokio executor. Ideally run_in_task can be
  // removed in the future.
  use crate::core_isolate::tests::run_in_task;

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
    type Output = Result<ModuleSource, ErrBox>;

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
        Some(src) => Poll::Ready(Ok(ModuleSource {
          code: src.0.to_owned(),
          module_url_specified: inner.url.clone(),
          module_url_found: src.1.to_owned(),
        })),
        None => Poll::Ready(Err(MockError::LoadErr.into())),
      }
    }
  }

  impl ModuleLoader for MockLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _is_root: bool,
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
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
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

  #[test]
  fn test_recursive_load() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut isolate = EsIsolate::new(Rc::new(loader), StartupData::None, false);
    let spec = ModuleSpecifier::resolve_url("file:///a.js").unwrap();
    let a_id_fut = isolate.load_module(&spec, None);
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

    let state_rc = EsIsolate::state(&isolate);
    let state = state_rc.borrow();
    let modules = &state.modules;
    assert_eq!(modules.get_id("file:///a.js"), Some(a_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();
    assert_eq!(
      modules.get_children(a_id),
      Some(&vec![
        ModuleSpecifier::resolve_url("file:///b.js").unwrap(),
        ModuleSpecifier::resolve_url("file:///c.js").unwrap()
      ])
    );
    assert_eq!(
      modules.get_children(b_id),
      Some(&vec![ModuleSpecifier::resolve_url("file:///c.js").unwrap()])
    );
    assert_eq!(
      modules.get_children(c_id),
      Some(&vec![ModuleSpecifier::resolve_url("file:///d.js").unwrap()])
    );
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
    let mut isolate = EsIsolate::new(Rc::new(loader), StartupData::None, false);

    let fut = async move {
      let spec = ModuleSpecifier::resolve_url("file:///circular1.js").unwrap();
      let result = isolate.load_module(&spec, None).await;
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

      let state_rc = EsIsolate::state(&isolate);
      let state = state_rc.borrow();
      let modules = &state.modules;

      assert_eq!(modules.get_id("file:///circular1.js"), Some(circular1_id));
      let circular2_id = modules.get_id("file:///circular2.js").unwrap();

      assert_eq!(
        modules.get_children(circular1_id),
        Some(&vec![
          ModuleSpecifier::resolve_url("file:///circular2.js").unwrap()
        ])
      );

      assert_eq!(
        modules.get_children(circular2_id),
        Some(&vec![
          ModuleSpecifier::resolve_url("file:///circular3.js").unwrap()
        ])
      );

      assert!(modules.get_id("file:///circular3.js").is_some());
      let circular3_id = modules.get_id("file:///circular3.js").unwrap();
      assert_eq!(
        modules.get_children(circular3_id),
        Some(&vec![
          ModuleSpecifier::resolve_url("file:///circular1.js").unwrap(),
          ModuleSpecifier::resolve_url("file:///circular2.js").unwrap()
        ])
      );
    }
    .boxed_local();

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
    let mut isolate = EsIsolate::new(Rc::new(loader), StartupData::None, false);

    let fut = async move {
      let spec = ModuleSpecifier::resolve_url("file:///redirect1.js").unwrap();
      let result = isolate.load_module(&spec, None).await;
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

      let state_rc = EsIsolate::state(&isolate);
      let state = state_rc.borrow();
      let modules = &state.modules;

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
    .boxed_local();

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
    // Does this trigger two ModuleLoader calls? It shouldn't.
    import "/never_ready.js";
    import "/a.js";
  "#;

  #[test]
  fn slow_never_ready_modules() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let loads = loader.loads.clone();
      let mut isolate =
        EsIsolate::new(Rc::new(loader), StartupData::None, false);
      let spec = ModuleSpecifier::resolve_url("file:///main.js").unwrap();
      let mut recursive_load = isolate.load_module(&spec, None).boxed_local();

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
        EsIsolate::new(Rc::new(loader), StartupData::None, false);
      let spec = ModuleSpecifier::resolve_url("file:///bad_import.js").unwrap();
      let mut load_fut = isolate.load_module(&spec, None).boxed_local();
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
    let mut isolate = EsIsolate::new(Rc::new(loader), StartupData::None, false);
    // In default resolution code should be empty.
    // Instead we explicitly pass in our own code.
    // The behavior should be very similar to /a.js.
    let spec =
      ModuleSpecifier::resolve_url("file:///main_with_code.js").unwrap();
    let main_id_fut = isolate
      .load_module(&spec, Some(MAIN_WITH_CODE_SRC.to_owned()))
      .boxed_local();
    let main_id =
      futures::executor::block_on(main_id_fut).expect("Failed to load");

    js_check(isolate.mod_evaluate(main_id));

    let l = loads.lock().unwrap();
    assert_eq!(
      l.to_vec(),
      vec!["file:///b.js", "file:///c.js", "file:///d.js"]
    );

    let state_rc = EsIsolate::state(&isolate);
    let state = state_rc.borrow();
    let modules = &state.modules;

    assert_eq!(modules.get_id("file:///main_with_code.js"), Some(main_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();

    assert_eq!(
      modules.get_children(main_id),
      Some(&vec![
        ModuleSpecifier::resolve_url("file:///b.js").unwrap(),
        ModuleSpecifier::resolve_url("file:///c.js").unwrap()
      ])
    );
    assert_eq!(
      modules.get_children(b_id),
      Some(&vec![ModuleSpecifier::resolve_url("file:///c.js").unwrap()])
    );
    assert_eq!(
      modules.get_children(c_id),
      Some(&vec![ModuleSpecifier::resolve_url("file:///d.js").unwrap()])
    );
    assert_eq!(modules.get_children(d_id), Some(&vec![]));
  }

  #[test]
  fn empty_deps() {
    let modules = Modules::new();
    let specifier = ModuleSpecifier::resolve_url("file:///foo").unwrap();
    assert!(modules.deps(&specifier).is_none());
  }

  #[test]
  fn deps_to_json() {
    fn dep(name: &str, deps: Option<Vec<Deps>>) -> Deps {
      Deps {
        name: name.to_string(),
        deps,
        prefix: "".to_string(),
        is_last: false,
      }
    }
    let deps = dep(
      "a",
      Some(vec![
        dep("b", Some(vec![dep("b2", None)])),
        dep("c", Some(vec![])),
      ]),
    );
    assert_eq!(
      serde_json::json!(["a", [["b", [["b2", []]]], ["c", []]]]),
      deps.to_json()
    );
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

  */
}
