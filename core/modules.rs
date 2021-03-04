// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::bindings::throw_type_error;
use crate::error::generic_error;
use crate::error::AnyError;
use crate::module_specifier::ModuleSpecifier;
use crate::runtime::exception_to_err_result;
use crate::JsRuntime;
use crate::OpState;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::convert::TryInto;
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

pub extern "C" fn host_import_module_dynamically_callback(
  context: v8::Local<v8::Context>,
  referrer: v8::Local<v8::ScriptOrModule>,
  specifier: v8::Local<v8::String>,
  _import_assertions: v8::Local<v8::FixedArray>,
) -> *mut v8::Promise {
  let scope = &mut unsafe { v8::CallbackScope::new(context) };

  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);
  let referrer_name = referrer.get_resource_name();
  let referrer_name_str = referrer_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  // TODO(ry) I'm not sure what HostDefinedOptions is for or if we're ever going
  // to use it. For now we check that it is not used. This check may need to be
  // changed in the future.
  let host_defined_options = referrer.get_host_defined_options();
  assert_eq!(host_defined_options.length(), 0);

  let resolver = v8::PromiseResolver::new(scope).unwrap();
  let promise = resolver.get_promise(scope);

  let resolver_handle = v8::Global::new(scope, resolver);
  {
    let state_rc = JsRuntime::state(scope);
    let mut state = state_rc.borrow_mut();
    state.dyn_import_cb(resolver_handle, &specifier_str, &referrer_name_str);
  }

  // Map errors from module resolution (not JS errors from module execution) to
  // ones rethrown from this scope, so they include the call stack of the
  // dynamic import site. Error objects without any stack frames are assumed to
  // be module resolution errors, other exception values are left as they are.
  let map_err = |scope: &mut v8::HandleScope,
                 args: v8::FunctionCallbackArguments,
                 _rv: v8::ReturnValue| {
    let arg = args.get(0);
    if arg.is_native_error() {
      let message = v8::Exception::create_message(scope, arg);
      if message.get_stack_trace(scope).unwrap().get_frame_count() == 0 {
        let arg: v8::Local<v8::Object> = arg.clone().try_into().unwrap();
        let message_key = v8::String::new(scope, "message").unwrap();
        let message = arg.get(scope, message_key.into()).unwrap();
        let exception =
          v8::Exception::type_error(scope, message.try_into().unwrap());
        scope.throw_exception(exception);
        return;
      }
    }
    scope.throw_exception(arg);
  };
  let map_err = v8::FunctionTemplate::new(scope, map_err);
  let map_err = map_err.get_function(scope).unwrap();
  let promise = promise.catch(scope, map_err).unwrap();

  &*promise as *const _ as *mut _
}

pub extern "C" fn host_initialize_import_meta_object_callback(
  context: v8::Local<v8::Context>,
  module: v8::Local<v8::Module>,
  meta: v8::Local<v8::Object>,
) {
  let scope = &mut unsafe { v8::CallbackScope::new(context) };
  let state_rc = JsRuntime::state(scope);
  let state = state_rc.borrow();

  let module_global = v8::Global::new(scope, module);
  let info = state
    .module_map
    .get_info(&module_global)
    .expect("Module not found");

  let url_key = v8::String::new(scope, "url").unwrap();
  let url_val = v8::String::new(scope, &info.name).unwrap();
  meta.create_data_property(scope, url_key.into(), url_val.into());

  let main_key = v8::String::new(scope, "main").unwrap();
  let main_val = v8::Boolean::new(scope, info.main);
  meta.create_data_property(scope, main_key.into(), main_val.into());
}

pub fn module_origin<'a>(
  s: &mut v8::HandleScope<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let source_map_url = v8::String::new(s, "").unwrap();
  v8::ScriptOrigin::new(
    s,
    resource_name.into(),
    0,
    0,
    false,
    123,
    source_map_url.into(),
    true,
    false,
    true,
  )
}

pub fn compile_module(
  scope: &mut v8::HandleScope,
  specifier: &str,
  source: &str,
) -> Result<v8::Global<v8::Module>, AnyError> {
  let specifier_str = v8::String::new(scope, specifier).unwrap();
  let source_str = v8::String::new(scope, source).unwrap();

  let origin = module_origin(scope, specifier_str);
  let source = v8::script_compiler::Source::new(source_str, &origin);

  let module = {
    let tc_scope = &mut v8::TryCatch::new(scope);
    let maybe_module = v8::script_compiler::compile_module(tc_scope, source);
    if tc_scope.has_caught() {
      assert!(maybe_module.is_none());
      let e = tc_scope.exception().unwrap();
      return exception_to_err_result(tc_scope, e, false);
    }
    maybe_module.unwrap()
  };

  Ok(v8::Global::<v8::Module>::new(scope, module))
}

// Called by V8 during `Isolate::mod_instantiate`.
pub fn module_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  _import_assertions: v8::Local<'s, v8::FixedArray>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  let scope = &mut unsafe { v8::CallbackScope::new(context) };

  let state_rc = JsRuntime::state(scope);
  let state = state_rc.borrow();

  let referrer_global = v8::Global::new(scope, referrer);
  let referrer_info = state
    .module_map
    .get_info(&referrer_global)
    .expect("ModuleInfo not found");
  let referrer_name = referrer_info.name.to_string();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  let resolved_specifier = state
    .module_map
    .loader
    .resolve(
      state.op_state.clone(),
      &specifier_str,
      &referrer_name,
      false,
    )
    .expect("Module should have been already resolved");

  if let Some(id) = state.module_map.get_id(resolved_specifier.as_str()) {
    if let Some(handle) = state.module_map.get_handle(id) {
      return Some(v8::Local::new(scope, handle));
    }
  }

  let msg = format!(
    r#"Cannot resolve module "{}" from "{}""#,
    specifier_str, referrer_name
  );
  throw_type_error(scope, msg);
  None
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
  ids_by_handle: HashMap<v8::Global<v8::Module>, ModuleId>,
  handles_by_id: HashMap<ModuleId, v8::Global<v8::Module>>,
  info: HashMap<ModuleId, ModuleInfo>,
  by_name: HashMap<String, SymbolicModule>,
  next_module_id: ModuleId,
  pub loader: Rc<dyn ModuleLoader>,
}

impl ModuleMap {
  pub fn new(loader: Rc<dyn ModuleLoader>) -> ModuleMap {
    Self {
      handles_by_id: HashMap::new(),
      ids_by_handle: HashMap::new(),
      info: HashMap::new(),
      by_name: HashMap::new(),
      next_module_id: 1,
      loader,
    }
  }

  // TODO(bartlomieju): remove `op_state` param
  pub fn create_module(
    &mut self,
    scope: &mut v8::HandleScope,
    specifier: &str,
    source: &str,
    main: bool,
    op_state: Rc<RefCell<OpState>>,
  ) -> Result<ModuleId, AnyError> {
    let module_handle = compile_module(scope, specifier, source)?;

    let module = module_handle.get(scope);
    let mut import_specifiers: Vec<ModuleSpecifier> = vec![];
    let module_requests = module.get_module_requests();
    for i in 0..module_requests.length() {
      let module_request = v8::Local::<v8::ModuleRequest>::try_from(
        module_requests.get(scope, i).unwrap(),
      )
      .unwrap();
      let import_specifier =
        module_request.get_specifier().to_rust_string_lossy(scope);
      let module_specifier = self.loader.resolve(
        op_state.clone(),
        &import_specifier,
        specifier,
        false,
      )?;
      import_specifiers.push(module_specifier);
    }

    let id = self.register(specifier, main, module_handle, import_specifiers);

    Ok(id)
  }

  // TODO(bartlomieju): remove `op_state` param
  pub fn register_during_load(
    &mut self,
    info: ModuleSource,
    load: &mut RecursiveModuleLoad,
    scope: &mut v8::HandleScope,
    op_state: Rc<RefCell<OpState>>,
  ) -> Result<(), AnyError> {
    let ModuleSource {
      code,
      module_url_specified,
      module_url_found,
    } = info;

    let is_main =
      load.state == LoadState::LoadingRoot && !load.is_dynamic_import();
    let referrer_specifier = crate::resolve_url(&module_url_found).unwrap();

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
    // TODO(bartlomieju): handle multiple redirects
    if module_url_specified != module_url_found {
      self.alias(&module_url_specified, &module_url_found);
    }

    let maybe_mod_id = self.get_id(&module_url_found);

    let module_id = match maybe_mod_id {
      Some(id) => {
        // Module has already been registered.
        debug!(
          "Already-registered module fetched again: {}",
          module_url_found
        );
        id
      }
      // Module not registered yet, do it now.
      None => self.create_module(
        scope,
        &module_url_found,
        &code,
        is_main,
        op_state,
      )?,
    };

    // Now we must iterate over all imports of the module and load them.
    let imports = { self.get_children(module_id).unwrap().clone() };

    for module_specifier in imports {
      let is_registered = self.is_registered(&module_specifier);
      if !is_registered {
        load
          .add_import(module_specifier.to_owned(), referrer_specifier.clone());
      }
    }

    // If we just finished loading the root module, store the root module id.
    if load.state == LoadState::LoadingRoot {
      load.root_module_id = Some(module_id);
      load.state = LoadState::LoadingImports;
    }

    if load.pending.is_empty() {
      load.state = LoadState::Done;
    }

    Ok(())
  }

  // TODO(bartlomieju): remove `op_state` param
  pub fn dynamic_import_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    referrer: &str,
  ) -> RecursiveModuleLoad {
    RecursiveModuleLoad::dynamic_import(
      op_state,
      specifier,
      referrer,
      self.loader.clone(),
    )
  }

  // TODO(bartlomieju): remove `op_state` param
  pub fn main_module_load(
    &self,
    op_state: Rc<RefCell<OpState>>,
    specifier: &str,
    maybe_code: Option<String>,
  ) -> RecursiveModuleLoad {
    RecursiveModuleLoad::main(
      op_state,
      specifier,
      maybe_code,
      self.loader.clone(),
    )
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

  pub fn get_children(&self, id: ModuleId) -> Option<&Vec<ModuleSpecifier>> {
    self.info.get(&id).map(|i| &i.import_specifiers)
  }

  pub fn is_registered(&self, specifier: &ModuleSpecifier) -> bool {
    self.get_id(&specifier.to_string()).is_some()
  }

  pub fn register(
    &mut self,
    name: &str,
    main: bool,
    handle: v8::Global<v8::Module>,
    import_specifiers: Vec<ModuleSpecifier>,
  ) -> ModuleId {
    let name = String::from(name);
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
        name,
        import_specifiers,
      },
    );
    id
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
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::JsRuntime;
  use crate::RuntimeOptions;
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

    let state_rc = JsRuntime::state(runtime.v8_isolate());
    let state = state_rc.borrow();
    let modules = &state.module_map;
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

      let state_rc = JsRuntime::state(runtime.v8_isolate());
      let state = state_rc.borrow();
      let modules = &state.module_map;

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

      let state_rc = JsRuntime::state(runtime.v8_isolate());
      let state = state_rc.borrow();
      let modules = &state.module_map;

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

    let state_rc = JsRuntime::state(runtime.v8_isolate());
    let state = state_rc.borrow();
    let modules = &state.module_map;

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
