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

  pub(crate) fn stable_handle_data(
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
    if let Some(key) = handle_data.active_read.get()
      && let Ok(mut callbacks) = handle_data.read_callbacks.try_borrow_mut()
    {
      let _ = callbacks.update_interceptor(key, interceptor);
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

    // Use try_borrow_mut to handle re-entrant calls gracefully.
    // This can happen when uv_read_start fires on_uv_read synchronously
    // (data already buffered), whose interceptor calls cycle() which
    // may call back into read_start.
    let Ok(mut callbacks) = handle_data.read_callbacks.try_borrow_mut() else {
      return 0;
    };
    let key = callbacks.insert(ReadCallbackState {
      isolate: v8::UnsafeRawIsolatePtr::null(),
      onread: None,
      stream_base_state: None,
      handle: None,
      bytes_read: handle_data.bytes_read.clone(),
      read_interceptor: handle_data.desired_read_interceptor.get(),
    });
    drop(callbacks);
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
      // Use try_borrow_mut to handle re-entrant calls from interceptor
      // callbacks that may fire during on_uv_read processing.
      if let Ok(mut callbacks) = handle_data.read_callbacks.try_borrow_mut() {
        let _ = callbacks.remove(key);
      }
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

/// Thread-local free list of 64KB read buffers. libuv calls the alloc
/// callback with a 65536-byte suggested size on every read.
///
/// We allocate read buffers with `std::alloc::alloc` which goes through
/// the system allocator (xzone on macOS). At ~70k reads/sec of a fixed
/// 64KB size, every free triggers a `mach_vm_reclaim_*` kernel trap
/// (~6% of CPU in the pre-pool profile). Pooling keeps the 64KB slab
/// owned by the runtime and reused across reads instead of handed back
/// to the kernel.
///
/// The pool is capped so long-lived idle processes don't retain excess
/// memory. Non-65536 sizes skip the pool entirely.
const POOLED_BUF_SIZE: usize = 65536;
const POOLED_BUF_MAX: usize = 128;

thread_local! {
  static READ_BUF_POOL: std::cell::RefCell<Vec<*mut u8>> =
    const { std::cell::RefCell::new(Vec::new()) };
}

#[inline]
fn pool_acquire_buf() -> Option<*mut u8> {
  READ_BUF_POOL.with(|p| p.borrow_mut().pop())
}

#[inline]
fn pool_release_buf(ptr: *mut u8) -> bool {
  READ_BUF_POOL.with(|p| {
    let mut pool = p.borrow_mut();
    if pool.len() < POOLED_BUF_MAX {
      pool.push(ptr);
      true
    } else {
      false
    }
  })
}

/// Alloc callback for uv_read_start. Returns a pooled 64KB slab when
/// `suggested_size` matches, otherwise falls back to `std::alloc::alloc`.
///
/// # Safety
/// `buf` must be the out-param provided by libuv per the `uv_alloc_cb` contract.
unsafe extern "C" fn on_uv_alloc(
  _handle: *mut uv_compat::uv_handle_t,
  suggested_size: usize,
  buf: *mut uv_buf_t,
) {
  let ptr = if suggested_size == POOLED_BUF_SIZE
    && let Some(ptr) = pool_acquire_buf()
  {
    ptr
  } else {
    let layout =
      std::alloc::Layout::from_size_align(suggested_size, 1).unwrap();
    // SAFETY: layout has non-zero size (libuv provides a positive suggested_size).
    unsafe { std::alloc::alloc(layout) }
  };
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
    if nread < 0 {
      // Socket-level error or EOF: don't hand it to the interceptor.
      // Match Node's PassReadErrorToPreviousListener — the consume
      // interceptor only handles data; terminal reads go back to the
      // normal JS read path so `socket.on('error')` / `'end'`
      // listeners (which the HTTP server relies on to detect aborts
      // and close connections) fire.
      let _ = LibUvStreamWrap::read_stop_for_stream(stream);
      // Fall through to the normal read_cb path below.
    } else {
      if nread > 0 {
        snapshot
          .bytes_read
          .set(snapshot.bytes_read.get() + nread as u64);
      }
      // SAFETY: interceptor registration guarantees the callback and payload are valid for this read dispatch.
      unsafe { (interceptor.callback)(interceptor.ptr, stream, nread, buf) };
      // The interceptor borrows `buf.base` during its callback but does
      // not take ownership — free the buffer here once it returns.
      // Skipping this previously leaked 64KB per read on the consume
      // path (RSS climbed into GBs under sustained HTTP traffic).
      free_uv_buf(buf);
      return;
    }
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
      // EOF/error path: don't report exceptions as fatal.
      // Socket errors (hang up, reset, etc.) are expected lifecycle
      // events that should be handled by the socket's error handler.
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
  let caught_exception = {
    v8::tc_scope!(tc, scope);
    let result = onread.call(tc, recv.into(), &[ab.into()]);
    if result.is_none() && tc.has_caught() {
      let exc = tc.exception();
      tc.reset();
      exc
    } else {
      None
    }
  };
  if let Some(exception) = caught_exception {
    call_fatal_exception(scope, exception);
  }
}

/// Handle uncaught exceptions from stream onread callbacks.
/// Uses globalThis.reportError() to report the exception as uncaught,
/// matching Node's MakeCallback behavior where unhandled exceptions
/// from native callbacks terminate the process.
fn call_fatal_exception(
  scope: &mut v8::ContextScope<v8::HandleScope>,
  exception: v8::Local<v8::Value>,
) {
  let global = scope.get_current_context().global(scope);
  let key = v8::String::new(scope, "reportError").unwrap();
  if let Some(report_fn_val) = global.get(scope, key.into())
    && let Ok(report_fn) = v8::Local::<v8::Function>::try_from(report_fn_val)
  {
    let undef = v8::undefined(scope);
    report_fn.call(scope, undef.into(), &[exception]);
  }
}

/// Free a buffer allocated by on_uv_alloc.
pub(crate) fn free_uv_buf(buf: *const uv_buf_t) {
  // SAFETY: buf is a valid uv_buf_t from on_uv_alloc; base was allocated with alloc(len, 1).
  unsafe {
    if !(*buf).base.is_null() && (*buf).len > 0 {
      let len = (*buf).len;
      let ptr = (*buf).base as *mut u8;
      if len == POOLED_BUF_SIZE && pool_release_buf(ptr) {
        return;
      }
      let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
      std::alloc::dealloc(ptr, layout);
    }
  }
}

/// Clone the V8 context registered on a uv loop without taking ownership of
/// the runtime's raw persistent handle stored in `loop_.data`.
///
/// # Safety
/// `loop_ptr` must be a valid, initialized `uv_loop_t` whose `data` field
/// contains the raw `Global<Context>` installed by `register_uv_loop`.
pub(crate) unsafe fn clone_context_from_uv_loop(
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
    let ptr = data as *mut u8;
    if alloc_size == POOLED_BUF_SIZE && pool_release_buf(ptr) {
      return;
    }
    let layout = std::alloc::Layout::from_size_align(alloc_size, 1).unwrap();
    // SAFETY: data was allocated via alloc(Layout::from_size_align(alloc_size, 1)) in on_uv_alloc.
    unsafe { std::alloc::dealloc(ptr, layout) };
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
  // Reclaim ownership of the request allocated in `do_write` /
  // `write_buffer`. Dropping at end-of-scope ensures every exit path
  // frees it, avoiding the leak that existed when only the detached
  // path called `Box::from_raw`.
  // SAFETY: `req` was allocated with `Box::new` and is valid per the
  // uv_write_cb contract.
  let req = unsafe { Box::from_raw(req) };
  let req_data = req.data;
  let handle_data = LibUvStreamWrap::stable_handle_data(req.handle);
  let Some(handle_data_ptr) = handle_data else {
    // Handle was detached (e.g. GC).
    return;
  };
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
  // SAFETY: req.handle and its loop_ field are valid per libuv guarantees.
  let loop_ptr = unsafe { (*req.handle).loop_ };
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
  // Reclaim ownership so every exit path frees the request allocated
  // in `do_shutdown`.
  // SAFETY: `req` was allocated with `Box::new` and is valid per the
  // uv_shutdown_cb contract.
  let req = unsafe { Box::from_raw(req) };
  let req_data = req.data;
  let handle_data = LibUvStreamWrap::stable_handle_data(req.handle);
  let Some(handle_data_ptr) = handle_data else {
    // Handle was detached (e.g. GC).
    return;
  };
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
  // SAFETY: req.handle and its loop_ field are valid per libuv guarantees.
  let loop_ptr = unsafe { (*req.handle).loop_ };
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

/// Resolve a writev encoding-name v8 String into a `StringEncoding`
/// variant without allocating. Encoding names are short ASCII tokens
/// (max 8 chars: "utf-16le"); read the bytes into a stack buffer and
/// match against literals. Replaces a `to_rust_string_lossy` +
/// `match as_deref` pair that allocated a fresh Rust String per chunk
/// per writev — ~5 allocs/request on the HTTP chunked-encoding path.
fn parse_encoding_no_alloc(
  scope: &mut v8::PinScope,
  encoding: v8::Local<v8::String>,
) -> StringEncoding {
  let len = encoding.length();
  if len == 0 || len > 8 {
    return StringEncoding::Utf8;
  }
  let mut buf = [0u8; 8];
  encoding.write_one_byte_v2(
    scope,
    0,
    &mut buf[..len],
    v8::WriteFlags::empty(),
  );
  match &buf[..len] {
    b"latin1" | b"binary" => StringEncoding::Latin1,
    b"ucs2" | b"ucs-2" | b"utf16le" | b"utf-16le" => StringEncoding::Ucs2,
    b"ascii" => StringEncoding::Ascii,
    _ => StringEncoding::Utf8,
  }
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

/// Upper bound on the encoded byte length of `string` under `encoding`.
/// Mirrors Node's `StringBytes::StorageSize`. For UTF-8 this is the
/// exact length; for UCS-2 it's length() * 2; for Latin1/ASCII it's
/// length(). Used as the pre-allocation size for the shared backing
/// store that holds all encoded strings in a writev call.
fn encoded_storage_size(
  scope: &mut v8::PinScope,
  string: v8::Local<v8::String>,
  encoding: &StringEncoding,
) -> usize {
  match encoding {
    StringEncoding::Utf8 => string.utf8_length(scope),
    StringEncoding::Latin1 | StringEncoding::Ascii => string.length(),
    StringEncoding::Ucs2 => string.length() * 2,
  }
}

/// Encode `string` directly into pre-allocated uninit storage. Returns
/// the number of bytes written. The storage is typically a window
/// inside a V8 `BackingStore` that will be held alive by the write
/// request's retention list.
fn encode_string_into_uninit(
  scope: &mut v8::PinScope,
  string: v8::Local<v8::String>,
  encoding: StringEncoding,
  out: &mut [std::mem::MaybeUninit<u8>],
) -> usize {
  match encoding {
    StringEncoding::Utf8 => {
      let len = string.utf8_length(scope);
      string.write_utf8_uninit_v2(
        scope,
        &mut out[..len],
        v8::WriteFlags::kReplaceInvalidUtf8,
        None,
      )
    }
    StringEncoding::Latin1 | StringEncoding::Ascii => {
      let len = string.length();
      string.write_one_byte_uninit_v2(
        scope,
        0,
        &mut out[..len],
        v8::WriteFlags::empty(),
      );
      len
    }
    StringEncoding::Ucs2 => {
      let len_chars = string.length();
      let mut tmp = vec![0u16; len_chars];
      string.write_v2(scope, 0, &mut tmp, v8::WriteFlags::empty());
      for (i, &ch) in tmp.iter().enumerate() {
        let bytes = ch.to_le_bytes();
        out[i * 2] = std::mem::MaybeUninit::new(bytes[0]);
        out[i * 2 + 1] = std::mem::MaybeUninit::new(bytes[1]);
      }
      len_chars * 2
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
        owned_buffers: smallvec::SmallVec::new(),
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

    // Scatter-gather writev matching Node's StreamBase::Writev
    // (src/stream_base.cc:180).
    //
    // Retention strategy (matches Node):
    //   - Buffer chunks: iovec points directly at the ArrayBuffer
    //     backing store; NO per-chunk retention. JS side keeps chunks
    //     alive via `req.buffer = data` on the WriteWrap (see
    //     stream_base_commons.js `writevGeneric`).
    //   - String chunks: encode into stack storage first. Only
    //     allocate a V8 BackingStore if we have to go async, and
    //     only size it to the unwritten tail. Matches Node's stack
    //     storage optimization in WriteString and its single-alloc
    //     `ArrayBuffer::NewBackingStore` for the string storage.
    let array_len = chunks.length();
    let (count, stride): (u32, u32) = if all_buffers {
      (array_len, 1)
    } else {
      (array_len / 2, 2)
    };

    // Pre-pass: compute string storage size. Buffer chunks contribute
    // zero since they're zero-copied.
    let mut string_size: usize = 0;
    if !all_buffers {
      for i in 0..count {
        let Some(chunk) = chunks.get_index(scope, i * 2) else {
          continue;
        };
        if TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk).is_ok() {
          continue;
        }
        if let Ok(s) = TryInto::<v8::Local<v8::String>>::try_into(chunk) {
          let enc = chunks
            .get_index(scope, i * 2 + 1)
            .and_then(|v| TryInto::<v8::Local<v8::String>>::try_into(v).ok())
            .map(|e| parse_encoding_no_alloc(scope, e))
            .unwrap_or(StringEncoding::Utf8);
          string_size += encoded_storage_size(scope, s, &enc);
        }
      }
    }

    // Single owned `Vec<u8>` for the concat of all encoded strings.
    // Functional shape mirrors Node's `NewBackingStore(storage_size)`
    // at stream_base.cc:247 but stays outside V8 entirely — no
    // persistent handles, no GC-visible ArrayBuffer objects, just a
    // heap allocation.
    //
    // Allocate-with-capacity-then-set-len so the pointer is stable
    // (no realloc during encode) and we can hand raw `&mut
    // [MaybeUninit<u8>]` windows to the encoders without going
    // through `Vec::spare_capacity_mut` each time.
    let mut string_storage: Vec<u8> = if string_size > 0 {
      let mut v = Vec::with_capacity(string_size);
      // SAFETY: we reserved `string_size` bytes; the encoders write
      // into this range before we expose any slice. `set_len` here
      // claims the full capacity so `as_mut_ptr().add(offset)` is
      // valid for arbitrary `offset < string_size`. Bytes may be
      // uninit at this point, but we never read before writing.
      unsafe { v.set_len(string_size) };
      v
    } else {
      Vec::new()
    };
    let string_storage_ptr: *mut u8 = string_storage.as_mut_ptr();

    let mut iovecs: smallvec::SmallVec<[uv_buf_t; 16]> =
      smallvec::SmallVec::new();
    let mut str_offset: usize = 0;

    for i in 0..count {
      let Some(chunk) = chunks.get_index(scope, i * stride) else {
        continue;
      };
      if let Ok(buf) = TryInto::<v8::Local<v8::Uint8Array>>::try_into(chunk) {
        let byte_len = buf.byte_length();
        if byte_len == 0 {
          continue;
        }
        let byte_off = buf.byte_offset();
        let ab = buf.buffer(scope).unwrap();
        let Some(data_ptr) = ab.data() else {
          continue;
        };
        // SAFETY: data_ptr is the ArrayBuffer backing store start;
        // the Uint8Array view guarantees byte_off + byte_len is
        // within the allocation. JS retains the chunk via
        // `req.buffer = data` on the write request — we don't need
        // our own retention.
        let base = unsafe {
          (data_ptr.as_ptr() as *mut u8).add(byte_off) as *mut c_char
        };
        iovecs.push(uv_buf_t {
          base,
          len: byte_len,
        });
      } else if let Ok(s) = TryInto::<v8::Local<v8::String>>::try_into(chunk) {
        // Only reached in !all_buffers path.
        let enc = chunks
          .get_index(scope, i * 2 + 1)
          .and_then(|v| TryInto::<v8::Local<v8::String>>::try_into(v).ok())
          .map(|e| parse_encoding_no_alloc(scope, e))
          .unwrap_or(StringEncoding::Utf8);
        let remaining = string_size.saturating_sub(str_offset);
        if remaining == 0 {
          continue;
        }
        // SAFETY: string_storage_ptr is non-null (string_size > 0) and
        // points at a `string_size`-byte buffer (stack or cage-backed).
        let dst_slice = unsafe {
          std::slice::from_raw_parts_mut(
            string_storage_ptr.add(str_offset)
              as *mut std::mem::MaybeUninit<u8>,
            remaining,
          )
        };
        let written = encode_string_into_uninit(scope, s, enc, dst_slice);
        if written > 0 {
          // SAFETY: we wrote `written` bytes starting at str_offset.
          let base =
            unsafe { string_storage_ptr.add(str_offset) as *mut c_char };
          iovecs.push(uv_buf_t {
            base,
            len: written,
          });
        }
        str_offset += written;
      }
    }

    let total_bytes: usize = iovecs.iter().map(|b| b.len).sum();
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

    // Sync scatter-gather try-write. Mirrors Node's
    // StreamBase::Write → DoTryWrite pre-async path. When this fully
    // drains (common for small HTTP responses) we skip the async
    // request entirely — zero heap allocations.
    let (sync_written, fully_drained) = {
      let slices: smallvec::SmallVec<[std::io::IoSlice; 16]> = iovecs
        .iter()
        .map(|b| {
          // SAFETY: iovec bases point at live memory for `len` bytes.
          unsafe {
            std::io::IoSlice::new(std::slice::from_raw_parts(
              b.base as *const u8,
              b.len,
            ))
          }
        })
        .collect();
      // SAFETY: stream is a valid non-null uv_stream_t.
      let rc = unsafe { uv_compat::uv_try_writev(stream, &slices) };
      if rc >= 0 {
        let n = rc as usize;
        (n, n == total_bytes)
      } else {
        (0, false)
      }
    };

    if fully_drained {
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

    // Partial drain: slice iovecs in place, matching libuv's
    // DoTryWrite pointer advance (stream_wrap.cc:370-382).
    if sync_written > 0 {
      let mut remaining = sync_written;
      while remaining > 0 && !iovecs.is_empty() {
        let head_len = iovecs[0].len;
        if remaining >= head_len {
          remaining -= head_len;
          iovecs.remove(0);
        } else {
          // SAFETY: sliced within bounds.
          iovecs[0].base = unsafe {
            (iovecs[0].base as *mut u8).add(remaining) as *mut c_char
          };
          iovecs[0].len = head_len - remaining;
          remaining = 0;
        }
      }
    }

    // Own the string concat buffer on the async request's callback
    // state. Buffer chunks are retained JS-side via `req.buffer =
    // data`, so `owned_buffers` only holds the string storage (or
    // nothing at all if this writev was Buffers-only).
    let mut owned: smallvec::SmallVec<[Vec<u8>; 1]> = smallvec::SmallVec::new();
    if !string_storage.is_empty() {
      owned.push(string_storage);
    }

    self.do_writev_async(
      scope,
      stream,
      iovecs,
      owned,
      total_bytes,
      req_wrap_obj,
      state_array,
    )
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

      // Partial try_write — async write only the remaining bytes.
      // split_off gives us an owned Vec of the tail without extra
      // allocation beyond the single tail-copy.
      if try_result > 0 {
        let written = try_result as usize;
        let tail = data.split_off(written);
        return self.do_write(
          scope,
          stream,
          tail,
          total_bytes,
          req_wrap_obj,
          state_array,
        );
      }
    }

    // Full async write (no try_write or try_write returned error/0)
    self.do_write(scope, stream, data, total_bytes, req_wrap_obj, state_array)
  }

  /// Queue an owned `Vec<u8>` as a pending write. Takes `data` by move
  /// so the Vec can be threaded all the way down into the uv_compat
  /// write queue without a re-allocation + memcpy (the old path went
  /// through `uv_write(bufs, nbufs)` which re-collected the bufs into
  /// a new Vec via `collect_bufs`).
  fn do_write(
    &self,
    scope: &mut v8::PinScope,
    stream: *mut uv_stream_t,
    data: Vec<u8>,
    total_bytes: usize,
    req_wrap_obj: v8::Local<v8::Object>,
    state_array: v8::Local<v8::Int32Array>,
  ) -> i32 {
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
        owned_buffers: smallvec::SmallVec::new(),
      }),
    );
    let req_ptr = Box::into_raw(req);

    // SAFETY: req_ptr is a valid uv_write_t and stream is a valid
    // initialized stream handle (TCP/pipe/TTY). Use the polymorphic
    // `uv_write_owned` so pipe stdio (e.g. child.stdin) isn't
    // mis-cast to TCP and corrupted on push_back into the wrong
    // struct layout.
    let err = unsafe {
      uv_compat::uv_write_owned(
        req_ptr,
        stream as *mut _,
        data,
        Some(after_uv_write),
      )
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

  /// Queue an iovec-based async write. The `iovecs` point at memory
  /// kept alive either JS-side (for Buffer chunks attached to
  /// `req.buffer`) or by `owned_buffers` on the request's callback
  /// state (for the encoded-strings concat buffer). Mirrors Node's
  /// `LibuvStreamWrap::DoWrite(req_wrap, bufs, count, ...)`
  /// (stream_wrap.cc:391) — no intermediate concat.
  fn do_writev_async(
    &self,
    scope: &mut v8::PinScope,
    stream: *mut uv_stream_t,
    iovecs: smallvec::SmallVec<[uv_buf_t; 16]>,
    owned_buffers: smallvec::SmallVec<[Vec<u8>; 1]>,
    total_bytes: usize,
    req_wrap_obj: v8::Local<v8::Object>,
    state_array: v8::Local<v8::Int32Array>,
  ) -> i32 {
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
        owned_buffers,
      }),
    );
    let req_ptr = Box::into_raw(req);

    // Narrow iovec SmallVec to the queue's inline capacity (4). Node
    // uses `MaybeStackBuffer<uv_buf_t, 16>` for the caller stack and
    // libuv's queue stores the array pointer directly; we use a 4-
    // element inline here because most calls have few chunks.
    let queue_bufs: smallvec::SmallVec<[uv_buf_t; 4]> =
      iovecs.into_iter().collect();

    // SAFETY: req_ptr is a valid uv_write_t; stream is a valid
    // initialized stream handle; each iovec points at memory retained
    // via `retention` on the callback state, which stays alive until
    // `after_uv_write` runs.
    let err = unsafe {
      uv_compat::uv_writev_owned(
        req_ptr,
        stream,
        queue_bufs,
        Some(after_uv_write),
      )
    };

    if err != 0 {
      // SAFETY: `req_ptr` is still owned locally because `uv_writev_owned` failed synchronously.
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
      // SAFETY: uv_writev_owned failed so the callback will never fire; reclaim.
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
