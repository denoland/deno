// Copyright 2018-2026 the Deno authors. MIT license.
//
// PipeWrap -- pipe handle inheriting from LibUvStreamWrap.
//
// Follows the TCPWrap pattern: inherits read/write/shutdown from the base
// class, only implements pipe-specific ops (bind, listen, connect, accept,
// open, fchmod, setPendingInstances). Close is overridden to clear the
// FdTable entry registered by `open()`.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UvConnect;
use deno_core::uv_compat::UvLoop;
use deno_core::uv_compat::UvStream;
use deno_core::v8;
use deno_permissions::PermissionsContainer;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::Handle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::LibUvStreamWrap;
use crate::ops::stream_wrap::clone_context_from_uv_loop;

type UvPipe = uv_compat::uv_pipe_t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum PipeType {
  Socket = 0,
  Server = 1,
  Ipc = 2,
}

// -- libuv callbacks (called from the event loop) --

/// Macro to set up a v8 scope from a uv stream's handle data and call a JS
/// callback. The stream's `data` must point to a valid `StreamHandleData`.
///
/// # Safety
/// The caller must ensure `$stream` is a valid `uv_stream_t` pointer whose
/// `data` field points to a live `StreamHandleData` allocation.
macro_rules! with_js_handle {
  ($stream:expr, |$scope:ident, $this:ident| $body:block) => {{
    let Some(handle_data_ptr) = LibUvStreamWrap::stable_handle_data($stream)
    else {
      return;
    };
    // SAFETY: handle_data_ptr is non-null and points to a live StreamHandleData.
    let handle_data = unsafe { handle_data_ptr.as_ref() };
    // SAFETY: isolate pointer was stored during set_js_handle and is valid
    // for the lifetime of the stream.
    let isolate_ptr = unsafe { *handle_data.isolate.get() };
    if isolate_ptr.is_null() {
      return;
    }
    // SAFETY: isolate_ptr is a valid raw isolate pointer stored during
    // set_js_handle.
    let mut isolate = unsafe { v8::Isolate::from_raw_isolate_ptr(isolate_ptr) };
    // SAFETY: $stream is valid per caller contract; loop_ is set by libuv.
    let loop_ptr = unsafe { (*$stream).loop_ };
    // SAFETY: loop_ptr comes from a valid uv stream whose loop has been
    // registered with a V8 context.
    let context =
      unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
    v8::scope!(let handle_scope, &mut isolate);
    let context_local = v8::Local::new(handle_scope, context);
    let $scope = &mut v8::ContextScope::new(handle_scope, context_local);

    // SAFETY: js_handle was stored via set_js_handle and is valid while
    // the stream is alive.
    let Some(js_global) =
      (unsafe { (*handle_data.js_handle.get()).to_global($scope) })
    else {
      return;
    };
    let $this: v8::Local<v8::Object> = v8::Local::new($scope, js_global);
    $body
  }};
}

/// Connection callback for `uv_pipe_listen`. Fires
/// `this.onconnection(status)` on the server handle's JS object. The JS
/// `setupListenWrap` shim intercepts this to allocate a client PipeWrap
/// and call `accept` before forwarding to the user's onconnection.
///
/// # Safety
/// Must only be called by libuv as a `uv_connection_cb`. `server` must be
/// a valid `uv_stream_t` whose `data` points to a live `StreamHandleData`.
#[allow(
  unused_unsafe,
  clippy::undocumented_unsafe_blocks,
  reason = "macro expands unsafe blocks inside unsafe fn"
)]
unsafe extern "C" fn server_connection_cb(server: *mut UvStream, status: i32) {
  with_js_handle!(server, |scope, this| {
    let key = v8::String::new(scope, "onconnection").unwrap();
    if let Some(onconnection) = this.get(scope, key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(onconnection)
    {
      let status_val: v8::Local<v8::Value> =
        v8::Integer::new(scope, status).into();
      func.call(scope, this.into(), &[status_val]);
    }
  });
}

// Wraps a UvConnect request together with the JS req object
// (PipeConnectWrap) so both stay alive until the callback fires.
#[repr(C)]
struct ConnectReqData {
  uv_req: UvConnect,
  js_req: v8::Global<v8::Object>,
}

/// Connect callback for `uv_pipe_connect`. Fires `req.oncomplete(status,
/// handle, req, readable, writable)` matching Node.js
/// ConnectionWrap::AfterConnect.
///
/// # Safety
/// Must only be called by libuv as a `uv_connect_cb`. `req` must point to
/// a `ConnectReqData` allocated via `Box::into_raw`.
#[allow(
  unused_unsafe,
  clippy::undocumented_unsafe_blocks,
  reason = "macro expands unsafe blocks inside unsafe fn"
)]
unsafe extern "C" fn connect_cb(req: *mut UvConnect, status: i32) {
  // SAFETY: req points to a ConnectReqData allocated via Box::into_raw
  // in the connect() op. We reclaim ownership here.
  let stream = unsafe { (*req).handle as *mut UvStream };
  let req_data = unsafe { Box::from_raw(req as *mut ConnectReqData) };
  let js_req_global = req_data.js_req;

  with_js_handle!(stream, |scope, this| {
    let js_req = v8::Local::new(scope, &js_req_global);
    let oncomplete_key = v8::String::new(scope, "oncomplete").unwrap();
    if let Some(oncomplete) = js_req.get(scope, oncomplete_key.into())
      && let Ok(func) = v8::Local::<v8::Function>::try_from(oncomplete)
    {
      let status_val: v8::Local<v8::Value> =
        v8::Integer::new(scope, status).into();
      let readable: v8::Local<v8::Value> =
        v8::Boolean::new(scope, status == 0).into();
      let writable: v8::Local<v8::Value> =
        v8::Boolean::new(scope, status == 0).into();
      func.call(
        scope,
        js_req.into(),
        &[status_val, this.into(), js_req.into(), readable, writable],
      );
    }
  });
}

// -- PipeWrap struct --

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct PipeWrap {
  base: LibUvStreamWrap,
  handle: Option<OwnedPtr<UvPipe>>,
  #[allow(dead_code, reason = "stored for parity with TCPWrap::socket_type")]
  pipe_type: Cell<PipeType>,
}

// SAFETY: PipeWrap is a cppgc-managed object; the GC traces it via the base field.
unsafe impl GarbageCollected for PipeWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Pipe"
  }

  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl Drop for PipeWrap {
  fn drop(&mut self) {
    self.base.detach_stream();
  }
}

impl PipeWrap {
  fn new(pipe_type: PipeType, op_state: &mut OpState) -> Self {
    let loop_ =
      &**op_state.borrow::<Box<UvLoop>>() as *const UvLoop as *mut UvLoop;
    let ipc = pipe_type == PipeType::Ipc;
    let pipe = OwnedPtr::from_box(Box::new(uv_compat::new_pipe(ipc)));
    // SAFETY: loop_ and pipe are valid pointers for uv_pipe_init.
    // uv_pipe_init always returns 0 (no error path).
    unsafe {
      uv_compat::uv_pipe_init(loop_, pipe.as_mut_ptr(), ipc as i32);
    }

    let provider = match pipe_type {
      PipeType::Server => ProviderType::PipeServerWrap,
      _ => ProviderType::PipeWrap,
    };

    let base = LibUvStreamWrap::new(
      HandleWrap::create(
        AsyncWrap::create(op_state, provider as i32),
        Some(Handle::New(pipe.as_ptr().cast())),
      ),
      -1,
      pipe.as_ptr().cast(),
    );

    // SAFETY: pipe pointer is valid; setting data field for libuv callbacks.
    unsafe {
      (*pipe.as_mut_ptr()).data = base.handle_data_ptr();
    }

    Self {
      base,
      handle: Some(pipe),
      pipe_type: Cell::new(pipe_type),
    }
  }

  fn pipe_ptr(&self) -> *mut UvPipe {
    match &self.handle {
      Some(h) => h.as_mut_ptr(),
      None => std::ptr::null_mut(),
    }
  }

  /// Get the underlying uv_stream_t pointer. Used by TLSWrap to attach
  /// to the pipe stream for encrypted I/O.
  pub fn stream_ptr(&self) -> *mut uv_compat::uv_stream_t {
    self.base.stream_ptr()
  }
}

// -- ops --

#[op2(inherit = LibUvStreamWrap)]
impl PipeWrap {
  #[constructor]
  #[cppgc]
  fn new_pipe(
    #[smi] pipe_type: i32,
    op_state: &mut OpState,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> PipeWrap {
    let pt = match pipe_type {
      1 => PipeType::Server,
      2 => PipeType::Ipc,
      _ => PipeType::Socket,
    };
    let pipe = PipeWrap::new(pt, op_state);
    // Store the JS handle so callbacks (connect, read, etc.) can find it.
    pipe.base.set_js_handle(this, scope);
    pipe
  }

  #[getter]
  fn fd(&self) -> i32 {
    let pipe = self.pipe_ptr();
    if pipe.is_null() {
      return -1;
    }
    #[cfg(unix)]
    {
      // SAFETY: pipe is valid (null-checked above).
      unsafe { &*pipe }.fd().unwrap_or(-1)
    }
    #[cfg(windows)]
    {
      -1
    }
  }

  #[fast]
  fn open(&self, state: &mut OpState, #[smi] fd: i32) -> i32 {
    // Check FdTable for duplicate fds. Stdio fds (0-2) are pre-registered
    // as TableOwned; for those, open is allowed (no-op check). Non-stdio
    // fds already in FdTable are rejected (EEXIST).
    {
      let fd_table = state.borrow::<deno_io::FdTable>();
      if fd_table.contains(fd) && !(0..=2).contains(&fd) {
        return -libc::EEXIST;
      }
    }
    let pipe = self.pipe_ptr();
    if pipe.is_null() {
      return uv_compat::UV_EBADF;
    }
    // SAFETY: pipe is valid (null-checked above).
    let ret = unsafe { uv_compat::uv_pipe_open(pipe, fd) };
    if ret == 0 {
      // Register as UvOwned - the native handle owns the fd.
      state.borrow_mut::<deno_io::FdTable>().register_uv_owned(fd);
      self.base.set_fd(fd);
    }
    ret
  }

  #[fast]
  fn bind(&self, #[string] path: &str) -> i32 {
    let pipe = self.pipe_ptr();
    if pipe.is_null() {
      return uv_compat::UV_EBADF;
    }
    // SAFETY: pipe is valid (null-checked above).
    unsafe { uv_compat::uv_pipe_bind(pipe, path) }
  }

  #[nofast]
  fn listen(
    &self,
    state: &mut OpState,
    #[smi] backlog: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    let pipe = self.pipe_ptr();
    if pipe.is_null() {
      return Ok(uv_compat::UV_EBADF);
    }
    // Permission check: verify the bind path is allowed.
    // SAFETY: pipe is valid (null-checked above).
    if let Some(path) = unsafe { &*pipe }.bind_path() {
      state.borrow_mut::<PermissionsContainer>().check_open(
        std::borrow::Cow::Borrowed(std::path::Path::new(path)),
        deno_permissions::OpenAccessKind::ReadWriteNoFollow,
        Some("node:net.Server.listen()"),
      )?;
    }
    // SAFETY: pipe is valid (null-checked above).
    Ok(unsafe {
      uv_compat::uv_pipe_listen(pipe, backlog, Some(server_connection_cb))
    })
  }

  #[fast]
  fn accept(&self, #[cppgc] client: &PipeWrap) -> i32 {
    let server = self.pipe_ptr();
    let client_pipe = client.pipe_ptr();
    if server.is_null() || client_pipe.is_null() {
      return uv_compat::UV_EBADF;
    }
    // SAFETY: both pipe pointers are valid (null-checked above).
    unsafe { uv_compat::uv_pipe_accept(server, client_pipe) }
  }

  /// Connect to a path. Takes (req, path) where req is a PipeConnectWrap
  /// with oncomplete callback, matching Node.js ConnectionWrap::AfterConnect.
  #[nofast]
  fn connect(
    &self,
    state: &mut OpState,
    js_req: v8::Local<v8::Object>,
    #[string] path: &str,
    scope: &mut v8::PinScope,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    state.borrow_mut::<PermissionsContainer>().check_open(
      std::borrow::Cow::Borrowed(std::path::Path::new(path)),
      deno_permissions::OpenAccessKind::ReadWriteNoFollow,
      Some("node:net.createConnection()"),
    )?;

    let pipe = self.pipe_ptr();
    if pipe.is_null() {
      return Ok(uv_compat::UV_EBADF);
    }
    let js_req_global = v8::Global::new(scope, js_req);
    let mut connect_req = Box::new(ConnectReqData {
      uv_req: uv_compat::new_connect(),
      js_req: js_req_global,
    });
    let req_ptr = &mut connect_req.uv_req as *mut UvConnect;
    let _ = Box::into_raw(connect_req);
    // SAFETY: pipe is valid (null-checked above); req_ptr is a valid
    // heap-allocated UvConnect. connect_req is leaked and will be
    // reclaimed in connect_cb.
    let ret = unsafe {
      uv_compat::uv_pipe_connect(req_ptr, pipe, path, Some(connect_cb))
    };
    if ret != 0 {
      // SAFETY: uv_pipe_connect failed synchronously; reclaim the request.
      unsafe {
        let _ = Box::from_raw(req_ptr as *mut ConnectReqData);
      }
    }
    Ok(ret)
  }

  /// Set the number of pending pipe instances (Windows named pipes only).
  /// On Unix this is a no-op.
  #[fast]
  #[rename("setPendingInstances")]
  fn set_pending_instances(&self, #[smi] instances: i32) {
    let pipe = self.pipe_ptr();
    if !pipe.is_null() {
      // SAFETY: pipe is valid (null-checked above).
      unsafe { uv_compat::uv_pipe_set_pending_instances(pipe, instances) };
    }
  }

  /// Change permissions on the bound pipe path. Takes already-translated
  /// POSIX mode bits; the JS wrapper translates UV_READABLE/UV_WRITABLE.
  #[fast]
  fn fchmod(&self, #[smi] mode: i32) -> i32 {
    #[cfg(unix)]
    {
      let pipe = self.pipe_ptr();
      if pipe.is_null() {
        return uv_compat::UV_EBADF;
      }
      // SAFETY: pipe is valid (null-checked above).
      if let Some(path) = unsafe { &*pipe }.bind_path() {
        let c_path = match std::ffi::CString::new(path) {
          Ok(p) => p,
          Err(_) => return uv_compat::UV_EINVAL,
        };
        // SAFETY: c_path is a valid null-terminated C string.
        if unsafe { libc::chmod(c_path.as_ptr(), mode as libc::mode_t) } != 0 {
          return -1;
        }
        0
      } else {
        uv_compat::UV_EBADF
      }
    }
    #[cfg(windows)]
    {
      let _ = mode;
      // Windows named pipes don't support chmod.
      0
    }
  }

  /// Override the base's close to remove the FdTable entry registered by
  /// `open()` before the kernel frees the fd via `uv_close`.
  #[reentrant]
  fn close(
    &self,
    op_state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[scoped] cb: Option<v8::Global<v8::Function>>,
  ) -> Result<(), ResourceError> {
    #[cfg(unix)]
    {
      let pipe = self.pipe_ptr();
      if !pipe.is_null() {
        // SAFETY: pipe is valid (null-checked above).
        if let Some(fd) = unsafe { &*pipe }.fd() {
          op_state
            .borrow_mut()
            .borrow_mut::<deno_io::FdTable>()
            .remove(fd);
        }
      }
    }
    self.base.clear_js_handle();
    self
      .base
      .handle_wrap()
      .close_handle(op_state, this, scope, cb)
  }
}
