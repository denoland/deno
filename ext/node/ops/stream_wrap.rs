// Copyright 2018-2026 the Deno authors. MIT license.

// Ported from Node.js:
// - src/stream_base.h
// - src/stream_base.cc
// - src/stream_base-inl.h
// - src/stream_wrap.h
// - src/stream_wrap.cc

use std::cell::Cell;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::ffi::c_char;
use std::ptr::NonNull;
use std::rc::Rc;

use deno_core::CppgcBase;
use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::uv_buf_t;
use deno_core::uv_compat::uv_shutdown_t;
use deno_core::uv_compat::uv_stream_t;
use deno_core::uv_compat::uv_write_t;
use deno_core::v8;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::GlobalHandle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap_state::ReadCallbackKey;
use crate::ops::stream_wrap_state::ReadCallbackRegistry;
use crate::ops::stream_wrap_state::ReadCallbackState;
use crate::ops::stream_wrap_state::ReadInterceptor;
use crate::ops::stream_wrap_state::RequestCallbackRegistry;
use crate::ops::stream_wrap_state::RequestCallbackState;
use crate::ops::stream_wrap_state::ShutdownRequestCallbackState;
use crate::ops::stream_wrap_state::WriteRequestCallbackState;

// ---------------------------------------------------------------------------
// StreamBase state fields — mirrors Node's StreamBaseStateFields enum.
// These index into a shared Uint8Array visible to JS.
// ---------------------------------------------------------------------------

/// Per-environment stream_base_state array, stored in OpState.
/// This is an Int32Array shared between JS and Rust (mirrors Node's AliasedInt32Array).
pub struct StreamBaseState {
  pub array: v8::Global<v8::Int32Array>,
}

pub(crate) struct StreamHandleData {
  pub js_handle: UnsafeCell<GlobalHandle<v8::Object>>,
  pub isolate: UnsafeCell<v8::UnsafeRawIsolatePtr>,
  pub bytes_read: Rc<Cell<u64>>,
  pub desired_read_interceptor: Cell<Option<ReadInterceptor>>,
  pub read_callbacks: RefCell<ReadCallbackRegistry>,
  pub active_read: Cell<Option<ReadCallbackKey>>,
  pub request_callbacks: RefCell<RequestCallbackRegistry>,
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

// SAFETY: WriteWrap is a cppgc-managed object; it holds no GC-traced references beyond its base.
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

// SAFETY: ShutdownWrap is a cppgc-managed object; it holds no GC-traced references beyond its base.
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
  fd: Cell<i32>,
  stream: *const uv_stream_t,
  bytes_read: Rc<Cell<u64>>,
  bytes_written: Rc<Cell<u64>>,
  /// Stable per-handle data referenced from `uv_stream_t.data` for the
  /// lifetime of the native stream.
  handle_data: Box<StreamHandleData>,
  /// Whether readStart has been called and the handle currently has an active
  /// callback state entry.
  reading_started: Cell<bool>,
}

impl LibUvStreamWrap {
  pub fn new(base: HandleWrap, fd: i32, stream: *const uv_stream_t) -> Self {
    let bytes_read = Rc::new(Cell::new(0));
    Self {
      base,
      fd: Cell::new(fd),
      stream,
      bytes_read: bytes_read.clone(),
      bytes_written: Rc::new(Cell::new(0)),
      handle_data: Box::new(StreamHandleData {
        js_handle: UnsafeCell::new(GlobalHandle::default()),
        isolate: UnsafeCell::new(v8::UnsafeRawIsolatePtr::null()),
        bytes_read,
        desired_read_interceptor: Cell::new(None),
        read_callbacks: RefCell::new(ReadCallbackRegistry::default()),
        active_read: Cell::new(None),
        request_callbacks: RefCell::new(RequestCallbackRegistry::default()),
      }),
      reading_started: Cell::new(false),
    }
  }

  #[inline]
  pub fn stream_ptr(&self) -> *mut uv_stream_t {
    self.stream as *mut uv_stream_t
  }

  #[allow(dead_code, reason = "used by upcoming TCPWrap/TLSWrap")]
  pub(crate) fn set_fd(&self, fd: i32) {
    self.fd.set(fd);
  }

  #[allow(dead_code, reason = "used by upcoming TCPWrap/TLSWrap")]
  pub(crate) fn handle_wrap(&self) -> &HandleWrap {
    &self.base
  }

  pub(crate) fn handle_data_ptr(&self) -> *mut std::ffi::c_void {
    (&*self.handle_data as *const StreamHandleData)
      .cast_mut()
      .cast()
  }

  pub(crate) fn js_handle_global(
    &self,
    scope: &mut v8::PinScope,
  ) -> Option<v8::Global<v8::Object>> {
    // SAFETY: js_handle is only written via set_handle which runs on the same thread before any read.
    unsafe { (*self.handle_data.js_handle.get()).to_global(scope) }
  }

  #[allow(dead_code, reason = "used by upcoming TCPWrap/TLSWrap")]
  pub(crate) fn set_js_handle(
    &self,
    handle: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
  ) {
    // SAFETY: single-threaded access.
    unsafe {
      *self.handle_data.js_handle.get() =
        GlobalHandle::new_strong(scope, v8::Local::new(scope, &handle));
      *self.handle_data.isolate.get() = scope.as_raw_isolate_ptr();
    }
  }

  /// Clear the JS handle reference when the native handle is closing so the
  /// wrapper no longer keeps the JS object alive.
  pub(crate) fn clear_js_handle(&self) {
    // SAFETY: single-threaded access.
    unsafe {
      *self.handle_data.js_handle.get() = GlobalHandle::None;
    }
  }

  /// Make the js_handle reference strong, preventing GC of the JS object.
  /// Should be called when the handle becomes actively referenced (e.g. read started).
  pub fn make_handle_strong(&self, scope: &mut v8::PinScope) {
    // SAFETY: single-threaded access, same as js_handle_global.
    unsafe {
      (*self.handle_data.js_handle.get()).make_strong(scope);
    }
  }

  /// Make the js_handle reference weak, allowing GC to collect the JS object.
  /// Should be called when the handle is no longer actively referenced (e.g. closed, read stopped).
  pub fn make_handle_weak(&self, scope: &mut v8::PinScope) {
    // SAFETY: single-threaded access, same as js_handle_global.
    unsafe {
      (*self.handle_data.js_handle.get()).make_weak(scope);
    }
  }

  fn install_read_state(&self, state: ReadCallbackState) {
    let key = self.handle_data.read_callbacks.borrow_mut().insert(state);
    self.handle_data.active_read.set(Some(key));
  }

  fn stable_handle_data(
    stream: *mut uv_stream_t,
  ) -> Option<NonNull<StreamHandleData>> {
    if stream.is_null() {
      return None;
    }
    // SAFETY: `stream` is non-null above and callers only pass valid uv stream pointers.
    NonNull::new(unsafe { (*stream).data as *mut StreamHandleData })
  }

  #[allow(dead_code, reason = "used by upcoming TLSWrap")]
  pub(crate) fn set_read_interceptor_for_stream(
    stream: *mut uv_stream_t,
    interceptor: Option<ReadInterceptor>,
  ) {
    let Some(handle_data_ptr) = Self::stable_handle_data(stream) else {
      return;
    };
    // SAFETY: `uv_stream_t.data` points at the owning handle's stable
    // `StreamHandleData` allocation while the native stream is alive.
    let handle_data = unsafe { handle_data_ptr.as_ref() };
    handle_data.desired_read_interceptor.set(interceptor);
    if let Some(key) = handle_data.active_read.get() {
      let _ = handle_data
        .read_callbacks
        .borrow_mut()
        .update_interceptor(key, interceptor);
    }
  }

  #[allow(dead_code, reason = "used by upcoming TLSWrap")]
  pub(crate) fn read_start_intercepted_for_stream(
    stream: *mut uv_stream_t,
  ) -> i32 {
    let Some(handle_data_ptr) = Self::stable_handle_data(stream) else {
      return UV_EBADF;
    };
    // SAFETY: `uv_stream_t.data` points at the owning handle's stable
    // `StreamHandleData` allocation while the native stream is alive.
    let handle_data = unsafe { handle_data_ptr.as_ref() };
    if handle_data.active_read.get().is_some() {
      return 0;
    }

    let key =
      handle_data
        .read_callbacks
        .borrow_mut()
        .insert(ReadCallbackState {
          isolate: v8::UnsafeRawIsolatePtr::null(),
          onread: None,
          stream_base_state: None,
          handle: None,
          bytes_read: handle_data.bytes_read.clone(),
          read_interceptor: handle_data.desired_read_interceptor.get(),
        });
    handle_data.active_read.set(Some(key));

    // SAFETY: `stream` is a valid libuv stream owned by this wrapper.
    unsafe {
      uv_compat::uv_read_start(stream, Some(on_uv_alloc), Some(on_uv_read))
    }
  }

  pub(crate) fn read_stop_for_stream(stream: *mut uv_stream_t) -> i32 {
    let Some(handle_data_ptr) = Self::stable_handle_data(stream) else {
      return UV_EBADF;
    };
    // SAFETY: `uv_stream_t.data` points at the owning handle's stable
    // `StreamHandleData` allocation while the native stream is alive.
    let handle_data = unsafe { handle_data_ptr.as_ref() };
    if let Some(key) = handle_data.active_read.take() {
      let _ = handle_data.read_callbacks.borrow_mut().remove(key);
    }
    // SAFETY: `stream` is a valid libuv stream owned by this wrapper.
    unsafe { uv_compat::uv_read_stop(stream) }
  }

  fn read_start_with_handle(
    &self,
    this: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }

    if self.reading_started.get() {
      return 0;
    }

    let onread_key =
      v8::String::new_external_onebyte_static(scope, b"onread").unwrap();
    let Some(onread_val) = this.get(scope, onread_key.into()) else {
      return UV_EBADF;
    };
    let Ok(onread) = v8::Local::<v8::Function>::try_from(onread_val) else {
      return UV_EBADF;
    };

    let state_global = &op_state.borrow::<StreamBaseState>().array;
    let state_array = v8::Local::new(scope, state_global);

    self.install_read_state(ReadCallbackState {
      // SAFETY: `scope` is the currently active isolate scope for this op call.
      isolate: unsafe { scope.as_raw_isolate_ptr() },
      onread: Some(v8::Global::new(scope, onread)),
      stream_base_state: Some(v8::Global::new(scope, state_array)),
      handle: Some(v8::Global::new(scope, this)),
      bytes_read: self.handle_data.bytes_read.clone(),
      read_interceptor: self.handle_data.desired_read_interceptor.get(),
    });
    self.reading_started.set(true);
    self.make_handle_strong(scope);

    // SAFETY: `stream` is a valid libuv stream owned by this wrapper.
    unsafe {
      uv_compat::uv_read_start(stream, Some(on_uv_alloc), Some(on_uv_read))
    }
  }

  pub(crate) fn read_stop_internal(&self) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }
    if LibUvStreamWrap::stable_handle_data(stream).is_none() {
      return UV_EBADF;
    }
    self.reading_started.set(false);
    Self::read_stop_for_stream(stream)
  }
}

// SAFETY: LibUvStreamWrap is a cppgc-managed object; trace() correctly delegates to the base HandleWrap.
unsafe impl GarbageCollected for LibUvStreamWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"LibUvStreamWrap"
  }

  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl LibUvStreamWrap {
  /// Detach the stream handle data so it won't be accessed after GC.
  /// Must be called before the uv handle memory is freed.
  /// Only call this on handles that OWN the uv stream (e.g. TCPWrap),
  /// not on wrappers that borrow it (e.g. TLSWrap).
  pub(crate) fn detach_stream(&mut self) {
    if !self.stream.is_null() {
      // SAFETY: stream pointer is non-null (checked above) and valid for the
      // lifetime of the owning handle; we null it to prevent dangling access.
      unsafe {
        (*(self.stream as *mut uv_stream_t)).data = std::ptr::null_mut();
      }
      self.stream = std::ptr::null();
    }
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
/// `handle` must be a valid uv_handle_t whose `data` field points to the
/// owning `StreamHandleData`.
unsafe extern "C" fn on_uv_alloc(
  _handle: *mut uv_compat::uv_handle_t,
  suggested_size: usize,
  buf: *mut uv_buf_t,
) {
  // Allocate raw memory for the read buffer.
  let layout = std::alloc::Layout::from_size_align(suggested_size, 1).unwrap();
  // SAFETY: layout has non-zero size (libuv provides a positive suggested_size).
  let ptr = unsafe { std::alloc::alloc(layout) };
  if ptr.is_null() {
    // SAFETY: buf is a valid pointer provided by libuv per the uv_alloc_cb contract.
    unsafe {
      (*buf).base = std::ptr::null_mut();
      (*buf).len = 0;
    }
    return;
  }
  // SAFETY: buf is a valid pointer provided by libuv per the uv_alloc_cb contract.
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
/// the owning `StreamHandleData`, whose active read key resolves to a
/// registered callback state. `buf` must be the buffer from on_uv_alloc.
/// `stream.loop_.data` must be a raw `Global<Context>` pointer set by
/// `register_uv_loop`.
unsafe extern "C" fn on_uv_read(
  stream: *mut uv_stream_t,
  nread: isize,
  buf: *const uv_buf_t,
) {
  let Some(handle_data_ptr) = LibUvStreamWrap::stable_handle_data(stream)
  else {
    free_uv_buf(buf);
    return;
  };
  // SAFETY: `uv_stream_t.data` points at the owning handle's stable
  // `StreamHandleData` allocation while the native stream is alive.
  let handle_data = unsafe { handle_data_ptr.as_ref() };
  let Some(snapshot) = handle_data
    .active_read
    .get()
    .and_then(|key| handle_data.read_callbacks.borrow().snapshot(key))
  else {
    free_uv_buf(buf);
    return;
  };

  if let Some(interceptor) = snapshot.read_interceptor {
    if nread > 0 {
      snapshot
        .bytes_read
        .set(snapshot.bytes_read.get() + nread as u64);
    }
    if nread < 0 {
      // Match libuv's EOF/error behavior: the underlying stream stops
      // itself before higher-level listeners observe the terminal read.
      let _ = LibUvStreamWrap::read_stop_for_stream(stream);
    }
    // SAFETY: interceptor registration guarantees the callback and payload are valid for this read dispatch.
    unsafe { (interceptor.callback)(interceptor.ptr, stream, nread, buf) };
    return;
  }

  let Some(stream_base_state) = snapshot.stream_base_state else {
    free_uv_buf(buf);
    return;
  };
  let Some(onread_global) = snapshot.onread else {
    free_uv_buf(buf);
    return;
  };
  let Some(handle_global) = snapshot.handle else {
    free_uv_buf(buf);
    return;
  };

  // Recover isolate + context from the uv_loop
  // SAFETY: isolate is the raw isolate pointer captured in read_start and is still valid.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(snapshot.isolate) };
  // SAFETY: stream is a valid uv_stream_t per the uv_read_cb contract.
  let loop_ptr = unsafe { (*stream).loop_ };
  // SAFETY: loop_ptr comes from a valid uv stream whose loop has been registered.
  let context = unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
  v8::scope!(let handle_scope, &mut isolate);
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  if nread <= 0 {
    free_uv_buf(buf);
    if nread < 0 {
      // Error/EOF path
      let state_array = v8::Local::new(scope, &stream_base_state);
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

      let onread = v8::Local::new(scope, &onread_global);
      let recv = v8::Local::new(scope, &handle_global);
      let undef = v8::undefined(scope);
      onread.call(scope, recv.into(), &[undef.into()]);
    }
    return;
  }

  // Update bytes_read counter (mirrors Node's EmitRead in stream_base-inl.h)
  snapshot
    .bytes_read
    .set(snapshot.bytes_read.get() + nread as u64);

  // Successful read: wrap data in ArrayBuffer
  let nread_usize = nread as usize;
  // SAFETY: buf is a valid uv_buf_t allocated by on_uv_alloc per the uv_read_cb contract.
  let buf_ref = unsafe { &*buf };

  // Create a backing store from the allocated memory.
  // The deleter will free the alloc when the ArrayBuffer is GC'd.
  // SAFETY: buf_ref.base points to memory allocated by on_uv_alloc with size buf_ref.len; ownership transfers to the backing store.
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
  let state_array = v8::Local::new(scope, &stream_base_state);
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
  // Matches Node's convention: nread is in streamBaseState[kReadBytesOrError].
  let onread = v8::Local::new(scope, &onread_global);
  let recv = v8::Local::new(scope, &handle_global);
  onread.call(scope, recv.into(), &[ab.into()]);
}

/// Free a buffer allocated by on_uv_alloc.
fn free_uv_buf(buf: *const uv_buf_t) {
  // SAFETY: buf is a valid uv_buf_t from on_uv_alloc; base was allocated with alloc(len, 1).
  unsafe {
    if !(*buf).base.is_null() && (*buf).len > 0 {
      let layout = std::alloc::Layout::from_size_align((*buf).len, 1).unwrap();
      std::alloc::dealloc((*buf).base as *mut u8, layout);
    }
  }
}

/// Clone the V8 context registered on a uv loop without taking ownership of
/// the runtime's raw persistent handle stored in `loop_.data`.
///
/// # Safety
/// `loop_ptr` must be a valid, initialized `uv_loop_t` whose `data` field
/// contains the raw `Global<Context>` installed by `register_uv_loop`.
unsafe fn clone_context_from_uv_loop(
  isolate: &mut v8::Isolate,
  loop_ptr: *mut uv_compat::uv_loop_t,
) -> v8::Global<v8::Context> {
  // SAFETY: `loop_ptr` is valid per the function contract above.
  let raw = NonNull::new(unsafe { (*loop_ptr).data as *mut v8::Context })
    .expect("uv loop missing registered V8 context");
  // SAFETY: `raw` came from `Global::into_raw()` and is still owned by the runtime.
  let global = unsafe { v8::Global::from_raw(isolate, raw) };
  let cloned = global.clone();
  // Leak the original back because the runtime still owns that persistent.
  global.into_raw();
  cloned
}

/// Backing store deleter that frees memory allocated by on_uv_alloc.
///
/// # Safety
/// `data` must be a pointer allocated by `std::alloc::alloc` with size
/// `deleter_data` (cast from usize) and alignment 1.
unsafe extern "C" fn backing_store_deleter(
  data: *mut std::ffi::c_void,
  _len: usize,
  deleter_data: *mut std::ffi::c_void,
) {
  // Use the original allocation size (passed via deleter_data), not `_len`
  // which may be smaller (nread < allocated size is common for partial reads).
  let alloc_size = deleter_data as usize;
  if !data.is_null() && alloc_size > 0 {
    let layout = std::alloc::Layout::from_size_align(alloc_size, 1).unwrap();
    // SAFETY: data was allocated via alloc(Layout::from_size_align(alloc_size, 1)) in on_uv_alloc.
    unsafe { std::alloc::dealloc(data as *mut u8, layout) };
  }
}

// ---------------------------------------------------------------------------
// Write completion callback for uv_write.
//
// Called by the uv_compat layer when a write completes. Fires the JS
// `oncomplete` callback on the WriteWrap request object.
// ---------------------------------------------------------------------------

/// # Safety
/// `req` must be a valid uv_write_t whose `handle.data` points to the
/// owning `StreamHandleData`. `req.handle.loop_.data` must be a raw
/// `Global<Context>` pointer.
unsafe extern "C" fn after_uv_write(req: *mut uv_write_t, status: i32) {
  // SAFETY: req is a valid uv_write_t per the uv_write_cb contract.
  let req_data = unsafe { (*req).data };
  // SAFETY: `req.handle` is a valid uv stream for the lifetime of this completion callback.
  let handle_data =
    unsafe { LibUvStreamWrap::stable_handle_data((*req).handle) };
  let Some(handle_data_ptr) = handle_data else {
    // Handle was detached (e.g. GC). Free the request to avoid a leak.
    // SAFETY: `req` was allocated with `Box::new` in `do_write` and is
    // valid per the uv_write_cb contract.
    unsafe { drop(Box::from_raw(req)) };
    return;
  };
  // SAFETY: req is a valid uv_write_t per the uv_write_cb contract.
  unsafe { (*req).data = std::ptr::null_mut() };
  // SAFETY: `uv_stream_t.data` points at the owning handle's stable
  // `StreamHandleData` allocation while the native stream is alive.
  let handle_data = unsafe { handle_data_ptr.as_ref() };
  let Some(RequestCallbackState::Write(cb_data)) =
    handle_data.request_callbacks.borrow_mut().take(req_data)
  else {
    return;
  };

  // SAFETY: cb_data.isolate is the raw isolate pointer captured during the write call and is still valid.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(cb_data.isolate) };
  // SAFETY: req is a valid uv_write_t; its handle and loop_ fields are valid per libuv guarantees.
  let loop_ptr = unsafe { (*(*req).handle).loop_ };
  // SAFETY: loop_ptr comes from a valid uv request whose loop has been registered.
  let context = unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
  v8::scope!(let handle_scope, &mut isolate);
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
  if let Some(Ok(oncomplete)) = req_obj
    .get(scope, oncomplete_str.into())
    .map(TryInto::<v8::Local<v8::Function>>::try_into)
  {
    let status_val = v8::Integer::new(scope, status);
    let undef = v8::undefined(scope);
    oncomplete.call(
      scope,
      req_obj.into(),
      &[status_val.into(), handle.into(), undef.into()],
    );
  }
}

// ---------------------------------------------------------------------------
// Shutdown completion callback for uv_shutdown.
// ---------------------------------------------------------------------------

/// # Safety
/// `req` must be a valid uv_shutdown_t whose `handle.data` points to the
/// owning `StreamHandleData`. `req.handle.loop_.data` must be a raw
/// `Global<Context>` pointer.
unsafe extern "C" fn after_uv_shutdown(req: *mut uv_shutdown_t, status: i32) {
  // SAFETY: req is a valid uv_shutdown_t per the uv_shutdown_cb contract.
  let req_data = unsafe { (*req).data };
  // SAFETY: `req.handle` is a valid uv stream for the lifetime of this completion callback.
  let handle_data =
    unsafe { LibUvStreamWrap::stable_handle_data((*req).handle) };
  let Some(handle_data_ptr) = handle_data else {
    // Handle was detached (e.g. GC). Free the request to avoid a leak.
    // SAFETY: `req` was allocated with `Box::new` in `do_shutdown` and is
    // valid per the uv_shutdown_cb contract.
    unsafe { drop(Box::from_raw(req)) };
    return;
  };
  // SAFETY: req is a valid uv_shutdown_t per the uv_shutdown_cb contract.
  unsafe { (*req).data = std::ptr::null_mut() };
  // SAFETY: `uv_stream_t.data` points at the owning handle's stable
  // `StreamHandleData` allocation while the native stream is alive.
  let handle_data = unsafe { handle_data_ptr.as_ref() };
  let Some(RequestCallbackState::Shutdown(cb_data)) =
    handle_data.request_callbacks.borrow_mut().take(req_data)
  else {
    return;
  };

  // SAFETY: cb_data.isolate is the raw isolate pointer captured during the shutdown call and is still valid.
  let mut isolate =
    unsafe { v8::Isolate::from_raw_isolate_ptr(cb_data.isolate) };
  // SAFETY: req is a valid uv_shutdown_t; its handle and loop_ fields are valid per libuv guarantees.
  let loop_ptr = unsafe { (*(*req).handle).loop_ };
  // SAFETY: loop_ptr comes from a valid uv request whose loop has been registered.
  let context = unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
  v8::scope!(let handle_scope, &mut isolate);
  let context = v8::Local::new(handle_scope, context);
  let scope = &mut v8::ContextScope::new(handle_scope, context);

  // Call req_wrap_obj.oncomplete(status, handle, error)
  let req_obj = v8::Local::new(scope, &cb_data.req_wrap_obj);
  let handle = v8::Local::new(scope, &cb_data.stream_handle);
  let oncomplete_str =
    v8::String::new_external_onebyte_static(scope, b"oncomplete").unwrap();
  if let Some(Ok(oncomplete)) = req_obj
    .get(scope, oncomplete_str.into())
    .map(TryInto::<v8::Local<v8::Function>>::try_into)
  {
    let status_val = v8::Integer::new(scope, status);
    let undef = v8::undefined(scope);
    oncomplete.call(
      scope,
      req_obj.into(),
      &[status_val.into(), handle.into(), undef.into()],
    );
  }
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
      // SAFETY: write_utf8_uninit_v2 initialized exactly `written` bytes in the spare capacity.
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
      // SAFETY: write_one_byte_uninit_v2 initialized exactly `len` bytes in the spare capacity.
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

#[op2(base, inherit = HandleWrap)]
impl LibUvStreamWrap {
  /// Called by JS immediately after construction to store the JS object
  /// reference: `stream.setHandle(stream)`
  #[fast]
  pub fn set_handle(
    &self,
    handle: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) {
    // Active libuv handles keep their JS wrappers alive in Node until close.
    // We mirror that here and explicitly clear the back-reference on close.
    //
    // SAFETY: set_handle is called once at construction on the same thread; no concurrent access occurs.
    unsafe {
      *self.handle_data.js_handle.get() =
        GlobalHandle::new_strong(scope, handle);
      *self.handle_data.isolate.get() = scope.as_raw_isolate_ptr();
    }
  }

  #[reentrant]
  fn close(
    &self,
    op_state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[scoped] cb: Option<v8::Global<v8::Function>>,
  ) -> Result<(), ResourceError> {
    self.clear_js_handle();
    self.base.close_handle(op_state, this, scope, cb)
  }

  #[fast]
  pub fn read_start(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> i32 {
    let this = if let Some(handle) = self.js_handle_global(scope) {
      v8::Local::new(scope, handle)
    } else {
      v8::Local::new(scope, &this)
    };
    self.read_start_with_handle(this, scope, op_state)
  }

  #[fast]
  #[reentrant]
  pub fn read_stop(&self, _scope: &mut v8::PinScope) -> i32 {
    self.read_stop_internal()
  }

  #[fast]
  #[rename("setBlocking")]
  pub fn set_blocking(&self, enable: bool) -> i32 {
    let stream = self.stream_ptr();
    if stream.is_null() {
      return UV_EBADF;
    }
    // SAFETY: stream is a valid non-null uv_stream_t (checked above).
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
      .js_handle_global(scope)
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_shutdown());
    req.data = self.handle_data.request_callbacks.borrow_mut().insert(
      RequestCallbackState::Shutdown(ShutdownRequestCallbackState {
        // SAFETY: scope is a valid PinScope for the current isolate.
        isolate: unsafe { scope.as_raw_isolate_ptr() },
        req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
        stream_handle,
      }),
    );
    let req_ptr = Box::into_raw(req);

    // SAFETY: req_ptr is a valid uv_shutdown_t and stream is a valid non-null uv_stream_t.
    let err = unsafe {
      uv_compat::uv_shutdown(req_ptr, stream, Some(after_uv_shutdown))
    };

    if err != 0 {
      // SAFETY: `req_ptr` is still owned locally because `uv_shutdown` failed synchronously.
      let req_data = unsafe { (*req_ptr).data };
      // SAFETY: `req_ptr` is valid and the callback will never observe this request after the synchronous failure.
      unsafe {
        (*req_ptr).data = std::ptr::null_mut();
      }
      let _ = self
        .handle_data
        .request_callbacks
        .borrow_mut()
        .take(req_data);
      // SAFETY: uv_shutdown failed so the callback will never fire; reclaim the request.
      unsafe {
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

    // Track bytes_written (mirrors Node's Write() in stream_base.cc)
    self
      .bytes_written
      .set(self.bytes_written.get() + byte_length as u64);

    let mut buf = [0; v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
    let data = buffer.get_contents(&mut buf);

    // SAFETY: stream is a valid non-null uv_stream_t (checked above).
    let try_result = unsafe { uv_compat::uv_try_write(stream, data) };
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
      (&data[written..], byte_length - written)
    } else {
      (data, byte_length)
    };
    let buf = uv_buf_t {
      base: write_data.as_ptr() as *mut c_char,
      len: write_len,
    };

    let stream_handle = self
      .js_handle_global(scope)
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_write());
    req.data = self.handle_data.request_callbacks.borrow_mut().insert(
      RequestCallbackState::Write(WriteRequestCallbackState {
        // SAFETY: scope is a valid PinScope for the current isolate.
        isolate: unsafe { scope.as_raw_isolate_ptr() },
        req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
        stream_handle,
        stream_base_state: v8::Global::new(scope, state_array),
        bytes: byte_length,
      }),
    );
    let req_ptr = Box::into_raw(req);

    // SAFETY: req_ptr is a valid uv_write_t and stream is a valid non-null uv_stream_t.
    let err = unsafe {
      uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(after_uv_write))
    };
    if err != 0 {
      // SAFETY: `req_ptr` is still owned locally because `uv_write` failed synchronously.
      let req_data = unsafe { (*req_ptr).data };
      // SAFETY: `req_ptr` is valid and the callback will never observe this request after the synchronous failure.
      unsafe {
        (*req_ptr).data = std::ptr::null_mut();
      }
      let _ = self
        .handle_data
        .request_callbacks
        .borrow_mut()
        .take(req_data);
      // SAFETY: uv_write failed so the callback will never fire; reclaim the request.
      unsafe {
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
          // SAFETY: ptr points to the backing store of the ArrayBuffer; byte_off + byte_len is within bounds as guaranteed by the Uint8Array view.
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
          // SAFETY: ptr points to the backing store of the ArrayBuffer; byte_off + byte_len is within bounds as guaranteed by the Uint8Array view.
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
  #[reentrant]
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
      // SAFETY: stream is a valid non-null uv_stream_t (checked at call site).
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
      .js_handle_global(scope)
      .unwrap_or_else(|| v8::Global::new(scope, v8::Object::new(scope)));
    let mut req = Box::new(uv_compat::new_write());
    req.data = self.handle_data.request_callbacks.borrow_mut().insert(
      RequestCallbackState::Write(WriteRequestCallbackState {
        // SAFETY: scope is a valid PinScope for the current isolate.
        isolate: unsafe { scope.as_raw_isolate_ptr() },
        req_wrap_obj: v8::Global::new(scope, req_wrap_obj),
        stream_handle,
        stream_base_state: v8::Global::new(scope, state_array),
        bytes: total_bytes,
      }),
    );
    let req_ptr = Box::into_raw(req);

    // SAFETY: req_ptr is a valid uv_write_t and stream is a valid non-null uv_stream_t.
    let err = unsafe {
      uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(after_uv_write))
    };

    if err != 0 {
      // SAFETY: `req_ptr` is still owned locally because `uv_write` failed synchronously.
      let req_data = unsafe { (*req_ptr).data };
      // SAFETY: `req_ptr` is valid and the callback will never observe this request after the synchronous failure.
      unsafe {
        (*req_ptr).data = std::ptr::null_mut();
      }
      let _ = self
        .handle_data
        .request_callbacks
        .borrow_mut()
        .take(req_data);
      // SAFETY: uv_write failed so the callback will never fire; reclaim the request.
      unsafe {
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
