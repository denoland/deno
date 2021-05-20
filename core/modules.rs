// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::bindings;
use crate::error::generic_error;
use crate::error::AnyError;
use crate::module_specifier::ModuleSpecifier;
use crate::runtime::exception_to_err_result;
use crate::OpState;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::StreamFuture;
use futures::stream::TryStreamExt;
use log::debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

lazy_static::lazy_static! {
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
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleSource {
  pub code: String,
  pub module_url_specified: String,
  pub module_url_found: String,
}

pub type PrepareLoadFuture =
  dyn Future<Output = (ModuleLoadId, Result<RecursiveModuleLoad, AnyError>)>;
pub type ModuleSourceFuture =
  dyn Future<Output = Result<ModuleSource, AnyError>>;

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
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError>;

  /// Given ModuleSpecifier, load its source code.
  ///
  /// `is_dyn_import` can be used to check permissions or deny
  /// dynamic imports altogether.
  fn load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>>;

  /// This hook can be used by implementors to do some preparation
  /// work before starting loading of modules.
  ///
  /// For example implementor might download multiple modules in
  /// parallel and transpile them to final JS sources before
  /// yielding control back to the runtime.
  ///
  /// It's not required to implement this method.
  fn prepare_load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    _load_id: ModuleLoadId,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    _is_dyn_import: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    async { Ok(()) }.boxed_local()
  }
}

/// Placeholder structure used when creating
/// a runtime that doesn't support module loading.
pub struct NoopModuleLoader;

impl ModuleLoader for NoopModuleLoader {
  fn resolve(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    _specifier: &str,
    _referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    Err(generic_error("Module loading is not supported"))
  }

  fn load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    _module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
    _is_dyn_import: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    async { Err(generic_error("Module loading is not supported")) }
      .boxed_local()
  }
}

/// Basic file system module loader.
///
/// Note that this loader will **block** event loop
/// when loading file as it uses synchronous FS API
/// from standard library.
pub struct FsModuleLoader;

impl ModuleLoader for FsModuleLoader {
  fn resolve(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    _is_main: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    Ok(crate::resolve_import(specifier, referrer)?)
  }

  fn load(
    &self,
    _op_state: Rc<RefCell<OpState>>,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<ModuleSourceFuture>> {
    let module_specifier = module_specifier.clone();
    async move {
      let path = module_specifier.to_file_path().map_err(|_| {
        generic_error(format!(
          "Provided module specifier \"{}\" is not a file URL.",
          module_specifier
        ))
      })?;
      let code = std::fs::read_to_string(path)?;
      let module = ModuleSource {
        code,
        module_url_specified: module_specifier.to_string(),
        module_url_found: module_specifier.to_string(),
      };
      Ok(module)
    }
    .boxed_local()
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

/// This future is used to implement parallel async module loading.
pub struct RecursiveModuleLoad {
  op_state: Rc<RefCell<OpState>>,
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
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    code: Option<String>,
    loader: Rc<dyn ModuleLoader>,
  ) -> Self {
    let kind = Kind::Main;
    let state = LoadState::ResolveMain(specifier.to_owned(), code);
    Self::new(op_state, kind, state, loader)
  }

  pub fn dynamic_import(
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
    loader: Rc<dyn ModuleLoader>,
  ) -> Self {
    let kind = Kind::DynamicImport;
    let state =
      LoadState::ResolveImport(specifier.to_owned(), referrer.to_owned());
    Self::new(op_state, kind, state, loader)
  }

  pub fn is_dynamic_import(&self) -> bool {
    self.kind != Kind::Main
  }

  fn new(
    op_state: Rc<RefCell<OpState>>,
    kind: Kind,
    state: LoadState,
    loader: Rc<dyn ModuleLoader>,
  ) -> Self {
    Self {
      id: NEXT_LOAD_ID.fetch_add(1, Ordering::SeqCst),
      root_module_id: None,
      op_state,
      kind,
      state,
      loader,
      pending: FuturesUnordered::new(),
      is_pending: HashSet::new(),
    }
  }

  pub async fn prepare(self) -> (ModuleLoadId, Result<Self, AnyError>) {
    let (module_specifier, maybe_referrer) = match self.state {
      LoadState::ResolveMain(ref specifier, _) => {
        let spec =
          match self
            .loader
            .resolve(self.op_state.clone(), specifier, ".", true)
          {
            Ok(spec) => spec,
            Err(e) => return (self.id, Err(e)),
          };
        (spec, None)
      }
      LoadState::ResolveImport(ref specifier, ref referrer) => {
        let spec = match self.loader.resolve(
          self.op_state.clone(),
          specifier,
          referrer,
          false,
        ) {
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
        self.op_state.clone(),
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

  fn add_root(&mut self) -> Result<(), AnyError> {
    let module_specifier = match self.state {
      LoadState::ResolveMain(ref specifier, _) => {
        self
          .loader
          .resolve(self.op_state.clone(), specifier, ".", true)?
      }
      LoadState::ResolveImport(ref specifier, ref referrer) => self
        .loader
        .resolve(self.op_state.clone(), specifier, referrer, false)?,

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
        .load(
          self.op_state.clone(),
          &module_specifier,
          None,
          self.is_dynamic_import(),
        )
        .boxed_local(),
    };

    self.pending.push(load_fut);

    self.state = LoadState::LoadingRoot;
    Ok(())
  }

  pub fn is_currently_loading_main_module(&self) -> bool {
    !self.is_dynamic_import() && self.state == LoadState::LoadingRoot
  }

  pub fn module_registered(&mut self, module_id: ModuleId) {
    // If we just finished loading the root module, store the root module id.
    if self.state == LoadState::LoadingRoot {
      self.root_module_id = Some(module_id);
      self.state = LoadState::LoadingImports;
    }

    if self.pending.is_empty() {
      self.state = LoadState::Done;
    }
  }

  /// Return root `ModuleId`; this function panics
  /// if load is not finished yet.
  pub fn expect_finished(&self) -> ModuleId {
    self.root_module_id.expect("Root module id empty")
  }

  pub fn add_import(
    &mut self,
    specifier: ModuleSpecifier,
    referrer: ModuleSpecifier,
  ) {
    if !self.is_pending.contains(&specifier) {
      let fut = self.loader.load(
        self.op_state.clone(),
        &specifier,
        Some(referrer),
        self.is_dynamic_import(),
      );
      self.pending.push(fut.boxed_local());
      self.is_pending.insert(specifier);
    }
  }
}

impl Stream for RecursiveModuleLoad {
  type Item = Result<ModuleSource, AnyError>;

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
  pub id: ModuleId,
  // Used in "bindings.rs" for "import.meta.main" property value.
  pub main: bool,
  pub name: String,
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

/// A collection of JS modules.
pub struct ModuleMap {
  // Handling of specifiers and v8 objects
  ids_by_handle: HashMap<v8::Global<v8::Module>, ModuleId>,
  handles_by_id: HashMap<ModuleId, v8::Global<v8::Module>>,
  info: HashMap<ModuleId, ModuleInfo>,
  by_name: HashMap<String, SymbolicModule>,
  next_module_id: ModuleId,

  // Handling of futures for loading module sources
  pub loader: Rc<dyn ModuleLoader>,
  op_state: Rc<RefCell<OpState>>,
  pub(crate) dynamic_import_map:
    HashMap<ModuleLoadId, v8::Global<v8::PromiseResolver>>,
  pub(crate) preparing_dynamic_imports:
    FuturesUnordered<Pin<Box<PrepareLoadFuture>>>,
  pub(crate) pending_dynamic_imports:
    FuturesUnordered<StreamFuture<RecursiveModuleLoad>>,
}

impl ModuleMap {
  pub fn new(
    loader: Rc<dyn ModuleLoader>,
    op_state: Rc<RefCell<OpState>>,
  ) -> ModuleMap {
    Self {
      ids_by_handle: HashMap::new(),
      handles_by_id: HashMap::new(),
      info: HashMap::new(),
      by_name: HashMap::new(),
      next_module_id: 1,
      loader,
      op_state,
      dynamic_import_map: HashMap::new(),
      preparing_dynamic_imports: FuturesUnordered::new(),
      pending_dynamic_imports: FuturesUnordered::new(),
    }
  }

  /// Get module id, following all aliases in case of module specifier
  /// that had been redirected.
  pub fn get_id(&self, name: &str) -> Option<ModuleId> {
    let mut mod_name = name;
    loop {
      let symbolic_module = self.by_name.get(mod_name)?;
      match symbolic_module {
        SymbolicModule::Alias(target) => {
          mod_name = target;
        }
        SymbolicModule::Mod(mod_id) => return Some(*mod_id),
      }
    }
  }

  // Create and compile an ES module.
  pub(crate) fn new_module(
    &mut self,
    scope: &mut v8::HandleScope,
    main: bool,
    name: &str,
    source: &str,
  ) -> Result<ModuleId, AnyError> {
    let name_str = v8::String::new(scope, name).unwrap();
    let source_str = v8::String::new(scope, source).unwrap();

    let origin = bindings::module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, Some(&origin));

    let tc_scope = &mut v8::TryCatch::new(scope);

    let maybe_module = v8::script_compiler::compile_module(tc_scope, source);

    if tc_scope.has_caught() {
      assert!(maybe_module.is_none());
      let e = tc_scope.exception().unwrap();
      return exception_to_err_result(tc_scope, e, false);
    }

    let module = maybe_module.unwrap();

    let mut import_specifiers: Vec<ModuleSpecifier> = vec![];
    let module_requests = module.get_module_requests();
    for i in 0..module_requests.length() {
      let module_request = v8::Local::<v8::ModuleRequest>::try_from(
        module_requests.get(tc_scope, i).unwrap(),
      )
      .unwrap();
      let import_specifier = module_request
        .get_specifier()
        .to_rust_string_lossy(tc_scope);
      let module_specifier = self.loader.resolve(
        self.op_state.clone(),
        &import_specifier,
        name,
        false,
      )?;
      import_specifiers.push(module_specifier);
    }

    let handle = v8::Global::<v8::Module>::new(tc_scope, module);
    let id = self.next_module_id;
    self.next_module_id += 1;
    self
      .by_name
      .insert(name.to_string(), SymbolicModule::Mod(id));
    self.handles_by_id.insert(id, handle.clone());
    self.ids_by_handle.insert(handle, id);
    self.info.insert(
      id,
      ModuleInfo {
        id,
        main,
        name: name.to_string(),
        import_specifiers,
      },
    );

    Ok(id)
  }

  pub fn register_during_load(
    &mut self,
    scope: &mut v8::HandleScope,
    module_source: ModuleSource,
    load: &mut RecursiveModuleLoad,
  ) -> Result<(), AnyError> {
    let referrer_specifier =
      crate::resolve_url(&module_source.module_url_found).unwrap();

    // #A There are 3 cases to handle at this moment:
    // 1. Source code resolved result have the same module name as requested
    //    and is not yet registered
    //     -> register
    // 2. Source code resolved result have a different name as requested:
    //   2a. The module with resolved module name has been registered
    //     -> alias
    //   2b. The module with resolved module name has not yet been registered
    //     -> register & alias

    // If necessary, register an alias.
    if module_source.module_url_specified != module_source.module_url_found {
      self.alias(
        &module_source.module_url_specified,
        &module_source.module_url_found,
      );
    }

    let maybe_mod_id = self.get_id(&module_source.module_url_found);

    let module_id = match maybe_mod_id {
      Some(id) => {
        // Module has already been registered.
        debug!(
          "Already-registered module fetched again: {}",
          module_source.module_url_found
        );
        id
      }
      // Module not registered yet, do it now.
      None => self.new_module(
        scope,
        load.is_currently_loading_main_module(),
        &module_source.module_url_found,
        &module_source.code,
      )?,
    };

    // Now we must iterate over all imports of the module and load them.
    let imports = self.get_children(module_id).unwrap().clone();

    for module_specifier in imports {
      let is_registered = self.is_registered(&module_specifier);
      if !is_registered {
        load
          .add_import(module_specifier.to_owned(), referrer_specifier.clone());
      }
    }

    load.module_registered(module_id);

    Ok(())
  }

  pub fn get_children(&self, id: ModuleId) -> Option<&Vec<ModuleSpecifier>> {
    self.info.get(&id).map(|i| &i.import_specifiers)
  }

  pub fn is_registered(&self, specifier: &ModuleSpecifier) -> bool {
    self.get_id(specifier.as_str()).is_some()
  }

  pub fn alias(&mut self, name: &str, target: &str) {
    self
      .by_name
      .insert(name.to_string(), SymbolicModule::Alias(target.to_string()));
  }

  #[cfg(test)]
  pub fn is_alias(&self, name: &str) -> bool {
    let cond = self.by_name.get(name);
    matches!(cond, Some(SymbolicModule::Alias(_)))
  }

  pub fn get_handle(&self, id: ModuleId) -> Option<v8::Global<v8::Module>> {
    self.handles_by_id.get(&id).cloned()
  }

  pub fn get_info(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<&ModuleInfo> {
    if let Some(id) = self.ids_by_handle.get(global) {
      return self.info.get(id);
    }

    None
  }

  pub fn get_info_by_id(&self, id: &ModuleId) -> Option<&ModuleInfo> {
    self.info.get(id)
  }

  pub fn load_main(
    &self,
    specifier: &str,
    code: Option<String>,
  ) -> RecursiveModuleLoad {
    RecursiveModuleLoad::main(
      self.op_state.clone(),
      specifier,
      code,
      self.loader.clone(),
    )
  }

  // Initiate loading of a module graph imported using `import()`.
  pub fn load_dynamic_import(
    &mut self,
    specifier: &str,
    referrer: &str,
    resolver_handle: v8::Global<v8::PromiseResolver>,
  ) {
    let load = RecursiveModuleLoad::dynamic_import(
      self.op_state.clone(),
      specifier,
      referrer,
      self.loader.clone(),
    );
    self.dynamic_import_map.insert(load.id, resolver_handle);
    let fut = load.prepare().boxed_local();
    self.preparing_dynamic_imports.push(fut);
  }

  pub fn has_pending_dynamic_imports(&self) -> bool {
    !(self.preparing_dynamic_imports.is_empty()
      && self.pending_dynamic_imports.is_empty())
  }

  /// Called by `module_resolve_callback` during module instantiation.
  pub fn resolve_callback<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    specifier: &str,
    referrer: &str,
  ) -> Option<v8::Local<'s, v8::Module>> {
    let resolved_specifier = self
      .loader
      .resolve(self.op_state.clone(), specifier, referrer, false)
      .expect("Module should have been already resolved");

    if let Some(id) = self.get_id(resolved_specifier.as_str()) {
      if let Some(handle) = self.get_handle(id) {
        return Some(v8::Local::new(scope, handle));
      }
    }

    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::serialize_op_result;
  use crate::JsRuntime;
  use crate::Op;
  use crate::OpPayload;
  use crate::RuntimeOptions;
  use futures::future::FutureExt;
  use std::error::Error;
  use std::fmt;
  use std::future::Future;
  use std::io;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;
  use std::sync::Mutex;

  // TODO(ry) Sadly FuturesUnordered requires the current task to be set. So
  // even though we are only using poll() in these tests and not Tokio, we must
  // nevertheless run it in the tokio executor. Ideally run_in_task can be
  // removed in the future.
  use crate::runtime::tests::run_in_task;

  #[derive(Default)]
  struct MockLoader {
    pub loads: Arc<Mutex<Vec<String>>>,
  }

  impl MockLoader {
    fn new() -> Rc<Self> {
      Default::default()
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
    type Output = Result<ModuleSource, AnyError>;

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
      _op_state: Rc<RefCell<OpState>>,
      specifier: &str,
      referrer: &str,
      _is_root: bool,
    ) -> Result<ModuleSpecifier, AnyError> {
      let referrer = if referrer == "." {
        "file:///"
      } else {
        referrer
      };

      eprintln!(">> RESOLVING, S: {}, R: {}", specifier, referrer);

      let output_specifier = match crate::resolve_import(specifier, referrer) {
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
      _op_state: Rc<RefCell<OpState>>,
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
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    let spec = crate::resolve_url("file:///a.js").unwrap();
    let a_id_fut = runtime.load_module(&spec, None);
    let a_id = futures::executor::block_on(a_id_fut).expect("Failed to load");

    runtime.mod_evaluate(a_id);
    futures::executor::block_on(runtime.run_event_loop()).unwrap();
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

    let module_map_rc = JsRuntime::module_map(runtime.v8_isolate());
    let modules = module_map_rc.borrow();

    assert_eq!(modules.get_id("file:///a.js"), Some(a_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();
    assert_eq!(
      modules.get_children(a_id),
      Some(&vec![
        crate::resolve_url("file:///b.js").unwrap(),
        crate::resolve_url("file:///c.js").unwrap()
      ])
    );
    assert_eq!(
      modules.get_children(b_id),
      Some(&vec![crate::resolve_url("file:///c.js").unwrap()])
    );
    assert_eq!(
      modules.get_children(c_id),
      Some(&vec![crate::resolve_url("file:///d.js").unwrap()])
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
  fn test_mods() {
    #[derive(Default)]
    struct ModsLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for ModsLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "./b.js");
        assert_eq!(referrer, "file:///a.js");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        unreachable!()
      }
    }

    let loader = Rc::new(ModsLoader::default());

    let resolve_count = loader.count.clone();
    let dispatch_count = Arc::new(AtomicUsize::new(0));
    let dispatch_count_ = dispatch_count.clone();

    let dispatcher = move |state, payload: OpPayload| -> Op {
      dispatch_count_.fetch_add(1, Ordering::Relaxed);
      let (control, _): (u8, ()) = payload.deserialize().unwrap();
      assert_eq!(control, 42);
      let resp = (0, serialize_op_result(Ok(43), state));
      Op::Async(Box::pin(futures::future::ready(resp)))
    };

    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    runtime.register_op("op_test", dispatcher);
    runtime.sync_ops_cache();

    runtime
      .execute(
        "setup.js",
        r#"
        function assert(cond) {
          if (!cond) {
            throw Error("assert");
          }
        }
        "#,
      )
      .unwrap();

    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    let module_map_rc = JsRuntime::module_map(runtime.v8_isolate());

    let (mod_a, mod_b) = {
      let scope = &mut runtime.handle_scope();
      let mut module_map = module_map_rc.borrow_mut();
      let specifier_a = "file:///a.js".to_string();
      let mod_a = module_map
        .new_module(
          scope,
          true,
          &specifier_a,
          r#"
          import { b } from './b.js'
          if (b() != 'b') throw Error();
          let control = 42;
          Deno.core.opAsync("op_test", control);
        "#,
        )
        .unwrap();

      assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
      let imports = module_map.get_children(mod_a);
      assert_eq!(
        imports,
        Some(&vec![crate::resolve_url("file:///b.js").unwrap()])
      );

      let mod_b = module_map
        .new_module(
          scope,
          false,
          "file:///b.js",
          "export function b() { return 'b' }",
        )
        .unwrap();
      let imports = module_map.get_children(mod_b).unwrap();
      assert_eq!(imports.len(), 0);
      (mod_a, mod_b)
    };

    runtime.instantiate_module(mod_b).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    runtime.instantiate_module(mod_a).unwrap();
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 0);

    runtime.mod_evaluate(mod_a);
    assert_eq!(dispatch_count.load(Ordering::Relaxed), 1);
  }

  #[test]
  fn dyn_import_err() {
    #[derive(Clone, Default)]
    struct DynImportErrLoader {
      pub count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for DynImportErrLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        self.count.fetch_add(1, Ordering::Relaxed);
        assert_eq!(specifier, "/foo.js");
        assert_eq!(referrer, "file:///dyn_import2.js");
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        _module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        async { Err(io::Error::from(io::ErrorKind::NotFound).into()) }.boxed()
      }
    }

    // Test an erroneous dynamic import where the specified module isn't found.
    run_in_task(|cx| {
      let loader = Rc::new(DynImportErrLoader::default());
      let count = loader.count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });

      runtime
        .execute(
          "file:///dyn_import2.js",
          r#"
        (async () => {
          await import("/foo.js");
        })();
        "#,
        )
        .unwrap();

      assert_eq!(count.load(Ordering::Relaxed), 0);
      // We should get an error here.
      let result = runtime.poll_event_loop(cx);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
      assert_eq!(count.load(Ordering::Relaxed), 2);
    })
  }

  #[derive(Clone, Default)]
  struct DynImportOkLoader {
    pub prepare_load_count: Arc<AtomicUsize>,
    pub resolve_count: Arc<AtomicUsize>,
    pub load_count: Arc<AtomicUsize>,
  }

  impl ModuleLoader for DynImportOkLoader {
    fn resolve(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      specifier: &str,
      referrer: &str,
      _is_main: bool,
    ) -> Result<ModuleSpecifier, AnyError> {
      let c = self.resolve_count.fetch_add(1, Ordering::Relaxed);
      assert!(c < 4);
      assert_eq!(specifier, "./b.js");
      assert_eq!(referrer, "file:///dyn_import3.js");
      let s = crate::resolve_import(specifier, referrer).unwrap();
      Ok(s)
    }

    fn load(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      specifier: &ModuleSpecifier,
      _maybe_referrer: Option<ModuleSpecifier>,
      _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
      self.load_count.fetch_add(1, Ordering::Relaxed);
      let info = ModuleSource {
        module_url_specified: specifier.to_string(),
        module_url_found: specifier.to_string(),
        code: "export function b() { return 'b' }".to_owned(),
      };
      async move { Ok(info) }.boxed()
    }

    fn prepare_load(
      &self,
      _op_state: Rc<RefCell<OpState>>,
      _load_id: ModuleLoadId,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<String>,
      _is_dyn_import: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
      self.prepare_load_count.fetch_add(1, Ordering::Relaxed);
      async { Ok(()) }.boxed_local()
    }
  }

  #[test]
  fn dyn_import_ok() {
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let resolve_count = loader.resolve_count.clone();
      let load_count = loader.load_count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });

      // Dynamically import mod_b
      runtime
        .execute(
          "file:///dyn_import3.js",
          r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad1");
            }
            // And again!
            mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad2");
            }
          })();
          "#,
        )
        .unwrap();

      // First poll runs `prepare_load` hook.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Pending));
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);

      // Second poll actually loads modules into the isolate.
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
      assert!(matches!(runtime.poll_event_loop(cx), Poll::Ready(Ok(_))));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 4);
      assert_eq!(load_count.load(Ordering::Relaxed), 2);
    })
  }

  #[test]
  fn dyn_import_borrow_mut_error() {
    // https://github.com/denoland/deno/issues/6054
    run_in_task(|cx| {
      let loader = Rc::new(DynImportOkLoader::default());
      let prepare_load_count = loader.prepare_load_count.clone();
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });
      runtime.sync_ops_cache();
      runtime
        .execute(
          "file:///dyn_import3.js",
          r#"
          (async () => {
            let mod = await import("./b.js");
            if (mod.b() !== 'b') {
              throw Error("bad");
            }
          })();
          "#,
        )
        .unwrap();
      // First poll runs `prepare_load` hook.
      let _ = runtime.poll_event_loop(cx);
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);
      // Second poll triggers error
      let _ = runtime.poll_event_loop(cx);
    })
  }

  #[test]
  fn test_circular_load() {
    let loader = MockLoader::new();
    let loads = loader.loads.clone();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });

    let fut = async move {
      let spec = crate::resolve_url("file:///circular1.js").unwrap();
      let result = runtime.load_module(&spec, None).await;
      assert!(result.is_ok());
      let circular1_id = result.unwrap();
      runtime.mod_evaluate(circular1_id);
      runtime.run_event_loop().await.unwrap();

      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///circular1.js",
          "file:///circular2.js",
          "file:///circular3.js"
        ]
      );

      let module_map_rc = JsRuntime::module_map(runtime.v8_isolate());
      let modules = module_map_rc.borrow();

      assert_eq!(modules.get_id("file:///circular1.js"), Some(circular1_id));
      let circular2_id = modules.get_id("file:///circular2.js").unwrap();

      assert_eq!(
        modules.get_children(circular1_id),
        Some(&vec![crate::resolve_url("file:///circular2.js").unwrap()])
      );

      assert_eq!(
        modules.get_children(circular2_id),
        Some(&vec![crate::resolve_url("file:///circular3.js").unwrap()])
      );

      assert!(modules.get_id("file:///circular3.js").is_some());
      let circular3_id = modules.get_id("file:///circular3.js").unwrap();
      assert_eq!(
        modules.get_children(circular3_id),
        Some(&vec![
          crate::resolve_url("file:///circular1.js").unwrap(),
          crate::resolve_url("file:///circular2.js").unwrap()
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
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });

    let fut = async move {
      let spec = crate::resolve_url("file:///redirect1.js").unwrap();
      let result = runtime.load_module(&spec, None).await;
      println!(">> result {:?}", result);
      assert!(result.is_ok());
      let redirect1_id = result.unwrap();
      runtime.mod_evaluate(redirect1_id);
      runtime.run_event_loop().await.unwrap();
      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///redirect1.js",
          "file:///redirect2.js",
          "file:///dir/redirect3.js"
        ]
      );

      let module_map_rc = JsRuntime::module_map(runtime.v8_isolate());
      let modules = module_map_rc.borrow();

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
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });
      let spec = crate::resolve_url("file:///main.js").unwrap();
      let mut recursive_load = runtime.load_module(&spec, None).boxed_local();

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
      let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        ..Default::default()
      });
      let spec = crate::resolve_url("file:///bad_import.js").unwrap();
      let mut load_fut = runtime.load_module(&spec, None).boxed_local();
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
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });
    // In default resolution code should be empty.
    // Instead we explicitly pass in our own code.
    // The behavior should be very similar to /a.js.
    let spec = crate::resolve_url("file:///main_with_code.js").unwrap();
    let main_id_fut = runtime
      .load_module(&spec, Some(MAIN_WITH_CODE_SRC.to_owned()))
      .boxed_local();
    let main_id =
      futures::executor::block_on(main_id_fut).expect("Failed to load");

    runtime.mod_evaluate(main_id);
    futures::executor::block_on(runtime.run_event_loop()).unwrap();

    let l = loads.lock().unwrap();
    assert_eq!(
      l.to_vec(),
      vec!["file:///b.js", "file:///c.js", "file:///d.js"]
    );

    let module_map_rc = JsRuntime::module_map(runtime.v8_isolate());
    let modules = module_map_rc.borrow();

    assert_eq!(modules.get_id("file:///main_with_code.js"), Some(main_id));
    let b_id = modules.get_id("file:///b.js").unwrap();
    let c_id = modules.get_id("file:///c.js").unwrap();
    let d_id = modules.get_id("file:///d.js").unwrap();

    assert_eq!(
      modules.get_children(main_id),
      Some(&vec![
        crate::resolve_url("file:///b.js").unwrap(),
        crate::resolve_url("file:///c.js").unwrap()
      ])
    );
    assert_eq!(
      modules.get_children(b_id),
      Some(&vec![crate::resolve_url("file:///c.js").unwrap()])
    );
    assert_eq!(
      modules.get_children(c_id),
      Some(&vec![crate::resolve_url("file:///d.js").unwrap()])
    );
    assert_eq!(modules.get_children(d_id), Some(&vec![]));
  }
}
