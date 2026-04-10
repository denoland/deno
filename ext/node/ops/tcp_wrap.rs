// Copyright 2018-2026 the Deno authors. MIT license.
//
// TCPWrap -- TCP handle inheriting from LibUvStreamWrap.
//
// Follows the TTY pattern: inherits read/write/shutdown from the base class,
// only implements TCP-specific ops (bind, listen, connect, accept, etc.).

use std::cell::Cell;
use std::net::ToSocketAddrs;

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::uv_compat;
use deno_core::uv_compat::UvConnect;
use deno_core::uv_compat::UvHandle;
use deno_core::uv_compat::UvLoop;
use deno_core::uv_compat::UvStream;
use deno_core::uv_compat::UvTcp;
use deno_core::v8;
use deno_net::io::TcpStreamResource;
use deno_permissions::PermissionsContainer;
use socket2::SockAddr as Socket2SockAddr;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::Handle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::clone_context_from_uv_loop;
use crate::ops::stream_wrap::LibUvStreamWrap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum SocketType {
  Socket = 0,
  Server = 1,
}

// -- libuv callbacks (called from the event loop) --

/// Macro to set up a v8 scope from a uv stream's handle data and call a JS
/// callback. The stream's `data` must point to a valid `StreamHandleData`.
macro_rules! with_js_handle {
  ($stream:expr, |$scope:ident, $this:ident| $body:block) => {
    unsafe {
      let Some(handle_data_ptr) = LibUvStreamWrap::stable_handle_data($stream)
      else {
        return;
      };
      let handle_data = handle_data_ptr.as_ref();
      let isolate_ptr = *handle_data.isolate.get();
      if isolate_ptr.is_null() {
        return;
      }
      let mut isolate = v8::Isolate::from_raw_isolate_ptr(isolate_ptr);
      let loop_ptr = (*$stream).loop_;
      let context = clone_context_from_uv_loop(&mut isolate, loop_ptr);
      v8::scope!(let handle_scope, &mut isolate);
      let context_local = v8::Local::new(handle_scope, context);
      let $scope = &mut v8::ContextScope::new(handle_scope, context_local);

      let Some(js_global) =
        (*handle_data.js_handle.get()).to_global($scope)
      else {
        return;
      };
      let $this: v8::Local<v8::Object> = v8::Local::new($scope, js_global);
      $body
    }
  };
}

/// Connection callback for `uv_listen`. Fires `this.onconnection(status)` on
/// the server handle's JS object.
pub(crate) unsafe extern "C" fn server_connection_cb(
  server: *mut UvStream,
  status: i32,
) {
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

// Wraps a UvConnect request so it stays alive until the callback fires.
#[repr(C)]
struct ConnectReq {
  uv_req: UvConnect,
}

/// Connect callback for `uv_tcp_connect`. Fires `this.onconnect(status)` on
/// the handle's JS object.
unsafe extern "C" fn connect_cb(req: *mut UvConnect, status: i32) {
  unsafe {
    let stream = (*req).handle as *mut UvStream;
    let _ = Box::from_raw(req as *mut ConnectReq);

    with_js_handle!(stream, |scope, this| {
      let key = v8::String::new(scope, "onconnect").unwrap();
      if let Some(onconnect) = this.get(scope, key.into())
        && let Ok(func) = v8::Local::<v8::Function>::try_from(onconnect)
      {
        let status_val: v8::Local<v8::Value> =
          v8::Integer::new(scope, status).into();
        func.call(scope, this.into(), &[status_val]);
      }
    });
  }
}

// -- TCPWrap struct --

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct TCPWrap {
  base: LibUvStreamWrap,
  handle: Option<OwnedPtr<UvTcp>>,
  socket_type: Cell<SocketType>,
}

// SAFETY: TCPWrap is a cppgc-managed object; the GC traces it via the base field.
unsafe impl GarbageCollected for TCPWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TCP"
  }

  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl Drop for TCPWrap {
  fn drop(&mut self) {
    self.base.detach_stream();
  }
}

impl TCPWrap {
  fn new(socket_type: SocketType, op_state: &mut OpState) -> Self {
    let loop_ = &**op_state.borrow::<Box<UvLoop>>() as *const UvLoop
      as *mut UvLoop;

    let tcp = OwnedPtr::from_box(Box::<UvTcp>::new_uninit());

    // SAFETY: loop_ and tcp are valid pointers for uv_tcp_init
    let err = unsafe { uv_compat::uv_tcp_init(loop_, tcp.as_mut_ptr().cast()) };

    if err == 0 {
      // SAFETY: uv_tcp_init succeeded, memory is initialized
      let tcp = unsafe { tcp.cast::<UvTcp>() };

      let provider = if socket_type == SocketType::Server {
        ProviderType::TcpServerWrap
      } else {
        ProviderType::TcpWrap
      };

      let base = LibUvStreamWrap::new(
        HandleWrap::create(
          AsyncWrap::create(op_state, provider as i32),
          Some(Handle::New(tcp.as_ptr().cast())),
        ),
        -1, // fd not known until bind/connect
        tcp.as_ptr().cast(),
      );

      // SAFETY: tcp pointer is valid; setting data field for libuv callbacks
      unsafe {
        (*tcp.as_mut_ptr()).data = base.handle_data_ptr();
      }

      Self {
        base,
        handle: Some(tcp),
        socket_type: Cell::new(socket_type),
      }
    } else {
      // Error path - create with null handle
      let provider = if socket_type == SocketType::Server {
        ProviderType::TcpServerWrap
      } else {
        ProviderType::TcpWrap
      };

      // Free uninit allocation
      unsafe {
        let layout = std::alloc::Layout::new::<UvTcp>();
        std::alloc::dealloc(tcp.as_mut_ptr() as *mut u8, layout);
        std::mem::forget(tcp);
      }

      Self {
        base: LibUvStreamWrap::new(
          HandleWrap::create(
            AsyncWrap::create(op_state, provider as i32),
            None,
          ),
          -1,
          std::ptr::null(),
        ),
        handle: None,
        socket_type: Cell::new(socket_type),
      }
    }
  }

  fn tcp_ptr(&self) -> *mut UvTcp {
    match &self.handle {
      Some(h) => h.as_mut_ptr(),
      None => std::ptr::null_mut(),
    }
  }
}

// -- ops --

#[op2(inherit = LibUvStreamWrap)]
impl TCPWrap {
  #[constructor]
  #[cppgc]
  fn new_tcp(
    #[smi] socket_type: i32,
    op_state: &mut OpState,
  ) -> TCPWrap {
    let st = if socket_type == 1 {
      SocketType::Server
    } else {
      SocketType::Socket
    };
    TCPWrap::new(st, op_state)
  }

  #[fast]
  fn open(&self, #[smi] fd: i32) -> i32 {
    if fd < 0 {
      return uv_compat::UV_EBADF;
    }
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return uv_compat::UV_EBADF;
    }
    unsafe {
      #[cfg(unix)]
      {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags != -1 {
          libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
      }
      #[cfg(windows)]
      {
        use windows_sys::Win32::Networking::WinSock::FIONBIO;
        use windows_sys::Win32::Networking::WinSock::ioctlsocket;
        let mut nonblocking: u32 = 1;
        ioctlsocket(fd as usize, FIONBIO, &mut nonblocking);
      }
      uv_compat::uv_tcp_open(tcp, fd)
    }
  }

  #[fast]
  fn open_from_rid(&self, state: &mut OpState, #[smi] rid: ResourceId) -> i32 {
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return -1;
    }
    let fd = state
      .resource_table
      .get::<TcpStreamResource>(rid)
      .ok()
      .and_then(|r| r.dup_raw_fd());
    match fd {
      Some(fd) => unsafe { uv_compat::uv_tcp_open(tcp, fd) },
      None => -1,
    }
  }

  #[nofast]
  fn bind(
    &self,
    state: &mut OpState,
    #[string] address: &str,
    #[smi] port: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.listen()")?;

    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(-1),
      },
      Err(_) => return Ok(-1),
    };

    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return Ok(-1);
      }
      let sock_addr = Socket2SockAddr::from(socket_addr);
      Ok(uv_compat::uv_tcp_bind(
        tcp,
        sock_addr.as_ptr() as *const _,
        #[allow(clippy::unnecessary_cast, reason = "depends on platform")]
        {
          sock_addr.len() as u32
        },
        0,
      ))
    }
  }

  #[nofast]
  fn bind6(
    &self,
    state: &mut OpState,
    #[string] address: &str,
    #[smi] port: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.listen()")?;

    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(-1),
      },
      Err(_) => return Ok(-1),
    };

    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return Ok(-1);
      }
      let sock_addr = Socket2SockAddr::from(socket_addr);
      Ok(uv_compat::uv_tcp_bind(
        tcp,
        sock_addr.as_ptr() as *const _,
        #[allow(clippy::unnecessary_cast, reason = "on some platforms")]
        {
          sock_addr.len() as u32
        },
        0,
      ))
    }
  }

  #[fast]
  fn listen(&self, #[smi] backlog: i32) -> i32 {
    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return -1;
      }
      let stream = tcp as *mut UvStream;
      uv_compat::uv_listen(stream, backlog, Some(server_connection_cb))
    }
  }

  #[fast]
  fn accept(&self, #[cppgc] client: &TCPWrap) -> i32 {
    unsafe {
      let server_tcp = self.tcp_ptr();
      let client_tcp = client.tcp_ptr();
      if server_tcp.is_null() || client_tcp.is_null() {
        return -1;
      }
      uv_compat::uv_accept(
        server_tcp as *mut UvStream,
        client_tcp as *mut UvStream,
      )
    }
  }

  #[fast]
  fn set_no_delay(&self, enable: bool) -> i32 {
    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return -1;
      }
      uv_compat::uv_tcp_nodelay(tcp, enable as i32)
    }
  }

  #[nofast]
  fn connect(
    &self,
    state: &mut OpState,
    #[string] address: &str,
    #[smi] port: i32,
  ) -> Result<i32, deno_permissions::PermissionCheckError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_net(&(address, Some(port as u16)), "node:net.connect()")?;

    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return Ok(-1),
      },
      Err(_) => return Ok(-1),
    };

    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return Ok(-1);
      }
      let sock_addr = Socket2SockAddr::from(socket_addr);
      let mut connect_req = Box::new(ConnectReq {
        uv_req: uv_compat::new_connect(),
      });
      let req_ptr = &mut connect_req.uv_req as *mut UvConnect;
      let _ = Box::into_raw(connect_req);
      let ret = uv_compat::uv_tcp_connect(
        req_ptr,
        tcp,
        sock_addr.as_ptr() as *const _,
        Some(connect_cb),
      );
      if ret != 0 {
        let _ = Box::from_raw(req_ptr as *mut ConnectReq);
      }
      Ok(ret)
    }
  }

  #[serde]
  fn getpeername(&self) -> Option<SockAddrInfo> {
    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return None;
      }
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_compat::uv_tcp_getpeername(
        tcp,
        storage.as_mut_ptr() as *mut _,
        &mut len,
      );
      if ret != 0 {
        return None;
      }
      sockaddr_from_socket2(&storage.assume_init())
    }
  }

  #[serde]
  fn getsockname(&self) -> Option<SockAddrInfo> {
    unsafe {
      let tcp = self.tcp_ptr();
      if tcp.is_null() {
        return None;
      }
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_compat::uv_tcp_getsockname(
        tcp,
        storage.as_mut_ptr() as *mut _,
        &mut len,
      );
      if ret != 0 {
        return None;
      }
      sockaddr_from_socket2(&storage.assume_init())
    }
  }
}

// -- helpers --

#[derive(serde::Serialize)]
struct SockAddrInfo {
  address: String,
  port: u16,
  family: String,
}

fn sockaddr_from_socket2(
  sock_addr: &socket2::SockAddr,
) -> Option<SockAddrInfo> {
  if let Some(addr) = sock_addr.as_socket_ipv4() {
    Some(SockAddrInfo {
      address: addr.ip().to_string(),
      port: addr.port(),
      family: "IPv4".to_string(),
    })
  } else {
    sock_addr.as_socket_ipv6().map(|addr| SockAddrInfo {
      address: addr.ip().to_string(),
      port: addr.port(),
      family: "IPv6".to_string(),
    })
  }
}
