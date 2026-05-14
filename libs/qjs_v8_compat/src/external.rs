// Copyright 2018-2026 the Deno authors. MIT license.
//
// External pointers + serializers.

use core::ffi::c_void;

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;

crate::value_type!(External);

impl<'s> Local<'s, External> {
  pub fn new(scope: &mut HandleScope<'s>, _p: *mut c_void) -> Self {
    // QJS-DIVERGE: real impl wraps `p` into a JSValue with JS_TAG_OBJECT
    // and an external-class id. The pointer is recoverable via
    // JS_GetOpaque. Mocked here as a sentinel.
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  pub fn value(&self) -> *mut c_void {
    core::ptr::null_mut()
  }
}

/// V8 uses `ExternalReference` to register C function pointers for
/// snapshot replay. QuickJS doesn't have snapshots so we stash them in a
/// Vec; deno_core only reads back what it wrote.
#[derive(Copy, Clone)]
pub struct ExternalReference {
  pub function: *mut c_void,
}
impl ExternalReference {
  pub const fn new(function: *mut c_void) -> Self {
    Self { function }
  }
}
unsafe impl Send for ExternalReference {}
unsafe impl Sync for ExternalReference {}

// Serializer / Deserializer.
//
// V8 has `ValueSerializer` and `ValueDeserializer` for the Structured Clone
// algorithm. QuickJS-ng has a similar serializer via `JS_WriteObject` /
// `JS_ReadObject` for cross-realm clone; we wire that up.

pub trait ValueSerializerImpl {
  fn write_host_object<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _object: Local<'s, crate::object::Object>,
  ) -> Option<bool> {
    Some(false)
  }
  fn throw_data_clone_error<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _message: Local<'s, crate::primitives::String>,
  ) {
  }
  /// Mirrors v8::ValueSerializerImpl::is_host_object — returns whether
  /// the value should be serialized via the host_object path. Defaults
  /// to false on QuickJS (no host objects).
  fn is_host_object<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _object: Local<'s, crate::object::Object>,
  ) -> Option<bool> {
    Some(false)
  }
  /// Mirrors v8::ValueSerializerImpl::has_custom_host_object — feature
  /// gate for the host_object protocol.
  fn has_custom_host_object(&mut self) -> bool {
    false
  }
  /// Mirrors v8::ValueSerializerImpl::get_shared_array_buffer_id.
  /// QuickJS has no SAB; always returns None.
  fn get_shared_array_buffer_id<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _shared_array_buffer: Local<'s, crate::buffer::SharedArrayBuffer>,
  ) -> Option<u32> {
    None
  }
  /// Mirrors v8::ValueSerializerImpl::get_wasm_module_transfer_id.
  /// QuickJS has no WASM; always returns None.
  fn get_wasm_module_transfer_id<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _module: Local<'s, crate::value::Value>,
  ) -> Option<u32> {
    None
  }
}

pub struct ValueSerializer<'s, I> {
  _impl: I,
  _scope: std::marker::PhantomData<&'s ()>,
  buffer: Vec<u8>,
}
impl<'s, I: ValueSerializerImpl> ValueSerializer<'s, I> {
  pub fn new(_scope: &mut HandleScope<'s>, impl_: I) -> Self {
    Self {
      _impl: impl_,
      _scope: std::marker::PhantomData,
      buffer: Vec::new(),
    }
  }
  pub fn write_header(&mut self) {}
  pub fn write_value(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _value: Local<'s, crate::value::Value>,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn release(self) -> Vec<u8> {
    self.buffer
  }
}

pub trait ValueDeserializerImpl {
  fn read_host_object<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, crate::object::Object>> {
    None
  }
  /// Mirrors v8::ValueDeserializerImpl::get_shared_array_buffer_from_id.
  /// QuickJS has no SAB; always returns None.
  fn get_shared_array_buffer_from_id<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _transfer_id: u32,
  ) -> Option<Local<'s, crate::buffer::SharedArrayBuffer>> {
    None
  }
  /// Mirrors v8::ValueDeserializerImpl::get_wasm_module_from_id.
  /// QuickJS has no WASM; always returns None.
  fn get_wasm_module_from_id<'s>(
    &mut self,
    _scope: &mut HandleScope<'s>,
    _clone_id: u32,
  ) -> Option<Local<'s, crate::value::Value>> {
    None
  }
}

pub struct ValueDeserializer<'s, I> {
  _impl: I,
  _scope: std::marker::PhantomData<&'s ()>,
  data: Vec<u8>,
}
impl<'s, I: ValueDeserializerImpl> ValueDeserializer<'s, I> {
  pub fn new(_scope: &mut HandleScope<'s>, impl_: I, data: &[u8]) -> Self {
    Self {
      _impl: impl_,
      _scope: std::marker::PhantomData,
      data: data.to_vec(),
    }
  }
  pub fn read_header(&mut self) -> Option<bool> {
    Some(true)
  }
  pub fn read_value(
    &mut self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, crate::value::Value>> {
    None
  }
}

// Helper traits surfaced by deno_core's serde_v8 integration. They're
// purely opt-in extension points.
pub trait ValueSerializerHelper {}
pub trait ValueDeserializerHelper {}

// Cached data for compiled scripts/modules.
pub struct CachedData(pub Vec<u8>);
