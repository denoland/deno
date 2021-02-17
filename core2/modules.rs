// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;

use crate::bindings::throw_type_error;
use crate::error::attach_handle_to_error;
use crate::error::generic_error;
use crate::error::AnyError;
use crate::module_specifier::ModuleSpecifier;
use crate::runtime::exception_to_err_result;
use crate::JsRuntime;
use futures::future::poll_fn;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::task::Poll;

pub extern "C" fn host_import_module_dynamically_callback(
  _context: v8::Local<v8::Context>,
  _referrer: v8::Local<v8::ScriptOrModule>,
  _specifier: v8::Local<v8::String>,
  _import_assertions: v8::Local<v8::FixedArray>,
) -> *mut v8::Promise {
  todo!()
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

  // FIXME(bartlomieju): import map support
  let resolved_specifier =
    ModuleSpecifier::resolve_import(&specifier_str, &referrer_name)
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

// TODO(bartlomieju): this can be a method on the `ModuleMap`
/// Low-level module creation.
///
/// Called during module loading or dynamic import loading.
pub fn create_module(
  js_runtime: &mut JsRuntime,
  info: ModuleSource,
  main: bool,
) -> Result<ModuleId, AnyError> {
  let state_rc = JsRuntime::state(js_runtime.v8_isolate());

  if info.module_url_specified != info.module_url_found {
    state_rc
      .borrow_mut()
      .module_map
      .alias(&info.module_url_specified, &info.module_url_found);
  }

  let maybe_module_id =
    state_rc.borrow().module_map.get_id(&info.module_url_found);

  if let Some(id) = maybe_module_id {
    // Module has already been registered.
    debug!(
      "Already-registered module fetched again: {}",
      info.module_url_found
    );
    return Ok(id);
  }

  let module_handle =
    compile_module(js_runtime, &info.module_url_found, &info.code)?;
  let id = state_rc.borrow_mut().module_map.register(
    &info.module_url_found,
    main,
    module_handle,
  );

  Ok(id)
}

pub fn compile_module(
  js_runtime: &mut JsRuntime,
  specifier: &str,
  source: &str,
) -> Result<v8::Global<v8::Module>, AnyError> {
  let context = js_runtime.global_context();
  let scope =
    &mut v8::HandleScope::with_context(js_runtime.v8_isolate(), context);

  let specifier_str = v8::String::new(scope, specifier).unwrap();
  let source_str = v8::String::new(scope, source).unwrap();

  let origin = crate::bindings::module_origin(scope, specifier_str);
  let source = v8::script_compiler::Source::new(source_str, &origin);

  let tc_scope = &mut v8::TryCatch::new(scope);

  let maybe_module = v8::script_compiler::compile_module(tc_scope, source);

  if tc_scope.has_caught() {
    assert!(maybe_module.is_none());
    let e = tc_scope.exception().unwrap();
    return exception_to_err_result(tc_scope, e, false);
  }

  let module = maybe_module.unwrap();
  let module_handle = v8::Global::<v8::Module>::new(tc_scope, module);
  Ok(module_handle)
}

/// Instantiates a ES module
///
/// `AnyError` can be downcast to a type that exposes additional information
/// about the V8 exception. By default this type is `JsError`, however it may
/// be a different type if `RuntimeOptions::js_error_create_fn` has been set.
pub fn mod_instantiate(
  js_runtime: &mut JsRuntime,
  id: ModuleId,
) -> Result<(), AnyError> {
  let state_rc = JsRuntime::state(js_runtime.v8_isolate());
  let context = js_runtime.global_context();

  let scope =
    &mut v8::HandleScope::with_context(js_runtime.v8_isolate(), context);
  let tc_scope = &mut v8::TryCatch::new(scope);

  let module = state_rc
    .borrow()
    .module_map
    .get_handle(id)
    .map(|handle| v8::Local::new(tc_scope, handle))
    .expect("ModuleInfo not found");

  if module.get_status() == v8::ModuleStatus::Errored {
    exception_to_err_result(tc_scope, module.get_exception(), false)?
  }

  let result = module.instantiate_module(tc_scope, module_resolve_callback);
  match result {
    Some(_) => Ok(()),
    None => {
      let exception = tc_scope.exception().unwrap();
      exception_to_err_result(tc_scope, exception, false)
    }
  }
}

pub async fn mod_evaluate(
  js_runtime: &mut JsRuntime,
  id: ModuleId,
) -> Result<(), AnyError> {
  let state_rc = JsRuntime::state(js_runtime.v8_isolate());

  let maybe_promise_handle = {
    let context = js_runtime.global_context();
    let scope =
      &mut v8::HandleScope::with_context(js_runtime.v8_isolate(), context);

    let module = state_rc
      .borrow()
      .module_map
      .get_handle(id)
      .map(|handle| v8::Local::new(scope, handle))
      .expect("ModuleInfo not found");
    let mut status = module.get_status();
    assert_eq!(status, v8::ModuleStatus::Instantiated);

    // IMPORTANT: Top-level-await is enabled, which means that return value
    // of module evaluation is a promise.
    //
    // Because that promise is created internally by V8, when error occurs during
    // module evaluation the promise is rejected, and since the promise has no rejection
    // handler it will result in call to `bindings::promise_reject_callback` adding
    // the promise to pending promise rejection table - meaning JsRuntime will return
    // error on next poll().
    //
    // This situation is not desirable as we want to manually return error at the
    // end of this function to handle it further. It means we need to manually
    // remove this promise from pending promise rejection table.
    //
    // For more details see:
    // https://github.com/denoland/deno/issues/4908
    // https://v8.dev/features/top-level-await#module-execution-order
    let maybe_value = module.evaluate(scope);

    // Update status after evaluating.
    status = module.get_status();

    if let Some(value) = maybe_value {
      assert!(
        status == v8::ModuleStatus::Evaluated
          || status == v8::ModuleStatus::Errored
      );
      let promise = v8::Local::<v8::Promise>::try_from(value)
        .expect("Expected to get promise as module evaluation result");
      let promise_global = v8::Global::new(scope, promise);
      // FIXME(bartlomieju): comment above
      state_rc
        .borrow_mut()
        .pending_promise_exceptions
        .remove(&promise_global);
      scope.perform_microtask_checkpoint();
      Some(promise_global)
    } else {
      // FIXME(bartlomieju): this path depends on the comment above and
      // promise rejection being added to `state.pending_promise_expcetions`
      assert!(status == v8::ModuleStatus::Errored);
      None
    }
  };

  // FIXME(bartlomieju): this path depends on the comment above and
  // promise rejection being added to `state.pending_promise_expcetions`
  if maybe_promise_handle.is_none() {
    let err = js_runtime.check_promise_exceptions().unwrap_err();
    return Err(err);
  }

  let promise_handle = maybe_promise_handle.unwrap();

  poll_fn(|cx| {
    let _r = js_runtime.poll_event_loop(cx)?;

    // Top level module
    let maybe_result = evaluate_pending_module(js_runtime, promise_handle.clone());

    if let Some(result) = maybe_result {
      // TODO(bartlomieju): is it ok?
      return Poll::Ready(result);
    }

    let state = state_rc.borrow();
    if state.pending_ops.is_empty() {
      let msg = "Module evaluation is still pending but there are no pending ops or dynamic imports. This situation is often caused by unresolved promise.";
      return Poll::Ready(Err(generic_error(msg)));
    }

    Poll::Pending
  })
  .await
}

// TODO(bartlomieju): rename me
/// "deno_core" runs V8 with "--harmony-top-level-await"
/// flag on - it means that each module evaluation returns a promise
/// from V8.
///
/// This promise resolves after all dependent modules have also
/// resolved. Each dependent module may perform calls to "import()" and APIs
/// using async ops will add futures to the runtime's event loop.
/// It means that the promise returned from module evaluation will
/// resolve only after all futures in the event loop are done.
///
/// Thus during turn of event loop we need to check if V8 has
/// resolved or rejected the promise. If the promise is still pending
/// then another turn of event loop must be performed.
fn evaluate_pending_module(
  js_runtime: &mut JsRuntime,
  promise_handle: v8::Global<v8::Promise>,
) -> Option<Result<(), AnyError>> {
  let context = js_runtime.global_context();
  let scope =
    &mut v8::HandleScope::with_context(js_runtime.v8_isolate(), context);

  let promise = promise_handle.get(scope);
  let promise_state = promise.state();

  match promise_state {
    v8::PromiseState::Pending => {
      // pass, poll_event_loop will decide if
      // runtime would be woken soon
      None
    }
    v8::PromiseState::Fulfilled => {
      scope.perform_microtask_checkpoint();
      Some(Ok(()))
    }
    v8::PromiseState::Rejected => {
      let exception = promise.result(scope);
      scope.perform_microtask_checkpoint();
      let err1 = exception_to_err_result::<()>(scope, exception, false)
        .map_err(|err| attach_handle_to_error(scope, err, exception))
        .unwrap_err();
      Some(Err(err1))
    }
  }
}

pub type ModuleId = i32;

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

pub struct ModuleInfo {
  pub id: ModuleId,
  pub main: bool,
  pub name: String,
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
#[derive(Default)]
pub struct ModuleMap {
  ids_by_handle: HashMap<v8::Global<v8::Module>, ModuleId>,
  handles_by_id: HashMap<ModuleId, v8::Global<v8::Module>>,
  info: HashMap<ModuleId, ModuleInfo>,
  by_name: HashMap<String, SymbolicModule>,
  next_module_id: ModuleId,
}

impl ModuleMap {
  pub fn new() -> ModuleMap {
    Self {
      handles_by_id: HashMap::new(),
      ids_by_handle: HashMap::new(),
      info: HashMap::new(),
      by_name: HashMap::new(),
      next_module_id: 1,
    }
  }

  /// Get the id of a module.
  /// If this module is internally represented as an alias,
  /// follow the alias chain to get the final module id.
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

  pub fn register(
    &mut self,
    name: &str,
    main: bool,
    handle: v8::Global<v8::Module>,
  ) -> ModuleId {
    let name = String::from(name);
    let id = self.next_module_id;
    self.next_module_id += 1;
    self.by_name.insert(name.clone(), SymbolicModule::Mod(id));
    self.handles_by_id.insert(id, handle.clone());
    self.ids_by_handle.insert(handle, id);
    self.info.insert(id, ModuleInfo { id, main, name });
    id
  }

  pub fn alias(&mut self, name: &str, target: &str) {
    self
      .by_name
      .insert(name.to_owned(), SymbolicModule::Alias(target.to_string()));
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

  #[cfg(test)]
  pub fn is_alias(&self, name: &str) -> bool {
    let cond = self.by_name.get(name);
    matches!(cond, Some(SymbolicModule::Alias(_)))
  }
}
