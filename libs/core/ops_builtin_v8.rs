// Copyright 2018-2025 the Deno authors. MIT license.

use crate::JsBuffer;
use crate::JsRuntime;
use crate::OpState;
use crate::convert::Uint8Array;
use crate::error;
use crate::error::CoreError;
use crate::error::JsError;
use crate::error::is_instance_of_error;
use crate::io::ResourceError;
use crate::modules::script_origin;
use crate::op2;
use crate::ops_builtin::WasmStreamingResource;
use crate::resolve_url;
use crate::runtime::JsRealm;
use crate::runtime::JsRuntimeState;
use crate::runtime::v8_static_strings;
use crate::source_map::SourceMapApplication;
use crate::stats::RuntimeActivityType;
use deno_error::JsErrorBox;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

#[op2(fast)]
pub fn op_add_main_module_handler(
  scope: &mut v8::PinScope,
  f: v8::Local<v8::Function>,
) {
  let f = v8::Global::new(scope, f);

  JsRealm::module_map_from(scope)
    .get_data()
    .borrow_mut()
    .main_module_callbacks
    .push(f);
}

#[op2(fast)]
pub fn op_set_handled_promise_rejection_handler(
  scope: &mut v8::PinScope,
  f: Option<v8::Local<v8::Function>>,
) {
  let exception_state = JsRealm::exception_state_from_scope(scope);
  *exception_state.js_handled_promise_rejection_cb.borrow_mut() =
    f.map(|f| v8::Global::new(scope, f));
}

#[op2(fast)]
pub fn op_ref_op(scope: &mut v8::PinScope, promise_id: i32) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.unrefed_ops.borrow_mut().remove(&promise_id);
}

#[op2(fast)]
pub fn op_unref_op(scope: &mut v8::PinScope, promise_id: i32) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.unrefed_ops.borrow_mut().insert(promise_id);
}

#[op2(fast)]
pub fn op_leak_tracing_enable(scope: &mut v8::PinScope, enabled: bool) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.activity_traces.set_enabled(enabled);
}

#[op2(fast)]
pub fn op_leak_tracing_submit(
  scope: &mut v8::PinScope,
  #[smi] kind: u8,
  #[smi] id: i32,
  #[string] trace: &str,
) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.activity_traces.submit(
    RuntimeActivityType::from_u8(kind),
    id as _,
    trace,
  );
}

#[op2]
pub fn op_leak_tracing_get_all<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Value> {
  let context_state = JsRealm::state_from_scope(scope);
  // This is relatively inefficient, but so is leak tracing
  let out = v8::Array::new(scope, 0);

  let mut idx = 0;
  context_state.activity_traces.get_all(|kind, id, trace| {
    let val =
      serde_v8::to_v8(scope, (kind as u8, id.to_string(), trace.to_owned()))
        .unwrap();
    out.set_index(scope, idx, val);
    idx += 1;
  });
  out.into()
}

#[op2]
pub fn op_leak_tracing_get<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  #[smi] kind: u8,
  #[smi] id: i32,
) -> v8::Local<'s, v8::Value> {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.activity_traces.get(
    RuntimeActivityType::from_u8(kind),
    id as _,
    |maybe_str| {
      if let Some(s) = maybe_str {
        let v8_str = v8::String::new(scope, s).unwrap();
        v8_str.into()
      } else {
        v8::undefined(scope).into()
      }
    },
  )
}

/// Queue a timer. We return a "large integer" timer ID in an f64 which allows for up
/// to `MAX_SAFE_INTEGER` (2^53) timers to exist, versus 2^32 timers if we used
/// `u32`.
#[op2(fast)]
pub fn op_timer_queue(
  scope: &mut v8::PinScope,
  depth: u32,
  repeat: bool,
  timeout_ms: f64,
  task: v8::Local<v8::Function>,
) -> f64 {
  let task = v8::Global::new(scope, task);
  let context_state = JsRealm::state_from_scope(scope);
  if repeat {
    context_state
      .timers
      .queue_timer_repeat(timeout_ms as _, (task, depth)) as _
  } else {
    context_state
      .timers
      .queue_timer(timeout_ms as _, (task, depth)) as _
  }
}

/// Queue a timer. We return a "large integer" timer ID in an f64 which allows for up
/// to `MAX_SAFE_INTEGER` (2^53) timers to exist, versus 2^32 timers if we used
/// `u32`.
#[op2(fast)]
pub fn op_timer_queue_system(
  scope: &mut v8::PinScope,
  repeat: bool,
  timeout_ms: f64,
  task: v8::Local<v8::Function>,
) -> f64 {
  let task = v8::Global::new(scope, task);
  let context_state = JsRealm::state_from_scope(scope);
  context_state
    .timers
    .queue_system_timer(repeat, timeout_ms as _, (task, 0)) as _
}

#[op2(fast)]
pub fn op_timer_cancel(scope: &mut v8::PinScope, id: f64) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.timers.cancel_timer(id as _);
  context_state
    .activity_traces
    .complete(RuntimeActivityType::Timer, id as _);
}

#[op2(fast)]
pub fn op_timer_ref(scope: &mut v8::PinScope, id: f64) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.timers.ref_timer(id as _);
}

#[op2(fast)]
pub fn op_timer_unref(scope: &mut v8::PinScope, id: f64) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.timers.unref_timer(id as _);
}

#[op2(reentrant)]
pub fn op_lazy_load_esm(
  scope: &mut v8::PinScope,
  #[string] module_specifier: String,
) -> Result<v8::Global<v8::Value>, CoreError> {
  let module_map_rc = JsRealm::module_map_from(scope);
  module_map_rc.lazy_load_esm_module(scope, &module_specifier)
}

// We run in a `nofast` op here so we don't get put into a `DisallowJavascriptExecutionScope` and we're
// allowed to touch JS heap.
#[op2(nofast)]
pub fn op_queue_microtask(
  isolate: &mut v8::Isolate,
  cb: v8::Local<v8::Function>,
) {
  isolate.enqueue_microtask(cb);
}

// We run in a `nofast` op here so we don't get put into a `DisallowJavascriptExecutionScope` and we're
// allowed to touch JS heap.
#[op2(nofast, reentrant)]
pub fn op_run_microtasks(isolate: &mut v8::Isolate) {
  isolate.perform_microtask_checkpoint()
}

#[op2(fast)]
pub fn op_has_tick_scheduled(scope: &mut v8::PinScope) -> bool {
  JsRealm::state_from_scope(scope)
    .has_next_tick_scheduled
    .get()
}

#[op2(fast)]
pub fn op_set_has_tick_scheduled(scope: &mut v8::PinScope, v: bool) {
  JsRealm::state_from_scope(scope)
    .has_next_tick_scheduled
    .set(v);
}

#[op2(fast)]
pub fn op_immediate_count(scope: &mut v8::PinScope, increase: bool) -> u32 {
  let state = JsRealm::state_from_scope(scope);
  let mut immediate_info = state.immediate_info.borrow_mut();

  if increase {
    immediate_info.count += 1;
  } else {
    immediate_info.count -= 1;
  }

  immediate_info.count
}

#[op2(fast)]
pub fn op_immediate_ref_count(scope: &mut v8::PinScope, increase: bool) -> u32 {
  let state = JsRealm::state_from_scope(scope);
  let mut immediate_info = state.immediate_info.borrow_mut();

  if increase {
    immediate_info.ref_count += 1;
  } else {
    immediate_info.ref_count -= 1;
  }

  immediate_info.ref_count
}

#[op2(fast)]
pub fn op_immediate_set_has_outstanding(
  scope: &mut v8::PinScope,
  has_outstanding: bool,
) {
  JsRealm::state_from_scope(scope)
    .immediate_info
    .borrow_mut()
    .has_outstanding = has_outstanding;
}

#[op2(fast)]
pub fn op_immediate_has_ref_count(scope: &mut v8::PinScope) -> bool {
  JsRealm::state_from_scope(scope)
    .immediate_info
    .borrow()
    .ref_count
    > 0
}

pub struct EvalContextError<'s> {
  thrown: v8::Local<'s, v8::Value>,
  is_native_error: bool,
  is_compile_error: bool,
}

impl<'s> EvalContextError<'s> {
  fn to_v8<'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> v8::Local<'s, v8::Value> {
    let arr = v8::Array::new(scope, 3);
    arr.set_index(scope, 0, self.thrown);
    let v = v8::Boolean::new(scope, self.is_native_error);
    arr.set_index(scope, 1, v.into());
    let v = v8::Boolean::new(scope, self.is_compile_error);
    arr.set_index(scope, 2, v.into());
    arr.into()
  }
}

#[op2(reentrant)]
pub fn op_eval_context<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  source: v8::Local<'s, v8::Value>,
  #[string] specifier: String,
  host_defined_options: Option<v8::Local<'s, v8::Array>>,
) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
  let out = v8::Array::new(scope, 2);
  let state = JsRuntime::state_from(scope);
  v8::tc_scope!(let tc_scope, scope);

  let source = v8::Local::<v8::String>::try_from(source)
    .map_err(|_| JsErrorBox::type_error("Invalid source"))?;
  let specifier = resolve_url(&specifier).map_err(JsErrorBox::from_err)?;
  let specifier_v8 = v8::String::new(tc_scope, specifier.as_str()).unwrap();
  let host_defined_options = match host_defined_options {
    Some(array) => {
      let output = v8::PrimitiveArray::new(tc_scope, array.length() as _);
      for i in 0..array.length() {
        let value = array.get_index(tc_scope, i).unwrap();
        let value = value
          .try_cast::<v8::Primitive>()
          .map_err(|e| JsErrorBox::from_err(crate::error::DataError(e)))?;
        output.set(tc_scope, i as _, value);
      }
      Some(output.into())
    }
    None => None,
  };
  let origin =
    script_origin(tc_scope, specifier_v8, false, host_defined_options);

  let (maybe_script, maybe_code_cache_hash) = state
    .eval_context_get_code_cache_cb
    .borrow()
    .as_ref()
    .map(|cb| {
      let code_cache = cb(&specifier, &source).unwrap();
      if let Some(code_cache_data) = &code_cache.data {
        let mut source = v8::script_compiler::Source::new_with_cached_data(
          source,
          Some(&origin),
          v8::CachedData::new(code_cache_data),
        );
        let script = v8::script_compiler::compile(
          tc_scope,
          &mut source,
          v8::script_compiler::CompileOptions::ConsumeCodeCache,
          v8::script_compiler::NoCacheReason::NoReason,
        );
        // Check if the provided code cache is rejected by V8.
        let rejected = match source.get_cached_data() {
          Some(cached_data) => cached_data.rejected(),
          _ => true,
        };
        let maybe_code_cache_hash = if rejected {
          Some(code_cache.hash) // recreate the cache
        } else {
          None
        };
        (Some(script), maybe_code_cache_hash)
      } else {
        (None, Some(code_cache.hash))
      }
    })
    .unwrap_or_else(|| (None, None));
  let script = maybe_script
    .unwrap_or_else(|| v8::Script::compile(tc_scope, source, Some(&origin)));

  let null = v8::null(tc_scope);
  let script = match script {
    Some(s) => s,
    None => {
      assert!(tc_scope.has_caught());
      let exception = tc_scope.exception().unwrap();
      let e = EvalContextError {
        thrown: exception,
        is_native_error: is_instance_of_error(tc_scope, exception),
        is_compile_error: true,
      };
      let eval_context_error = e.to_v8(tc_scope);
      out.set_index(tc_scope, 0, null.into());
      out.set_index(tc_scope, 1, eval_context_error);
      return Ok(out.into());
    }
  };

  if let Some(code_cache_hash) = maybe_code_cache_hash
    && let Some(cb) = state.eval_context_code_cache_ready_cb.borrow().as_ref()
  {
    let unbound_script = script.get_unbound_script(tc_scope);
    let code_cache = unbound_script.create_code_cache().ok_or_else(|| {
      JsErrorBox::type_error(
        "Unable to get code cache from unbound module script",
      )
    })?;
    cb(specifier, code_cache_hash, &code_cache);
  }

  match script.run(tc_scope) {
    Some(result) => {
      out.set_index(tc_scope, 0, result);
      out.set_index(tc_scope, 1, null.into());
      Ok(out.into())
    }
    None => {
      assert!(tc_scope.has_caught());
      let exception = tc_scope.exception().unwrap();
      let e = EvalContextError {
        thrown: exception,
        is_native_error: is_instance_of_error(tc_scope, exception),
        is_compile_error: false,
      };
      let eval_context_error = e.to_v8(tc_scope);
      out.set_index(tc_scope, 0, null.into());
      out.set_index(tc_scope, 1, eval_context_error);
      Ok(out.into())
    }
  }
}

#[op2]
pub fn op_encode<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  text: v8::Local<'s, v8::Value>,
) -> Result<v8::Local<'s, v8::Uint8Array>, JsErrorBox> {
  let text = v8::Local::<v8::String>::try_from(text)
    .map_err(|_| JsErrorBox::type_error("Invalid argument"))?;
  let text_str = serde_v8::to_utf8(text, scope);
  let bytes = text_str.into_bytes();
  let len = bytes.len();
  let backing_store =
    v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
  let buffer = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
  let u8array = v8::Uint8Array::new(scope, buffer, 0, len).unwrap();
  Ok(u8array)
}

#[op2]
pub fn op_decode<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  #[buffer] zero_copy: &[u8],
) -> Result<v8::Local<'s, v8::String>, JsErrorBox> {
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
    Some(text) => Ok(text),
    None => Err(JsErrorBox::range_error("string too long")),
  }
}

struct SerializeDeserialize<'a> {
  host_objects: Option<v8::Local<'a, v8::Array>>,
  error_callback: Option<v8::Local<'a, v8::Function>>,
  for_storage: bool,
  host_object_brand: Option<v8::Local<'a, v8::Symbol>>,
  deserializers: Option<v8::Local<'a, v8::Object>>,
}

impl v8::ValueSerializerImpl for SerializeDeserialize<'_> {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    message: v8::Local<'s, v8::String>,
  ) {
    if let Some(cb) = self.error_callback {
      v8::tc_scope!(let scope, scope);

      let undefined = v8::undefined(scope).into();
      cb.call(scope, undefined, &[message.into()]);
      if scope.has_caught() || scope.has_terminated() {
        scope.rethrow();
        return;
      };
    }
    let error = v8::Exception::type_error(scope, message);
    scope.throw_exception(error);
  }

  fn get_shared_array_buffer_id<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
  ) -> Option<u32> {
    if self.for_storage {
      return None;
    }
    let state = JsRuntime::state_from(scope);
    match &state.shared_array_buffer_store {
      Some(shared_array_buffer_store) => {
        let backing_store = shared_array_buffer.get_backing_store();
        let id = shared_array_buffer_store.insert(backing_store);
        Some(id)
      }
      _ => None,
    }
  }

  fn get_wasm_module_transfer_id<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    module: v8::Local<v8::WasmModuleObject>,
  ) -> Option<u32> {
    if self.for_storage {
      let message = v8::String::new(scope, "Wasm modules cannot be stored")?;
      self.throw_data_clone_error(scope, message);
      return None;
    }
    let state = JsRuntime::state_from(scope);
    match &state.compiled_wasm_module_store {
      Some(compiled_wasm_module_store) => {
        let compiled_wasm_module = module.get_compiled_module();
        let id = compiled_wasm_module_store.insert(compiled_wasm_module);
        Some(id)
      }
      _ => None,
    }
  }

  fn has_custom_host_object(&self, _isolate: &v8::Isolate) -> bool {
    self.host_object_brand.is_some()
  }

  fn is_host_object<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    object: v8::Local<'s, v8::Object>,
  ) -> Option<bool> {
    match self.host_object_brand {
      Some(symbol) => object.has(scope, symbol.into()),
      _ => Some(false),
    }
  }

  fn write_host_object<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    object: v8::Local<'s, v8::Object>,
    value_serializer: &dyn v8::ValueSerializerHelper,
  ) -> Option<bool> {
    if let Some(host_object_brand) = self.host_object_brand {
      let value = object.get(scope, host_object_brand.into())?;
      if let Ok(func) = value.try_cast::<v8::Function>() {
        let result = func.call(scope, object.into(), &[])?;
        value_serializer.write_uint32(u32::MAX);
        value_serializer.write_value(scope.get_current_context(), result);
        return Some(true);
      }
    }
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

impl v8::ValueDeserializerImpl for SerializeDeserialize<'_> {
  fn get_shared_array_buffer_from_id<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    transfer_id: u32,
  ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
    if self.for_storage {
      return None;
    }
    let state = JsRuntime::state_from(scope);
    match &state.shared_array_buffer_store {
      Some(shared_array_buffer_store) => {
        let backing_store = shared_array_buffer_store.take(transfer_id)?;
        let shared_array_buffer =
          v8::SharedArrayBuffer::with_backing_store(scope, &backing_store);
        Some(shared_array_buffer)
      }
      _ => None,
    }
  }

  fn get_wasm_module_from_id<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    clone_id: u32,
  ) -> Option<v8::Local<'s, v8::WasmModuleObject>> {
    if self.for_storage {
      return None;
    }
    let state = JsRuntime::state_from(scope);
    match &state.compiled_wasm_module_store {
      Some(compiled_wasm_module_store) => {
        let compiled_module = compiled_wasm_module_store.take(clone_id)?;
        v8::WasmModuleObject::from_compiled_module(scope, &compiled_module)
      }
      _ => None,
    }
  }

  fn read_host_object<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    value_deserializer: &dyn v8::ValueDeserializerHelper,
  ) -> Option<v8::Local<'s, v8::Object>> {
    let mut i = 0;
    if !value_deserializer.read_uint32(&mut i) {
      return None;
    }
    if i == u32::MAX {
      if let Some(deserializers) = self.deserializers
        && let Some(value) =
          value_deserializer.read_value(scope.get_current_context())
        && let Some(object) = value.to_object(scope)
      {
        let key = crate::runtime::v8_static_strings::TYPE
          .v8_string(scope)
          .unwrap();
        let ty = object.get(scope, key.into())?;
        let func = deserializers.get(scope, ty)?;
        let recv = v8::null(scope).into();
        let scope =
          std::pin::pin!(v8::AllowJavascriptExecutionScope::new(scope));
        let scope = &mut scope.init();
        let res = func.cast::<v8::Function>().call(scope, recv, &[value])?;
        return res.to_object(scope);
      }
    } else if let Some(host_objects) = self.host_objects {
      let maybe_value = host_objects.get_index(scope, i);
      if let Some(value) = maybe_value {
        return value.to_object(scope);
      }
    }

    let message: v8::Local<v8::String> =
      v8::String::new(scope, "Failed to deserialize host object").unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
    None
  }
}

// May be reentrant in the case of errors.
#[op2(reentrant)]
pub fn op_serialize<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
  host_objects: Option<v8::Local<'s, v8::Value>>,
  transferred_array_buffers: Option<v8::Local<'s, v8::Value>>,
  for_storage: bool,
  error_callback: Option<v8::Local<'s, v8::Value>>,
) -> Result<Uint8Array, JsErrorBox> {
  let error_callback = match error_callback {
    Some(cb) => Some(
      v8::Local::<v8::Function>::try_from(cb)
        .map_err(|_| JsErrorBox::type_error("Invalid error callback"))?,
    ),
    None => None,
  };
  let host_objects = match host_objects {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value)
        .map_err(|_| JsErrorBox::type_error("hostObjects not an array"))?,
    ),
    None => None,
  };
  let transferred_array_buffers = match transferred_array_buffers {
    Some(value) => {
      Some(v8::Local::<v8::Array>::try_from(value).map_err(|_| {
        JsErrorBox::type_error("transferredArrayBuffers not an array")
      })?)
    }
    None => None,
  };

  let key = v8_static_strings::HOST_OBJECT.v8_string(scope).unwrap();
  let symbol = v8::Symbol::for_key(scope, key);
  let host_object_brand = Some(symbol);

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects,
    error_callback,
    for_storage,
    host_object_brand,
    deserializers: None,
  });
  let value_serializer = v8::ValueSerializer::new(scope, serialize_deserialize);
  value_serializer.write_header();

  if let Some(transferred_array_buffers) = transferred_array_buffers {
    let state = JsRuntime::state_from(scope);
    for index in 0..transferred_array_buffers.length() {
      let i = v8::Number::new(scope, index as f64).into();
      let buf = transferred_array_buffers.get(scope, i).unwrap();
      let buf = v8::Local::<v8::ArrayBuffer>::try_from(buf).map_err(|_| {
        JsErrorBox::type_error(
          "item in transferredArrayBuffers not an ArrayBuffer",
        )
      })?;
      if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store
      {
        if !buf.is_detachable() {
          return Err(JsErrorBox::type_error(
            "item in transferredArrayBuffers is not transferable",
          ));
        }

        if buf.was_detached() {
          return Err(JsErrorBox::new(
            "DOMExceptionOperationError",
            format!("ArrayBuffer at index {index} is already detached"),
          ));
        }

        let backing_store = buf.get_backing_store();
        buf.detach(None);
        let id = shared_array_buffer_store.insert(backing_store);
        value_serializer.transfer_array_buffer(id, buf);
        let id = v8::Number::new(scope, id as f64).into();
        transferred_array_buffers.set(scope, i, id);
      }
    }
  }

  v8::tc_scope!(let scope, scope);

  let ret = value_serializer.write_value(scope.get_current_context(), value);
  if scope.has_caught() || scope.has_terminated() {
    scope.rethrow();
    // Dummy value, this result will be discarded because an error was thrown.
    Ok(vec![].into())
  } else if let Some(true) = ret {
    let vector = value_serializer.release();
    Ok(vector.into())
  } else {
    Err(JsErrorBox::type_error("Failed to serialize response"))
  }
}

#[op2]
pub fn op_deserialize<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  #[buffer] zero_copy: JsBuffer,
  host_objects: Option<v8::Local<'s, v8::Value>>,
  transferred_array_buffers: Option<v8::Local<'s, v8::Value>>,
  deserializers: Option<v8::Local<'s, v8::Value>>,
  for_storage: bool,
) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
  let host_objects = match host_objects {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value)
        .map_err(|_| JsErrorBox::type_error("hostObjects not an array"))?,
    ),
    None => None,
  };
  let transferred_array_buffers = match transferred_array_buffers {
    Some(value) => {
      Some(v8::Local::<v8::Array>::try_from(value).map_err(|_| {
        JsErrorBox::type_error("transferredArrayBuffers not an array")
      })?)
    }
    None => None,
  };
  let deserializers = match deserializers {
    Some(value) => Some(
      v8::Local::<v8::Object>::try_from(value)
        .map_err(|_| JsErrorBox::type_error("deserializers not an object"))?,
    ),
    None => None,
  };

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects,
    error_callback: None,
    for_storage,
    host_object_brand: None,
    deserializers,
  });
  let value_deserializer =
    v8::ValueDeserializer::new(scope, serialize_deserialize, &zero_copy);
  let parsed_header = value_deserializer
    .read_header(scope.get_current_context())
    .unwrap_or_default();
  if !parsed_header {
    return Err(JsErrorBox::range_error("could not deserialize value"));
  }

  if let Some(transferred_array_buffers) = transferred_array_buffers {
    let state = JsRuntime::state_from(scope);
    if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store {
      for i in 0..transferred_array_buffers.length() {
        let i = v8::Number::new(scope, i as f64).into();
        let id_val = transferred_array_buffers.get(scope, i).unwrap();
        let id = match id_val.number_value(scope) {
          Some(id) => id as u32,
          None => {
            return Err(JsErrorBox::type_error(
              "item in transferredArrayBuffers not number",
            ));
          }
        };
        match shared_array_buffer_store.take(id) {
          Some(backing_store) => {
            let array_buffer =
              v8::ArrayBuffer::with_backing_store(scope, &backing_store);
            value_deserializer.transfer_array_buffer(id, array_buffer);
            transferred_array_buffers.set(scope, i, array_buffer.into());
          }
          _ => {
            return Err(JsErrorBox::type_error(
              "transferred array buffer not present in shared_array_buffer_store",
            ));
          }
        }
      }
    }
  }

  let value = value_deserializer.read_value(scope.get_current_context());
  match value {
    Some(deserialized) => Ok(deserialized),
    None => Err(JsErrorBox::range_error("could not deserialize value")),
  }
}

// Specialized op for `structuredClone` API called with no `options` argument.
#[op2]
pub fn op_structured_clone<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
  deserializers: Option<v8::Local<'s, v8::Object>>,
) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
  let key = v8_static_strings::HOST_OBJECT.v8_string(scope).unwrap();
  let symbol = v8::Symbol::for_key(scope, key);
  let host_object_brand = Some(symbol);

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects: None,
    error_callback: None,
    for_storage: false,
    host_object_brand,
    deserializers: None,
  });
  let value_serializer = v8::ValueSerializer::new(scope, serialize_deserialize);
  value_serializer.write_header();

  v8::tc_scope!(let scope, scope);

  let ret = value_serializer.write_value(scope.get_current_context(), value);
  if scope.has_caught() || scope.has_terminated() {
    scope.rethrow();
    // Dummy value, this result will be discarded because an error was thrown.
    let v = v8::undefined(scope);
    return Ok(v.into());
  }

  if !matches!(ret, Some(true)) {
    return Err(JsErrorBox::type_error("Failed to serialize response"));
  }

  let vector = value_serializer.release();

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects: None,
    error_callback: None,
    for_storage: false,
    host_object_brand,
    deserializers,
  });
  let value_deserializer =
    v8::ValueDeserializer::new(scope, serialize_deserialize, &vector);
  let parsed_header = value_deserializer
    .read_header(scope.get_current_context())
    .unwrap_or_default();
  if !parsed_header {
    return Err(JsErrorBox::range_error("could not deserialize value"));
  }

  let value = value_deserializer.read_value(scope.get_current_context());
  match value {
    Some(deserialized) => Ok(deserialized),
    None => Err(JsErrorBox::range_error("could not deserialize value")),
  }
}

#[op2]
pub fn op_get_promise_details<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  promise: v8::Local<'s, v8::Promise>,
) -> v8::Local<'s, v8::Value> {
  let out = v8::Array::new(scope, 2);

  let (i, val) = match promise.state() {
    v8::PromiseState::Pending => {
      (v8::Integer::new(scope, 0), v8::null(scope).into())
    }
    v8::PromiseState::Fulfilled => {
      (v8::Integer::new(scope, 1), promise.result(scope))
    }
    v8::PromiseState::Rejected => {
      (v8::Integer::new(scope, 2), promise.result(scope))
    }
  };

  out.set_index(scope, 0, i.into());
  out.set_index(scope, 1, val);

  out.into()
}

#[op2(fast)]
pub fn op_set_promise_hooks<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  init_hook: v8::Local<'s, v8::Value>,
  before_hook: v8::Local<'s, v8::Value>,
  after_hook: v8::Local<'s, v8::Value>,
  resolve_hook: v8::Local<'s, v8::Value>,
) -> Result<(), crate::error::DataError> {
  let v8_fns = [init_hook, before_hook, after_hook, resolve_hook]
    .into_iter()
    .enumerate()
    .filter(|(_, hook)| !hook.is_undefined())
    .try_fold([None; 4], |mut v8_fns, (i, hook)| {
      let v8_fn = v8::Local::<v8::Function>::try_from(hook)?;
      v8_fns[i] = Some(v8_fn);
      Ok::<_, crate::error::DataError>(v8_fns)
    })?;

  scope.set_promise_hooks(
    v8_fns[0], // init
    v8_fns[1], // before
    v8_fns[2], // after
    v8_fns[3], // resolve
  );

  Ok(())
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

#[op2]
pub fn op_get_proxy_details<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  proxy: v8::Local<'s, v8::Value>,
) -> v8::Local<'s, v8::Value> {
  let Ok(proxy) = v8::Local::<v8::Proxy>::try_from(proxy) else {
    return v8::null(scope).into();
  };
  let out_array = v8::Array::new(scope, 2);
  let target = proxy.get_target(scope);
  out_array.set_index(scope, 0, target);
  let handler = proxy.get_handler(scope);
  out_array.set_index(scope, 1, handler);
  out_array.into()
}

#[op2]
pub fn op_get_non_index_property_names<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Value>,
  filter: u32,
) -> Option<v8::Local<'s, v8::Value>> {
  let obj = match v8::Local::<v8::Object>::try_from(obj) {
    Ok(proxy) => proxy,
    Err(_) => return None,
  };

  let mut property_filter = v8::PropertyFilter::ALL_PROPERTIES;
  if filter & 1 == 1 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_WRITABLE
  }
  if filter & 2 == 2 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_ENUMERABLE
  }
  if filter & 4 == 4 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_CONFIGURABLE
  }
  if filter & 8 == 8 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_STRINGS
  }
  if filter & 16 == 16 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_SYMBOLS
  }

  let maybe_names = obj.get_property_names(
    scope,
    v8::GetPropertyNamesArgs {
      mode: v8::KeyCollectionMode::OwnOnly,
      property_filter,
      index_filter: v8::IndexFilter::SkipIndices,
      ..Default::default()
    },
  );

  maybe_names.map(|names| names.into())
}

#[op2]
#[string]
pub fn op_get_constructor_name<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  obj: v8::Local<'s, v8::Value>,
) -> Option<String> {
  let obj = match v8::Local::<v8::Object>::try_from(obj) {
    Ok(proxy) => proxy,
    Err(_) => return None,
  };

  let name = obj.get_constructor_name().to_rust_string_lossy(scope);
  Some(name)
}

// HeapStats stores values from a isolate.get_heap_statistics() call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
  physical_total: usize,
  heap_total: usize,
  heap_used: usize,
  external: usize,
  // TODO: track ArrayBuffers, would require using a custom allocator to track
  // but it's otherwise a subset of external so can be indirectly tracked
  // array_buffers: usize,
}

#[op2]
#[serde]
pub fn op_memory_usage(scope: &mut v8::PinScope<'_, '_>) -> MemoryUsage {
  let s = scope.get_heap_statistics();
  MemoryUsage {
    physical_total: s.total_physical_size(),
    heap_total: s.total_heap_size(),
    heap_used: s.used_heap_size(),
    external: s.external_memory(),
  }
}

#[op2]
pub fn op_get_ext_import_meta_proto<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Value> {
  let context_state_rc = JsRealm::state_from_scope(scope);
  if let Some(proto) = context_state_rc.ext_import_meta_proto.borrow().clone() {
    v8::Local::new(scope, proto).into()
  } else {
    v8::null(scope).into()
  }
}

#[op2(fast)]
pub fn op_set_wasm_streaming_callback(
  scope: &mut v8::PinScope,
  cb: v8::Local<v8::Function>,
) -> Result<(), JsErrorBox> {
  let cb = v8::Global::new(scope, cb);
  let context_state_rc = JsRealm::state_from_scope(scope);
  // The callback to pass to the v8 API has to be a unit type, so it can't
  // borrow or move any local variables. Therefore, we're storing the JS
  // callback in a JsRuntimeState slot.
  if context_state_rc.js_wasm_streaming_cb.borrow().is_some() {
    return Err(JsErrorBox::type_error(
      "op_set_wasm_streaming_callback already called",
    ));
  }
  *context_state_rc.js_wasm_streaming_cb.borrow_mut() = Some(cb);

  scope.set_wasm_streaming_callback(|scope, arg, wasm_streaming| {
    let (cb_handle, streaming_rid) = {
      let context_state_rc = JsRealm::state_from_scope(scope);
      let cb_handle = context_state_rc
        .js_wasm_streaming_cb
        .borrow()
        .as_ref()
        .unwrap()
        .clone();
      let state = JsRuntime::state_from(scope);
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
  Ok(())
}

// This op is re-entrant as it makes a v8 call. It also cannot be fast because
// we require a JS execution scope.
#[allow(clippy::let_and_return)]
#[op2(nofast, reentrant)]
pub fn op_abort_wasm_streaming(
  state: Rc<RefCell<OpState>>,
  rid: u32,
  error: v8::Local<v8::Value>,
) -> Result<(), ResourceError> {
  // NOTE: v8::WasmStreaming::abort can't be called while `state` is borrowed;
  let wasm_streaming = state
    .borrow_mut()
    .resource_table
    .take::<WasmStreamingResource>(rid)?;

  // At this point there are no clones of Rc<WasmStreamingResource> on the
  // resource table, and no one should own a reference because we're never
  // cloning them. So we can be sure `wasm_streaming` is the only reference.
  match std::rc::Rc::try_unwrap(wasm_streaming) {
    Ok(wsr) => {
      wsr.0.into_inner().abort(Some(error));
    }
    _ => {
      panic!("Couldn't consume WasmStreamingResource.");
    }
  }
  Ok(())
}

// This op calls `op_apply_source_map` re-entrantly.
#[op2(reentrant)]
#[serde]
pub fn op_destructure_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Value>,
) -> JsError {
  *JsError::from_v8_exception(scope, error)
}

/// Effectively throw an uncatchable error. This will terminate runtime
/// execution before any more JS code can run, except in the REPL where it
/// should just output the error to the console.
#[op2(fast, reentrant)]
pub fn op_dispatch_exception<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  exception: v8::Local<'s, v8::Value>,
  promise: bool,
) {
  error::dispatch_exception(scope, exception, promise);
}

#[op2]
#[serde]
pub fn op_op_names(scope: &mut v8::PinScope<'_, '_>) -> Vec<String> {
  let state = JsRealm::state_from_scope(scope);
  state
    .op_ctxs
    .iter()
    .map(|o| o.decl.name.to_string())
    .collect()
}

fn write_line_and_col_to_ret_buf(
  ret_buf: &mut [u8],
  line_number: u32,
  column_number: u32,
) {
  ret_buf[0..4].copy_from_slice(&line_number.to_le_bytes());
  ret_buf[4..8].copy_from_slice(&column_number.to_le_bytes());
}

#[op2]
#[string]
pub fn op_current_user_call_site(
  scope: &mut v8::PinScope,
  js_runtime_state: &JsRuntimeState,
  #[buffer] ret_buf: &mut [u8],
) -> String {
  let stack_trace = v8::StackTrace::current_stack_trace(scope, 10).unwrap();
  let frame_count = stack_trace.get_frame_count();
  for i in 0..frame_count {
    let frame = stack_trace.get_frame(scope, i).unwrap();
    if !frame.is_user_javascript() {
      continue;
    }
    let line_number = frame.get_line_number() as u32;
    let column_number = frame.get_column() as u32;
    let (file_name, application) = match frame.get_script_name(scope) {
      Some(name) => {
        let file_name = name.to_rust_string_lossy(scope);
        // TODO: this condition should be configurable. It's a CLI assumption.
        if (!file_name.starts_with("file:")
          || file_name.contains("/node_modules/"))
          && i != frame_count - 1
        {
          continue;
        }
        let application = js_runtime_state
          .source_mapper
          .borrow_mut()
          .apply_source_map(&file_name, line_number, column_number);
        (file_name, application)
      }
      None => {
        if frame.is_eval() {
          ("[eval]".to_string(), SourceMapApplication::Unchanged)
        } else {
          ("[unknown]".to_string(), SourceMapApplication::Unchanged)
        }
      }
    };
    match application {
      SourceMapApplication::Unchanged => {
        write_line_and_col_to_ret_buf(ret_buf, line_number, column_number);
        return file_name;
      }
      SourceMapApplication::LineAndColumn {
        line_number,
        column_number,
      } => {
        write_line_and_col_to_ret_buf(ret_buf, line_number, column_number);
        return file_name;
      }
      SourceMapApplication::LineAndColumnAndFileName {
        line_number,
        column_number,
        file_name,
      } => {
        write_line_and_col_to_ret_buf(ret_buf, line_number, column_number);
        return file_name;
      }
    }
  }

  unreachable!("No stack frames found on stack at all");
}

/// Set a callback which formats exception messages as stored in
/// `JsError::exception_message`. The callback is passed the error value and
/// should return a string or `null`. If no callback is set or the callback
/// returns `null`, the built-in default formatting will be used.
#[op2]
pub fn op_set_format_exception_callback<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  cb: v8::Local<v8::Function>,
) -> Option<v8::Local<'s, v8::Value>> {
  let cb = v8::Global::new(scope, cb);
  let context_state_rc = JsRealm::state_from_scope(scope);
  let old = context_state_rc
    .exception_state
    .js_format_exception_cb
    .borrow_mut()
    .replace(cb);
  let old = old.map(|v| v8::Local::new(scope, &v));
  old.map(|func| func.into())
}

#[op2(fast)]
pub fn op_event_loop_has_more_work(scope: &mut v8::PinScope<'_, '_>) -> bool {
  JsRuntime::has_more_work(scope)
}

#[op2]
pub fn op_get_extras_binding_object<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Value> {
  let context = scope.get_current_context();
  context.get_extras_binding_object(scope).into()
}
