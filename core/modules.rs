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
use std::collections::VecDeque;
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

/// Describes the entrypoint of a recursive module load.
#[derive(Debug)]
enum LoadInit {
  /// Main module specifier.
  Main(String),
  /// Dynamic import specifier with referrer.
  DynamicImport(String, String),
}

#[derive(Debug, Eq, PartialEq)]
pub enum LoadState {
  Init,
  LoadingRoot,
  LoadingImports,
  Done,
}

/// This future is used to implement parallel async module loading.
pub struct RecursiveModuleLoad {
  init: LoadInit,
  // TODO(bartlomieju): in future this value should
  // be randomized
  pub id: ModuleLoadId,
  pub root_module_id: Option<ModuleId>,
  pub state: LoadState,
  pub module_map_rc: Rc<RefCell<ModuleMap>>,
  // These two fields are copied from `module_map_rc`, but they are cloned ahead
  // of time to avoid already-borrowed errors.
  pub op_state: Rc<RefCell<OpState>>,
  pub loader: Rc<dyn ModuleLoader>,
  pub pending: FuturesUnordered<Pin<Box<ModuleSourceFuture>>>,
  pub visited: HashSet<ModuleSpecifier>,
}

impl RecursiveModuleLoad {
  /// Starts a new parallel load of the given URL of the main module.
  pub fn main(specifier: &str, module_map_rc: Rc<RefCell<ModuleMap>>) -> Self {
    Self::new(LoadInit::Main(specifier.to_string()), module_map_rc)
  }

  pub fn dynamic_import(
    specifier: &str,
    referrer: &str,
    module_map_rc: Rc<RefCell<ModuleMap>>,
  ) -> Self {
    let init =
      LoadInit::DynamicImport(specifier.to_string(), referrer.to_string());
    Self::new(init, module_map_rc)
  }

  pub fn is_dynamic_import(&self) -> bool {
    matches!(self.init, LoadInit::DynamicImport(..))
  }

  fn new(init: LoadInit, module_map_rc: Rc<RefCell<ModuleMap>>) -> Self {
    let op_state = module_map_rc.borrow().op_state.clone();
    let loader = module_map_rc.borrow().loader.clone();
    let mut load = Self {
      id: NEXT_LOAD_ID.fetch_add(1, Ordering::SeqCst),
      root_module_id: None,
      init,
      state: LoadState::Init,
      module_map_rc: module_map_rc.clone(),
      op_state,
      loader,
      pending: FuturesUnordered::new(),
      visited: HashSet::new(),
    };
    // Ignore the error here, let it be hit in `Stream::poll_next()`.
    if let Ok(root_specifier) = load.resolve_root() {
      if let Some(module_id) =
        module_map_rc.borrow().get_id(root_specifier.as_str())
      {
        load.root_module_id = Some(module_id);
      }
    }
    load
  }

  pub fn resolve_root(&self) -> Result<ModuleSpecifier, AnyError> {
    match self.init {
      LoadInit::Main(ref specifier) => {
        self
          .loader
          .resolve(self.op_state.clone(), specifier, ".", true)
      }
      LoadInit::DynamicImport(ref specifier, ref referrer) => self
        .loader
        .resolve(self.op_state.clone(), specifier, referrer, false),
    }
  }

  pub async fn prepare(&self) -> Result<(), AnyError> {
    let op_state = self.op_state.clone();
    let (module_specifier, maybe_referrer) = match self.init {
      LoadInit::Main(ref specifier) => {
        let spec =
          self
            .loader
            .resolve(op_state.clone(), specifier, ".", true)?;
        (spec, None)
      }
      LoadInit::DynamicImport(ref specifier, ref referrer) => {
        let spec =
          self
            .loader
            .resolve(op_state.clone(), specifier, referrer, false)?;
        (spec, Some(referrer.to_string()))
      }
    };

    self
      .loader
      .prepare_load(
        op_state,
        self.id,
        &module_specifier,
        maybe_referrer,
        self.is_dynamic_import(),
      )
      .await
  }

  pub fn is_currently_loading_main_module(&self) -> bool {
    !self.is_dynamic_import() && self.state == LoadState::LoadingRoot
  }

  pub fn register_and_recurse(
    &mut self,
    scope: &mut v8::HandleScope,
    module_source: &ModuleSource,
  ) -> Result<(), AnyError> {
    // Register the module in the module map unless it's already there. If the
    // specified URL and the "true" URL are different, register the alias.
    if module_source.module_url_specified != module_source.module_url_found {
      self.module_map_rc.borrow_mut().alias(
        &module_source.module_url_specified,
        &module_source.module_url_found,
      );
    }
    let maybe_module_id = self
      .module_map_rc
      .borrow()
      .get_id(&module_source.module_url_found);
    let module_id = match maybe_module_id {
      Some(id) => {
        debug!(
          "Already-registered module fetched again: {}",
          module_source.module_url_found
        );
        id
      }
      None => self.module_map_rc.borrow_mut().new_module(
        scope,
        self.is_currently_loading_main_module(),
        &module_source.module_url_found,
        &module_source.code,
      )?,
    };

    // Recurse the module's imports. There are two cases for each import:
    // 1. If the module is not in the module map, start a new load for it in
    //    `self.pending`. The result of that load should eventually be passed to
    //    this function for recursion.
    // 2. If the module is already in the module map, queue it up to be
    //    recursed synchronously here.
    // This robustly ensures that the whole graph is in the module map before
    // `LoadState::Done` is set.
    let specifier =
      crate::resolve_url(&module_source.module_url_found).unwrap();
    let mut already_registered = VecDeque::new();
    already_registered.push_back((module_id, specifier.clone()));
    self.visited.insert(specifier);
    while let Some((module_id, referrer)) = already_registered.pop_front() {
      let imports = self
        .module_map_rc
        .borrow()
        .get_children(module_id)
        .unwrap()
        .clone();
      for specifier in imports {
        if !self.visited.contains(&specifier) {
          if let Some(module_id) =
            self.module_map_rc.borrow().get_id(specifier.as_str())
          {
            already_registered.push_back((module_id, specifier.clone()));
          } else {
            let fut = self.loader.load(
              self.op_state.clone(),
              &specifier,
              Some(referrer.clone()),
              self.is_dynamic_import(),
            );
            self.pending.push(fut.boxed_local());
          }
          self.visited.insert(specifier);
        }
      }
    }

    // Update `self.state` however applicable.
    if self.state == LoadState::LoadingRoot {
      self.root_module_id = Some(module_id);
      self.state = LoadState::LoadingImports;
    }
    if self.pending.is_empty() {
      self.state = LoadState::Done;
    }

    Ok(())
  }
}

impl Stream for RecursiveModuleLoad {
  type Item = Result<ModuleSource, AnyError>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    // IMPORTANT: Do not borrow `inner.module_map_rc` here. It may not be
    // available.
    match inner.state {
      LoadState::Init => {
        let module_specifier = match inner.resolve_root() {
          Ok(url) => url,
          Err(error) => return Poll::Ready(Some(Err(error))),
        };
        let load_fut = if let Some(_module_id) = inner.root_module_id {
          // The root module is already in the module map.
          // TODO(nayeemrmn): In this case we would ideally skip to
          // `LoadState::LoadingImports` and synchronously recurse the imports
          // like the bottom of `RecursiveModuleLoad::register_and_recurse()`.
          // But the module map cannot be borrowed here. Instead fake a load
          // event so it gets passed to that function and recursed eventually.
          futures::future::ok(ModuleSource {
            module_url_specified: module_specifier.to_string(),
            module_url_found: module_specifier.to_string(),
            // The code will be discarded, since this module is already in the
            // module map.
            code: Default::default(),
          })
          .boxed()
        } else {
          let maybe_referrer = match inner.init {
            LoadInit::DynamicImport(_, ref referrer) => {
              crate::resolve_url(referrer).ok()
            }
            _ => None,
          };
          inner
            .loader
            .load(
              inner.op_state.clone(),
              &module_specifier,
              maybe_referrer,
              inner.is_dynamic_import(),
            )
            .boxed_local()
        };
        inner.pending.push(load_fut);
        inner.state = LoadState::LoadingRoot;
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

  pub async fn load_main(
    module_map_rc: Rc<RefCell<ModuleMap>>,
    specifier: &str,
  ) -> Result<RecursiveModuleLoad, AnyError> {
    let load = RecursiveModuleLoad::main(specifier, module_map_rc.clone());
    load.prepare().await?;
    Ok(load)
  }

  // Initiate loading of a module graph imported using `import()`.
  pub fn load_dynamic_import(
    module_map_rc: Rc<RefCell<ModuleMap>>,
    specifier: &str,
    referrer: &str,
    resolver_handle: v8::Global<v8::PromiseResolver>,
  ) {
    let load = RecursiveModuleLoad::dynamic_import(
      specifier,
      referrer,
      module_map_rc.clone(),
    );
    module_map_rc
      .borrow_mut()
      .dynamic_import_map
      .insert(load.id, resolver_handle);
    let resolve_result = module_map_rc.borrow().loader.resolve(
      module_map_rc.borrow().op_state.clone(),
      specifier,
      referrer,
      false,
    );
    let fut = match resolve_result {
      Ok(module_specifier) => {
        if module_map_rc.borrow().is_registered(&module_specifier) {
          async move { (load.id, Ok(load)) }.boxed_local()
        } else {
          async move { (load.id, load.prepare().await.map(|()| load)) }
            .boxed_local()
        }
      }
      Err(error) => async move { (load.id, Err(error)) }.boxed_local(),
    };
    module_map_rc
      .borrow_mut()
      .preparing_dynamic_imports
      .push(fut);
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
  use parking_lot::Mutex;
  use std::error::Error;
  use std::fmt;
  use std::future::Future;
  use std::io;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

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
      let mut loads = self.loads.lock();
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

    let _ = runtime.mod_evaluate(a_id);
    futures::executor::block_on(runtime.run_event_loop(false)).unwrap();
    let l = loads.lock();
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
      .execute_script(
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

    let _ = runtime.mod_evaluate(mod_a);
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
        .execute_script(
          "file:///dyn_import2.js",
          r#"
        (async () => {
          await import("/foo.js");
        })();
        "#,
        )
        .unwrap();

      // We should get an error here.
      let result = runtime.poll_event_loop(cx, false);
      if let Poll::Ready(Ok(_)) = result {
        unreachable!();
      }
      assert_eq!(count.load(Ordering::Relaxed), 4);
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
      assert!(c < 7);
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
        .execute_script(
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
      assert!(matches!(runtime.poll_event_loop(cx, false), Poll::Pending));
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);

      // Second poll actually loads modules into the isolate.
      assert!(matches!(
        runtime.poll_event_loop(cx, false),
        Poll::Ready(Ok(_))
      ));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 7);
      assert_eq!(load_count.load(Ordering::Relaxed), 1);
      assert!(matches!(
        runtime.poll_event_loop(cx, false),
        Poll::Ready(Ok(_))
      ));
      assert_eq!(resolve_count.load(Ordering::Relaxed), 7);
      assert_eq!(load_count.load(Ordering::Relaxed), 1);
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
        .execute_script(
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
      let _ = runtime.poll_event_loop(cx, false);
      assert_eq!(prepare_load_count.load(Ordering::Relaxed), 1);
      // Second poll triggers error
      let _ = runtime.poll_event_loop(cx, false);
    })
  }

  // Regression test for https://github.com/denoland/deno/issues/3736.
  #[test]
  fn dyn_concurrent_circular_import() {
    #[derive(Clone, Default)]
    struct DynImportCircularLoader {
      pub resolve_count: Arc<AtomicUsize>,
      pub load_count: Arc<AtomicUsize>,
    }

    impl ModuleLoader for DynImportCircularLoader {
      fn resolve(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
      ) -> Result<ModuleSpecifier, AnyError> {
        self.resolve_count.fetch_add(1, Ordering::Relaxed);
        let s = crate::resolve_import(specifier, referrer).unwrap();
        Ok(s)
      }

      fn load(
        &self,
        _op_state: Rc<RefCell<OpState>>,
        specifier: &ModuleSpecifier,
        maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
      ) -> Pin<Box<ModuleSourceFuture>> {
        self.load_count.fetch_add(1, Ordering::Relaxed);
        let filename = PathBuf::from(specifier.to_string())
          .file_name()
          .unwrap()
          .to_string_lossy()
          .to_string();
        eprintln!("{} from {:?}", filename.as_str(), maybe_referrer);
        let code = match filename.as_str() {
          "a.js" => "import './b.js';",
          "b.js" => "import './c.js';\nimport './a.js';",
          "c.js" => "import './d.js';",
          "d.js" => "// pass",
          _ => unreachable!(),
        };
        let info = ModuleSource {
          module_url_specified: specifier.to_string(),
          module_url_found: specifier.to_string(),
          code: code.to_owned(),
        };
        async move { Ok(info) }.boxed()
      }
    }

    let loader = Rc::new(DynImportCircularLoader::default());
    let resolve_count = loader.resolve_count.clone();
    let load_count = loader.load_count.clone();
    let mut runtime = JsRuntime::new(RuntimeOptions {
      module_loader: Some(loader),
      ..Default::default()
    });

    runtime
      .execute_script(
        "file:///entry.js",
        "import('./b.js');\nimport('./a.js');",
      )
      .unwrap();

    let result = futures::executor::block_on(runtime.run_event_loop(false));
    eprintln!("result {:?}", result);
    assert!(result.is_ok());
    eprintln!("{}", resolve_count.load(Ordering::Relaxed));
    eprintln!("{}", load_count.load(Ordering::Relaxed));
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
      let _ = runtime.mod_evaluate(circular1_id);
      runtime.run_event_loop(false).await.unwrap();

      let l = loads.lock();
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
      let _ = runtime.mod_evaluate(redirect1_id);
      runtime.run_event_loop(false).await.unwrap();
      let l = loads.lock();
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
        let l = loads.lock();
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

    let _ = runtime.mod_evaluate(main_id);
    futures::executor::block_on(runtime.run_event_loop(false)).unwrap();

    let l = loads.lock();
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
