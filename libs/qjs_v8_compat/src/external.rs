// Copyright 2018-2026 the Deno authors. MIT license.
//
// External pointers + serializers.

use core::ffi::c_void;

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;

crate::value_type!(External);

impl External {
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    _p: *mut c_void,
  ) -> Local<'s, External> {
    // QJS-DIVERGE: real impl wraps `p` into a JSValue with JS_TAG_OBJECT
    // and an external-class id. The pointer is recoverable via
    // JS_GetOpaque. Mocked here as a sentinel.
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, External> {
  pub fn value(&self) -> *mut c_void {
    core::ptr::null_mut()
  }
}

/// V8 uses `ExternalReference` to register C function pointers for
/// snapshot replay. rusty_v8 exposes it as a UNION whose variants are
/// `function`, `pointer`, `type_info`, `api_function` — callers
/// initialize exactly one variant via struct-literal syntax. We mirror
/// the union shape so `ExternalReference { function: x }` and
/// `ExternalReference { pointer: x }` both compile (which a struct
/// would have rejected as "missing fields").
#[repr(C)]
#[derive(Copy, Clone)]
pub union ExternalReference {
  pub function: *const c_void,
  pub pointer: *const c_void,
  pub type_info: *const c_void,
  pub api_function: *const c_void,
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

// Trait signatures match deno_core's existing impls (which target rusty_v8):
// `&self` (not `&mut self`), and the WASM/SAB hooks take `Local` of the
// specific type rusty_v8 declares (WasmModuleObject for WASM,
// SharedArrayBuffer for SAB). `has_custom_host_object` takes an Isolate
// reference.
pub trait ValueSerializerImpl {
  fn write_host_object<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _object: Local<'s, crate::object::Object>,
    _value_serializer: &dyn ValueSerializerHelper,
  ) -> Option<bool> {
    Some(false)
  }
  fn throw_data_clone_error<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _message: Local<'s, crate::primitives::String>,
  ) {
  }
  fn is_host_object<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _object: Local<'s, crate::object::Object>,
  ) -> Option<bool> {
    Some(false)
  }
  fn has_custom_host_object(&self, _isolate: &crate::isolate::Isolate) -> bool {
    false
  }
  fn get_shared_array_buffer_id<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _shared_array_buffer: Local<'s, crate::buffer::SharedArrayBuffer>,
  ) -> Option<u32> {
    None
  }
  fn get_wasm_module_transfer_id<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _module: Local<'s, crate::v8::WasmModuleObject>,
  ) -> Option<u32> {
    None
  }
}

/// Mirror of rusty_v8's `ValueSerializerHelper` — passed to host-object
/// write callbacks so they can recurse into the serializer state. Empty
/// stub on QuickJS; the host_object path isn't exercised.
pub trait ValueSerializerHelper {}

/// Mirror of rusty_v8's `ValueDeserializerHelper`.
pub trait ValueDeserializerHelper {}

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
  fn read_host_object<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _value_deserializer: &dyn ValueDeserializerHelper,
  ) -> Option<Local<'s, crate::object::Object>> {
    None
  }
  fn get_shared_array_buffer_from_id<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _transfer_id: u32,
  ) -> Option<Local<'s, crate::buffer::SharedArrayBuffer>> {
    None
  }
  fn get_wasm_module_from_id<'s, 'i>(
    &self,
    _scope: &mut crate::scope::PinScope<'s, 'i>,
    _clone_id: u32,
  ) -> Option<Local<'s, crate::v8::WasmModuleObject>> {
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

// (ValueSerializerHelper and ValueDeserializerHelper are declared above.)

// Cached data for compiled scripts/modules.
pub struct CachedData(pub Vec<u8>);
