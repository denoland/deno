// Copyright 2018-2026 the Deno authors. MIT license.

// Ported from Node.js:
// - src/stream_base.h
// - src/stream_base.cc
// - src/stream_base-inl.h
// - src/stream_wrap.h
// - src/stream_wrap.cc

#![allow(non_snake_case)]

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::ffi::c_char;
use std::ptr::NonNull;

use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::uv_buf_t;
use deno_core::uv_compat::uv_shutdown_t;
use deno_core::uv_compat::uv_stream_t;
use deno_core::uv_compat::uv_write_t;
use deno_core::v8;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::tty_wrap::OwnedPtr;

// ---------------------------------------------------------------------------
// StreamBase state fields — mirrors Node's StreamBaseStateFields enum.
// These index into a shared Uint8Array visible to JS.
// ---------------------------------------------------------------------------

/// Per-environment stream_base_state array, stored in OpState.
/// This is an Int32Array shared between JS and Rust (mirrors Node's AliasedInt32Array).
pub struct StreamBaseState {
  pub array: v8::Global<v8::Int32Array>,
}

#[op2(fast)]
pub fn op_stream_base_register_state(
  state: &mut OpState,
  array: v8::Local<v8::Int32Array>,
  scope: &mut v8::PinScope,
) {
  state.put(StreamBaseState {
    array: v8::Global::new(scope, array),
  });
}

#[repr(usize)]
enum StreamBaseStateFields {
  ReadBytesOrError = 0,
  ArrayBufferOffset = 1,
  BytesWritten = 2,
  LastWriteWasAsync = 3,
  // NumStreamBaseStateFields = 4,
}

// ---------------------------------------------------------------------------
// WriteWrap — cppgc object that wraps a uv_write_t request.
// ---------------------------------------------------------------------------

#[derive(CppgcBase, CppgcInherits)]
#[cppgc_inherits_from(AsyncWrap)]
#[repr(C)]
pub struct WriteWrap {
  base: AsyncWrap,
  req: OwnedPtr<uv_write_t>,
}

unsafe impl GarbageCollected for WriteWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"WriteWrap"
  }

  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
}

impl WriteWrap {
  pub fn new(op_state: &mut OpState) -> Self {
    Self {
      base: AsyncWrap::create(op_state, ProviderType::WriteWrap as i32),
      req: OwnedPtr::from_box(Box::new(uv_compat::new_write())),
    }
  }

  pub fn req_ptr(&mut self) -> *mut uv_write_t {
    self.req.as_mut_ptr()
  }
}

// ---------------------------------------------------------------------------
// ShutdownWrap — cppgc object that wraps a uv_shutdown_t request.
// ---------------------------------------------------------------------------

#[derive(CppgcBase, CppgcInherits)]
#[cppgc_inherits_from(AsyncWrap)]
#[repr(C)]
pub struct ShutdownWrap {
  base: AsyncWrap,
  req: OwnedPtr<uv_shutdown_t>,
}

unsafe impl GarbageCollected for ShutdownWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ShutdownWrap"
  }

  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
}

impl ShutdownWrap {
  pub fn new(op_state: &mut OpState) -> Self {
    Self {
      base: AsyncWrap::create(op_state, ProviderType::ShutdownWrap as i32),
      req: OwnedPtr::from_box(Box::new(uv_compat::new_shutdown())),
    }
  }

  pub fn req_ptr(&mut self) -> *mut uv_shutdown_t {
    self.req.as_mut_ptr()
  }
}

// ---------------------------------------------------------------------------
// LibUvStreamWrap — the core stream handle, mirrors Node's LibuvStreamWrap.
//
// Inherits: AsyncWrap -> HandleWrap -> LibUvStreamWrap
// ---------------------------------------------------------------------------

#[derive(CppgcBase, CppgcInherits)]
#[repr(C)]
#[cppgc_inherits_from(HandleWrap)]
pub struct LibUvStreamWrap {
  base: HandleWrap,
  fd: i32,
  stream: *const uv_stream_t,
  bytes_read: Cell<u64>,
  bytes_written: Cell<u64>,
  /// The JS object wrapping this cppgc object. Set by JS via set_handle()
  /// after construction. Needed so completion callbacks can pass the handle
  /// to oncomplete(status, handle, error).
  js_handle: UnsafeCell<Option<v8::Global<v8::Object>>>,
}

impl LibUvStreamWrap {
  pub fn new(base: HandleWrap, fd: i32, stream: *const uv_stream_t) -> Self {
    Self {
      base,
      fd,
      stream,
      bytes_read: Cell::new(0),
      bytes_written: Cell::new(0),
      js_handle: UnsafeCell::new(None),
    }
  }

  #[inline]
  pub fn stream_ptr(&self) -> *mut uv_stream_t {
    self.stream as *mut uv_stream_t
  }

  fn js_handle_global(&self) -> Option<v8::Global<v8::Object>> {
    unsafe { (*self.js_handle.get()).clone() }
  }
}

unsafe impl GarbageCollected for LibUvStreamWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"LibUvStreamWrap"
  }

  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

// ---------------------------------------------------------------------------
// Static alloc/read callbacks for uv_read_start.
//
// These are `extern "C"` callbacks that libuv (our uv_compat layer) invokes.
// They recover the JS object via `handle.data` (a pointer to the cppgc
// traced Member<LibUvStreamWrap>) and then call into JS.
//
// In Node, these live as LibuvStreamWrap::OnUvAlloc / LibuvStreamWrap::OnUvRead.
// ---------------------------------------------------------------------------

/// Alloc callback for uv_read_start. Allocates a buffer via
/// `ArrayBuffer::new_backing_store_uninit`.
///
/// # Safety
/// `handle` must be a valid uv_handle_t whose `data` field points to
/// CallbackData that was set up by readStart.
unsafe extern "C" fn on_uv_alloc(
  _handle: *mut uv_compat::uv_handle_t,
  suggested_size: usize,
  buf: *mut uv_buf_t,
) {
  // Allocate raw memory for the read buffer.
  let layout = std::alloc::Layout::from_size_align(suggested_size, 1).unwrap();
  let ptr = unsafe { std::alloc::alloc(layout) };
  if ptr.is_null() {
    unsafe {
      (*buf).base = std::ptr::null_mut();
      (*buf).len = 0;
    }
    return;
  }
  unsafe {
    (*buf).base = ptr as *mut c_char;
    (*buf).len = suggested_size;
  }
}

/// Read callback for uv_read_start. Called when data arrives or an error
/// occurs.
///
/// This callback:
/// 1. Wraps the read data in an ArrayBuffer
/// 2. Stores nread in stream_base_state
/// 3. Calls the JS `onread` function stored on the handle
///
/// # Safety
/// `stream` must be a valid uv_stream_t whose `data` field points to
/// a CallbackData set up by readStart. `buf` must be the buffer from
/// on_uv_alloc. `stream.loop_.data` must be a raw `Global<Context>`
/// pointer set by `register_uv_loop`.
unsafe extern "C" fn on_uv_read(
  stream: *mut uv_stream_t,
  nread: isize,
  buf: *const uv_buf_t,
) {
  let cb_data_ptr = unsafe { (*stream).data as *mut CallbackData };
  if cb_data_ptr.is_null() {
    free_uv_buf(buf);
    return;
  }
  let cb_data = unsafe { &mut *cb_data_ptr };

  // Recover isolate + context from the uv_loop
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(cb_data.isolate) };
  v8::scope!(let handle_scope, &mut isolate);
  // SAFETY: loop_.data holds a raw Global<Context> set by register_uv_loop.
  // We reconstruct it, clone for use, then leak it back so it stays alive.
  let loop_ptr = unsafe { (*stream).loop_ };
  let context = unsafe {
    let raw = NonNull::new_unchecked((*loop_ptr).data as *mut v8::Context);
    let global = v8::Global::from_raw(handle_scope, raw);
    let cloned = global.clone();
    // Leak the original back — it's owned by the runtime
    global.into_raw();
    cloned
  };
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  if nread <= 0 {
    free_uv_buf(buf);
    if nread < 0 {
      // Error/EOF path
      let state_array = v8::Local::new(scope, &cb_data.stream_base_state);
      state_array.set_index(
        scope,
        StreamBaseStateFields::ReadBytesOrError as u32,
        v8::Integer::new(scope, nread as i32).into(),
      );
      state_array.set_index(
        scope,
        StreamBaseStateFields::ArrayBufferOffset as u32,
        v8::Integer::new(scope, 0).into(),
      );

      let onread = v8::Local::new(scope, &cb_data.onread);
      let recv = v8::Local::new(scope, &cb_data.handle);
      let undef = v8::undefined(scope);
      onread.call(scope, recv.into(), &[undef.into()]);
    }
    return;
  }

  // Update bytes_read counter (mirrors Node's EmitRead in stream_base-inl.h)
  unsafe {
    let counter = &*cb_data.bytes_read;
    counter.set(counter.get() + nread as u64);
  }

  // Successful read: wrap data in ArrayBuffer
  let nread_usize = nread as usize;
  let buf_ref = unsafe { &*buf };

  // Create a backing store from the allocated memory.
  // The deleter will free the alloc when the ArrayBuffer is GC'd.
  let backing_store = unsafe {
    v8::ArrayBuffer::new_backing_store_from_ptr(
      buf_ref.base as *mut std::ffi::c_void,
      nread_usize,
      backing_store_deleter,
      buf_ref.len as *mut std::ffi::c_void,
    )
  };

  let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store.into());

  // Update stream_base_state
  let state_array = v8::Local::new(scope, &cb_data.stream_base_state);
  state_array.set_index(
    scope,
    StreamBaseStateFields::ReadBytesOrError as u32,
    v8::Integer::new(scope, nread as i32).into(),
  );
  state_array.set_index(
    scope,
    StreamBaseStateFields::ArrayBufferOffset as u32,
    v8::Integer::new(scope, 0).into(),
  );

  // Call onread(arrayBuffer) with handle as `this`
  let onread = v8::Local::new(scope, &cb_data.onread);
  let recv = v8::Local::new(scope, &cb_data.handle);
  onread.call(scope, recv.into(), &[ab.into()]);
}

/// Free a buffer allocated by on_uv_alloc.
fn free_uv_buf(buf: *const uv_buf_t) {
  unsafe {
    if !(*buf).base.is_null() && (*buf).len > 0 {
      let layout = std::alloc::Layout::from_size_align((*buf).len, 1).unwrap();
      std::alloc::dealloc((*buf).base as *mut u8, layout);
    }
  }
}

/// Backing store deleter that frees memory allocated by on_uv_alloc.
///
/// # Safety
/// `data` must be a pointer allocated by `std::alloc::alloc` with size
/// `deleter_data` (cast from usize) and alignment 1.
unsafe extern "C" fn backing_store_deleter(
  data: *mut std::ffi::c_void,
  len: usize,
  deleter_data: *mut std::ffi::c_void,
) {
  let _ = deleter_data;
  if !data.is_null() && len > 0 {
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe { std::alloc::dealloc(data as *mut u8, layout) };
  }
}

/// Data stored on uv_stream_t.data to bridge between C callbacks and JS.
struct CallbackData {
  isolate: v8::UnsafeRawIsolatePtr,
  onread: v8::Global<v8::Function>,
  stream_base_state: v8::Global<v8::Int32Array>,
  /// The JS handle object — used as `this` when calling onread
  handle: v8::Global<v8::Object>,
  /// Pointer to LibUvStreamWrap.bytes_read for updating from C callback
  bytes_read: *const Cell<u64>,
}

// ---------------------------------------------------------------------------
// Write completion callback for uv_write.
//
// Called by the uv_compat layer when a write completes. Fires the JS
// `oncomplete` callback on the WriteWrap request object.
// ---------------------------------------------------------------------------

/// # Safety
/// `req` must be a valid uv_write_t whose `data` field points to a
/// WriteCallbackData. `req.handle.loop_.data` must be a raw
/// `Global<Context>` pointer.
unsafe extern "C" fn after_uv_write(req: *mut uv_write_t, status: i32) {
  let cb_data_ptr = unsafe { (*req).data as *mut WriteCallbackData };
  if cb_data_ptr.is_null() {
    return;
  }
  // Take ownership back so it gets dropped after this callback
  let cb_data = unsafe { Box::from_raw(cb_data_ptr) };
  unsafe { (*req).data = std::ptr::null_mut() };

  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(cb_data.isolate) };
  v8::scope!(let handle_scope, &mut isolate);
  let loop_ptr = unsafe { (*(*req).handle).loop_ };
  let context = unsafe {
    let raw = NonNull::new_unchecked((*loop_ptr).data as *mut v8::Context);
    let global = v8::Global::from_raw(handle_scope, raw);
    let cloned = global.clone();
    global.into_raw();
    cloned
  };
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  // Update stream_base_state[kBytesWritten]
  let state = v8::Local::new(scope, &cb_data.stream_base_state);
  state.set_index(
    scope,
    StreamBaseStateFields::BytesWritten as u32,
    v8::Number::new(scope, cb_data.bytes as f64).into(),
  );

  // Call req_wrap_obj.oncomplete(status, handle, error)
  // Matches Node's ReportWritesToJSStreamListener::OnStreamAfterReqFinished
  let req_obj = v8::Local::new(scope, &cb_data.req_wrap_obj);
  let handle = v8::Local::new(scope, &cb_data.stream_handle);
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into()) {
    if let Ok(oncomplete) = oncomplete.try_into() {
      let oncomplete: v8::Local<v8::Function> = oncomplete;
      let status_val = v8::Integer::new(scope, status);
      let undef = v8::undefined(scope);
      oncomplete.call(
        scope,
        req_obj.into(),
        &[status_val.into(), handle.into(), undef.into()],
      );
    }
  }
}

struct WriteCallbackData {
  isolate: v8::UnsafeRawIsolatePtr,
  req_wrap_obj: v8::Global<v8::Object>,
  stream_handle: v8::Global<v8::Object>,
  stream_base_state: v8::Global<v8::Int32Array>,
  bytes: usize,
}

// ---------------------------------------------------------------------------
// Shutdown completion callback for uv_shutdown.
// ---------------------------------------------------------------------------

/// # Safety
/// `req` must be a valid uv_shutdown_t whose `data` field points to a
/// ShutdownCallbackData. `req.handle.loop_.data` must be a raw
/// `Global<Context>` pointer.
unsafe extern "C" fn after_uv_shutdown(req: *mut uv_shutdown_t, status: i32) {
  let cb_data_ptr = unsafe { (*req).data as *mut ShutdownCallbackData };
  if cb_data_ptr.is_null() {
    return;
  }
  let cb_data = unsafe { Box::from_raw(cb_data_ptr) };
  unsafe { (*req).data = std::ptr::null_mut() };

  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(cb_data.isolate) };
  v8::scope!(let handle_scope, &mut isolate);
  let loop_ptr = unsafe { (*(*req).handle).loop_ };
  let context = unsafe {
    let raw = NonNull::new_unchecked((*loop_ptr).data as *mut v8::Context);
    let global = v8::Global::from_raw(handle_scope, raw);
    let cloned = global.clone();
    global.into_raw();
    cloned
  };
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  // Call req_wrap_obj.oncomplete(status, handle, error)
  let req_obj = v8::Local::new(scope, &cb_data.req_wrap_obj);
  let handle = v8::Local::new(scope, &cb_data.stream_handle);
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into()) {
    if let Ok(oncomplete) = oncomplete.try_into() {
      let oncomplete: v8::Local<v8::Function> = oncomplete;
      let status_val = v8::Integer::new(scope, status);
      let undef = v8::undefined(scope);
      oncomplete.call(
        scope,
        req_obj.into(),
        &[status_val.into(), handle.into(), undef.into()],
      );
    }
  }
}

struct ShutdownCallbackData {
  isolate: v8::UnsafeRawIsolatePtr,
  req_wrap_obj: v8::Global<v8::Object>,
  stream_handle: v8::Global<v8::Object>,
}

// ---------------------------------------------------------------------------
// Op methods on LibUvStreamWrap — exposed to JS as prototype methods.
//
// These mirror Node's StreamBase JS methods:
//   readStart, readStop, shutdown, writeBuffer, writev,
//   writeUtf8String, writeAsciiString, writeLatin1String, writeUcs2String
// ---------------------------------------------------------------------------

enum StringEncoding {
  Utf8,
  Ascii,
  Latin1,
  Ucs2,
}

fn encode_string_to_vec(
  scope: &mut v8::PinScope,
  string: v8::Local<v8::String>,
  encoding: StringEncoding,
  out: &mut Vec<u8>,
) {
  match encoding {
    StringEncoding::Utf8 => {
      let len = string.utf8_length(scope);
      let start = out.len();
      out.reserve(len);
      let written = string.write_utf8_uninit_v2(
        scope,
        &mut out.spare_capacity_mut()[..len],
        v8::WriteFlags::kReplaceInvalidUtf8,
        None,
      );
      unsafe { out.set_len(start + written) };
    }
    StringEncoding::Latin1 | StringEncoding::Ascii => {
      let len = string.length();
      let start = out.len();
      out.reserve(len);
      string.write_one_byte_uninit_v2(
        scope,
        0,
        &mut out.spare_capacity_mut()[..len],
        v8::WriteFlags::empty(),
      );
      unsafe { out.set_len(start + len) };
    }
    StringEncoding::Ucs2 => {
      let len = string.length();
      let mut buf = vec![0u16; len];
      string.write_v2(scope, 0, &mut buf, v8::WriteFlags::empty());
      out.reserve(len * 2);
      for &ch in &buf {
        out.extend_from_slice(&ch.to_le_bytes());
      }
    }
  }
}

#[op2(base)]
impl LibUvStreamWrap {
  /// Called by JS immediately after construction to store the JS object
  /// reference: `stream.setHandle(stream)`
  #[fast]
  pub fn set_handle(
    &self,
    handle: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) {
    unsafe {
      *self.js_handle.get() = Some(v8::Global::new(scope, handle));
    }
  }

  #[fast]
  pub fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let this = v8::Local::new(scope, &this);

    // Get onread callback from JS object property (set by JS layer)
    let onread_key =
      v8::String::new_external_onebyte_static(scope, b"onread").unwrap();
    let Some(onread_val) = this.get(scope, onread_key.into()) else {
      return UV_EBADF;
    };
    let Ok(onread) = v8::Local::<v8::Function>::try_from(onread_val) else {
      return UV_EBADF;
    };

    // Get stream_base_state from OpState (registered by JS via
    // op_stream_base_register_state)
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);

    let cb_data = Box::new(CallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      onread: v8::Global::new(scope, onread),
      stream_base_state: v8::Global::new(scope, state_array),
      handle: v8::Global::new(scope, this),
      bytes_read: &self.bytes_read as *const Cell<u64>,
    });
    unsafe {
      (*stream).data = Box::into_raw(cb_data) as *mut std::ffi::c_void;
    }

    unsafe {
      uv_compat::uv_read_start(stream, Some(on_uv_alloc), Some(on_uv_read))
    }
  }

  #[fast]
  pub fn read_stop(&self) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    unsafe {
      let data = (*stream).data as *mut CallbackData;
      if !data.is_null() {
        drop(Box::from_raw(data));
        (*stream).data = std::ptr::null_mut();
      }
    }

    unsafe { uv_compat::uv_read_stop(stream) }
  }

  #[fast]
  #[rename("setBlocking")]
  pub fn set_blocking(&self, enable: bool) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }
    unsafe { uv_compat::uv_stream_set_blocking(stream, enable as i32) }
  }

  #[fast]
  pub fn shutdown(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let stream_handle = self
      .js_handle_global()
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_shutdown());
    let cb_data = Box::new(ShutdownCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
      stream_handle,
    });
    req.data = Box::into_raw(cb_data) as *mut std::ffi::c_void;
    let req_ptr = Box::into_raw(req);

    let err = unsafe {
      uv_compat::uv_shutdown(req_ptr, stream, Some(after_uv_shutdown))
    };

    if err != 0 {
      unsafe {
        let data = (*req_ptr).data as *mut ShutdownCallbackData;
        if !data.is_null() {
          drop(Box::from_raw(data));
        }
        drop(Box::from_raw(req_ptr));
      }
    }

    err
  }

  #[fast]
  pub fn write_buffer(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    buffer: v8::Local<v8::Uint8Array>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);

    let byte_length = buffer.byte_length();
    let byte_offset = buffer.byte_offset();

    // Track bytes_written (mirrors Node's Write() in stream_base.cc)
    self
      .bytes_written
      .set(self.bytes_written.get() + byte_length as u64);

    let ab = buffer.buffer(scope).unwrap();
    let data = ab.data().unwrap().as_ptr() as *const u8;
    let data = unsafe { data.add(byte_offset) };

    let try_result = unsafe {
      uv_compat::uv_try_write(
        stream,
        std::slice::from_raw_parts(data, byte_length),
      )
    };

    if try_result >= 0 && try_result as usize == byte_length {
      state_array.set_index(
        scope,
        StreamBaseStateFields::BytesWritten as u32,
        v8::Number::new(scope, byte_length as f64).into(),
      );
      state_array.set_index(
        scope,
        StreamBaseStateFields::LastWriteWasAsync as u32,
        v8::Integer::new(scope, 0).into(),
      );
      return 0;
    }

    let (write_data, write_len) = if try_result > 0 {
      let written = try_result as usize;
      (unsafe { data.add(written) }, byte_length - written)
    } else {
      (data, byte_length)
    };

    let buf = uv_buf_t {
      base: write_data as *mut c_char,
      len: write_len,
    };

    let stream_handle = self
      .js_handle_global()
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_write());
    let cb_data = Box::new(WriteCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
      stream_handle,
      stream_base_state: v8::Global::new(scope, state_array),
      bytes: byte_length,
    });
    req.data = Box::into_raw(cb_data) as *mut std::ffi::c_void;
    let req_ptr = Box::into_raw(req);

    let err = unsafe {
      uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(after_uv_write))
    };

    if err != 0 {
      unsafe {
        let data = (*req_ptr).data as *mut WriteCallbackData;
        if !data.is_null() {
          drop(Box::from_raw(data));
        }
        drop(Box::from_raw(req_ptr));
      }
      return err;
    }

    state_array.set_index(
      scope,
      StreamBaseStateFields::BytesWritten as u32,
      v8::Number::new(scope, byte_length as f64).into(),
    );
    state_array.set_index(
      scope,
      StreamBaseStateFields::LastWriteWasAsync as u32,
      v8::Integer::new(scope, 1).into(),
    );

    0
  }

  #[fast]
  pub fn writev(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    chunks: v8::Local<v8::Array>,
    all_buffers: bool,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);

    let mut data = Vec::new();

    if all_buffers {
      let len = chunks.length();
      for i in 0..len {
        let Some(chunk) = chunks.get_index(scope, i) else {
          continue;
        };
        if let Ok(buf) = TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk) {
          let byte_len = buf.byte_length();
          let byte_off = buf.byte_offset();
          let ab = buf.buffer(scope).unwrap();
          let ptr = ab.data().unwrap().as_ptr() as *const u8;
          let slice =
            unsafe { std::slice::from_raw_parts(ptr.add(byte_off), byte_len) };
          data.extend_from_slice(slice);
        }
      }
    } else {
      let len = chunks.length();
      let count = len / 2;
      for i in 0..count {
        let Some(chunk) = chunks.get_index(scope, i * 2) else {
          continue;
        };
        if let Ok(buf) = TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk) {
          let byte_len = buf.byte_length();
          let byte_off = buf.byte_offset();
          let ab = buf.buffer(scope).unwrap();
          let ptr = ab.data().unwrap().as_ptr() as *const u8;
          let slice =
            unsafe { std::slice::from_raw_parts(ptr.add(byte_off), byte_len) };
          data.extend_from_slice(slice);
        } else if let Ok(s) = TryInto::<v8::Local<v8::String>>::try_into(chunk)
        {
          let encoding = chunks
            .get_index(scope, i * 2 + 1)
            .and_then(|v| TryInto::<v8::Local<v8::String>>::try_into(v).ok())
            .map(|v| v.to_rust_string_lossy(scope));
          let enc = match encoding.as_deref() {
            Some("latin1" | "binary") => StringEncoding::Latin1,
            Some("ucs2" | "ucs-2" | "utf16le" | "utf-16le") => {
              StringEncoding::Ucs2
            }
            Some("ascii") => StringEncoding::Ascii,
            _ => StringEncoding::Utf8,
          };
          encode_string_to_vec(scope, s, enc, &mut data);
        }
      }
    }

    let total_bytes = data.len();

    // Track bytes_written
    self
      .bytes_written
      .set(self.bytes_written.get() + total_bytes as u64);

    if total_bytes == 0 {
      state_array.set_index(
        scope,
        StreamBaseStateFields::BytesWritten as u32,
        v8::Integer::new(scope, 0).into(),
      );
      state_array.set_index(
        scope,
        StreamBaseStateFields::LastWriteWasAsync as u32,
        v8::Integer::new(scope, 0).into(),
      );
      return 0;
    }

    self.do_write(scope, stream, &data, total_bytes, req_wrap_obj, state_array)
  }

  #[fast]
  pub fn write_utf8_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);
    self.write_string(
      scope,
      req_wrap_obj,
      string,
      state_array,
      StringEncoding::Utf8,
    )
  }

  #[fast]
  pub fn write_ascii_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);
    self.write_string(
      scope,
      req_wrap_obj,
      string,
      state_array,
      StringEncoding::Ascii,
    )
  }

  #[fast]
  pub fn write_latin1_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);
    self.write_string(
      scope,
      req_wrap_obj,
      string,
      state_array,
      StringEncoding::Latin1,
    )
  }

  #[fast]
  pub fn write_ucs2_string(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);
    self.write_string(
      scope,
      req_wrap_obj,
      string,
      state_array,
      StringEncoding::Ucs2,
    )
  }

  #[fast]
  #[no_side_effects]
  pub fn get_bytes_read(&self) -> f64 {
    self.bytes_read.get() as f64
  }

  #[fast]
  #[no_side_effects]
  pub fn get_bytes_written(&self) -> f64 {
    self.bytes_written.get() as f64
  }
}

impl LibUvStreamWrap {
  fn write_string(
    &self,
    scope: &mut v8::PinScope,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    state_array: v8::Local<v8::Int32Array>,
    encoding: StringEncoding,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let mut data = Vec::new();
    encode_string_to_vec(scope, string, encoding, &mut data);

    let total_bytes = data.len();

    // Track bytes_written (mirrors Node's Write() in stream_base.cc)
    self
      .bytes_written
      .set(self.bytes_written.get() + total_bytes as u64);

    // For small strings, try synchronous write first
    // (mirrors Node's WriteString stack_storage[16384] optimization)
    if total_bytes <= 16384 {
      let try_result = unsafe { uv_compat::uv_try_write(stream, &data) };

      if try_result >= 0 && try_result as usize == total_bytes {
        // Fully written synchronously
        state_array.set_index(
          scope,
          StreamBaseStateFields::BytesWritten as u32,
          v8::Number::new(scope, total_bytes as f64).into(),
        );
        state_array.set_index(
          scope,
          StreamBaseStateFields::LastWriteWasAsync as u32,
          v8::Integer::new(scope, 0).into(),
        );
        return 0;
      }

      // Partial try_write — async write only the remaining bytes
      if try_result > 0 {
        let written = try_result as usize;
        return self.do_write(
          scope,
          stream,
          &data[written..],
          total_bytes,
          req_wrap_obj,
          state_array,
        );
      }
    }

    // Full async write (no try_write or try_write returned error/0)
    self.do_write(scope, stream, &data, total_bytes, req_wrap_obj, state_array)
  }

  fn do_write(
    &self,
    scope: &mut v8::PinScope,
    stream: *mut uv_stream_t,
    data: &[u8],
    total_bytes: usize,
    req_wrap_obj: v8::Local<v8::Object>,
    state_array: v8::Local<v8::Int32Array>,
  ) -> i32 {
    let buf = uv_buf_t {
      base: data.as_ptr() as *mut c_char,
      len: data.len(),
    };

    let stream_handle = self
      .js_handle_global()
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_write());
    let cb_data = Box::new(WriteCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
      stream_handle,
      stream_base_state: v8::Global::new(scope, state_array),
      bytes: total_bytes,
    });
    req.data = Box::into_raw(cb_data) as *mut std::ffi::c_void;
    let req_ptr = Box::into_raw(req);

    let err = unsafe {
      uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(after_uv_write))
    };

    if err != 0 {
      unsafe {
        let d = (*req_ptr).data as *mut WriteCallbackData;
        if !d.is_null() {
          drop(Box::from_raw(d));
        }
        drop(Box::from_raw(req_ptr));
      }
      return err;
    }

    state_array.set_index(
      scope,
      StreamBaseStateFields::BytesWritten as u32,
      v8::Number::new(scope, total_bytes as f64).into(),
    );
    state_array.set_index(
      scope,
      StreamBaseStateFields::LastWriteWasAsync as u32,
      v8::Integer::new(scope, 1).into(),
    );

    0
  }
}
