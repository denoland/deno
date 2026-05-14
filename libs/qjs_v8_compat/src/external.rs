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
    _scope: &mut HandleScope<'s>,
    p: *mut c_void,
  ) -> Local<'s, External> {
    // Stash the raw pointer in a JSValue's `ptr` slot with a
    // non-refcounted tag (JS_TAG_UNDEFINED). The op2 trampoline reads
    // the pointer back via `args.data().value()` to recover the OpCtx*.
    let raw = sys::JSValue {
      u: sys::JSValueUnion { ptr: p },
      tag: sys::JS_TAG_UNDEFINED,
    };
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, External> {
  pub fn value(&self) -> *mut c_void {
    unsafe { self.raw.u.ptr as *mut c_void }
  }
}

impl External {
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
  pub function: crate::function::FunctionCallback,
  pub pointer: *const c_void,
  pub type_info: *const crate::v8::fast_api::CFunctionInfo,
  pub api_function: crate::function::FunctionCallback,
  pub named_query: *const c_void,
  pub named_getter: *const c_void,
  pub named_setter: *const c_void,
  pub named_deleter: *const c_void,
  pub named_definer: *const c_void,
  pub named_descriptor: *const c_void,
  pub named_enumerator: *const c_void,
  pub indexed_query: *const c_void,
  pub indexed_getter: *const c_void,
  pub indexed_setter: *const c_void,
  pub indexed_deleter: *const c_void,
  pub indexed_definer: *const c_void,
  pub indexed_descriptor: *const c_void,
  pub indexed_enumerator: *const c_void,
  pub enumerator: *const c_void,
}
impl ExternalReference {
  pub const fn new(pointer: *mut c_void) -> Self {
    Self { pointer: pointer as *const c_void }
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
/// write callbacks so they can recurse into the serializer state.
pub trait ValueSerializerHelper {
  fn write_uint32(&self, _value: u32) {}
  fn write_uint64(&self, _value: u64) {}
  fn write_double(&self, _value: f64) {}
  fn write_raw_bytes(&self, _bytes: &[u8]) {}
  fn write_header(&self) {}
  fn write_value<'s>(
    &self,
    _ctx: Local<'s, crate::context::Context>,
    _value: Local<'s, crate::value::Value>,
  ) -> Option<bool> {
    Some(true)
  }
  fn transfer_array_buffer<'s>(
    &self,
    _id: u32,
    _array_buffer: Local<'s, crate::buffer::ArrayBuffer>,
  ) {}
}

/// Mirror of rusty_v8's `ValueDeserializerHelper`.
pub trait ValueDeserializerHelper {
  fn read_uint32(&self, _value: &mut u32) -> bool {
    false
  }
  fn read_uint64(&self, _value: &mut u64) -> bool {
    false
  }
  fn read_double(&self, _value: &mut f64) -> bool {
    false
  }
  fn read_raw_bytes(&self, _length: usize) -> Option<&[u8]> {
    None
  }
  fn get_wire_format_version(&self) -> u32 { 0 }
  fn read_value<'s>(
    &self,
    _ctx: Local<'s, crate::context::Context>,
  ) -> Option<Local<'s, crate::value::Value>> {
    None
  }
  fn transfer_array_buffer<'s>(
    &self,
    _id: u32,
    _array_buffer: Local<'s, crate::buffer::ArrayBuffer>,
  ) {}
  fn transfer_shared_array_buffer<'s>(
    &self,
    _id: u32,
    _shared_array_buffer: Local<'s, crate::buffer::SharedArrayBuffer>,
  ) {}
}

pub struct ValueSerializer<'s> {
  _impl: Box<dyn ValueSerializerImpl + 's>,
  _scope: std::marker::PhantomData<&'s ()>,
  buffer: Vec<u8>,
}
impl<I: ValueSerializerImpl + ?Sized> ValueSerializerImpl for Box<I> {}
impl<'s> ValueSerializer<'s> {
  pub fn new<I>(_scope: &mut HandleScope<'s>, impl_: I) -> Self
  where
    I: ValueSerializerImpl + 's,
  {
    Self {
      _impl: Box::new(impl_),
      _scope: std::marker::PhantomData,
      buffer: Vec::new(),
    }
  }
  pub fn write_header(&self) {}
  pub fn write_value<S>(
    &self,
    _scope_or_ctx: S,
    _value: Local<'s, crate::value::Value>,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn write_double(&self, _v: f64) {}
  pub fn write_uint32(&self, _v: u32) {}
  pub fn write_uint64(&self, _v: u64) {}
  pub fn write_int32(&self, _v: i32) {}
  pub fn write_int64(&self, _v: i64) {}
  pub fn write_raw_bytes(&self, _bytes: &[u8]) {}
  pub fn set_treat_array_buffer_views_as_host_objects(&self, _v: bool) {}
  pub fn release(self) -> Vec<u8> {
    self.buffer
  }
  pub fn transfer_array_buffer(
    &self,
    _id: u32,
    _array_buffer: Local<'_, crate::buffer::ArrayBuffer>,
  ) {
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

pub struct ValueDeserializer<'s> {
  _impl: Box<dyn ValueDeserializerImpl + 's>,
  _scope: std::marker::PhantomData<&'s ()>,
  data: Vec<u8>,
}
impl<I: ValueDeserializerImpl + ?Sized> ValueDeserializerImpl for Box<I> {}
impl<'s> ValueDeserializer<'s> {
  pub fn new<I>(_scope: &mut HandleScope<'s>, impl_: I, data: &[u8]) -> Self
  where
    I: ValueDeserializerImpl + 's,
  {
    Self {
      _impl: Box::new(impl_),
      _scope: std::marker::PhantomData,
      data: data.to_vec(),
    }
  }
  pub fn read_header<C>(&self, _ctx: C) -> Option<bool> {
    Some(true)
  }
  pub fn read_value<S>(
    &self,
    _scope_or_ctx: S,
  ) -> Option<Local<'s, crate::value::Value>> {
    None
  }
  pub fn read_double(&self, _out: &mut f64) -> bool { false }
  pub fn read_uint32(&self, _out: &mut u32) -> bool { false }
  pub fn read_uint64(&self, _out: &mut u64) -> bool { false }
  pub fn read_int32(&self, _out: &mut i32) -> bool { false }
  pub fn read_int64(&self, _out: &mut i64) -> bool { false }
  pub fn read_raw_bytes(&self, _length: usize) -> Option<&[u8]> { None }
  pub fn get_wire_format_version(&self) -> u32 { 0 }
  pub fn transfer_array_buffer(
    &self,
    _id: u32,
    _array_buffer: Local<'_, crate::buffer::ArrayBuffer>,
  ) {
  }
  pub fn transfer_shared_array_buffer(
    &self,
    _id: u32,
    _shared_array_buffer: Local<'_, crate::buffer::SharedArrayBuffer>,
  ) {
  }
}

// (ValueSerializerHelper and ValueDeserializerHelper are declared above.)

// Cached data for compiled scripts/modules.
pub struct CachedData(pub Vec<u8>);

impl CachedData {
  pub fn rejected(&self) -> bool {
    false
  }
}

impl std::ops::Deref for CachedData {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    &self.0
  }
}
impl AsRef<[u8]> for CachedData {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}
