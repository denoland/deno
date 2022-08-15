// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::error::is_instance_of_error;
use crate::modules::get_asserted_module_type_from_assertions;
use crate::modules::parse_import_assertions;
use crate::modules::validate_import_assertions;
use crate::modules::ImportAssertionsKind;
use crate::modules::ModuleMap;
use crate::ops::OpCtx;
use crate::JsRuntime;
use log::debug;
use once_cell::sync::Lazy;
use std::option::Option;
use std::os::raw::c_void;
use v8::MapFnTo;

pub static EXTERNAL_REFERENCES: Lazy<v8::ExternalReferences> =
  Lazy::new(|| {
    v8::ExternalReferences::new(&[v8::ExternalReference {
      function: call_console.map_fn_to(),
    }])
  });

// TODO(nayeemrmn): Move to runtime and/or make `pub(crate)`.
pub fn script_origin<'a>(
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
    false,
  )
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

pub fn initialize_context<'s>(
  scope: &mut v8::HandleScope<'s, ()>,
  op_ctxs: &[OpCtx],
  snapshot_loaded: bool,
) -> v8::Local<'s, v8::Context> {
  let scope = &mut v8::EscapableHandleScope::new(scope);

  let context = v8::Context::new(scope);
  let global = context.global(scope);

  let scope = &mut v8::ContextScope::new(scope, context);

  // Snapshot already registered `Deno.core.ops` but
  // extensions may provide ops that aren't part of the snapshot.
  //
  // TODO(@littledivy): This is extra complexity for
  // a really weird usecase. Remove this once all
  // tsc ops are static at snapshot time.
  if snapshot_loaded {
    // Grab the Deno.core.ops object & init it
    let ops_obj = JsRuntime::grab_global::<v8::Object>(scope, "Deno.core.ops")
      .expect("Deno.core.ops to exist");
    initialize_ops(scope, ops_obj, op_ctxs);
    return scope.escape(context);
  }

  // global.Deno = { core: { } };
  let core_val = JsRuntime::ensure_objs(scope, global, "Deno.core").unwrap();

  // Bind functions to Deno.core.*
  set_func(scope, core_val, "callConsole", call_console);

  // Bind functions to Deno.core.ops.*
  let ops_obj = JsRuntime::ensure_objs(scope, global, "Deno.core.ops").unwrap();
  initialize_ops(scope, ops_obj, op_ctxs);
  scope.escape(context)
}

fn initialize_ops(
  scope: &mut v8::HandleScope,
  ops_obj: v8::Local<v8::Object>,
  op_ctxs: &[OpCtx],
) {
  for ctx in op_ctxs {
    let ctx_ptr = ctx as *const OpCtx as *const c_void;
    set_func_raw(scope, ops_obj, ctx.decl.name, ctx.decl.v8_fn_ptr, ctx_ptr);
  }
}

pub fn set_func(
  scope: &mut v8::HandleScope<'_>,
  obj: v8::Local<v8::Object>,
  name: &'static str,
  callback: impl v8::MapFnTo<v8::FunctionCallback>,
) {
  let key = v8::String::new(scope, name).unwrap();
  let val = v8::Function::new(scope, callback).unwrap();
  val.set_name(key);
  obj.set(scope, key.into(), val.into());
}

// Register a raw v8::FunctionCallback
// with some external data.
pub fn set_func_raw(
  scope: &mut v8::HandleScope<'_>,
  obj: v8::Local<v8::Object>,
  name: &'static str,
  callback: v8::FunctionCallback,
  external_data: *const c_void,
) {
  let key = v8::String::new(scope, name).unwrap();
  let external = v8::External::new(scope, external_data as *mut c_void);
  let val = v8::Function::builder_raw(callback)
    .data(external.into())
    .build(scope)
    .unwrap();
  val.set_name(key);
  obj.set(scope, key.into(), val.into());
}

pub extern "C" fn wasm_async_resolve_promise_callback(
  _isolate: *mut v8::Isolate,
  context: v8::Local<v8::Context>,
  resolver: v8::Local<v8::PromiseResolver>,
  compilation_result: v8::Local<v8::Value>,
  success: v8::WasmAsyncSuccess,
) {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  let scope = &mut unsafe { v8::CallbackScope::new(context) };
  if success == v8::WasmAsyncSuccess::Success {
    resolver.resolve(scope, compilation_result).unwrap();
  } else {
    resolver.reject(scope, compilation_result).unwrap();
  }
}

pub extern "C" fn host_import_module_dynamically_callback(
  context: v8::Local<v8::Context>,
  _host_defined_options: v8::Local<v8::Data>,
  resource_name: v8::Local<v8::Value>,
  specifier: v8::Local<v8::String>,
  import_assertions: v8::Local<v8::FixedArray>,
) -> *mut v8::Promise {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  let scope = &mut unsafe { v8::CallbackScope::new(context) };

  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);
  let referrer_name_str = resource_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  let resolver = v8::PromiseResolver::new(scope).unwrap();
  let promise = resolver.get_promise(scope);

  let assertions = parse_import_assertions(
    scope,
    import_assertions,
    ImportAssertionsKind::DynamicImport,
  );

  {
    let tc_scope = &mut v8::TryCatch::new(scope);
    validate_import_assertions(tc_scope, &assertions);
    if tc_scope.has_caught() {
      let e = tc_scope.exception().unwrap();
      resolver.reject(tc_scope, e);
    }
  }
  let asserted_module_type =
    get_asserted_module_type_from_assertions(&assertions);

  let resolver_handle = v8::Global::new(scope, resolver);
  {
    let state_rc = JsRuntime::state(scope);
    let module_map_rc = JsRuntime::module_map(scope);

    debug!(
      "dyn_import specifier {} referrer {} ",
      specifier_str, referrer_name_str
    );
    ModuleMap::load_dynamic_import(
      module_map_rc,
      &specifier_str,
      &referrer_name_str,
      asserted_module_type,
      resolver_handle,
    );
    state_rc.borrow_mut().notify_new_dynamic_import();
  }

  // Map errors from module resolution (not JS errors from module execution) to
  // ones rethrown from this scope, so they include the call stack of the
  // dynamic import site. Error objects without any stack frames are assumed to
  // be module resolution errors, other exception values are left as they are.
  let map_err = |scope: &mut v8::HandleScope,
                 args: v8::FunctionCallbackArguments,
                 _rv: v8::ReturnValue| {
    let arg = args.get(0);
    if is_instance_of_error(scope, arg) {
      let e: crate::error::NativeJsError =
        serde_v8::from_v8(scope, arg).unwrap();
      let name = e.name.unwrap_or_else(|| "Error".to_string());
      let message = v8::Exception::create_message(scope, arg);
      if message.get_stack_trace(scope).unwrap().get_frame_count() == 0 {
        let arg: v8::Local<v8::Object> = arg.try_into().unwrap();
        let message_key = v8::String::new(scope, "message").unwrap();
        let message = arg.get(scope, message_key.into()).unwrap();
        let exception = match name.as_str() {
          "RangeError" => {
            v8::Exception::range_error(scope, message.try_into().unwrap())
          }
          "TypeError" => {
            v8::Exception::type_error(scope, message.try_into().unwrap())
          }
          "SyntaxError" => {
            v8::Exception::syntax_error(scope, message.try_into().unwrap())
          }
          "ReferenceError" => {
            v8::Exception::reference_error(scope, message.try_into().unwrap())
          }
          _ => v8::Exception::error(scope, message.try_into().unwrap()),
        };
        let code_key = v8::String::new(scope, "code").unwrap();
        let code_value =
          v8::String::new(scope, "ERR_MODULE_NOT_FOUND").unwrap();
        let exception_obj = exception.to_object(scope).unwrap();
        exception_obj.set(scope, code_key.into(), code_value.into());
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
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  let scope = &mut unsafe { v8::CallbackScope::new(context) };
  let module_map_rc = JsRuntime::module_map(scope);
  let module_map = module_map_rc.borrow();

  let module_global = v8::Global::new(scope, module);
  let info = module_map
    .get_info(&module_global)
    .expect("Module not found");

  let url_key = v8::String::new(scope, "url").unwrap();
  let url_val = v8::String::new(scope, &info.name).unwrap();
  meta.create_data_property(scope, url_key.into(), url_val.into());

  let main_key = v8::String::new(scope, "main").unwrap();
  let main_val = v8::Boolean::new(scope, info.main);
  meta.create_data_property(scope, main_key.into(), main_val.into());

  let builder =
    v8::FunctionBuilder::new(import_meta_resolve).data(url_val.into());
  let val = v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();
  let resolve_key = v8::String::new(scope, "resolve").unwrap();
  meta.set(scope, resolve_key.into(), val.into());
}

fn import_meta_resolve(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  if args.length() > 1 {
    return throw_type_error(scope, "Invalid arguments");
  }

  let maybe_arg_str = args.get(0).to_string(scope);
  if maybe_arg_str.is_none() {
    return throw_type_error(scope, "Invalid arguments");
  }
  let specifier = maybe_arg_str.unwrap();
  let referrer = {
    let url_prop = args.data().unwrap();
    url_prop.to_rust_string_lossy(scope)
  };
  let module_map_rc = JsRuntime::module_map(scope);
  let loader = {
    let module_map = module_map_rc.borrow();
    module_map.loader.clone()
  };
  match loader.resolve(&specifier.to_rust_string_lossy(scope), &referrer, false)
  {
    Ok(resolved) => {
      let resolved_val = serde_v8::to_v8(scope, resolved.as_str()).unwrap();
      rv.set(resolved_val);
    }
    Err(err) => {
      throw_type_error(scope, &err.to_string());
    }
  };
}

pub extern "C" fn promise_reject_callback(message: v8::PromiseRejectMessage) {
  use v8::PromiseRejectEvent::*;

  // SAFETY: `CallbackScope` can be safely constructed from `&PromiseRejectMessage`
  let scope = &mut unsafe { v8::CallbackScope::new(&message) };

  let state_rc = JsRuntime::state(scope);
  let mut state = state_rc.borrow_mut();

  if let Some(js_promise_reject_cb) = state.js_promise_reject_cb.clone() {
    let tc_scope = &mut v8::TryCatch::new(scope);
    let undefined: v8::Local<v8::Value> = v8::undefined(tc_scope).into();
    let type_ = v8::Integer::new(tc_scope, message.get_event() as i32);
    let promise = message.get_promise();
    drop(state); // Drop borrow, callbacks can call back into runtime.

    let reason = match message.get_event() {
      PromiseRejectWithNoHandler
      | PromiseRejectAfterResolved
      | PromiseResolveAfterResolved => message.get_value().unwrap_or(undefined),
      PromiseHandlerAddedAfterReject => undefined,
    };

    let promise_global = v8::Global::new(tc_scope, promise);
    let args = &[type_.into(), promise.into(), reason];
    let maybe_has_unhandled_rejection_handler = js_promise_reject_cb
      .open(tc_scope)
      .call(tc_scope, undefined, args);

    let has_unhandled_rejection_handler =
      if let Some(value) = maybe_has_unhandled_rejection_handler {
        value.is_true()
      } else {
        false
      };

    if has_unhandled_rejection_handler {
      let mut state = state_rc.borrow_mut();
      if let Some(pending_mod_evaluate) = state.pending_mod_evaluate.as_mut() {
        if !pending_mod_evaluate.has_evaluated {
          pending_mod_evaluate
            .handled_promise_rejections
            .push(promise_global);
        }
      }
    }
  } else {
    let promise = message.get_promise();
    let promise_global = v8::Global::new(scope, promise);
    match message.get_event() {
      PromiseRejectWithNoHandler => {
        let error = message.get_value().unwrap();
        let error_global = v8::Global::new(scope, error);
        state
          .pending_promise_exceptions
          .insert(promise_global, error_global);
      }
      PromiseHandlerAddedAfterReject => {
        state.pending_promise_exceptions.remove(&promise_global);
      }
      PromiseRejectAfterResolved => {}
      PromiseResolveAfterResolved => {
        // Should not warn. See #1272
      }
    }
  }
}

/// This binding should be used if there's a custom console implementation
/// available. Using it will make sure that proper stack frames are displayed
/// in the inspector console.
///
/// Each method on console object should be bound to this function, eg:
/// ```ignore
/// function wrapConsole(consoleFromDeno, consoleFromV8) {
///   const callConsole = core.callConsole;
///
///   for (const key of Object.keys(consoleFromV8)) {
///     if (consoleFromDeno.hasOwnProperty(key)) {
///       consoleFromDeno[key] = callConsole.bind(
///         consoleFromDeno,
///         consoleFromV8[key],
///         consoleFromDeno[key],
///       );
///     }
///   }
/// }
/// ```
///
/// Inspired by:
/// https://github.com/nodejs/node/blob/1317252dfe8824fd9cfee125d2aaa94004db2f3b/src/inspector_js_api.cc#L194-L222
fn call_console(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  if args.length() < 2
    || !args.get(0).is_function()
    || !args.get(1).is_function()
  {
    return throw_type_error(scope, "Invalid arguments");
  }

  let mut call_args = vec![];
  for i in 2..args.length() {
    call_args.push(args.get(i));
  }

  let receiver = args.this();
  let inspector_console_method =
    v8::Local::<v8::Function>::try_from(args.get(0)).unwrap();
  let deno_console_method =
    v8::Local::<v8::Function>::try_from(args.get(1)).unwrap();

  inspector_console_method.call(scope, receiver.into(), &call_args);
  deno_console_method.call(scope, receiver.into(), &call_args);
}

/// Called by V8 during `JsRuntime::instantiate_module`.
///
/// This function borrows `ModuleMap` from the isolate slot,
/// so it is crucial to ensure there are no existing borrows
/// of `ModuleMap` when `JsRuntime::instantiate_module` is called.
pub fn module_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  import_assertions: v8::Local<'s, v8::FixedArray>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  let scope = &mut unsafe { v8::CallbackScope::new(context) };

  let module_map_rc = JsRuntime::module_map(scope);
  let module_map = module_map_rc.borrow();

  let referrer_global = v8::Global::new(scope, referrer);

  let referrer_info = module_map
    .get_info(&referrer_global)
    .expect("ModuleInfo not found");
  let referrer_name = referrer_info.name.to_string();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  let assertions = parse_import_assertions(
    scope,
    import_assertions,
    ImportAssertionsKind::StaticImport,
  );
  let maybe_module = module_map.resolve_callback(
    scope,
    &specifier_str,
    &referrer_name,
    assertions,
  );
  if let Some(module) = maybe_module {
    return Some(module);
  }

  let msg = format!(
    r#"Cannot resolve module "{}" from "{}""#,
    specifier_str, referrer_name
  );
  throw_type_error(scope, msg);
  None
}

pub fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
  let message = v8::String::new(scope, message.as_ref()).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}
