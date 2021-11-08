// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::is_instance_of_error;
use crate::error::AnyError;
use crate::modules::ModuleMap;
use crate::resolve_url_or_path;
use crate::JsRuntime;
use crate::Op;
use crate::OpId;
use crate::OpPayload;
use crate::OpResult;
use crate::OpTable;
use crate::PromiseId;
use crate::ZeroCopyBuf;
use log::debug;
use serde::Deserialize;
use serde::Serialize;
use serde_v8::to_v8;
use std::cell::RefCell;
use std::option::Option;
use url::Url;
use v8::HandleScope;
use v8::Local;
use v8::MapFnTo;
use v8::SharedArrayBuffer;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

lazy_static::lazy_static! {
  pub static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[
      v8::ExternalReference {
        function: opcall_async.map_fn_to()
      },
      v8::ExternalReference {
        function: opcall_sync.map_fn_to()
      },
      v8::ExternalReference {
        function: set_macrotask_callback.map_fn_to()
      },
      v8::ExternalReference {
        function: eval_context.map_fn_to()
      },
      v8::ExternalReference {
        function: queue_microtask.map_fn_to()
      },
      v8::ExternalReference {
        function: create_host_object.map_fn_to()
      },
      v8::ExternalReference {
        function: encode.map_fn_to()
      },
      v8::ExternalReference {
        function: decode.map_fn_to()
      },
      v8::ExternalReference {
        function: serialize.map_fn_to()
      },
      v8::ExternalReference {
        function: deserialize.map_fn_to()
      },
      v8::ExternalReference {
        function: get_promise_details.map_fn_to()
      },
      v8::ExternalReference {
        function: get_proxy_details.map_fn_to()
      },
      v8::ExternalReference {
        function: is_proxy.map_fn_to()
      },
      v8::ExternalReference {
        function: memory_usage.map_fn_to(),
      },
      v8::ExternalReference {
        function: call_console.map_fn_to(),
      },
      v8::ExternalReference {
        function: set_wasm_streaming_callback.map_fn_to()
      }
    ]);
}

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
) -> v8::Local<'s, v8::Context> {
  let scope = &mut v8::EscapableHandleScope::new(scope);

  let context = v8::Context::new(scope);
  let global = context.global(scope);

  let scope = &mut v8::ContextScope::new(scope, context);

  // global.Deno = { core: {} };
  let deno_key = v8::String::new(scope, "Deno").unwrap();
  let deno_val = v8::Object::new(scope);
  global.set(scope, deno_key.into(), deno_val.into());
  let core_key = v8::String::new(scope, "core").unwrap();
  let core_val = v8::Object::new(scope);
  deno_val.set(scope, core_key.into(), core_val.into());

  // Bind functions to Deno.core.*
  set_func(scope, core_val, "opcallSync", opcall_sync);
  set_func(scope, core_val, "opcallAsync", opcall_async);
  set_func(
    scope,
    core_val,
    "setMacrotaskCallback",
    set_macrotask_callback,
  );
  set_func(scope, core_val, "evalContext", eval_context);
  set_func(scope, core_val, "encode", encode);
  set_func(scope, core_val, "decode", decode);
  set_func(scope, core_val, "serialize", serialize);
  set_func(scope, core_val, "deserialize", deserialize);
  set_func(scope, core_val, "getPromiseDetails", get_promise_details);
  set_func(scope, core_val, "getProxyDetails", get_proxy_details);
  set_func(scope, core_val, "isProxy", is_proxy);
  set_func(scope, core_val, "memoryUsage", memory_usage);
  set_func(scope, core_val, "callConsole", call_console);
  set_func(scope, core_val, "createHostObject", create_host_object);
  set_func(
    scope,
    core_val,
    "setWasmStreamingCallback",
    set_wasm_streaming_callback,
  );
  // Direct bindings on `window`.
  set_func(scope, global, "queueMicrotask", queue_microtask);

  scope.escape(context)
}

#[inline(always)]
pub fn set_func(
  scope: &mut v8::HandleScope<'_>,
  obj: v8::Local<v8::Object>,
  name: &'static str,
  callback: impl v8::MapFnTo<v8::FunctionCallback>,
) {
  let key = v8::String::new(scope, name).unwrap();
  let tmpl = v8::FunctionTemplate::new(scope, callback);
  let val = tmpl.get_function(scope).unwrap();
  val.set_name(key);
  obj.set(scope, key.into(), val.into());
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
    let module_map_rc = JsRuntime::module_map(scope);

    debug!(
      "dyn_import specifier {} referrer {} ",
      specifier_str, referrer_name_str
    );
    ModuleMap::load_dynamic_import(
      module_map_rc,
      &specifier_str,
      &referrer_name_str,
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
      let message = v8::Exception::create_message(scope, arg);
      if message.get_stack_trace(scope).unwrap().get_frame_count() == 0 {
        let arg: v8::Local<v8::Object> = arg.try_into().unwrap();
        let message_key = v8::String::new(scope, "message").unwrap();
        let message = arg.get(scope, message_key.into()).unwrap();
        let exception =
          v8::Exception::type_error(scope, message.try_into().unwrap());
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
}

pub extern "C" fn promise_reject_callback(message: v8::PromiseRejectMessage) {
  let scope = &mut unsafe { v8::CallbackScope::new(&message) };

  let state_rc = JsRuntime::state(scope);
  let mut state = state_rc.borrow_mut();

  let promise = message.get_promise();
  let promise_global = v8::Global::new(scope, promise);

  match message.get_event() {
    v8::PromiseRejectEvent::PromiseRejectWithNoHandler => {
      let error = message.get_value().unwrap();
      let error_global = v8::Global::new(scope, error);
      state
        .pending_promise_exceptions
        .insert(promise_global, error_global);
    }
    v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject => {
      state.pending_promise_exceptions.remove(&promise_global);
    }
    v8::PromiseRejectEvent::PromiseRejectAfterResolved => {}
    v8::PromiseRejectEvent::PromiseResolveAfterResolved => {
      // Should not warn. See #1272
    }
  };
}

fn opcall_sync<'s>(
  scope: &mut v8::HandleScope<'s>,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let state_rc = JsRuntime::state(scope);
  let state = state_rc.borrow_mut();

  let op_id = match v8::Local::<v8::Integer>::try_from(args.get(0))
    .map(|l| l.value() as OpId)
    .map_err(AnyError::from)
  {
    Ok(op_id) => op_id,
    Err(err) => {
      throw_type_error(scope, format!("invalid op id: {}", err));
      return;
    }
  };

  // opcall(0) returns obj of all ops, handle as special case
  if op_id == 0 {
    // TODO: Serialize as HashMap when serde_v8 supports maps ...
    let ops = OpTable::op_entries(state.op_state.clone());
    rv.set(to_v8(scope, ops).unwrap());
    return;
  }

  // Deserializable args (may be structured args or ZeroCopyBuf)
  let a = args.get(1);
  let b = args.get(2);

  let payload = OpPayload {
    scope,
    a,
    b,
    op_id,
    promise_id: 0,
  };
  let op = OpTable::route_op(op_id, state.op_state.clone(), payload);
  match op {
    Op::Sync(result) => {
      state.op_state.borrow().tracker.track_sync(op_id);
      rv.set(result.to_v8(scope).unwrap());
    }
    Op::NotFound => {
      throw_type_error(scope, format!("Unknown op id: {}", op_id));
    }
    // Async ops (ref or unref)
    _ => {
      throw_type_error(
        scope,
        format!("Can not call an async op [{}] with opSync()", op_id),
      );
    }
  }
}

fn opcall_async<'s>(
  scope: &mut v8::HandleScope<'s>,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let state_rc = JsRuntime::state(scope);
  let mut state = state_rc.borrow_mut();

  let op_id = match v8::Local::<v8::Integer>::try_from(args.get(0))
    .map(|l| l.value() as OpId)
    .map_err(AnyError::from)
  {
    Ok(op_id) => op_id,
    Err(err) => {
      throw_type_error(scope, format!("invalid op id: {}", err));
      return;
    }
  };

  // PromiseId
  let arg1 = args.get(1);
  let promise_id = v8::Local::<v8::Integer>::try_from(arg1)
    .map(|l| l.value() as PromiseId)
    .map_err(AnyError::from);
  // Fail if promise id invalid (not an int)
  let promise_id: PromiseId = match promise_id {
    Ok(promise_id) => promise_id,
    Err(err) => {
      throw_type_error(scope, format!("invalid promise id: {}", err));
      return;
    }
  };

  // Deserializable args (may be structured args or ZeroCopyBuf)
  let a = args.get(2);
  let b = args.get(3);

  let payload = OpPayload {
    scope,
    a,
    b,
    op_id,
    promise_id,
  };
  let op = OpTable::route_op(op_id, state.op_state.clone(), payload);
  match op {
    Op::Sync(result) => match result {
      OpResult::Ok(_) => throw_type_error(
        scope,
        format!("Can not call a sync op [{}] with opAsync()", op_id),
      ),
      OpResult::Err(_) => rv.set(result.to_v8(scope).unwrap()),
    },
    Op::Async(fut) => {
      state.op_state.borrow().tracker.track_async(op_id);
      state.pending_ops.push(fut);
      state.have_unpolled_ops = true;
    }
    Op::AsyncUnref(fut) => {
      state.op_state.borrow().tracker.track_unref(op_id);
      state.pending_unref_ops.push(fut);
      state.have_unpolled_ops = true;
    }
    Op::NotFound => {
      throw_type_error(scope, format!("Unknown op id: {}", op_id));
    }
  }
}

fn set_macrotask_callback(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let state_rc = JsRuntime::state(scope);
  let mut state = state_rc.borrow_mut();

  let cb = match v8::Local::<v8::Function>::try_from(args.get(0)) {
    Ok(cb) => cb,
    Err(err) => return throw_type_error(scope, err.to_string()),
  };

  let slot = match &mut state.js_macrotask_cb {
    slot @ None => slot,
    _ => {
      return throw_type_error(
        scope,
        "Deno.core.setMacrotaskCallback() already called",
      );
    }
  };

  slot.replace(v8::Global::new(scope, cb));
}

fn eval_context(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let source = match v8::Local::<v8::String>::try_from(args.get(0)) {
    Ok(s) => s,
    Err(_) => {
      throw_type_error(scope, "Missing first argument");
      return;
    }
  };

  let url = match v8::Local::<v8::String>::try_from(args.get(1)) {
    Ok(s) => match resolve_url_or_path(&s.to_rust_string_lossy(scope)) {
      Ok(s) => Some(s),
      Err(err) => {
        throw_type_error(scope, &format!("Invalid specifier: {}", err));
        return;
      }
    },
    Err(_) => None,
  };

  #[derive(Serialize)]
  struct Output<'s>(Option<serde_v8::Value<'s>>, Option<ErrInfo<'s>>);

  #[derive(Serialize)]
  #[serde(rename_all = "camelCase")]
  struct ErrInfo<'s> {
    thrown: serde_v8::Value<'s>,
    is_native_error: bool,
    is_compile_error: bool,
  }

  let tc_scope = &mut v8::TryCatch::new(scope);
  let name = v8::String::new(
    tc_scope,
    url.as_ref().map_or(crate::DUMMY_SPECIFIER, Url::as_str),
  )
  .unwrap();
  let origin = script_origin(tc_scope, name);
  let maybe_script = v8::Script::compile(tc_scope, source, Some(&origin));

  if maybe_script.is_none() {
    assert!(tc_scope.has_caught());
    let exception = tc_scope.exception().unwrap();
    let output = Output(
      None,
      Some(ErrInfo {
        thrown: exception.into(),
        is_native_error: is_instance_of_error(tc_scope, exception),
        is_compile_error: true,
      }),
    );
    rv.set(to_v8(tc_scope, output).unwrap());
    return;
  }

  let result = maybe_script.unwrap().run(tc_scope);

  if result.is_none() {
    assert!(tc_scope.has_caught());
    let exception = tc_scope.exception().unwrap();
    let output = Output(
      None,
      Some(ErrInfo {
        thrown: exception.into(),
        is_native_error: is_instance_of_error(tc_scope, exception),
        is_compile_error: false,
      }),
    );
    rv.set(to_v8(tc_scope, output).unwrap());
    return;
  }

  let output = Output(Some(result.unwrap().into()), None);
  rv.set(to_v8(tc_scope, output).unwrap());
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
  assert!(args.length() >= 2);

  assert!(args.get(0).is_function());
  assert!(args.get(1).is_function());

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

fn set_wasm_streaming_callback(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  use crate::ops_builtin::WasmStreamingResource;

  let state_rc = JsRuntime::state(scope);
  let mut state = state_rc.borrow_mut();

  let cb = match v8::Local::<v8::Function>::try_from(args.get(0)) {
    Ok(cb) => cb,
    Err(err) => return throw_type_error(scope, err.to_string()),
  };

  // The callback to pass to the v8 API has to be a unit type, so it can't
  // borrow or move any local variables. Therefore, we're storing the JS
  // callback in a JsRuntimeState slot.
  if let slot @ None = &mut state.js_wasm_streaming_cb {
    slot.replace(v8::Global::new(scope, cb));
  } else {
    return throw_type_error(
      scope,
      "Deno.core.setWasmStreamingCallback() already called",
    );
  }

  scope.set_wasm_streaming_callback(|scope, arg, wasm_streaming| {
    let (cb_handle, streaming_rid) = {
      let state_rc = JsRuntime::state(scope);
      let state = state_rc.borrow();
      let cb_handle = state.js_wasm_streaming_cb.as_ref().unwrap().clone();
      let streaming_rid = state
        .op_state
        .borrow_mut()
        .resource_table
        .add(WasmStreamingResource(RefCell::new(wasm_streaming)));
      (cb_handle, streaming_rid)
    };

    let undefined = v8::undefined(scope);
    let rid = serde_v8::to_v8(scope, streaming_rid).unwrap();
    cb_handle
      .open(scope)
      .call(scope, undefined.into(), &[arg, rid]);
  });
}

fn encode(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let text = match v8::Local::<v8::String>::try_from(args.get(0)) {
    Ok(s) => s,
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };
  let text_str = text.to_rust_string_lossy(scope);
  let zbuf: ZeroCopyBuf = text_str.into_bytes().into();

  rv.set(to_v8(scope, zbuf).unwrap())
}

fn decode(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let zero_copy: ZeroCopyBuf = match serde_v8::from_v8(scope, args.get(0)) {
    Ok(zbuf) => zbuf,
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };
  let buf = &zero_copy;

  // Strip BOM
  let buf =
    if buf.len() >= 3 && buf[0] == 0xef && buf[1] == 0xbb && buf[2] == 0xbf {
      &buf[3..]
    } else {
      buf
    };

  // If `String::new_from_utf8()` returns `None`, this means that the
  // length of the decoded string would be longer than what V8 can
  // handle. In this case we return `RangeError`.
  //
  // For more details see:
  // - https://encoding.spec.whatwg.org/#dom-textdecoder-decode
  // - https://github.com/denoland/deno/issues/6649
  // - https://github.com/v8/v8/blob/d68fb4733e39525f9ff0a9222107c02c28096e2a/include/v8.h#L3277-L3278
  match v8::String::new_from_utf8(scope, buf, v8::NewStringType::Normal) {
    Some(text) => rv.set(text.into()),
    None => {
      let msg = v8::String::new(scope, "string too long").unwrap();
      let exception = v8::Exception::range_error(scope, msg);
      scope.throw_exception(exception);
    }
  };
}

struct SerializeDeserialize<'a> {
  host_objects: Option<v8::Local<'a, v8::Array>>,
}

impl<'a> v8::ValueSerializerImpl for SerializeDeserialize<'a> {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
  }

  fn get_shared_array_buffer_id<'s>(
    &mut self,
    scope: &mut HandleScope<'s>,
    shared_array_buffer: Local<'s, SharedArrayBuffer>,
  ) -> Option<u32> {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store {
      let backing_store = shared_array_buffer.get_backing_store();
      let id = shared_array_buffer_store.insert(backing_store);
      Some(id)
    } else {
      None
    }
  }

  fn get_wasm_module_transfer_id(
    &mut self,
    scope: &mut HandleScope<'_>,
    module: Local<v8::WasmModuleObject>,
  ) -> Option<u32> {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    if let Some(compiled_wasm_module_store) = &state.compiled_wasm_module_store
    {
      let compiled_wasm_module = module.get_compiled_module();
      let id = compiled_wasm_module_store.insert(compiled_wasm_module);
      Some(id)
    } else {
      None
    }
  }

  fn write_host_object<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    object: v8::Local<'s, v8::Object>,
    value_serializer: &mut dyn v8::ValueSerializerHelper,
  ) -> Option<bool> {
    if let Some(host_objects) = self.host_objects {
      for i in 0..host_objects.length() {
        let value = host_objects.get_index(scope, i).unwrap();
        if value == object {
          value_serializer.write_uint32(i);
          return Some(true);
        }
      }
    }
    let message = v8::String::new(scope, "Unsupported object type").unwrap();
    self.throw_data_clone_error(scope, message);
    None
  }
}

impl<'a> v8::ValueDeserializerImpl for SerializeDeserialize<'a> {
  fn get_shared_array_buffer_from_id<'s>(
    &mut self,
    scope: &mut HandleScope<'s>,
    transfer_id: u32,
  ) -> Option<Local<'s, SharedArrayBuffer>> {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store {
      let backing_store = shared_array_buffer_store.take(transfer_id)?;
      let shared_array_buffer =
        v8::SharedArrayBuffer::with_backing_store(scope, &backing_store);
      Some(shared_array_buffer)
    } else {
      None
    }
  }

  fn get_wasm_module_from_id<'s>(
    &mut self,
    scope: &mut HandleScope<'s>,
    clone_id: u32,
  ) -> Option<Local<'s, v8::WasmModuleObject>> {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    if let Some(compiled_wasm_module_store) = &state.compiled_wasm_module_store
    {
      let compiled_module = compiled_wasm_module_store.take(clone_id)?;
      v8::WasmModuleObject::from_compiled_module(scope, &compiled_module)
    } else {
      None
    }
  }

  fn read_host_object<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    value_deserializer: &mut dyn v8::ValueDeserializerHelper,
  ) -> Option<v8::Local<'s, v8::Object>> {
    if let Some(host_objects) = self.host_objects {
      let mut i = 0;
      if !value_deserializer.read_uint32(&mut i) {
        return None;
      }
      let maybe_value = host_objects.get_index(scope, i);
      if let Some(value) = maybe_value {
        return value.to_object(scope);
      }
    }

    let message =
      v8::String::new(scope, "Failed to deserialize host object").unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
    None
  }
}

fn serialize(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let value = args.get(0);

  let options: Option<SerializeDeserializeOptions> =
    match serde_v8::from_v8(scope, args.get(1)) {
      Ok(opts) => opts,
      Err(_) => {
        throw_type_error(scope, "Invalid argument 2");
        return;
      }
    };

  let options = options.unwrap_or(SerializeDeserializeOptions {
    host_objects: None,
    transfered_array_buffers: None,
  });

  let host_objects = match options.host_objects {
    Some(value) => match v8::Local::<v8::Array>::try_from(value.v8_value) {
      Ok(host_objects) => Some(host_objects),
      Err(_) => {
        throw_type_error(scope, "host_objects not an array");
        return;
      }
    },
    None => None,
  };

  let transfered_array_buffers = match options.transfered_array_buffers {
    Some(value) => match v8::Local::<v8::Array>::try_from(value.v8_value) {
      Ok(transfered_array_buffers) => Some(transfered_array_buffers),
      Err(_) => {
        throw_type_error(scope, "transfered_array_buffers not an array");
        return;
      }
    },
    None => None,
  };

  let serialize_deserialize = Box::new(SerializeDeserialize { host_objects });
  let mut value_serializer =
    v8::ValueSerializer::new(scope, serialize_deserialize);

  value_serializer.write_header();

  if let Some(transfered_array_buffers) = transfered_array_buffers {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    for i in 0..transfered_array_buffers.length() {
      let i = v8::Number::new(scope, i as f64).into();
      let buf = transfered_array_buffers.get(scope, i).unwrap();
      let buf = match v8::Local::<v8::ArrayBuffer>::try_from(buf) {
        Ok(buf) => buf,
        Err(_) => {
          throw_type_error(
            scope,
            "item in transfered_array_buffers not an ArrayBuffer",
          );
          return;
        }
      };
      if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store
      {
        // TODO(lucacasonato): we need to check here that the buffer is not
        // already detached. We can not do that because V8 does not provide
        // a way to check if a buffer is already detached.
        if !buf.is_detachable() {
          throw_type_error(
            scope,
            "item in transfered_array_buffers is not transferable",
          );
          return;
        }
        let backing_store = buf.get_backing_store();
        buf.detach();
        let id = shared_array_buffer_store.insert(backing_store);
        value_serializer.transfer_array_buffer(id, buf);
        let id = v8::Number::new(scope, id as f64).into();
        transfered_array_buffers.set(scope, i, id);
      }
    }
  }

  match value_serializer.write_value(scope.get_current_context(), value) {
    Some(true) => {
      let vector = value_serializer.release();
      let zbuf: ZeroCopyBuf = vector.into();
      rv.set(to_v8(scope, zbuf).unwrap());
    }
    _ => {
      throw_type_error(scope, "Failed to serialize response");
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializeDeserializeOptions<'a> {
  host_objects: Option<serde_v8::Value<'a>>,
  transfered_array_buffers: Option<serde_v8::Value<'a>>,
}

fn deserialize(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let zero_copy: ZeroCopyBuf = match serde_v8::from_v8(scope, args.get(0)) {
    Ok(zbuf) => zbuf,
    Err(_) => {
      throw_type_error(scope, "Invalid argument 1");
      return;
    }
  };

  let options: Option<SerializeDeserializeOptions> =
    match serde_v8::from_v8(scope, args.get(1)) {
      Ok(opts) => opts,
      Err(_) => {
        throw_type_error(scope, "Invalid argument 2");
        return;
      }
    };

  let options = options.unwrap_or(SerializeDeserializeOptions {
    host_objects: None,
    transfered_array_buffers: None,
  });

  let host_objects = match options.host_objects {
    Some(value) => match v8::Local::<v8::Array>::try_from(value.v8_value) {
      Ok(host_objects) => Some(host_objects),
      Err(_) => {
        throw_type_error(scope, "host_objects not an array");
        return;
      }
    },
    None => None,
  };

  let transfered_array_buffers = match options.transfered_array_buffers {
    Some(value) => match v8::Local::<v8::Array>::try_from(value.v8_value) {
      Ok(transfered_array_buffers) => Some(transfered_array_buffers),
      Err(_) => {
        throw_type_error(scope, "transfered_array_buffers not an array");
        return;
      }
    },
    None => None,
  };

  let serialize_deserialize = Box::new(SerializeDeserialize { host_objects });
  let mut value_deserializer =
    v8::ValueDeserializer::new(scope, serialize_deserialize, &zero_copy);

  let parsed_header = value_deserializer
    .read_header(scope.get_current_context())
    .unwrap_or_default();
  if !parsed_header {
    let msg = v8::String::new(scope, "could not deserialize value").unwrap();
    let exception = v8::Exception::range_error(scope, msg);
    scope.throw_exception(exception);
    return;
  }

  if let Some(transfered_array_buffers) = transfered_array_buffers {
    let state_rc = JsRuntime::state(scope);
    let state = state_rc.borrow_mut();
    if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store {
      for i in 0..transfered_array_buffers.length() {
        let i = v8::Number::new(scope, i as f64).into();
        let id_val = transfered_array_buffers.get(scope, i).unwrap();
        let id = match id_val.number_value(scope) {
          Some(id) => id as u32,
          None => {
            throw_type_error(
              scope,
              "item in transfered_array_buffers not number",
            );
            return;
          }
        };
        if let Some(backing_store) = shared_array_buffer_store.take(id) {
          let array_buffer =
            v8::ArrayBuffer::with_backing_store(scope, &backing_store);
          value_deserializer.transfer_array_buffer(id, array_buffer);
          transfered_array_buffers.set(scope, id_val, array_buffer.into());
        } else {
          throw_type_error(
            scope,
            "transfered array buffer not present in shared_array_buffer_store",
          );
          return;
        }
      }
    }
  }

  let value = value_deserializer.read_value(scope.get_current_context());

  match value {
    Some(deserialized) => rv.set(deserialized),
    None => {
      let msg = v8::String::new(scope, "could not deserialize value").unwrap();
      let exception = v8::Exception::range_error(scope, msg);
      scope.throw_exception(exception);
    }
  };
}

fn queue_microtask(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  match v8::Local::<v8::Function>::try_from(args.get(0)) {
    Ok(f) => scope.enqueue_microtask(f),
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
    }
  };
}

fn create_host_object(
  scope: &mut v8::HandleScope,
  _args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let template = v8::ObjectTemplate::new(scope);
  template.set_internal_field_count(1);
  if let Some(obj) = template.new_instance(scope) {
    rv.set(obj.into());
  };
}

/// Called by V8 during `JsRuntime::instantiate_module`.
///
/// This function borrows `ModuleMap` from the isolate slot,
/// so it is crucial to ensure there are no existing borrows
/// of `ModuleMap` when `JsRuntime::instantiate_module` is called.
pub fn module_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  _import_assertions: v8::Local<'s, v8::FixedArray>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  let scope = &mut unsafe { v8::CallbackScope::new(context) };

  let module_map_rc = JsRuntime::module_map(scope);
  let module_map = module_map_rc.borrow();

  let referrer_global = v8::Global::new(scope, referrer);

  let referrer_info = module_map
    .get_info(&referrer_global)
    .expect("ModuleInfo not found");
  let referrer_name = referrer_info.name.to_string();

  let specifier_str = specifier.to_rust_string_lossy(scope);

  let maybe_module =
    module_map.resolve_callback(scope, &specifier_str, &referrer_name);
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

// Returns promise details or throw TypeError, if argument passed isn't a Promise.
// Promise details is a js_two elements array.
// promise_details = [State, Result]
// State = enum { Pending = 0, Fulfilled = 1, Rejected = 2}
// Result = PromiseResult<T> | PromiseError
fn get_promise_details(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let promise = match v8::Local::<v8::Promise>::try_from(args.get(0)) {
    Ok(val) => val,
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };

  #[derive(Serialize)]
  struct PromiseDetails<'s>(u32, Option<serde_v8::Value<'s>>);

  match promise.state() {
    v8::PromiseState::Pending => {
      rv.set(to_v8(scope, PromiseDetails(0, None)).unwrap());
    }
    v8::PromiseState::Fulfilled => {
      let promise_result = promise.result(scope);
      rv.set(
        to_v8(scope, PromiseDetails(1, Some(promise_result.into()))).unwrap(),
      );
    }
    v8::PromiseState::Rejected => {
      let promise_result = promise.result(scope);
      rv.set(
        to_v8(scope, PromiseDetails(2, Some(promise_result.into()))).unwrap(),
      );
    }
  }
}

// Based on https://github.com/nodejs/node/blob/1e470510ff74391d7d4ec382909ea8960d2d2fbc/src/node_util.cc
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.
fn get_proxy_details(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  // Return undefined if it's not a proxy.
  let proxy = match v8::Local::<v8::Proxy>::try_from(args.get(0)) {
    Ok(val) => val,
    Err(_) => {
      return;
    }
  };

  let target = proxy.get_target(scope);
  let handler = proxy.get_handler(scope);
  let p: (serde_v8::Value, serde_v8::Value) = (target.into(), handler.into());
  rv.set(to_v8(scope, p).unwrap());
}

fn is_proxy(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  rv.set(v8::Boolean::new(scope, args.get(0).is_proxy()).into())
}

fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
  let message = v8::String::new(scope, message.as_ref()).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}

fn memory_usage(
  scope: &mut v8::HandleScope,
  _args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let stats = get_memory_usage(scope);
  rv.set(to_v8(scope, stats).unwrap());
}

// HeapStats stores values from a isolate.get_heap_statistics() call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryUsage {
  rss: usize,
  heap_total: usize,
  heap_used: usize,
  external: usize,
  // TODO: track ArrayBuffers, would require using a custom allocator to track
  // but it's otherwise a subset of external so can be indirectly tracked
  // array_buffers: usize,
}
fn get_memory_usage(isolate: &mut v8::Isolate) -> MemoryUsage {
  let mut s = v8::HeapStatistics::default();
  isolate.get_heap_statistics(&mut s);

  MemoryUsage {
    rss: s.total_physical_size(),
    heap_total: s.total_heap_size(),
    heap_used: s.used_heap_size(),
    external: s.external_memory(),
  }
}
