// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Traced;
use deno_core::FastString;
use deno_core::GarbageCollected;
use deno_core::ToJsBuffer;
use v8::ValueDeserializerHelper;
use v8::ValueSerializerHelper;

#[op2(fast)]
pub fn op_v8_cached_data_version_tag() -> u32 {
  v8::script_compiler::cached_data_version_tag()
}

#[op2(fast)]
pub fn op_v8_get_heap_statistics(
  scope: &mut v8::HandleScope,
  #[buffer] buffer: &mut [f64],
) {
  let mut stats = v8::HeapStatistics::default();
  scope.get_heap_statistics(&mut stats);

  buffer[0] = stats.total_heap_size() as f64;
  buffer[1] = stats.total_heap_size_executable() as f64;
  buffer[2] = stats.total_physical_size() as f64;
  buffer[3] = stats.total_available_size() as f64;
  buffer[4] = stats.used_heap_size() as f64;
  buffer[5] = stats.heap_size_limit() as f64;
  buffer[6] = stats.malloced_memory() as f64;
  buffer[7] = stats.peak_malloced_memory() as f64;
  buffer[8] = stats.does_zap_garbage() as f64;
  buffer[9] = stats.number_of_native_contexts() as f64;
  buffer[10] = stats.number_of_detached_contexts() as f64;
  buffer[11] = stats.total_global_handles_size() as f64;
  buffer[12] = stats.used_global_handles_size() as f64;
  buffer[13] = stats.external_memory() as f64;
}

pub struct Serializer<'a> {
  inner: Option<v8::ValueSerializer<'a>>,
}

impl<'a> Serializer<'a> {
  fn inner(&self) -> &v8::ValueSerializer<'a> {
    self.inner.as_ref().unwrap()
  }
}

pub struct SerializerDelegate {
  obj: v8::Global<v8::Object>,
}

impl<'a> v8::cppgc::GarbageCollected for Serializer<'a> {
  fn trace(&self, _visitor: &v8::cppgc::Visitor) {}
}

impl v8::ValueSerializerImpl for SerializerDelegate {
  fn get_shared_array_buffer_id<'s>(
    &self,
    _scope: &mut v8::HandleScope<'s>,
    _shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
  ) -> Option<u32> {
    None
  }
  fn has_custom_host_object(&self, _isolate: &mut v8::Isolate) -> bool {
    false
  }
  fn throw_data_clone_error<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    message: v8::Local<'s, v8::String>,
  ) {
    let error = v8::Exception::type_error(scope, message);
    scope.throw_exception(error);
  }

  fn write_host_object<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    object: v8::Local<'s, v8::Object>,
    _value_serializer: &dyn ValueSerializerHelper,
  ) -> Option<bool> {
    let obj = v8::Local::new(scope, &self.obj);
    let key = FastString::from_static("_writeHostObject")
      .v8_string(scope)
      .into();
    if let Some(v) = obj.get(scope, key) {
      if let Ok(v) = v.try_cast::<v8::Function>() {
        if v.call(scope, obj.into(), &[object.into()]).is_none() {
          return None;
        }
        return Some(true);
      }
    }

    None
  }

  fn is_host_object<'s>(
    &self,
    _scope: &mut v8::HandleScope<'s>,
    _object: v8::Local<'s, v8::Object>,
  ) -> Option<bool> {
    // should never be called because has_custom_host_object returns false
    None
  }
}

#[op2]
#[cppgc]
pub fn op_v8_new_serializer(
  scope: &mut v8::HandleScope,
  obj: v8::Local<v8::Object>,
) -> Serializer<'static> {
  let obj = v8::Global::new(scope, obj);
  let inner =
    v8::ValueSerializer::new(scope, Box::new(SerializerDelegate { obj }));
  Serializer { inner: Some(inner) }
}

#[op2(fast)]
pub fn op_v8_set_treat_array_buffer_views_as_host_objects(
  #[cppgc] ser: &Serializer,
  value: bool,
) {
  ser
    .inner()
    .set_treat_array_buffer_views_as_host_objects(value);
}

#[op2]
#[serde]
pub fn op_v8_release_buffer(#[cppgc] ser: &Serializer) -> ToJsBuffer {
  ser.inner().release().into()
}

#[op2(fast)]
pub fn op_v8_transfer_array_buffer(
  #[cppgc] ser: &Serializer,
  #[smi] id: u32,
  array_buffer: v8::Local<v8::ArrayBuffer>,
) {
  ser.inner().transfer_array_buffer(id, array_buffer);
}

#[op2(fast)]
pub fn op_v8_write_double(#[cppgc] ser: &Serializer, double: f64) {
  ser.inner().write_double(double);
}

#[op2(fast)]
pub fn op_v8_write_header(#[cppgc] ser: &Serializer) {
  ser.inner().write_header();
}

#[op2]
pub fn op_v8_write_raw_bytes(
  #[cppgc] ser: &Serializer,
  #[anybuffer] source: &[u8],
) {
  ser.inner().write_raw_bytes(source);
}

#[op2(fast)]
pub fn op_v8_write_uint32(#[cppgc] ser: &Serializer, num: u32) {
  ser.inner().write_uint32(num);
}

#[op2(fast)]
pub fn op_v8_write_uint64(#[cppgc] ser: &Serializer, hi: u32, lo: u32) {
  let num = ((hi as u64) << 32) | (lo as u64);
  ser.inner().write_uint64(num);
}

#[op2(nofast, reentrant)]
pub fn op_v8_write_value(
  scope: &mut v8::HandleScope,
  #[cppgc] ser: &Serializer,
  value: v8::Local<v8::Value>,
) {
  let context = scope.get_current_context();
  ser.inner().write_value(context, value);
}

pub struct Deserializer<'a> {
  buf_ptr: *mut u8,
  inner: Option<v8::ValueDeserializer<'a>>,
}

impl<'a> deno_core::GarbageCollected for Deserializer<'a> {}

impl<'a> Deserializer<'a> {
  unsafe fn inner(&self) -> &v8::ValueDeserializer<'a> {
    self.inner.as_ref().unwrap()
  }
}

pub struct DeserializerDelegate {
  obj: v8::TracedReference<v8::Object>,
}

impl GarbageCollected for DeserializerDelegate {
  fn trace(&self, visitor: &v8::cppgc::Visitor) {
    self.obj.trace(visitor);
  }
}

impl v8::ValueDeserializerImpl for DeserializerDelegate {
  fn read_host_object<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    _value_deserializer: &dyn v8::ValueDeserializerHelper,
  ) -> Option<v8::Local<'s, v8::Object>> {
    let obj = self.obj.get(scope).unwrap();
    let key = FastString::from_static("_readHostObject")
      .v8_string(scope)
      .into();
    let scope = &mut v8::AllowJavascriptExecutionScope::new(scope);
    if let Some(v) = obj.get(scope, key) {
      if let Ok(v) = v.try_cast::<v8::Function>() {
        let result = v.call(scope, obj.into(), &[])?;
        match result.try_cast() {
          Ok(res) => return Some(res),
          Err(e) => {
            eprintln!("bad return value: {e}");
            return None;
          }
        }
      }
    }
    None
  }

  fn get_shared_array_buffer_from_id<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    _transfer_id: u32,
  ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
    let msg = v8::String::new(
      scope,
      "Deno deserializer: get_shared_array_buffer_from_id not implemented",
    )
    .unwrap();
    let exc = v8::Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }

  fn get_wasm_module_from_id<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    _clone_id: u32,
  ) -> Option<v8::Local<'s, v8::WasmModuleObject>> {
    let msg = v8::String::new(
      scope,
      "Deno deserializer: get_wasm_module_from_id not implemented",
    )
    .unwrap();
    let exc = v8::Exception::error(scope, msg);
    scope.throw_exception(exc);
    None
  }
}

#[op2]
#[cppgc]
pub fn op_v8_new_deserializer(
  scope: &mut v8::HandleScope,
  obj: v8::Local<v8::Object>,
  buffer: v8::Local<v8::ArrayBufferView>,
) -> Deserializer<'static> {
  let offset = buffer.byte_offset();
  let len = buffer.byte_length();
  let buffer = buffer.buffer(scope).unwrap();
  let data =
    unsafe { buffer.data().unwrap().as_ptr().cast::<u8>().add(offset) };
  let obj = v8::TracedReference::new(scope, obj);
  let inner = v8::ValueDeserializer::new(
    scope,
    Box::new(DeserializerDelegate { obj }),
    unsafe { std::slice::from_raw_parts(data.cast_const().cast(), len) },
  );
  Deserializer {
    inner: Some(inner),
    buf_ptr: data.cast(),
  }
}

#[op2(fast)]
pub fn op_v8_transfer_array_buffer_de(
  #[cppgc] deser: &Deserializer,
  #[smi] id: u32,
  array_buffer: v8::Local<v8::ArrayBuffer>,
) {
  unsafe {
    deser.inner().transfer_array_buffer(id, array_buffer);
  }
}

#[op2(fast)]
pub fn op_v8_read_double(#[cppgc] deser: &Deserializer) -> f64 {
  let mut double = 0f64;
  unsafe {
    deser.inner().read_double(&mut double);
  }
  double
}

#[op2(nofast)]
pub fn op_v8_read_header(
  scope: &mut v8::HandleScope,
  #[cppgc] deser: &Deserializer,
) -> bool {
  let context = scope.get_current_context();
  let res = unsafe { deser.inner().read_header(context) };
  res.unwrap_or_default()
}

#[op2(fast)]
#[number]
pub fn op_v8_read_raw_bytes(
  #[cppgc] deser: &Deserializer,
  #[number] length: usize,
) -> usize {
  unsafe {
    if let Some(buf) = deser.inner().read_raw_bytes(length) {
      let ptr = buf.as_ptr();
      let offset = (ptr as usize) - (deser.buf_ptr as usize);
      offset
    } else {
      0
    }
  }
}

#[op2(fast)]
pub fn op_v8_read_uint32(#[cppgc] deser: &Deserializer) -> u32 {
  let mut value = 0;
  unsafe {
    deser.inner().read_uint32(&mut value);
  }
  value
}

#[op2]
#[serde]
pub fn op_v8_read_uint64(#[cppgc] deser: &Deserializer) -> (u32, u32) {
  let mut val = 0;
  unsafe {
    deser.inner().read_uint64(&mut val);
  }
  ((val >> 32) as u32, val as u32)
}

#[op2(fast)]
pub fn op_v8_get_wire_format_version(#[cppgc] deser: &Deserializer) -> u32 {
  unsafe { deser.inner().get_wire_format_version() }
}

#[op2(reentrant)]
pub fn op_v8_read_value<'s>(
  scope: &mut v8::HandleScope<'s>,
  #[cppgc] deser: &Deserializer,
) -> v8::Local<'s, v8::Value> {
  let context = scope.get_current_context();
  let val = unsafe { deser.inner().read_value(context) };
  val.unwrap_or_else(|| v8::null(scope).into())
}
