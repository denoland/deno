// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::net::ToSocketAddrs;

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::UV_EINVAL;
use deno_core::uv_compat::uv_accept;
use deno_core::uv_compat::uv_connect_t;
use deno_core::uv_compat::uv_listen;
use deno_core::uv_compat::uv_loop_t;
use deno_core::uv_compat::uv_tcp_bind;
use deno_core::uv_compat::uv_tcp_connect;
use deno_core::uv_compat::uv_tcp_getpeername;
use deno_core::uv_compat::uv_tcp_getsockname;
use deno_core::uv_compat::uv_tcp_init;
use deno_core::uv_compat::uv_tcp_keepalive;
use deno_core::uv_compat::uv_tcp_nodelay;
use deno_core::uv_compat::uv_tcp_open;
use deno_core::uv_compat::uv_tcp_t;
use deno_core::v8;
use deno_permissions::PermissionsContainer;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::Handle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::LibUvStreamWrap;

/// Socket type constants matching Node's TCPWrap::SocketType.
const SERVER: i32 = 1;

/// UV_TCP_IPV6ONLY flag value matching libuv.
const UV_TCP_IPV6ONLY: u32 = 1;

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct TCPWrap {
  pub(crate) base: LibUvStreamWrap,
  // UnsafeCell because `detach()` needs to take the handle via `&self`
  // (CppGC objects are always accessed through shared references).
  // SAFETY: single-threaded access only.
  pub(crate) handle: UnsafeCell<Option<OwnedPtr<uv_tcp_t>>>,
}

// SAFETY: TCPWrap correctly traces its CppGc member (base) in the trace method.
unsafe impl GarbageCollected for TCPWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TCPWrap"
  }

  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl Drop for TCPWrap {
  fn drop(&mut self) {
    // Detach the stream data pointer so pending uv callbacks see null
    // and bail out instead of dereferencing freed StreamHandleData.
    self.base.detach_stream();
  }
}

impl TCPWrap {
  /// Access the handle through the UnsafeCell.
  /// SAFETY: single-threaded access only (CppGC guarantee).
  fn handle(&self) -> Option<&OwnedPtr<uv_tcp_t>> {
    // SAFETY: single-threaded access only (CppGC guarantee).
    unsafe { (*self.handle.get()).as_ref() }
  }

  pub fn new(fd: i32, socket_type: i32, op_state: &mut OpState) -> (Self, i32) {
    let loop_ = &**op_state.borrow::<Box<uv_loop_t>>() as *const uv_loop_t
      as *mut uv_loop_t;

    let tcp = OwnedPtr::from_box(Box::<uv_tcp_t>::new_uninit());

    // SAFETY: libuv call, valid (albeit uninitialized) pointer to uv_tcp_t
    let err = unsafe { uv_tcp_init(loop_, tcp.as_mut_ptr().cast()) };

    let provider = if socket_type == SERVER {
      ProviderType::TcpServerWrap
    } else {
      ProviderType::TcpWrap
    };

    if err == 0 {
      // SAFETY: uv_tcp_init succeeded (err == 0), so the uv_tcp_t is fully initialized.
      let tcp = unsafe { tcp.cast::<uv_tcp_t>() };
      let base = LibUvStreamWrap::new(
        HandleWrap::create(
          AsyncWrap::create(op_state, provider as i32),
          Some(Handle::New(tcp.as_ptr().cast())),
        ),
        fd,
        tcp.as_ptr().cast(),
      );
      // SAFETY: tcp pointer is valid and initialized; setting data field for libuv callbacks.
      unsafe {
        (*tcp.as_mut_ptr()).data = base.handle_data_ptr();
      }
      (
        Self {
          base,
          handle: UnsafeCell::new(Some(tcp)),
        },
        0,
      )
    } else {
      // Just drop tcp — it's an OwnedPtr<MaybeUninit<uv_tcp_t>>,
      // and MaybeUninit's drop is a no-op, so this safely frees the allocation.
      drop(tcp);
      (
        Self {
          base: LibUvStreamWrap::new(
            HandleWrap::create(
              AsyncWrap::create(op_state, provider as i32),
              None,
            ),
            fd,
            std::ptr::null(),
          ),
          handle: UnsafeCell::new(None),
        },
        err,
      )
    }
  }
}

/// Helper to convert address info from a sockaddr into JS object properties.
fn address_to_js(
  scope: &mut v8::PinScope<'_, '_>,
  sock_addr: &socket2::SockAddr,
  obj: v8::Local<v8::Object>,
) -> bool {
  if let Some(addr) = sock_addr.as_socket_ipv4() {
    let address_key =
      v8::String::new_external_onebyte_static(scope, b"address").unwrap();
    let address_val = v8::String::new(scope, &addr.ip().to_string()).unwrap();
    obj.set(scope, address_key.into(), address_val.into());

    let family_key =
      v8::String::new_external_onebyte_static(scope, b"family").unwrap();
    let family_val =
      v8::String::new_external_onebyte_static(scope, b"IPv4").unwrap();
    obj.set(scope, family_key.into(), family_val.into());

    let port_key =
      v8::String::new_external_onebyte_static(scope, b"port").unwrap();
    let port_val = v8::Integer::new(scope, addr.port() as i32);
    obj.set(scope, port_key.into(), port_val.into());

    true
  } else if let Some(addr) = sock_addr.as_socket_ipv6() {
    let address_key =
      v8::String::new_external_onebyte_static(scope, b"address").unwrap();
    let address_val = v8::String::new(scope, &addr.ip().to_string()).unwrap();
    obj.set(scope, address_key.into(), address_val.into());

    let family_key =
      v8::String::new_external_onebyte_static(scope, b"family").unwrap();
    let family_val =
      v8::String::new_external_onebyte_static(scope, b"IPv6").unwrap();
    obj.set(scope, family_key.into(), family_val.into());

    let port_key =
      v8::String::new_external_onebyte_static(scope, b"port").unwrap();
    let port_val = v8::Integer::new(scope, addr.port() as i32);
    obj.set(scope, port_key.into(), port_val.into());

    true
  } else {
    false
  }
}

/// Wraps a uv_connect_t request so it stays alive until the callback fires.
#[repr(C)]
struct ConnectReq {
  uv_req: uv_connect_t,
}

unsafe extern "C" fn after_connect(req: *mut uv_connect_t, status: i32) {
  // SAFETY: pointer was allocated by Box::into_raw in connect/connect6
  unsafe {
    let stream = (*req).handle;
    // Free the ConnectReq
    let _ = Box::from_raw(req as *mut ConnectReq);

    if stream.is_null() {
      return;
    }
    let data = (*stream).data;
    if data.is_null() {
      return;
    }

    let loop_ = (*stream).loop_;
    let ctx_ptr = (*loop_).data;
    if ctx_ptr.is_null() {
      return;
    }

    let handle_data =
      &*(data as *const crate::ops::stream_wrap::StreamHandleData);
    let isolate_ptr = *handle_data.isolate.get();
    if isolate_ptr.is_null() {
      return;
    }

    let mut isolate = v8::Isolate::from_raw_isolate_ptr(isolate_ptr);
    v8::scope!(let handle_scope, &mut isolate);
    let raw = std::ptr::NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
    let global = v8::Global::from_raw(handle_scope, raw);
    let cloned = global.clone();
    global.into_raw();
    let context = v8::Local::new(handle_scope, cloned);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let js_obj = match &*handle_data.js_handle.get() {
      crate::ops::handle_wrap::GlobalHandle::Strong(global) => {
        v8::Local::new(scope, global)
      }
      crate::ops::handle_wrap::GlobalHandle::Weak(weak) => {
        match weak.to_local(scope) {
          Some(local) => local,
          None => return,
        }
      }
      crate::ops::handle_wrap::GlobalHandle::None => return,
    };

    let key =
      v8::String::new_external_onebyte_static(scope, b"onconnect").unwrap();
    let onconnect = js_obj.get(scope, key.into());

    if let Some(Ok(func)) = onconnect.map(v8::Local::<v8::Function>::try_from) {
      let status_val = v8::Integer::new(scope, status);
      func.call(scope, js_obj.into(), &[status_val.into()]);
    }
  }
}

unsafe extern "C" fn on_connection(
  server: *mut deno_core::uv_compat::uv_stream_t,
  status: i32,
) {
  // SAFETY: pointers are valid per libuv connection callback contract
  unsafe {
    let data = (*server).data;
    if data.is_null() {
      return;
    }

    let loop_ = (*server).loop_;
    let ctx_ptr = (*loop_).data;
    if ctx_ptr.is_null() {
      return;
    }

    let handle_data =
      &*(data as *const crate::ops::stream_wrap::StreamHandleData);
    let isolate_ptr = *handle_data.isolate.get();
    if isolate_ptr.is_null() {
      return;
    }

    let mut isolate = v8::Isolate::from_raw_isolate_ptr(isolate_ptr);
    v8::scope!(let handle_scope, &mut isolate);
    let raw = std::ptr::NonNull::new_unchecked(ctx_ptr as *mut v8::Context);
    let global = v8::Global::from_raw(handle_scope, raw);
    let cloned = global.clone();
    global.into_raw();
    let context = v8::Local::new(handle_scope, cloned);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let js_obj = match &*handle_data.js_handle.get() {
      crate::ops::handle_wrap::GlobalHandle::Strong(global) => {
        v8::Local::new(scope, global)
      }
      crate::ops::handle_wrap::GlobalHandle::Weak(weak) => {
        match weak.to_local(scope) {
          Some(local) => local,
          None => return,
        }
      }
      crate::ops::handle_wrap::GlobalHandle::None => return,
    };

    let key =
      v8::String::new_external_onebyte_static(scope, b"onconnection").unwrap();
    let onconnection = js_obj.get(scope, key.into());

    if let Some(Ok(func)) =
      onconnection.map(v8::Local::<v8::Function>::try_from)
    {
      let status_val = v8::Integer::new(scope, status);
      func.call(scope, js_obj.into(), &[status_val.into()]);
    }
  }
}

#[op2(inherit = LibUvStreamWrap)]
impl TCPWrap {
  #[constructor]
  #[cppgc]
  pub fn new_tcp(
    #[smi] socket_type: i32,
    ctx: v8::Local<v8::Value>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> TCPWrap {
    let (tcp, err) = TCPWrap::new(-1, socket_type, op_state);
    // Store the JS object reference so libuv callbacks can call back into JS.
    tcp.base.set_js_handle(this, scope);
    if err != 0
      && let Ok(ctx_obj) = v8::Local::<v8::Object>::try_from(ctx)
    {
      let (code_name, message) = uv_error_info(err);

      let code_key =
        v8::String::new_external_onebyte_static(scope, b"code").unwrap();
      let code_str = v8::String::new(scope, code_name).unwrap();
      ctx_obj.set(scope, code_key.into(), code_str.into());

      let msg_key =
        v8::String::new_external_onebyte_static(scope, b"message").unwrap();
      let msg_str = v8::String::new(scope, message).unwrap();
      ctx_obj.set(scope, msg_key.into(), msg_str.into());

      let errno_key =
        v8::String::new_external_onebyte_static(scope, b"errno").unwrap();
      let errno_val = v8::Integer::new(scope, err);
      ctx_obj.set(scope, errno_key.into(), errno_val.into());

      let syscall_key =
        v8::String::new_external_onebyte_static(scope, b"syscall").unwrap();
      let syscall_str =
        v8::String::new_external_onebyte_static(scope, b"uv_tcp_init").unwrap();
      ctx_obj.set(scope, syscall_key.into(), syscall_str.into());
    }
    tcp
  }

  /// Detach the native handle so that this TCPWrap no longer owns it.
  /// Called by HTTP/2 after `consumeStream` transfers ownership of the
  /// underlying uv_tcp_t to the HTTP/2 session.
  #[fast]
  pub fn detach(&self) {
    // SAFETY: single-threaded access (CppGC guarantee).
    // Take the OwnedPtr out so Drop doesn't free memory that
    // the HTTP/2 session now owns. Also null the base stream pointer
    // so Drop::detach_stream() is a no-op (HTTP/2 has already
    // overwritten stream.data with its own session pointer).
    unsafe {
      // Take the OwnedPtr out and forget it (leak the memory).
      // The HTTP/2 session now owns the uv_tcp_t — we must NOT
      // drop the OwnedPtr which would free the memory.
      if let Some(owned) = (*self.handle.get()).take() {
        std::mem::forget(owned);
      }
    }
    // Null the base stream pointer so Drop::detach_stream() is a no-op.
    // HTTP/2's consume_stream has already overwritten stream.data with
    // its session pointer, so we must not touch it.
    self.base.forget_stream();
  }

  #[fast]
  pub fn open(&self, #[smi] fd: i32) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };
    // SAFETY: handle is valid, initialized by uv_tcp_init in constructor.
    let err = unsafe { uv_tcp_open(handle.as_mut_ptr(), fd) };
    if err == 0 {
      self.base.set_fd(fd);
    }
    err
  }

  /// Open the TCP handle from an existing Deno resource ID.
  /// Used by HTTP/2 to transfer a Deno TcpConn resource into the
  /// native libuv TCP handle.
  #[fast]
  pub fn open_from_rid(
    &self,
    op_state: &mut OpState,
    #[smi] rid: deno_core::ResourceId,
  ) -> i32 {
    let Some(handle) = self.handle() else {
      return -1;
    };
    let fd = op_state
      .resource_table
      .get::<deno_net::io::TcpStreamResource>(rid)
      .ok()
      .and_then(|r| r.dup_raw_fd());
    match fd {
      // SAFETY: handle is valid (checked above), fd is a valid dup'd descriptor.
      Some(fd) => unsafe { uv_tcp_open(handle.as_mut_ptr(), fd) },
      None => -1,
    }
  }

  pub fn bind(
    &self,
    op_state: std::rc::Rc<std::cell::RefCell<OpState>>,
    #[string] address: &str,
    #[smi] port: i32,
    #[smi] flags: Option<u32>,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    op_state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.listen()")?;

    let Some(handle) = self.handle() else {
      return Ok(UV_EBADF);
    };

    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(UV_EINVAL),
      },
      Err(_) => return Ok(UV_EINVAL),
    };

    let flags = flags.unwrap_or(0);
    // Cannot set IPV6 flags on IPv4 socket
    let flags = flags & !UV_TCP_IPV6ONLY;

    // SAFETY: handle is a valid uv_tcp_t; sock_addr is valid for the duration of the call.
    Ok(unsafe {
      let sock_addr = socket2::SockAddr::from(socket_addr);
      uv_tcp_bind(
        handle.as_mut_ptr(),
        sock_addr.as_ptr() as *const c_void,
        #[allow(
          clippy::unnecessary_cast,
          reason = "socklen_t may not be u32 on all platforms"
        )]
        {
          sock_addr.len() as u32
        },
        flags,
      )
    })
  }

  pub fn bind6(
    &self,
    op_state: std::rc::Rc<std::cell::RefCell<OpState>>,
    #[string] address: &str,
    #[smi] port: i32,
    #[smi] flags: Option<u32>,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    op_state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.listen()")?;

    let Some(handle) = self.handle() else {
      return Ok(UV_EBADF);
    };

    let addr_str = format!("[{}]:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(UV_EINVAL),
      },
      Err(_) => return Ok(UV_EINVAL),
    };

    let flags = flags.unwrap_or(0);

    // SAFETY: handle is a valid uv_tcp_t; sock_addr is valid for the duration of the call.
    Ok(unsafe {
      let sock_addr = socket2::SockAddr::from(socket_addr);
      uv_tcp_bind(
        handle.as_mut_ptr(),
        sock_addr.as_ptr() as *const c_void,
        #[allow(
          clippy::unnecessary_cast,
          reason = "socklen_t may not be u32 on all platforms"
        )]
        {
          sock_addr.len() as u32
        },
        flags,
      )
    })
  }

  #[fast]
  pub fn listen(&self, #[smi] backlog: i32) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };

    // SAFETY: handle is a valid uv_tcp_t which can be cast to uv_stream_t for uv_listen.
    unsafe {
      uv_listen(
        handle.as_mut_ptr() as *mut deno_core::uv_compat::uv_stream_t,
        backlog,
        Some(on_connection),
      )
    }
  }

  #[fast]
  pub fn connect(
    &self,
    op_state: std::rc::Rc<std::cell::RefCell<OpState>>,
    #[string] address: &str,
    #[smi] port: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    op_state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.connect()")?;

    let Some(handle) = self.handle() else {
      return Ok(UV_EBADF);
    };

    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(UV_EINVAL),
      },
      Err(_) => return Ok(UV_EINVAL),
    };

    // SAFETY: handle is valid; ConnectReq is heap-allocated and freed in after_connect callback.
    // mem::zeroed is safe for uv_connect_t (plain C struct). On error, req is reclaimed immediately.
    Ok(unsafe {
      let sock_addr = socket2::SockAddr::from(socket_addr);
      let mut connect_req = Box::new(ConnectReq {
        uv_req: std::mem::zeroed(),
      });
      let req_ptr = &mut connect_req.uv_req as *mut uv_connect_t;
      let _ = Box::into_raw(connect_req); // leak; freed in after_connect
      let ret = uv_tcp_connect(
        req_ptr,
        handle.as_mut_ptr(),
        sock_addr.as_ptr() as *const c_void,
        Some(after_connect),
      );
      if ret != 0 {
        let _ = Box::from_raw(req_ptr as *mut ConnectReq);
      }
      ret
    })
  }

  #[fast]
  pub fn connect6(
    &self,
    op_state: std::rc::Rc<std::cell::RefCell<OpState>>,
    #[string] address: &str,
    #[smi] port: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    op_state
      .borrow_mut()
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.connect()")?;

    let Some(handle) = self.handle() else {
      return Ok(UV_EBADF);
    };

    let addr_str = format!("[{}]:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(UV_EINVAL),
      },
      Err(_) => return Ok(UV_EINVAL),
    };

    // SAFETY: handle is valid; ConnectReq is heap-allocated and freed in after_connect callback.
    // mem::zeroed is safe for uv_connect_t (plain C struct). On error, req is reclaimed immediately.
    Ok(unsafe {
      let sock_addr = socket2::SockAddr::from(socket_addr);
      let mut connect_req = Box::new(ConnectReq {
        uv_req: std::mem::zeroed(),
      });
      let req_ptr = &mut connect_req.uv_req as *mut uv_connect_t;
      let _ = Box::into_raw(connect_req); // leak; freed in after_connect
      let ret = uv_tcp_connect(
        req_ptr,
        handle.as_mut_ptr(),
        sock_addr.as_ptr() as *const c_void,
        Some(after_connect),
      );
      if ret != 0 {
        let _ = Box::from_raw(req_ptr as *mut ConnectReq);
      }
      ret
    })
  }

  #[fast]
  pub fn getsockname(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };

    // SAFETY: handle is valid; storage is written by uv_tcp_getsockname before assume_init.
    unsafe {
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_tcp_getsockname(
        handle.as_ptr(),
        storage.as_mut_ptr() as *mut c_void,
        &mut len,
      );
      if ret != 0 {
        return ret;
      }
      let sock_addr = storage.assume_init();
      if !address_to_js(scope, &sock_addr, out) {
        return UV_EINVAL;
      }
      0
    }
  }

  #[fast]
  pub fn getpeername(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };

    // SAFETY: handle is valid; storage is written by uv_tcp_getpeername before assume_init.
    unsafe {
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_tcp_getpeername(
        handle.as_ptr(),
        storage.as_mut_ptr() as *mut c_void,
        &mut len,
      );
      if ret != 0 {
        return ret;
      }
      let sock_addr = storage.assume_init();
      if !address_to_js(scope, &sock_addr, out) {
        return UV_EINVAL;
      }
      0
    }
  }

  #[fast]
  #[rename("setNoDelay")]
  pub fn set_no_delay(&self, enable: bool) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };
    // SAFETY: handle is a valid, initialized uv_tcp_t.
    unsafe { uv_tcp_nodelay(handle.as_mut_ptr(), enable as i32) }
  }

  #[fast]
  #[rename("setKeepAlive")]
  pub fn set_keep_alive(&self, enable: bool, #[smi] delay: u32) -> i32 {
    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };
    // SAFETY: handle is a valid, initialized uv_tcp_t.
    unsafe { uv_tcp_keepalive(handle.as_mut_ptr(), enable as i32, delay) }
  }

  #[fast]
  pub fn accept(&self, #[cppgc] client: &TCPWrap) -> i32 {
    let Some(server_handle) = self.handle() else {
      return UV_EBADF;
    };
    let Some(client_handle) = client.handle() else {
      return UV_EBADF;
    };

    // SAFETY: both server and client handles are valid uv_tcp_t, castable to uv_stream_t.
    unsafe {
      uv_accept(
        server_handle.as_mut_ptr() as *mut deno_core::uv_compat::uv_stream_t,
        client_handle.as_mut_ptr() as *mut deno_core::uv_compat::uv_stream_t,
      )
    }
  }

  pub fn reset(
    &self,
    op_state: std::rc::Rc<std::cell::RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'_, '_>,
    #[scoped] cb: Option<v8::Global<v8::Function>>,
  ) -> i32 {
    let handle_wrap = self.base.handle_wrap();
    if !handle_wrap.is_alive() {
      return 0;
    }

    let Some(handle) = self.handle() else {
      return UV_EBADF;
    };

    // uv_tcp_close_reset sets SO_LINGER to zero and calls uv_close
    // internally, which handles the native handle cleanup.
    // SAFETY: handle is a valid uv_tcp_t; uv_tcp_close_reset sets SO_LINGER and closes.
    let err = unsafe {
      deno_core::uv_compat::uv_tcp_close_reset(handle.as_mut_ptr(), None)
    };

    // Transition state to Closing regardless of error (matches Node).
    handle_wrap.set_state_closing();

    if err == 0 {
      let this = self.base.js_handle_global(scope).unwrap_or(this);
      // Fire _onClose() JS callback and schedule the close callback.
      handle_wrap.run_close_callback(op_state, this, scope, cb);
    }

    err
  }
}

/// Map a uv error code to (name, message).
fn uv_error_info(err: i32) -> (&'static str, &'static str) {
  use deno_core::uv_compat::*;
  match err {
    x if x == UV_EAGAIN => ("EAGAIN", "resource temporarily unavailable"),
    x if x == UV_EADDRINUSE => ("EADDRINUSE", "address already in use"),
    x if x == UV_EBADF => ("EBADF", "bad file descriptor"),
    x if x == UV_EBUSY => ("EBUSY", "resource busy or locked"),
    x if x == UV_ECANCELED => ("ECANCELED", "operation canceled"),
    x if x == UV_ECONNREFUSED => ("ECONNREFUSED", "connection refused"),
    x if x == UV_EINVAL => ("EINVAL", "invalid argument"),
    x if x == UV_ENOBUFS => ("ENOBUFS", "no buffer space available"),
    x if x == UV_ENOTCONN => ("ENOTCONN", "socket is not connected"),
    x if x == UV_ENOTSUP => ("ENOTSUP", "operation not supported on socket"),
    x if x == UV_EPIPE => ("EPIPE", "broken pipe"),
    _ => ("UNKNOWN", "unknown error"),
  }
}
