// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::error::custom_error;
use crate::error::is_instance_of_error;
use crate::error::range_error;
use crate::error::type_error;
use crate::error::JsError;
use crate::ops_builtin::WasmStreamingResource;
use crate::resolve_url;
use crate::runtime::script_origin;
use crate::serde_v8::from_v8;
use crate::source_map::apply_source_map;
use crate::JsBuffer;
use crate::JsRealm;
use crate::JsRuntime;
use crate::ToJsBuffer;
use anyhow::Error;
use deno_ops::op;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

fn to_v8_fn(
  scope: &mut v8::HandleScope,
  value: serde_v8::Value,
) -> Result<v8::Global<v8::Function>, Error> {
  v8::Local::<v8::Function>::try_from(value.v8_value)
    .map(|cb| v8::Global::new(scope, cb))
    .map_err(|err| type_error(err.to_string()))
}

#[inline]
fn to_v8_local_fn(
  value: serde_v8::Value,
) -> Result<v8::Local<v8::Function>, Error> {
  v8::Local::<v8::Function>::try_from(value.v8_value)
    .map_err(|err| type_error(err.to_string()))
}

#[op(v8)]
fn op_ref_op(scope: &mut v8::HandleScope, promise_id: i32) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.borrow_mut().unrefed_ops.remove(&promise_id);
}

#[op(v8)]
fn op_unref_op(scope: &mut v8::HandleScope, promise_id: i32) {
  let context_state = JsRealm::state_from_scope(scope);
  context_state.borrow_mut().unrefed_ops.insert(promise_id);
}

#[op(v8)]
fn op_set_promise_reject_callback<'a>(
  scope: &mut v8::HandleScope<'a>,
  cb: serde_v8::Value,
) -> Result<Option<serde_v8::Value<'a>>, Error> {
  let cb = to_v8_fn(scope, cb)?;
  let context_state_rc = JsRealm::state_from_scope(scope);
  let old = context_state_rc
    .borrow_mut()
    .js_promise_reject_cb
    .replace(Rc::new(cb));
  let old = old.map(|v| v8::Local::new(scope, &*v));
  Ok(old.map(|v| from_v8(scope, v.into()).unwrap()))
}

#[op(v8)]
fn op_run_microtasks(scope: &mut v8::HandleScope) {
  scope.perform_microtask_checkpoint();
}

#[op(v8)]
fn op_has_tick_scheduled(scope: &mut v8::HandleScope) -> bool {
  let state_rc = JsRuntime::state_from(scope);
  let state = state_rc.borrow();
  state.has_tick_scheduled
}

#[op(v8)]
fn op_set_has_tick_scheduled(scope: &mut v8::HandleScope, v: bool) {
  let state_rc = JsRuntime::state_from(scope);
  state_rc.borrow_mut().has_tick_scheduled = v;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvalContextError<'s> {
  thrown: serde_v8::Value<'s>,
  is_native_error: bool,
  is_compile_error: bool,
}

#[derive(Serialize)]
struct EvalContextResult<'s>(
  Option<serde_v8::Value<'s>>,
  Option<EvalContextError<'s>>,
);

#[op(v8)]
fn op_eval_context<'a>(
  scope: &mut v8::HandleScope<'a>,
  source: serde_v8::Value<'a>,
  specifier: String,
) -> Result<EvalContextResult<'a>, Error> {
  let tc_scope = &mut v8::TryCatch::new(scope);
  let source = v8::Local::<v8::String>::try_from(source.v8_value)
    .map_err(|_| type_error("Invalid source"))?;
  let specifier = resolve_url(&specifier)?.to_string();
  let specifier = v8::String::new(tc_scope, &specifier).unwrap();
  let origin = script_origin(tc_scope, specifier);

  let script = match v8::Script::compile(tc_scope, source, Some(&origin)) {
    Some(s) => s,
    None => {
      assert!(tc_scope.has_caught());
      let exception = tc_scope.exception().unwrap();
      return Ok(EvalContextResult(
        None,
        Some(EvalContextError {
          thrown: exception.into(),
          is_native_error: is_instance_of_error(tc_scope, exception),
          is_compile_error: true,
        }),
      ));
    }
  };

  match script.run(tc_scope) {
    Some(result) => Ok(EvalContextResult(Some(result.into()), None)),
    None => {
      assert!(tc_scope.has_caught());
      let exception = tc_scope.exception().unwrap();
      Ok(EvalContextResult(
        None,
        Some(EvalContextError {
          thrown: exception.into(),
          is_native_error: is_instance_of_error(tc_scope, exception),
          is_compile_error: false,
        }),
      ))
    }
  }
}

#[op(v8)]
fn op_queue_microtask(
  scope: &mut v8::HandleScope,
  cb: serde_v8::Value,
) -> Result<(), Error> {
  scope.enqueue_microtask(to_v8_local_fn(cb)?);
  Ok(())
}

#[op(v8)]
fn op_create_host_object<'a>(
  scope: &mut v8::HandleScope<'a>,
) -> serde_v8::Value<'a> {
  let template = v8::ObjectTemplate::new(scope);
  template.set_internal_field_count(1);
  let object = template.new_instance(scope).unwrap();
  from_v8(scope, object.into()).unwrap()
}

#[op(v8)]
fn op_encode<'a>(
  scope: &mut v8::HandleScope<'a>,
  text: serde_v8::Value<'a>,
) -> Result<serde_v8::Value<'a>, Error> {
  let text = v8::Local::<v8::String>::try_from(text.v8_value)
    .map_err(|_| type_error("Invalid argument"))?;
  let text_str = serde_v8::to_utf8(text, scope);
  let bytes = text_str.into_bytes();
  let len = bytes.len();
  let backing_store =
    v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
  let buffer = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
  let u8array = v8::Uint8Array::new(scope, buffer, 0, len).unwrap();
  Ok((from_v8(scope, u8array.into()))?)
}

#[op(v8)]
fn op_decode<'a>(
  scope: &mut v8::HandleScope<'a>,
  zero_copy: &[u8],
) -> Result<serde_v8::Value<'a>, Error> {
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
    Some(text) => Ok(from_v8(scope, text.into())?),
    None => Err(range_error("string too long")),
  }
}

struct SerializeDeserialize<'a> {
  host_objects: Option<v8::Local<'a, v8::Array>>,
  error_callback: Option<v8::Local<'a, v8::Function>>,
  for_storage: bool,
}

impl<'a> v8::ValueSerializerImpl for SerializeDeserialize<'a> {
  #[allow(unused_variables)]
  fn throw_data_clone_error<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    if let Some(cb) = self.error_callback {
      let scope = &mut v8::TryCatch::new(scope);
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

  fn get_shared_array_buffer_id<'s>(
    &mut self,
    scope: &mut v8::HandleScope<'s>,
    shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
  ) -> Option<u32> {
    if self.for_storage {
      return None;
    }
    let state_rc = JsRuntime::state_from(scope);
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
    scope: &mut v8::HandleScope<'_>,
    module: v8::Local<v8::WasmModuleObject>,
  ) -> Option<u32> {
    if self.for_storage {
      let message = v8::String::new(scope, "Wasm modules cannot be stored")?;
      self.throw_data_clone_error(scope, message);
      return None;
    }
    let state_rc = JsRuntime::state_from(scope);
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
    scope: &mut v8::HandleScope<'s>,
    transfer_id: u32,
  ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
    if self.for_storage {
      return None;
    }
    let state_rc = JsRuntime::state_from(scope);
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
    scope: &mut v8::HandleScope<'s>,
    clone_id: u32,
  ) -> Option<v8::Local<'s, v8::WasmModuleObject>> {
    if self.for_storage {
      return None;
    }
    let state_rc = JsRuntime::state_from(scope);
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

    let message: v8::Local<v8::String> =
      v8::String::new(scope, "Failed to deserialize host object").unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
    None
  }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializeDeserializeOptions<'a> {
  host_objects: Option<serde_v8::Value<'a>>,
  transferred_array_buffers: Option<serde_v8::Value<'a>>,
  #[serde(default)]
  for_storage: bool,
}

#[op(v8)]
fn op_serialize(
  scope: &mut v8::HandleScope,
  value: serde_v8::Value,
  options: Option<SerializeDeserializeOptions>,
  error_callback: Option<serde_v8::Value>,
) -> Result<ToJsBuffer, Error> {
  let options = options.unwrap_or_default();
  let error_callback = match error_callback {
    Some(cb) => Some(
      v8::Local::<v8::Function>::try_from(cb.v8_value)
        .map_err(|_| type_error("Invalid error callback"))?,
    ),
    None => None,
  };
  let host_objects = match options.host_objects {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value.v8_value)
        .map_err(|_| type_error("hostObjects not an array"))?,
    ),
    None => None,
  };
  let transferred_array_buffers = match options.transferred_array_buffers {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value.v8_value)
        .map_err(|_| type_error("transferredArrayBuffers not an array"))?,
    ),
    None => None,
  };

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects,
    error_callback,
    for_storage: options.for_storage,
  });
  let mut value_serializer =
    v8::ValueSerializer::new(scope, serialize_deserialize);
  value_serializer.write_header();

  if let Some(transferred_array_buffers) = transferred_array_buffers {
    let state_rc = JsRuntime::state_from(scope);
    let state = state_rc.borrow_mut();
    for index in 0..transferred_array_buffers.length() {
      let i = v8::Number::new(scope, index as f64).into();
      let buf = transferred_array_buffers.get(scope, i).unwrap();
      let buf = v8::Local::<v8::ArrayBuffer>::try_from(buf).map_err(|_| {
        type_error("item in transferredArrayBuffers not an ArrayBuffer")
      })?;
      if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store
      {
        if !buf.is_detachable() {
          return Err(type_error(
            "item in transferredArrayBuffers is not transferable",
          ));
        }

        if buf.was_detached() {
          return Err(custom_error(
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

  let scope = &mut v8::TryCatch::new(scope);
  let ret =
    value_serializer.write_value(scope.get_current_context(), value.v8_value);
  if scope.has_caught() || scope.has_terminated() {
    scope.rethrow();
    // Dummy value, this result will be discarded because an error was thrown.
    Ok(ToJsBuffer::empty())
  } else if let Some(true) = ret {
    let vector = value_serializer.release();
    Ok(vector.into())
  } else {
    Err(type_error("Failed to serialize response"))
  }
}

#[op(v8)]
fn op_deserialize<'a>(
  scope: &mut v8::HandleScope<'a>,
  zero_copy: JsBuffer,
  options: Option<SerializeDeserializeOptions>,
) -> Result<serde_v8::Value<'a>, Error> {
  let options = options.unwrap_or_default();
  let host_objects = match options.host_objects {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value.v8_value)
        .map_err(|_| type_error("hostObjects not an array"))?,
    ),
    None => None,
  };
  let transferred_array_buffers = match options.transferred_array_buffers {
    Some(value) => Some(
      v8::Local::<v8::Array>::try_from(value.v8_value)
        .map_err(|_| type_error("transferredArrayBuffers not an array"))?,
    ),
    None => None,
  };

  let serialize_deserialize = Box::new(SerializeDeserialize {
    host_objects,
    error_callback: None,
    for_storage: options.for_storage,
  });
  let mut value_deserializer =
    v8::ValueDeserializer::new(scope, serialize_deserialize, &zero_copy);
  let parsed_header = value_deserializer
    .read_header(scope.get_current_context())
    .unwrap_or_default();
  if !parsed_header {
    return Err(range_error("could not deserialize value"));
  }

  if let Some(transferred_array_buffers) = transferred_array_buffers {
    let state_rc = JsRuntime::state_from(scope);
    let state = state_rc.borrow_mut();
    if let Some(shared_array_buffer_store) = &state.shared_array_buffer_store {
      for i in 0..transferred_array_buffers.length() {
        let i = v8::Number::new(scope, i as f64).into();
        let id_val = transferred_array_buffers.get(scope, i).unwrap();
        let id = match id_val.number_value(scope) {
          Some(id) => id as u32,
          None => {
            return Err(type_error(
              "item in transferredArrayBuffers not number",
            ))
          }
        };
        if let Some(backing_store) = shared_array_buffer_store.take(id) {
          let array_buffer =
            v8::ArrayBuffer::with_backing_store(scope, &backing_store);
          value_deserializer.transfer_array_buffer(id, array_buffer);
          transferred_array_buffers.set(scope, id_val, array_buffer.into());
        } else {
          return Err(type_error(
            "transferred array buffer not present in shared_array_buffer_store",
          ));
        }
      }
    }
  }

  let value = value_deserializer.read_value(scope.get_current_context());
  match value {
    Some(deserialized) => Ok(deserialized.into()),
    None => Err(range_error("could not deserialize value")),
  }
}

#[derive(Serialize)]
struct PromiseDetails<'s>(u32, Option<serde_v8::Value<'s>>);

#[op(v8)]
fn op_get_promise_details<'a>(
  scope: &mut v8::HandleScope<'a>,
  promise: serde_v8::Value<'a>,
) -> Result<PromiseDetails<'a>, Error> {
  let promise = v8::Local::<v8::Promise>::try_from(promise.v8_value)
    .map_err(|_| type_error("Invalid argument"))?;
  match promise.state() {
    v8::PromiseState::Pending => Ok(PromiseDetails(0, None)),
    v8::PromiseState::Fulfilled => {
      Ok(PromiseDetails(1, Some(promise.result(scope).into())))
    }
    v8::PromiseState::Rejected => {
      Ok(PromiseDetails(2, Some(promise.result(scope).into())))
    }
  }
}

#[op(v8)]
fn op_set_promise_hooks(
  scope: &mut v8::HandleScope,
  init_hook: serde_v8::Value,
  before_hook: serde_v8::Value,
  after_hook: serde_v8::Value,
  resolve_hook: serde_v8::Value,
) -> Result<(), Error> {
  let v8_fns = [init_hook, before_hook, after_hook, resolve_hook]
    .into_iter()
    .enumerate()
    .filter(|(_, hook)| !hook.v8_value.is_undefined())
    .try_fold([None; 4], |mut v8_fns, (i, hook)| {
      let v8_fn = v8::Local::<v8::Function>::try_from(hook.v8_value)
        .map_err(|err| type_error(err.to_string()))?;
      v8_fns[i] = Some(v8_fn);
      Ok::<_, Error>(v8_fns)
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
#[op(v8)]
fn op_get_proxy_details<'a>(
  scope: &mut v8::HandleScope<'a>,
  proxy: serde_v8::Value<'a>,
) -> Option<(serde_v8::Value<'a>, serde_v8::Value<'a>)> {
  let proxy = match v8::Local::<v8::Proxy>::try_from(proxy.v8_value) {
    Ok(proxy) => proxy,
    Err(_) => return None,
  };
  let target = proxy.get_target(scope);
  let handler = proxy.get_handler(scope);
  Some((target.into(), handler.into()))
}

#[op(v8)]
fn op_get_non_index_property_names<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: serde_v8::Value<'a>,
  filter: u32,
) -> Option<serde_v8::Value<'a>> {
  let obj = match v8::Local::<v8::Object>::try_from(obj.v8_value) {
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

  if let Some(names) = maybe_names {
    let names_val: v8::Local<v8::Value> = names.into();
    Some(names_val.into())
  } else {
    None
  }
}

#[op(v8)]
fn op_get_constructor_name<'a>(
  scope: &mut v8::HandleScope<'a>,
  obj: serde_v8::Value<'a>,
) -> Option<String> {
  let obj = match v8::Local::<v8::Object>::try_from(obj.v8_value) {
    Ok(proxy) => proxy,
    Err(_) => return None,
  };

  let name = obj.get_constructor_name().to_rust_string_lossy(scope);
  Some(name)
}

// HeapStats stores values from a isolate.get_heap_statistics() call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryUsage {
  physical_total: usize,
  heap_total: usize,
  heap_used: usize,
  external: usize,
  // TODO: track ArrayBuffers, would require using a custom allocator to track
  // but it's otherwise a subset of external so can be indirectly tracked
  // array_buffers: usize,
}

#[op(v8)]
fn op_memory_usage(scope: &mut v8::HandleScope) -> MemoryUsage {
  let mut s = v8::HeapStatistics::default();
  scope.get_heap_statistics(&mut s);
  MemoryUsage {
    physical_total: s.total_physical_size(),
    heap_total: s.total_heap_size(),
    heap_used: s.used_heap_size(),
    external: s.external_memory(),
  }
}

#[op(v8)]
fn op_set_wasm_streaming_callback(
  scope: &mut v8::HandleScope,
  cb: serde_v8::Value,
) -> Result<(), Error> {
  let cb = to_v8_fn(scope, cb)?;
  let context_state_rc = JsRealm::state_from_scope(scope);
  let mut context_state = context_state_rc.borrow_mut();
  // The callback to pass to the v8 API has to be a unit type, so it can't
  // borrow or move any local variables. Therefore, we're storing the JS
  // callback in a JsRuntimeState slot.
  if context_state.js_wasm_streaming_cb.is_some() {
    return Err(type_error("op_set_wasm_streaming_callback already called"));
  }
  context_state.js_wasm_streaming_cb = Some(Rc::new(cb));

  scope.set_wasm_streaming_callback(|scope, arg, wasm_streaming| {
    let (cb_handle, streaming_rid) = {
      let context_state_rc = JsRealm::state_from_scope(scope);
      let cb_handle = context_state_rc
        .borrow()
        .js_wasm_streaming_cb
        .as_ref()
        .unwrap()
        .clone();
      let state_rc = JsRuntime::state_from(scope);
      let streaming_rid = state_rc
        .borrow()
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

#[allow(clippy::let_and_return)]
#[op(v8)]
fn op_abort_wasm_streaming(
  scope: &mut v8::HandleScope,
  rid: u32,
  error: serde_v8::Value,
) -> Result<(), Error> {
  let wasm_streaming = {
    let state_rc = JsRuntime::state_from(scope);
    let state = state_rc.borrow();
    let wsr = state
      .op_state
      .borrow_mut()
      .resource_table
      .take::<WasmStreamingResource>(rid)?;
    wsr
  };

  // At this point there are no clones of Rc<WasmStreamingResource> on the
  // resource table, and no one should own a reference because we're never
  // cloning them. So we can be sure `wasm_streaming` is the only reference.
  if let Ok(wsr) = std::rc::Rc::try_unwrap(wasm_streaming) {
    // NOTE: v8::WasmStreaming::abort can't be called while `state` is borrowed;
    // see https://github.com/denoland/deno/issues/13917
    wsr.0.into_inner().abort(Some(error.v8_value));
  } else {
    panic!("Couldn't consume WasmStreamingResource.");
  }
  Ok(())
}

#[op(v8)]
fn op_destructure_error(
  scope: &mut v8::HandleScope,
  error: serde_v8::Value,
) -> JsError {
  JsError::from_v8_exception(scope, error.v8_value)
}

/// Effectively throw an uncatchable error. This will terminate runtime
/// execution before any more JS code can run, except in the REPL where it
/// should just output the error to the console.
#[op(v8)]
fn op_dispatch_exception(
  scope: &mut v8::HandleScope,
  exception: serde_v8::Value,
) {
  let state_rc = JsRuntime::state_from(scope);
  let mut state = state_rc.borrow_mut();
  if let Some(inspector) = &state.inspector {
    let inspector = inspector.borrow();
    inspector.exception_thrown(scope, exception.v8_value, false);
    // This indicates that the op is being called from a REPL. Skip termination.
    if inspector.is_dispatching_message() {
      return;
    }
  }
  state.dispatched_exception = Some(v8::Global::new(scope, exception.v8_value));
  scope.terminate_execution();
}

#[op(v8)]
fn op_op_names(scope: &mut v8::HandleScope) -> Vec<String> {
  let state_rc = JsRealm::state_from_scope(scope);
  let state = state_rc.borrow();
  state
    .op_ctxs
    .iter()
    .map(|o| o.decl.name.to_string())
    .collect()
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
  file_name: String,
  line_number: u32,
  column_number: u32,
}

#[op(v8)]
fn op_apply_source_map(
  scope: &mut v8::HandleScope,
  location: Location,
) -> Result<Location, Error> {
  let state_rc = JsRuntime::state_from(scope);
  let (getter, cache) = {
    let state = state_rc.borrow();
    (
      state.source_map_getter.clone(),
      state.source_map_cache.clone(),
    )
  };

  if let Some(source_map_getter) = getter {
    let mut cache = cache.borrow_mut();
    let mut location = location;
    let (f, l, c) = apply_source_map(
      location.file_name,
      location.line_number.into(),
      location.column_number.into(),
      &mut cache,
      &**source_map_getter,
    );
    location.file_name = f;
    location.line_number = l as u32;
    location.column_number = c as u32;
    Ok(location)
  } else {
    Ok(location)
  }
}

/// Set a callback which formats exception messages as stored in
/// `JsError::exception_message`. The callback is passed the error value and
/// should return a string or `null`. If no callback is set or the callback
/// returns `null`, the built-in default formatting will be used.
#[op(v8)]
fn op_set_format_exception_callback<'a>(
  scope: &mut v8::HandleScope<'a>,
  cb: serde_v8::Value<'a>,
) -> Result<Option<serde_v8::Value<'a>>, Error> {
  let cb = to_v8_fn(scope, cb)?;
  let context_state_rc = JsRealm::state_from_scope(scope);
  let old = context_state_rc
    .borrow_mut()
    .js_format_exception_cb
    .replace(Rc::new(cb));
  let old = old.map(|v| v8::Local::new(scope, &*v));
  Ok(old.map(|v| from_v8(scope, v.into()).unwrap()))
}

#[op(v8)]
fn op_event_loop_has_more_work(scope: &mut v8::HandleScope) -> bool {
  JsRuntime::event_loop_pending_state_from_scope(scope).is_pending()
}

#[op(v8)]
fn op_store_pending_promise_rejection<'a>(
  scope: &mut v8::HandleScope<'a>,
  promise: serde_v8::Value<'a>,
  reason: serde_v8::Value<'a>,
) {
  let context_state_rc = JsRealm::state_from_scope(scope);
  let mut context_state = context_state_rc.borrow_mut();
  let promise_value =
    v8::Local::<v8::Promise>::try_from(promise.v8_value).unwrap();
  let promise_global = v8::Global::new(scope, promise_value);
  let error_global = v8::Global::new(scope, reason.v8_value);
  context_state
    .pending_promise_rejections
    .push_back((promise_global, error_global));
}

#[op(v8)]
fn op_remove_pending_promise_rejection<'a>(
  scope: &mut v8::HandleScope<'a>,
  promise: serde_v8::Value<'a>,
) {
  let context_state_rc = JsRealm::state_from_scope(scope);
  let mut context_state = context_state_rc.borrow_mut();
  let promise_value =
    v8::Local::<v8::Promise>::try_from(promise.v8_value).unwrap();
  let promise_global = v8::Global::new(scope, promise_value);
  context_state
    .pending_promise_rejections
    .retain(|(key, _)| key != &promise_global);
}

#[op(v8)]
fn op_has_pending_promise_rejection<'a>(
  scope: &mut v8::HandleScope<'a>,
  promise: serde_v8::Value<'a>,
) -> bool {
  let context_state_rc = JsRealm::state_from_scope(scope);
  let context_state = context_state_rc.borrow();
  let promise_value =
    v8::Local::<v8::Promise>::try_from(promise.v8_value).unwrap();
  let promise_global = v8::Global::new(scope, promise_value);
  context_state
    .pending_promise_rejections
    .iter()
    .any(|(key, _)| key == &promise_global)
}

#[op(v8)]
fn op_arraybuffer_was_detached<'a>(
  _scope: &mut v8::HandleScope<'a>,
  input: serde_v8::Value<'a>,
) -> Result<bool, Error> {
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(input.v8_value)?;
  Ok(ab.was_detached())
}
