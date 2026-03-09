// Copyright 2018-2026 the Deno authors. MIT license.

// Ported from Node.js:
// - src/stream_base.h
// - src/stream_base.cc
// - src/stream_base-inl.h
// - src/stream_wrap.h
// - src/stream_wrap.cc

#![allow(non_snake_case)]

use std::ffi::c_char;
use std::ptr::NonNull;

use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::UV_EINVAL;
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

const STREAM_BASE_STATE_FIELDS: usize = 5;

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
  bytes_read: u64,
  bytes_written: u64,
}

impl LibUvStreamWrap {
  pub fn new(base: HandleWrap, fd: i32, stream: *const uv_stream_t) -> Self {
    Self {
      base,
      fd,
      stream,
      bytes_read: 0,
      bytes_written: 0,
    }
  }

  #[inline]
  pub fn stream_ptr(&self) -> *mut uv_stream_t {
    self.stream as *mut uv_stream_t
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
      let recv = v8::undefined(scope);
      let undef = v8::undefined(scope);
      onread.call(scope, recv.into(), &[undef.into()]);
    }
    return;
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

  // Call onread(arrayBuffer)
  let onread = v8::Local::new(scope, &cb_data.onread);
  let recv = v8::undefined(scope);
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
  stream_base_state: v8::Global<v8::Array>,
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

  // Call req_wrap_obj.oncomplete(status)
  let req_obj = v8::Local::new(scope, &cb_data.req_wrap_obj);
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into()) {
    if let Ok(oncomplete) = oncomplete.try_into() {
      let oncomplete: v8::Local<v8::Function> = oncomplete;
      let status_val = v8::Integer::new(scope, status);
      oncomplete.call(scope, req_obj.into(), &[status_val.into()]);
    }
  }
}

struct WriteCallbackData {
  isolate: v8::UnsafeRawIsolatePtr,
  req_wrap_obj: v8::Global<v8::Object>,
  stream_base_state: v8::Global<v8::Array>,
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

  // Call req_wrap_obj.oncomplete(status)
  let req_obj = v8::Local::new(scope, &cb_data.req_wrap_obj);
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  if let Some(oncomplete) = req_obj.get(scope, oncomplete_str.into()) {
    if let Ok(oncomplete) = oncomplete.try_into() {
      let oncomplete: v8::Local<v8::Function> = oncomplete;
      let status_val = v8::Integer::new(scope, status);
      oncomplete.call(scope, req_obj.into(), &[status_val.into()]);
    }
  }
}

struct ShutdownCallbackData {
  isolate: v8::UnsafeRawIsolatePtr,
  req_wrap_obj: v8::Global<v8::Object>,
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

#[op2(base)]
impl LibUvStreamWrap {
  #[fast]
  pub fn read_start(
    &self,
    state_array: v8::Local<v8::Array>,
    onread: v8::Local<v8::Function>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let cb_data = Box::new(CallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      onread: v8::Global::new(scope, onread),
      stream_base_state: v8::Global::new(scope, state_array),
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
  pub fn shutdown(
    &self,
    req_wrap_obj: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let mut req = Box::new(uv_compat::new_shutdown());
    let cb_data = Box::new(ShutdownCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let byte_length = buffer.byte_length();
    let byte_offset = buffer.byte_offset();

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

    let mut req = Box::new(uv_compat::new_write());
    let cb_data = Box::new(WriteCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

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
          // TODO: handle latin1, ucs2, etc. based on encoding arg
          let len = s.utf8_length(scope);
          let start = data.len();
          data.reserve(len);
          let written = s.write_utf8_uninit_v2(
            scope,
            &mut data.spare_capacity_mut()[..len],
            v8::WriteFlags::kReplaceInvalidUtf8,
            None,
          );
          unsafe { data.set_len(start + written) };
        }
      }
    }

    let total_bytes = data.len();

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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
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
    state_array: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
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
    self.bytes_read as f64
  }

  #[fast]
  #[no_side_effects]
  pub fn get_bytes_written(&self) -> f64 {
    self.bytes_written as f64
  }
}

impl LibUvStreamWrap {
  fn write_string(
    &self,
    scope: &mut v8::PinScope,
    req_wrap_obj: v8::Local<v8::Object>,
    string: v8::Local<v8::String>,
    state_array: v8::Local<v8::Array>,
    encoding: StringEncoding,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    let data: Vec<u8> = match encoding {
      StringEncoding::Utf8 | StringEncoding::Ascii => {
        let len = string.utf8_length(scope);
        let mut buf = Vec::with_capacity(len);
        let written = string.write_utf8_uninit_v2(
          scope,
          buf.spare_capacity_mut(),
          v8::WriteFlags::kReplaceInvalidUtf8,
          None,
        );
        unsafe { buf.set_len(written) };
        buf
      }
      StringEncoding::Latin1 => {
        let len = string.length();
        let mut buf = Vec::with_capacity(len);
        string.write_one_byte_uninit_v2(
          scope,
          0,
          buf.spare_capacity_mut(),
          v8::WriteFlags::empty(),
        );
        unsafe { buf.set_len(len) };
        buf
      }
      StringEncoding::Ucs2 => {
        let len = string.length();
        let mut buf = vec![0u16; len];
        string.write_v2(scope, 0, &mut buf, v8::WriteFlags::empty());
        let mut bytes = Vec::with_capacity(len * 2);
        for &ch in &buf {
          bytes.extend_from_slice(&ch.to_le_bytes());
        }
        bytes
      }
    };

    let total_bytes = data.len();

    // For small strings, try synchronous write first
    if total_bytes <= 16384 {
      let try_result = unsafe { uv_compat::uv_try_write(stream, &data) };

      if try_result >= 0 && try_result as usize == total_bytes {
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
    }

    self.do_write(scope, stream, &data, total_bytes, req_wrap_obj, state_array)
  }

  fn do_write(
    &self,
    scope: &mut v8::PinScope,
    stream: *mut uv_stream_t,
    data: &[u8],
    total_bytes: usize,
    req_wrap_obj: v8::Local<v8::Object>,
    state_array: v8::Local<v8::Array>,
  ) -> i32 {
    let buf = uv_buf_t {
      base: data.as_ptr() as *mut c_char,
      len: data.len(),
    };

    let mut req = Box::new(uv_compat::new_write());
    let cb_data = Box::new(WriteCallbackData {
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
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
