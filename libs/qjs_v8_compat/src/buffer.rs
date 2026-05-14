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

impl BackingStore {
  pub fn data(&self) -> *mut u8 {
    self.data.as_ptr() as *mut u8
  }
  pub fn byte_length(&self) -> usize {
    self.data.len()
  }
  pub fn as_slice(&self) -> &[u8] {
    &self.data
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
  pub fn new_backing_store(
    _scope: &mut crate::isolate::Isolate,
    byte_length: usize,
  ) -> Box<BackingStore> {
    Box::new(BackingStore {
      data: vec![0u8; byte_length].into_boxed_slice(),
    })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_bytes`.
  pub fn new_backing_store_from_bytes(bytes: Box<[u8]>) -> Box<BackingStore> {
    Box::new(BackingStore { data: bytes })
  }
  /// Mirror of `v8::ArrayBuffer::new_backing_store_from_vec`.
  pub fn new_backing_store_from_vec(bytes: Vec<u8>) -> Box<BackingStore> {
    Box::new(BackingStore {
      data: bytes.into_boxed_slice(),
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
}

impl Uint8Array {
  pub fn new<'s>(
    scope: &mut HandleScope<'s>,
    _buffer: Local<'s, ArrayBuffer>,
    _offset: usize,
    _length: usize,
  ) -> Option<Local<'s, Uint8Array>> {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
}

impl<'s> Local<'s, ArrayBufferView> {
  pub fn buffer(&self, _scope: &mut HandleScope<'s>) -> Local<'s, ArrayBuffer> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn byte_offset(&self) -> usize {
    0
  }
  pub fn byte_length(&self) -> usize {
    0
  }
}

/// V8's wasm module wrapper. QuickJS has no WASM execution.
pub struct WasmModuleObject;
pub struct CompiledWasmModule;
pub enum WasmAsyncSuccess {
  Success,
  Fail,
}
pub struct WasmStreaming;
impl WasmStreaming {
  pub fn on_bytes_received(&self, _bytes: &[u8]) {}
  pub fn finish(self) {}
  pub fn abort(self, _err: Option<Local<'_, Value>>) {}
}
