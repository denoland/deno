// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::JsRuntime;
use crate::Op;
use crate::OpId;
use crate::OpPayload;
use crate::OpResponse;
use crate::OpTable;
use crate::PromiseId;
use crate::ZeroCopyBuf;
use rusty_v8 as v8;
use serde::Serialize;
use serde_v8::to_v8;
use std::cell::Cell;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::io::{stdout, Write};
use std::option::Option;
use url::Url;
use v8::MapFnTo;

lazy_static::lazy_static! {
  pub static ref EXTERNAL_REFERENCES: v8::ExternalReferences =
    v8::ExternalReferences::new(&[
      v8::ExternalReference {
        function: print.map_fn_to()
      },
      v8::ExternalReference {
        function: recv.map_fn_to()
      },
      v8::ExternalReference {
        function: send.map_fn_to()
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
        function: heap_stats.map_fn_to(),
      },
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
  set_func(scope, core_val, "print", print);
  set_func(scope, core_val, "recv", recv);
  set_func(scope, core_val, "send", send);
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
  set_func(scope, core_val, "heapStats", heap_stats);

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
  obj.set(scope, key.into(), val.into());
}

pub fn boxed_slice_to_uint8array<'sc>(
  scope: &mut v8::HandleScope<'sc>,
  buf: Box<[u8]>,
) -> v8::Local<'sc, v8::Uint8Array> {
  assert!(!buf.is_empty());
  let buf_len = buf.len();
  let backing_store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(buf);
  let backing_store_shared = backing_store.make_shared();
  let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
  v8::Uint8Array::new(scope, ab, 0, buf_len)
    .expect("Failed to create UintArray8")
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

pub(crate) unsafe fn get_backing_store_slice(
  backing_store: &v8::SharedRef<v8::BackingStore>,
  byte_offset: usize,
  byte_length: usize,
) -> &[u8] {
  let cells: *const [Cell<u8>] =
    &backing_store[byte_offset..byte_offset + byte_length];
  let bytes = cells as *const [u8];
  &*bytes
}

#[allow(clippy::mut_from_ref)]
pub(crate) unsafe fn get_backing_store_slice_mut(
  backing_store: &v8::SharedRef<v8::BackingStore>,
  byte_offset: usize,
  byte_length: usize,
) -> &mut [u8] {
  let cells: *const [Cell<u8>] =
    &backing_store[byte_offset..byte_offset + byte_length];
  let bytes = cells as *const _ as *mut [u8];
  &mut *bytes
}

fn print(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let arg_len = args.length();
  if !(0..=2).contains(&arg_len) {
    return throw_type_error(scope, "Expected a maximum of 2 arguments.");
  }

  let obj = args.get(0);
  let is_err_arg = args.get(1);

  let mut is_err = false;
  if arg_len == 2 {
    let int_val = match is_err_arg.integer_value(scope) {
      Some(v) => v,
      None => return throw_type_error(scope, "Invalid arugment. Argument 2 should indicate wheter or not to print to stderr."),
    };
    is_err = int_val != 0;
  };
  let tc_scope = &mut v8::TryCatch::new(scope);
  let str_ = match obj.to_string(tc_scope) {
    Some(s) => s,
    None => v8::String::new(tc_scope, "").unwrap(),
  };
  if is_err {
    eprint!("{}", str_.to_rust_string_lossy(tc_scope));
    stdout().flush().unwrap();
  } else {
    print!("{}", str_.to_rust_string_lossy(tc_scope));
    stdout().flush().unwrap();
  }
}

fn recv(
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

  let slot = match &mut state.js_recv_cb {
    slot @ None => slot,
    _ => return throw_type_error(scope, "Deno.core.recv() already called"),
  };

  slot.replace(v8::Global::new(scope, cb));
}

fn send<'s>(
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

  // send(0) returns obj of all ops, handle as special case
  if op_id == 0 {
    // TODO: Serialize as HashMap when serde_v8 supports maps ...
    let ops = OpTable::op_entries(state.op_state.clone());
    rv.set(to_v8(scope, ops).unwrap());
    return;
  }

  // PromiseId
  let arg1 = args.get(1);
  let promise_id = if arg1.is_null_or_undefined() {
    Ok(0) // Accept null or undefined as 0
  } else {
    // Otherwise expect int
    v8::Local::<v8::Integer>::try_from(arg1)
      .map(|l| l.value() as PromiseId)
      .map_err(AnyError::from)
  };
  // Fail if promise id invalid (not null/undefined or int)
  let promise_id: PromiseId = match promise_id {
    Ok(promise_id) => promise_id,
    Err(err) => {
      throw_type_error(scope, format!("invalid promise id: {}", err));
      return;
    }
  };

  // Structured args
  let v = args.get(2);

  // Buf arg (optional)
  let arg3 = args.get(3);
  let buf: Option<ZeroCopyBuf> = if arg3.is_null_or_undefined() {
    None
  } else {
    match v8::Local::<v8::ArrayBufferView>::try_from(arg3)
      .map(|view| ZeroCopyBuf::new(scope, view))
      .map_err(AnyError::from)
    {
      Ok(buf) => Some(buf),
      Err(err) => {
        throw_type_error(scope, format!("Err with buf arg: {}", err));
        return;
      }
    }
  };

  let payload = OpPayload::new(scope, v, promise_id);
  let op = OpTable::route_op(op_id, state.op_state.clone(), payload, buf);
  match op {
    Op::Sync(resp) => match resp {
      OpResponse::Value(v) => {
        rv.set(v.to_v8(scope).unwrap());
      }
      OpResponse::Buffer(buf) => {
        rv.set(boxed_slice_to_uint8array(scope, buf).into());
      }
    },
    Op::Async(fut) => {
      state.pending_ops.push(fut);
      state.have_unpolled_ops = true;
    }
    Op::AsyncUnref(fut) => {
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
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };

  let url = v8::Local::<v8::String>::try_from(args.get(1))
    .map(|n| Url::from_file_path(n.to_rust_string_lossy(scope)).unwrap());

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
        is_native_error: exception.is_native_error(),
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
        is_native_error: exception.is_native_error(),
        is_compile_error: false,
      }),
    );
    rv.set(to_v8(tc_scope, output).unwrap());
    return;
  }

  let output = Output(Some(result.unwrap().into()), None);
  rv.set(to_v8(tc_scope, output).unwrap());
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
  let text_bytes = text_str.as_bytes().to_vec().into_boxed_slice();

  let buf = if text_bytes.is_empty() {
    let ab = v8::ArrayBuffer::new(scope, 0);
    v8::Uint8Array::new(scope, ab, 0, 0).expect("Failed to create UintArray8")
  } else {
    let buf_len = text_bytes.len();
    let backing_store =
      v8::ArrayBuffer::new_backing_store_from_boxed_slice(text_bytes);
    let backing_store_shared = backing_store.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
    v8::Uint8Array::new(scope, ab, 0, buf_len)
      .expect("Failed to create UintArray8")
  };

  rv.set(buf.into())
}

fn decode(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let view = match v8::Local::<v8::ArrayBufferView>::try_from(args.get(0)) {
    Ok(view) => view,
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };

  let backing_store = view.buffer(scope).unwrap().get_backing_store();
  let buf = unsafe {
    get_backing_store_slice(
      &backing_store,
      view.byte_offset(),
      view.byte_length(),
    )
  };

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
  match v8::String::new_from_utf8(scope, &buf, v8::NewStringType::Normal) {
    Some(text) => rv.set(text.into()),
    None => {
      let msg = v8::String::new(scope, "string too long").unwrap();
      let exception = v8::Exception::range_error(scope, msg);
      scope.throw_exception(exception);
    }
  };
}

struct SerializeDeserialize {}

impl v8::ValueSerializerImpl for SerializeDeserialize {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
  }
}

impl v8::ValueDeserializerImpl for SerializeDeserialize {}

fn serialize(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let serialize_deserialize = Box::new(SerializeDeserialize {});
  let mut value_serializer =
    v8::ValueSerializer::new(scope, serialize_deserialize);
  match value_serializer.write_value(scope.get_current_context(), args.get(0)) {
    Some(true) => {
      let vector = value_serializer.release();
      let buf = {
        let buf_len = vector.len();
        let backing_store = v8::ArrayBuffer::new_backing_store_from_boxed_slice(
          vector.into_boxed_slice(),
        );
        let backing_store_shared = backing_store.make_shared();
        let ab =
          v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
        v8::Uint8Array::new(scope, ab, 0, buf_len)
          .expect("Failed to create UintArray8")
      };

      rv.set(buf.into());
    }
    _ => {
      throw_type_error(scope, "Invalid argument");
    }
  }
}

fn deserialize(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let view = match v8::Local::<v8::ArrayBufferView>::try_from(args.get(0)) {
    Ok(view) => view,
    Err(_) => {
      throw_type_error(scope, "Invalid argument");
      return;
    }
  };

  let backing_store = view.buffer(scope).unwrap().get_backing_store();
  let buf = unsafe {
    get_backing_store_slice(
      &backing_store,
      view.byte_offset(),
      view.byte_length(),
    )
  };

  let serialize_deserialize = Box::new(SerializeDeserialize {});
  let mut value_deserializer =
    v8::ValueDeserializer::new(scope, serialize_deserialize, buf);
  let value = value_deserializer.read_value(scope.get_current_context());

  match value {
    Some(deserialized) => rv.set(deserialized),
    None => {
      let msg = v8::String::new(scope, "string too long").unwrap();
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

fn throw_type_error(scope: &mut v8::HandleScope, message: impl AsRef<str>) {
  let message = v8::String::new(scope, message.as_ref()).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}

fn heap_stats(
  scope: &mut v8::HandleScope,
  _args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let stats = get_heap_stats(scope);
  rv.set(to_v8(scope, stats).unwrap());
}

// HeapStats stores values from a isolate.get_heap_statistics() call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HeapStats {
  total_heap_size: usize,
  total_heap_size_executable: usize,
  total_physical_size: usize,
  total_available_size: usize,
  total_global_handles_size: usize,
  used_global_handles_size: usize,
  used_heap_size: usize,
  heap_size_limit: usize,
  malloced_memory: usize,
  external_memory: usize,
  peak_malloced_memory: usize,
  number_of_native_contexts: usize,
  number_of_detached_contexts: usize,
}
fn get_heap_stats(isolate: &mut v8::Isolate) -> HeapStats {
  let mut s = v8::HeapStatistics::default();
  isolate.get_heap_statistics(&mut s);

  HeapStats {
    total_heap_size: s.total_heap_size(),
    total_heap_size_executable: s.total_heap_size_executable(),
    total_physical_size: s.total_physical_size(),
    total_available_size: s.total_available_size(),
    total_global_handles_size: s.total_global_handles_size(),
    used_global_handles_size: s.used_global_handles_size(),
    used_heap_size: s.used_heap_size(),
    heap_size_limit: s.heap_size_limit(),
    malloced_memory: s.malloced_memory(),
    external_memory: s.external_memory(),
    peak_malloced_memory: s.peak_malloced_memory(),
    number_of_native_contexts: s.number_of_native_contexts(),
    number_of_detached_contexts: s.number_of_detached_contexts(),
  }
}
