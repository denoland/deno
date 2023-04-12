// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::option::Option;
use std::os::raw::c_void;

use log::debug;
use v8::MapFnTo;

use crate::error::is_instance_of_error;
use crate::error::JsStackFrame;
use crate::modules::get_asserted_module_type_from_assertions;
use crate::modules::parse_import_assertions;
use crate::modules::resolve_helper;
use crate::modules::validate_import_assertions;
use crate::modules::ImportAssertionsKind;
use crate::modules::ModuleMap;
use crate::modules::ResolutionKind;
use crate::ops::OpCtx;
use crate::snapshot_util::SnapshotOptions;
use crate::JsRealm;
use crate::JsRuntime;

pub(crate) fn external_references(ops: &[OpCtx]) -> v8::ExternalReferences {
  // Overallocate a bit, it's better than having to resize the vector.
  let mut references = Vec::with_capacity(4 + ops.len() * 4);

  references.push(v8::ExternalReference {
    function: call_console.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: import_meta_resolve.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: catch_dynamic_import_promise_error.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: empty_fn.map_fn_to(),
  });

  for ctx in ops {
    let ctx_ptr = ctx as *const OpCtx as _;
    references.push(v8::ExternalReference { pointer: ctx_ptr });
    references.push(v8::ExternalReference {
      function: ctx.decl.v8_fn_ptr,
    });
    if let Some(fast_fn) = &ctx.decl.fast_fn {
      references.push(v8::ExternalReference {
        pointer: fast_fn.function as _,
      });
      references.push(v8::ExternalReference {
        pointer: ctx.fast_fn_c_info.unwrap().as_ptr() as _,
      });
    }
  }

  let refs = v8::ExternalReferences::new(&references);
  // Leak, V8 takes ownership of the references.
  std::mem::forget(references);
  refs
}

// TODO(nayeemrmn): Move to runtime and/or make `pub(crate)`.
pub fn script_origin<'a>(
  s: &mut v8::HandleScope<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let source_map_url = v8::String::empty(s);
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
  let source_map_url = v8::String::empty(s);
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

pub(crate) fn initialize_context<'s>(
  scope: &mut v8::HandleScope<'s, ()>,
  op_ctxs: &[OpCtx],
  snapshot_options: SnapshotOptions,
) -> v8::Local<'s, v8::Context> {
  let context = v8::Context::new(scope);
  let global = context.global(scope);

  let scope = &mut v8::ContextScope::new(scope, context);

  let deno_str =
    v8::String::new_external_onebyte_static(scope, b"Deno").unwrap();
  let core_str =
    v8::String::new_external_onebyte_static(scope, b"core").unwrap();
  let ops_str = v8::String::new_external_onebyte_static(scope, b"ops").unwrap();

  let ops_obj = if snapshot_options.loaded() {
    // Snapshot already registered `Deno.core.ops` but
    // extensions may provide ops that aren't part of the snapshot.
    // Grab the Deno.core.ops object & init it
    let deno_obj: v8::Local<v8::Object> = global
      .get(scope, deno_str.into())
      .unwrap()
      .try_into()
      .unwrap();
    let core_obj: v8::Local<v8::Object> = deno_obj
      .get(scope, core_str.into())
      .unwrap()
      .try_into()
      .unwrap();
    let ops_obj: v8::Local<v8::Object> = core_obj
      .get(scope, ops_str.into())
      .expect("Deno.core.ops to exist")
      .try_into()
      .unwrap();
    ops_obj
  } else {
    // globalThis.Deno = { core: { } };
    let deno_obj = v8::Object::new(scope);
    global.set(scope, deno_str.into(), deno_obj.into());

    let core_obj = v8::Object::new(scope);
    deno_obj.set(scope, core_str.into(), core_obj.into());

    // Bind functions to Deno.core.*
    set_func(scope, core_obj, "callConsole", call_console);

    // Bind v8 console object to Deno.core.console
    let extra_binding_obj = context.get_extras_binding_object(scope);
    let console_str =
      v8::String::new_external_onebyte_static(scope, b"console").unwrap();
    let console_obj = extra_binding_obj.get(scope, console_str.into()).unwrap();
    core_obj.set(scope, console_str.into(), console_obj);

    // Bind functions to Deno.core.ops.*
    let ops_obj = v8::Object::new(scope);
    core_obj.set(scope, ops_str.into(), ops_obj.into());
    ops_obj
  };

  if matches!(snapshot_options, SnapshotOptions::Load) {
    // Only register ops that have `force_registration` flag set to true,
    // the remaining ones should already be in the snapshot.
    for op_ctx in op_ctxs
      .iter()
      .filter(|op_ctx| op_ctx.decl.force_registration)
    {
      add_op_to_deno_core_ops(scope, ops_obj, op_ctx);
    }
  } else if matches!(snapshot_options, SnapshotOptions::CreateFromExisting) {
    // Register all ops, probing for which ones are already registered.
    for op_ctx in op_ctxs {
      let key = v8::String::new_external_onebyte_static(
        scope,
        op_ctx.decl.name.as_bytes(),
      )
      .unwrap();
      if ops_obj.get(scope, key.into()).is_some() {
        continue;
      }
      add_op_to_deno_core_ops(scope, ops_obj, op_ctx);
    }
  } else {
    // In other cases register all ops unconditionally.
    for op_ctx in op_ctxs {
      add_op_to_deno_core_ops(scope, ops_obj, op_ctx);
    }
  }

  context
}

fn set_func(
  scope: &mut v8::HandleScope<'_>,
  obj: v8::Local<v8::Object>,
  name: &'static str,
  callback: impl v8::MapFnTo<v8::FunctionCallback>,
) {
  let key =
    v8::String::new_external_onebyte_static(scope, name.as_bytes()).unwrap();
  let val = v8::Function::new(scope, callback).unwrap();
  val.set_name(key);
  obj.set(scope, key.into(), val.into());
}

fn add_op_to_deno_core_ops(
  scope: &mut v8::HandleScope<'_>,
  obj: v8::Local<v8::Object>,
  op_ctx: &OpCtx,
) {
  let op_ctx_ptr = op_ctx as *const OpCtx as *const c_void;
  let key =
    v8::String::new_external_onebyte_static(scope, op_ctx.decl.name.as_bytes())
      .unwrap();
  let external = v8::External::new(scope, op_ctx_ptr as *mut c_void);
  let builder = v8::FunctionTemplate::builder_raw(op_ctx.decl.v8_fn_ptr)
    .data(external.into());

  let templ = if let Some(fast_function) = &op_ctx.decl.fast_fn {
    builder.build_fast(
      scope,
      fast_function,
      Some(op_ctx.fast_fn_c_info.unwrap().as_ptr()),
      None,
      None,
    )
  } else {
    builder.build(scope)
  };
  let val = templ.get_function(scope).unwrap();
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

pub fn host_import_module_dynamically_callback<'s>(
  scope: &mut v8::HandleScope<'s>,
  _host_defined_options: v8::Local<'s, v8::Data>,
  resource_name: v8::Local<'s, v8::Value>,
  specifier: v8::Local<'s, v8::String>,
  import_assertions: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);
  let referrer_name_str = resource_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  let is_ext_module = specifier_str.starts_with("ext:");
  let resolver = v8::PromiseResolver::new(scope).unwrap();
  let promise = resolver.get_promise(scope);

  if !is_ext_module {
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
  }
  // Map errors from module resolution (not JS errors from module execution) to
  // ones rethrown from this scope, so they include the call stack of the
  // dynamic import site. Error objects without any stack frames are assumed to
  // be module resolution errors, other exception values are left as they are.
  let builder = v8::FunctionBuilder::new(catch_dynamic_import_promise_error);

  let map_err =
    v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();

  let promise = promise.catch(scope, map_err).unwrap();

  if is_ext_module {
    let message = v8::String::new_external_onebyte_static(
      scope,
      b"Cannot load extension module from external code",
    )
    .unwrap();
    let exception = v8::Exception::type_error(scope, message);
    resolver.reject(scope, exception);
  }

  Some(promise)
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

  let url_key = v8::String::new_external_onebyte_static(scope, b"url").unwrap();
  let url_val = info.name.v8(scope);
  meta.create_data_property(scope, url_key.into(), url_val.into());

  let main_key =
    v8::String::new_external_onebyte_static(scope, b"main").unwrap();
  let main_val = v8::Boolean::new(scope, info.main);
  meta.create_data_property(scope, main_key.into(), main_val.into());

  let builder =
    v8::FunctionBuilder::new(import_meta_resolve).data(url_val.into());
  let val = v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();
  let resolve_key =
    v8::String::new_external_onebyte_static(scope, b"resolve").unwrap();
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
    let url_prop = args.data();
    url_prop.to_rust_string_lossy(scope)
  };
  let module_map_rc = JsRuntime::module_map(scope);
  let (loader, snapshot_loaded_and_not_snapshotting) = {
    let module_map = module_map_rc.borrow();
    (
      module_map.loader.clone(),
      module_map.snapshot_loaded_and_not_snapshotting,
    )
  };
  let specifier_str = specifier.to_rust_string_lossy(scope);

  if specifier_str.starts_with("npm:") {
    throw_type_error(scope, "\"npm:\" specifiers are currently not supported in import.meta.resolve()");
    return;
  }

  match resolve_helper(
    snapshot_loaded_and_not_snapshotting,
    loader,
    &specifier_str,
    &referrer,
    ResolutionKind::DynamicImport,
  ) {
    Ok(resolved) => {
      let resolved_val = serde_v8::to_v8(scope, resolved.as_str()).unwrap();
      rv.set(resolved_val);
    }
    Err(err) => {
      throw_type_error(scope, &err.to_string());
    }
  };
}

fn empty_fn(
  _scope: &mut v8::HandleScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  //Do Nothing
}

//It creates a reference to an empty function which can be mantained after the snapshots
pub fn create_empty_fn<'s>(
  scope: &mut v8::HandleScope<'s>,
) -> Option<v8::Local<'s, v8::Function>> {
  let empty_fn = v8::FunctionTemplate::new(scope, empty_fn);
  empty_fn.get_function(scope)
}

fn catch_dynamic_import_promise_error(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let arg = args.get(0);
  if is_instance_of_error(scope, arg) {
    let e: crate::error::NativeJsError = serde_v8::from_v8(scope, arg).unwrap();
    let name = e.name.unwrap_or_else(|| "Error".to_string());
    let msg = v8::Exception::create_message(scope, arg);
    if msg.get_stack_trace(scope).unwrap().get_frame_count() == 0 {
      let arg: v8::Local<v8::Object> = arg.try_into().unwrap();
      let message_key =
        v8::String::new_external_onebyte_static(scope, b"message").unwrap();
      let message = arg.get(scope, message_key.into()).unwrap();
      let mut message: v8::Local<v8::String> = message.try_into().unwrap();
      if let Some(stack_frame) = JsStackFrame::from_v8_message(scope, msg) {
        if let Some(location) = stack_frame.maybe_format_location() {
          let str =
            format!("{} at {location}", message.to_rust_string_lossy(scope));
          message = v8::String::new(scope, &str).unwrap();
        }
      }
      let exception = match name.as_str() {
        "RangeError" => v8::Exception::range_error(scope, message),
        "TypeError" => v8::Exception::type_error(scope, message),
        "SyntaxError" => v8::Exception::syntax_error(scope, message),
        "ReferenceError" => v8::Exception::reference_error(scope, message),
        _ => v8::Exception::error(scope, message),
      };
      let code_key =
        v8::String::new_external_onebyte_static(scope, b"code").unwrap();
      let code_value =
        v8::String::new_external_onebyte_static(scope, b"ERR_MODULE_NOT_FOUND")
          .unwrap();
      let exception_obj = exception.to_object(scope).unwrap();
      exception_obj.set(scope, code_key.into(), code_value.into());
      scope.throw_exception(exception);
      return;
    }
  }
  scope.throw_exception(arg);
}

pub extern "C" fn promise_reject_callback(message: v8::PromiseRejectMessage) {
  use v8::PromiseRejectEvent::*;

  // SAFETY: `CallbackScope` can be safely constructed from `&PromiseRejectMessage`
  let scope = &mut unsafe { v8::CallbackScope::new(&message) };

  let context_state_rc = JsRealm::state_from_scope(scope);
  let mut context_state = context_state_rc.borrow_mut();

  if let Some(js_promise_reject_cb) = context_state.js_promise_reject_cb.clone()
  {
    drop(context_state);

    let tc_scope = &mut v8::TryCatch::new(scope);
    let undefined: v8::Local<v8::Value> = v8::undefined(tc_scope).into();
    let type_ = v8::Integer::new(tc_scope, message.get_event() as i32);
    let promise = message.get_promise();

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
      let state_rc = JsRuntime::state(tc_scope);
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
        context_state
          .pending_promise_rejections
          .insert(promise_global, error_global);
      }
      PromiseHandlerAddedAfterReject => {
        context_state
          .pending_promise_rejections
          .remove(&promise_global);
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
  let referrer_name = referrer_info.name.as_str();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  let assertions = parse_import_assertions(
    scope,
    import_assertions,
    ImportAssertionsKind::StaticImport,
  );
  let maybe_module = module_map.resolve_callback(
    scope,
    &specifier_str,
    referrer_name,
    assertions,
  );
  if let Some(module) = maybe_module {
    return Some(module);
  }

  let msg = format!(
    r#"Cannot resolve module "{specifier_str}" from "{referrer_name}""#
  );
  throw_type_error(scope, msg);
  None
}

pub fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
  let message = v8::String::new(scope, message.as_ref()).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}
