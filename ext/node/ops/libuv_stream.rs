// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UvBuf;
use deno_core::uv_compat::UvConnect;
use deno_core::uv_compat::UvHandle;
use deno_core::uv_compat::UvLoop;
use deno_core::uv_compat::UvShutdown;
use deno_core::uv_compat::UvStream;
use deno_core::uv_compat::UvWrite;
use deno_core::v8;
use deno_permissions::PermissionsContainer;

use super::handle_wrap::AsyncId;

struct StreamHandleData {
  js_object: Option<v8::Global<v8::Object>>,
  read_buf: Vec<u8>,
}

// Wraps a UvWrite request together with the write data buffer
// so the data stays alive until the write callback fires.
#[repr(C)]
struct WriteReq {
  uv_req: UvWrite,
  _data: Vec<u8>,
}

unsafe fn context_from_loop(
  loop_ptr: *mut UvLoop,
) -> Option<v8::Local<'static, v8::Context>> {
  // SAFETY: NonNull<v8::Context> is layout-compatible with v8::Local<v8::Context>
  unsafe {
    let ctx_ptr = (*loop_ptr).data;
    if ctx_ptr.is_null() {
      return None;
    }
    Some(std::mem::transmute::<
      std::ptr::NonNull<v8::Context>,
      v8::Local<'_, v8::Context>,
    >(std::ptr::NonNull::new_unchecked(
      ctx_ptr as *mut v8::Context,
    )))
  }
}

unsafe extern "C" fn stream_alloc_cb(
  handle: *mut UvHandle,
  _suggested_size: usize,
  buf: *mut UvBuf,
) {
  // SAFETY: pointers are valid per libuv alloc callback contract
  unsafe {
    let data = (*handle).data as *mut StreamHandleData;
    if data.is_null() {
      (*buf).base = ptr::null_mut();
      (*buf).len = 0;
      return;
    }
    (*buf).base = (*data).read_buf.as_mut_ptr() as *mut _;
    (*buf).len = (*data).read_buf.len() as _;
  }
}

unsafe extern "C" fn stream_read_cb(
  stream: *mut UvStream,
  nread: isize,
  _buf: *const UvBuf,
) {
  // SAFETY: pointers are valid per libuv read callback contract
  unsafe {
    let data = (*stream).data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => {
        return;
      }
    };

    let context = match context_from_loop((*stream).loop_) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this: v8::Local<v8::Object> = v8::Local::new(scope, js_obj);

    let key = v8::String::new(scope, "onread").unwrap();
    let onread = this.get(scope, key.into());

    if let Some(Ok(func)) = onread.map(v8::Local::<v8::Function>::try_from) {
      let nread_val = v8::Integer::new(scope, nread as i32);

      if nread > 0 {
        let len = nread as usize;
        let store = v8::ArrayBuffer::new(scope, len);
        let backing = store.get_backing_store();
        let src = std::slice::from_raw_parts((*data).read_buf.as_ptr(), len);
        let dst = &backing[..len];
        for (i, byte) in src.iter().enumerate() {
          dst[i].set(*byte);
        }
        let buf = v8::Uint8Array::new(scope, store, 0, len).unwrap();
        func.call(scope, this.into(), &[nread_val.into(), buf.into()]);
      } else {
        let undefined = v8::undefined(scope);
        func.call(scope, this.into(), &[nread_val.into(), undefined.into()]);
      }
    }
  }
}

unsafe extern "C" fn server_connection_cb(server: *mut UvStream, status: i32) {
  // SAFETY: pointers are valid per libuv connection callback contract
  unsafe {
    let data = (*server).data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => return,
    };

    let context = match context_from_loop((*server).loop_) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this: v8::Local<v8::Object> = v8::Local::new(scope, js_obj);

    let key = v8::String::new(scope, "onconnection").unwrap();
    let onconnection = this.get(scope, key.into());

    if let Some(Ok(func)) =
      onconnection.map(v8::Local::<v8::Function>::try_from)
    {
      let status_val = v8::Integer::new(scope, status);
      func.call(scope, this.into(), &[status_val.into()]);
    }
  }
}

unsafe extern "C" fn write_cb(req: *mut UvWrite, _status: i32) {
  // SAFETY: pointer was allocated by Box::into_raw in write_buffer
  unsafe {
    // req is the first field of WriteReq (#[repr(C)]),
    // so the pointer is the same as the WriteReq pointer.
    let _ = Box::from_raw(req as *mut WriteReq);
  }
}

// Wraps a UvConnect request so it stays alive until the callback fires.
#[repr(C)]
struct ConnectReq {
  uv_req: UvConnect,
}

unsafe extern "C" fn connect_cb(req: *mut UvConnect, status: i32) {
  // SAFETY: pointers are valid per libuv connect callback contract
  unsafe {
    // The handle is the stream we connected on.
    let stream = (*req).handle as *mut UvStream;
    // Free the ConnectReq.
    let _ = Box::from_raw(req as *mut ConnectReq);

    if stream.is_null() {
      return;
    }
    let data = (*stream).data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => return,
    };

    let context = match context_from_loop((*stream).loop_) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this: v8::Local<v8::Object> = v8::Local::new(scope, js_obj);

    let key = v8::String::new(scope, "onconnect").unwrap();
    let onconnect = this.get(scope, key.into());

    if let Some(Ok(func)) = onconnect.map(v8::Local::<v8::Function>::try_from) {
      let status_val = v8::Integer::new(scope, status);
      func.call(scope, this.into(), &[status_val.into()]);
    }
  }
}

unsafe extern "C" fn shutdown_cb(req: *mut UvShutdown, status: i32) {
  // SAFETY: pointers are valid per libuv shutdown callback contract.
  // Signal JS via the `onshutdown` property so the stream layer
  // waits for writes to drain before closing the handle.
  unsafe {
    let stream = (*req).handle;
    let _ = Box::from_raw(req);

    if stream.is_null() {
      return;
    }
    let data = (*(stream as *mut UvHandle)).data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => return,
    };

    let context = match context_from_loop((*stream).loop_) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this: v8::Local<v8::Object> = v8::Local::new(scope, js_obj);
    let key = v8::String::new(scope, "onshutdown").unwrap();
    let onshutdown = this.get(scope, key.into());

    if let Some(Ok(func)) = onshutdown.map(v8::Local::<v8::Function>::try_from)
    {
      let status_val = v8::Integer::new(scope, status);
      func.call(scope, this.into(), &[status_val.into()]);
    }
  }
}

// ---------------------------------------------------------------------------
// NativePipe -- native pipe handle backed by uv_pipe_t
//
// Uses C callbacks (stream_alloc_cb, stream_read_cb, connect_cb,
// server_connection_cb) defined above.
// ---------------------------------------------------------------------------

type UvPipe = deno_core::uv_compat::uv_pipe_t;

unsafe extern "C" fn pipe_close_cb(handle: *mut UvHandle) {
  // SAFETY: pointer was allocated by Box::into_raw in NativePipe::new
  unsafe {
    let _ = Box::from_raw(handle as *mut UvPipe);
  }
}

pub struct NativePipe {
  handle: RefCell<*mut UvPipe>,
  provider: i32,
  async_id: i64,
  handle_data: RefCell<Option<Box<StreamHandleData>>>,
  closed: Cell<bool>,
  #[allow(dead_code, reason = "will be exposed via getter like TCP")]
  bytes_read: Cell<u64>,
  bytes_written: Cell<u64>,
}

// SAFETY: NativePipe is a cppgc-managed object traced by the GC.
unsafe impl GarbageCollected for NativePipe {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"NativePipe"
  }
}

impl NativePipe {
  fn init_handle(&self, state: &mut OpState, ipc: bool) {
    let loop_ptr: *mut UvLoop = &mut **state.borrow_mut::<Box<UvLoop>>();
    // SAFETY: loop_ptr and pipe are valid pointers
    unsafe {
      let pipe = Box::into_raw(Box::new(deno_core::uv_compat::new_pipe(ipc)));
      deno_core::uv_compat::uv_pipe_init(
        loop_ptr,
        pipe,
        if ipc { 1 } else { 0 },
      );
      *self.handle.borrow_mut() = pipe;
    }
  }

  fn raw(&self) -> *mut UvPipe {
    *self.handle.borrow()
  }

  fn stream(&self) -> *mut UvStream {
    self.raw() as *mut UvStream
  }

  fn set_js_object(&self, obj: v8::Global<v8::Object>) {
    if let Some(ref mut data) = *self.handle_data.borrow_mut() {
      data.js_object = Some(obj);
    }
  }
}

#[op2]
impl NativePipe {
  #[constructor]
  #[cppgc]
  fn new(state: &mut OpState, #[smi] pipe_type: i32) -> NativePipe {
    let async_id = state.borrow_mut::<AsyncId>().next();
    const PROVIDER_PIPEWRAP: i32 = 24;
    let ipc = pipe_type == 2;
    let pipe = NativePipe {
      handle: RefCell::new(ptr::null_mut()),
      provider: PROVIDER_PIPEWRAP,
      async_id,
      handle_data: RefCell::new(None),
      closed: Cell::new(false),
      bytes_read: Cell::new(0),
      bytes_written: Cell::new(0),
    };
    pipe.init_handle(state, ipc);

    // Create handle data and set on the uv handle for C callbacks.
    let handle_data = Box::new(StreamHandleData {
      js_object: None,
      read_buf: vec![0u8; 65536],
    });
    let data_ptr =
      &*handle_data as *const StreamHandleData as *mut StreamHandleData;
    // SAFETY: handle pointer is valid and initialized
    unsafe {
      (*(pipe.raw() as *mut UvHandle)).data = data_ptr as *mut c_void;
    }
    pipe.handle_data.replace(Some(handle_data));
    pipe
  }

  #[getter]
  fn provider(&self) -> i32 {
    self.provider
  }

  #[fast]
  fn get_async_id(&self) -> f64 {
    self.async_id as f64
  }

  #[fast]
  fn get_provider_type(&self) -> i32 {
    self.provider
  }

  #[getter]
  fn fd(&self) -> i32 {
    let pipe = self.raw();
    if pipe.is_null() {
      return -1;
    }
    #[cfg(unix)]
    {
      // SAFETY: pipe is valid.
      unsafe { &*pipe }.fd().unwrap_or(-1)
    }
    #[cfg(windows)]
    {
      -1
    }
  }

  #[nofast]
  fn set_owner(&self, #[this] this: v8::Global<v8::Object>) {
    self.set_js_object(this);
  }

  #[fast]
  fn open(&self, state: &mut OpState, #[smi] fd: i32) -> i32 {
    // Check FdTable for duplicate fds. Stdio fds (0-2) are pre-registered
    // as TableOwned; for those, Pipe.open is allowed (no-op check).
    // Non-stdio fds already in FdTable are rejected (EEXIST).
    {
      let fd_table = state.borrow::<deno_io::FdTable>();
      if fd_table.contains(fd) && !(0..=2).contains(&fd) {
        return -libc::EEXIST;
      }
    }
    // SAFETY: handle is valid
    let ret = unsafe {
      let pipe = self.raw();
      if pipe.is_null() {
        return uv_compat::UV_EBADF;
      }
      uv_compat::uv_pipe_open(pipe, fd)
    };
    if ret == 0 {
      // Register as UvOwned - the native handle owns the fd.
      state.borrow_mut::<deno_io::FdTable>().register_uv_owned(fd);
    }
    ret
  }

  #[fast]
  #[rename("pipeBindToPath")]
  fn pipe_bind_to_path(&self, #[string] path: &str) -> i32 {
    // SAFETY: handle is valid
    unsafe {
      let pipe = self.raw();
      if pipe.is_null() {
        return uv_compat::UV_EBADF;
      }
      uv_compat::uv_pipe_bind(pipe, path)
    }
  }

  #[nofast]
  fn listen(
    &self,
    state: &mut OpState,
    #[smi] backlog: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    // Permission check: verify the bind path is allowed.
    let pipe = self.raw();
    if !pipe.is_null() {
      // SAFETY: pipe is valid.
      if let Some(path) = unsafe { &*pipe }.bind_path() {
        state.borrow_mut::<PermissionsContainer>().check_open(
          std::borrow::Cow::Borrowed(std::path::Path::new(path)),
          deno_permissions::OpenAccessKind::ReadWriteNoFollow,
          Some("node:net.Server.listen()"),
        )?;
      }
    }
    // SAFETY: handle is valid
    Ok(unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return Ok(-1);
      }
      uv_compat::uv_pipe_listen(self.raw(), backlog, Some(server_connection_cb))
    })
  }

  #[fast]
  fn accept(&self, #[cppgc] client: &NativePipe) -> i32 {
    // SAFETY: both handles are valid
    unsafe { uv_compat::uv_pipe_accept(self.raw(), client.raw()) }
  }

  #[nofast]
  fn connect(
    &self,
    state: &mut OpState,
    #[string] path: &str,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    // Permission check: verify the connect path is allowed.
    state.borrow_mut::<PermissionsContainer>().check_open(
      std::borrow::Cow::Borrowed(std::path::Path::new(path)),
      deno_permissions::OpenAccessKind::ReadWriteNoFollow,
      Some("node:net.createConnection()"),
    )?;
    // SAFETY: handle is valid; ConnectReq freed in connect_cb
    Ok(unsafe {
      let pipe = self.raw();
      if pipe.is_null() {
        return Ok(uv_compat::UV_EBADF);
      }
      let req = Box::into_raw(Box::new(ConnectReq {
        uv_req: uv_compat::new_connect(),
      }));
      let ret = uv_compat::uv_pipe_connect(
        &mut (*req).uv_req as *mut UvConnect,
        pipe,
        path,
        Some(connect_cb),
      );
      if ret != 0 {
        let _ = Box::from_raw(req);
      }
      ret
    })
  }

  /// Set the number of pending pipe instances (Windows named pipes only).
  /// On Unix this is a no-op.
  #[fast]
  #[rename("setPendingInstances")]
  fn set_pending_instances(&self, #[smi] instances: i32) {
    let pipe = self.raw();
    if !pipe.is_null() {
      // SAFETY: pipe handle is valid.
      unsafe {
        deno_core::uv_compat::uv_pipe_set_pending_instances(pipe, instances);
      }
    }
  }

  /// Change permissions on the bound pipe path.
  #[fast]
  fn fchmod(&self, #[smi] mode: i32) -> i32 {
    #[cfg(unix)]
    {
      let pipe = self.raw();
      if pipe.is_null() {
        return uv_compat::UV_EBADF;
      }
      // SAFETY: pipe is valid.
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

  #[fast]
  fn read_start(&self) -> i32 {
    // SAFETY: stream pointer is valid
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      uv_compat::uv_read_start(
        stream,
        Some(stream_alloc_cb),
        Some(stream_read_cb),
      )
    }
  }

  #[fast]
  fn read_stop(&self) -> i32 {
    // SAFETY: stream pointer is valid
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      uv_compat::uv_read_stop(stream)
    }
  }

  fn write_buffer(&self, #[buffer] data: JsBuffer) -> i32 {
    // SAFETY: stream pointer is valid; WriteReq freed in write_cb
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      let data_vec = data.to_vec();
      let data_len = data_vec.len();
      let mut write_req = Box::new(WriteReq {
        uv_req: uv_compat::new_write(),
        _data: data_vec,
      });
      let buf = UvBuf {
        base: write_req._data.as_mut_ptr() as *mut _,
        len: data_len as _,
      };
      let req_ptr = &mut write_req.uv_req as *mut UvWrite;
      let _ = Box::into_raw(write_req);
      let ret = uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(write_cb));
      if ret != 0 {
        let _ = Box::from_raw(req_ptr as *mut WriteReq);
      } else {
        self
          .bytes_written
          .set(self.bytes_written.get() + data_len as u64);
      }
      ret
    }
  }

  #[fast]
  fn shutdown(&self) -> i32 {
    // SAFETY: stream pointer is valid; req freed in shutdown_cb
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      let req = Box::into_raw(Box::new(uv_compat::new_shutdown()));
      let ret = uv_compat::uv_shutdown(req, stream, Some(shutdown_cb));
      if ret != 0 {
        let _ = Box::from_raw(req);
      }
      ret
    }
  }

  #[fast]
  fn close(&self, state: &mut OpState) {
    if self.closed.get() {
      return;
    }
    self.closed.set(true);
    // Remove the UvOwned entry from FdTable.
    #[cfg(unix)]
    {
      let pipe = self.raw();
      if !pipe.is_null() {
        // SAFETY: pipe is valid, fd() is a safe public accessor.
        if let Some(fd) = unsafe { &*pipe }.fd() {
          state.borrow_mut::<deno_io::FdTable>().remove(fd);
        }
      }
    }
    let _ = &state; // suppress unused warning on Windows
    // SAFETY: handle is valid; freed in pipe_close_cb
    unsafe {
      let pipe = self.raw();
      if !pipe.is_null() {
        (*(pipe as *mut UvHandle)).data = ptr::null_mut();
        uv_compat::uv_close(pipe as *mut UvHandle, Some(pipe_close_cb));
      }
      *self.handle.borrow_mut() = ptr::null_mut();
    }
    self.handle_data.replace(None);
  }

  #[fast]
  #[rename("ref")]
  fn ref_method(&self) {
    let pipe = self.raw();
    // SAFETY: handle is valid
    unsafe {
      if !pipe.is_null() {
        uv_compat::uv_ref(pipe.cast());
      }
    }
  }

  #[fast]
  fn unref(&self) {
    let pipe = self.raw();
    // SAFETY: handle is valid
    unsafe {
      if !pipe.is_null() {
        uv_compat::uv_unref(pipe.cast());
      }
    }
  }

  #[fast]
  fn has_ref(&self) -> bool {
    let pipe = self.raw();
    // SAFETY: handle is valid
    unsafe {
      if !pipe.is_null() {
        uv_compat::uv_has_ref(pipe.cast()) != 0
      } else {
        false
      }
    }
  }
}
