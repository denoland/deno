// Copyright 2018-2026 the Deno authors. MIT license.
//
// ArrayBuffer, TypedArray, DataView, BackingStore.

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(
  ArrayBuffer,
  ArrayBufferView,
  SharedArrayBuffer,
  TypedArray,
  Uint8Array,
  Uint32Array,
  Float32Array,
  Float64Array,
  DataView,
);

pub const TYPED_ARRAY_MAX_SIZE_IN_HEAP: usize = 64;

/// QuickJS-side BackingStore.
pub struct BackingStore {
  data: Box<[u8]>,
}

/// Mirror of rusty_v8's `SharedRef<T>` — a wrapper around `Arc<T>`
/// that provides `.len()` and other accessors directly.
pub type SharedRef<T> = std::sync::Arc<T>;

impl BackingStore {
  pub fn data(&self) -> Option<core::ptr::NonNull<u8>> {
    core::ptr::NonNull::new(self.data.as_ptr() as *mut u8)
  }
  pub fn byte_length(&self) -> usize {
    self.data.len()
  }
  pub fn as_slice(&self) -> &[u8] {
    &self.data
  }
  pub fn len(&self) -> usize {
    self.data.len()
  }
  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
  /// Mirror of `BackingStore::is_shared` — whether multiple Arc holders
  /// can read/write concurrently. Always false on QuickJS (no SAB).
  pub fn is_shared(&self) -> bool {
    false
  }
  /// Mirror of `BackingStore::is_resizable_by_user_javascript`.
  pub fn is_resizable_by_user_javascript(&self) -> bool {
    false
  }
  /// Mirror of rusty_v8's `Box<BackingStore>::make_shared` — uses the
  /// `self: Box<Self>` receiver trick. Converts an exclusively-owned
  /// BackingStore into a refcounted Arc.
  pub fn make_shared(self: Box<Self>) -> std::sync::Arc<BackingStore> {
    std::sync::Arc::from(self)
  }
}

impl ArrayBuffer {
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    byte_length: usize,
  ) -> Local<'s, ArrayBuffer> {
    // QJS-DIVERGE: real impl routes through JS_NewArrayBuffer; mock allocates
    // an object placeholder.
    let _ = byte_length;
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  /// Mirror of v8's `ArrayBuffer::with_backing_store`.
  pub fn with_backing_store<'s>(
    scope: &mut HandleScope<'s>,
    _store: &std::sync::Arc<BackingStore>,
  ) -> Local<'s, ArrayBuffer> {
    Self::new(scope, 0)
  }
  pub fn new_backing_store<S>(
    _scope: &mut S,
    byte_length: usize,
  ) -> Box<BackingStore> {
    Box::new(BackingStore {
      data: vec![0u8; byte_length].into_boxed_slice(),
    })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_bytes`.
  /// Generic over POD element type so callers passing Box<[i8]>,
  /// Box<[u16]>, etc. (typed-array bodies) compile.
  pub fn new_backing_store_from_bytes<T: Copy + 'static>(
    bytes: Box<[T]>,
  ) -> Box<BackingStore> {
    let len = bytes.len() * std::mem::size_of::<T>();
    let raw = Box::into_raw(bytes) as *mut u8;
    let v = unsafe { Vec::from_raw_parts(raw, len, len) };
    Box::new(BackingStore {
      data: v.into_boxed_slice(),
    })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_boxed_slice`.
  pub fn new_backing_store_from_boxed_slice(
    bytes: Box<[u8]>,
  ) -> Box<BackingStore> {
    Box::new(BackingStore { data: bytes })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_vec`.
  pub fn new_backing_store_from_vec(bytes: Vec<u8>) -> Box<BackingStore> {
    Box::new(BackingStore {
      data: bytes.into_boxed_slice(),
    })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_ptr` — wraps a
  /// raw pointer + length + custom deleter. We copy into our own
  /// boxed slice (deleter is ignored on QuickJS).
  pub unsafe fn new_backing_store_from_ptr(
    data: *mut core::ffi::c_void,
    byte_length: usize,
    _deleter_callback: extern "C" fn(
      *mut core::ffi::c_void,
      usize,
      *mut core::ffi::c_void,
    ),
    _deleter_data: *mut core::ffi::c_void,
  ) -> Box<BackingStore> {
    let slice =
      unsafe { std::slice::from_raw_parts(data as *const u8, byte_length) };
    Box::new(BackingStore {
      data: slice.to_vec().into_boxed_slice(),
    })
  }
}

impl<'s> Local<'s, ArrayBuffer> {
  pub fn byte_length(&self) -> usize {
    0
  }
  pub fn get_backing_store(&self) -> std::sync::Arc<BackingStore> {
    std::sync::Arc::new(BackingStore { data: Box::new([]) })
  }
  pub fn data(&self) -> Option<std::ptr::NonNull<core::ffi::c_void>> {
    None
  }
  pub fn was_detached(&self) -> bool {
    false
  }
}

impl SharedArrayBuffer {
  pub fn new<'s>(
    _scope: &mut HandleScope<'s>,
    _byte_length: usize,
  ) -> Local<'s, SharedArrayBuffer> {
    // QJS-DIVERGE: SharedArrayBuffer requires threading semantics QuickJS
    // does not provide. Using SAB on the QuickJS backend throws at runtime
    // (deno_core tests that need SAB are gated to V8).
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn with_backing_store<'s>(
    _scope: &mut HandleScope<'s>,
    _backing: &std::sync::Arc<BackingStore>,
  ) -> Local<'s, SharedArrayBuffer> {
    Local::from_raw(sys::jsv_undefined())
  }
}

impl<'s> Local<'s, SharedArrayBuffer> {
  pub fn get_backing_store(&self) -> std::sync::Arc<BackingStore> {
    std::sync::Arc::new(BackingStore { data: Box::new([]) })
  }
}

impl Uint8Array {
  pub fn new<'s, 'b>(
    scope: &mut HandleScope<'s>,
    _buffer: Local<'b, ArrayBuffer>,
    _offset: usize,
    _length: usize,
  ) -> Option<Local<'s, Uint8Array>> {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
}

impl<'s> Local<'s, ArrayBufferView> {
  pub fn buffer(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Option<Local<'s, ArrayBuffer>>
  where
    Self: Sized,
  {
    Some(self.buffer_unwrap(_scope))
  }
  pub fn buffer_unwrap(
    &self,
    _scope: &mut HandleScope<'s>,
  ) -> Local<'s, ArrayBuffer> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn byte_offset(&self) -> usize {
    0
  }
  pub fn byte_length(&self) -> usize {
    0
  }
  pub fn data(&self) -> Option<std::ptr::NonNull<core::ffi::c_void>> {
    None
  }
  pub fn get_backing_store(&self) -> std::sync::Arc<BackingStore> {
    std::sync::Arc::new(BackingStore { data: Box::new([]) })
  }
  pub fn get_contents_raw_parts<S>(
    &self,
    _storage: S,
  ) -> (std::ptr::NonNull<core::ffi::c_void>, usize, usize) {
    (std::ptr::NonNull::dangling(), 0, 0)
  }
}

/// V8's wasm module wrapper. QuickJS has no WASM execution.
pub struct WasmModuleObject;
pub struct CompiledWasmModule;
pub enum WasmAsyncSuccess {
  Success,
  Fail,
}
pub struct WasmStreaming<const FOR_ASYNC_COMPILE: bool = true>;
impl<const FOR_ASYNC_COMPILE: bool> WasmStreaming<FOR_ASYNC_COMPILE> {
  pub fn on_bytes_received(&mut self, _bytes: &[u8]) {}
  pub fn finish(&mut self) {}
  pub fn abort(&mut self, _err: Option<Local<'_, Value>>) {}
  pub fn set_url(&mut self, _url: &str) {}
}

impl WasmModuleObject {
  pub fn compile<'s, S>(
    _scope: &mut S,
    _bytes: &[u8],
  ) -> Option<Local<'s, WasmModuleObject>> {
    None
  }
  pub fn from_compiled_module<'s, S>(
    _scope: &mut S,
    _module: &CompiledWasmModule,
  ) -> Option<Local<'s, WasmModuleObject>> {
    None
  }
}

impl<'s> Local<'s, WasmModuleObject> {
  pub fn get_compiled_module(&self) -> CompiledWasmModule {
    CompiledWasmModule
  }
}
