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
use crate::ops::stream_wrap::LibUvStreamWrap;
use crate::ops::stream_wrap::clone_context_from_uv_loop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum SocketType {
  Socket = 0,
  Server = 1,
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
    let Some(handle_data_ptr) =
      // SAFETY: caller guarantees $stream is a valid uv_stream_t with
      // data pointing to a live StreamHandleData.
      (unsafe { LibUvStreamWrap::stable_handle_data($stream) })
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
    let context = unsafe { clone_context_from_uv_loop(&mut isolate, loop_ptr) };
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

/// Connection callback for `uv_listen`. Fires `this.onconnection(status)` on
/// the server handle's JS object.
///
/// # Safety
/// Must only be called by libuv as a `uv_connection_cb`. `server` must be a
/// valid `uv_stream_t` whose `data` points to a live `StreamHandleData`.
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

// Wraps a UvConnect request together with the JS req object (TCPConnectWrap)
// so both stay alive until the callback fires.
#[repr(C)]
struct ConnectReqData {
  uv_req: UvConnect,
  js_req: v8::Global<v8::Object>,
}

/// Connect callback for `uv_tcp_connect`. Fires `req.oncomplete(status,
/// handle, req, readable, writable)` matching Node.js ConnectionWrap::AfterConnect.
///
/// # Safety
/// Must only be called by libuv as a `uv_connect_cb`. `req` must point to a
/// `ConnectReqData` allocated via `Box::into_raw`.
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
    let loop_ =
      &**op_state.borrow::<Box<UvLoop>>() as *const UvLoop as *mut UvLoop;

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

      // SAFETY: tcp was allocated via OwnedPtr::from_box but uv_tcp_init
      // failed, so the memory is uninitialized. Free it without dropping.
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
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> TCPWrap {
    let st = if socket_type == 1 {
      SocketType::Server
    } else {
      SocketType::Socket
    };
    let tcp = TCPWrap::new(st, op_state);
    // Store the JS handle so callbacks (connect, read, etc.) can find it.
    tcp.base.set_js_handle(this, scope);
    tcp
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
    // SAFETY: tcp handle is valid (null-checked above); fd is validated above.
    // Platform-specific non-blocking setup and uv_tcp_open are safe with valid args.
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
      // SAFETY: tcp is valid (null-checked above); fd is a valid dup'd descriptor.
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

    // SAFETY: tcp is valid; socket2 SockAddr is properly initialized from
    // a resolved std::net::SocketAddr.
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

    // SAFETY: tcp is valid; socket2 SockAddr is properly initialized.
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
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return -1;
    }
    // SAFETY: tcp is valid (null-checked above); cast to uv_stream_t is
    // safe because uv_tcp_t embeds uv_stream_t at offset 0.
    unsafe {
      let stream = tcp as *mut UvStream;
      uv_compat::uv_listen(stream, backlog, Some(server_connection_cb))
    }
  }

  #[fast]
  fn accept(&self, #[cppgc] client: &TCPWrap) -> i32 {
    let server_tcp = self.tcp_ptr();
    let client_tcp = client.tcp_ptr();
    if server_tcp.is_null() || client_tcp.is_null() {
      return -1;
    }
    // SAFETY: both tcp pointers are valid (null-checked above); cast to
    // uv_stream_t is safe per uv_tcp_t layout.
    unsafe {
      uv_compat::uv_accept(
        server_tcp as *mut UvStream,
        client_tcp as *mut UvStream,
      )
    }
  }

  #[fast]
  fn set_no_delay(&self, enable: bool) -> i32 {
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return -1;
    }
    // SAFETY: tcp is valid (null-checked above).
    unsafe { uv_compat::uv_tcp_nodelay(tcp, enable as i32) }
  }

  /// Connect to an address. Takes (req, address, port) where req is a
  /// TCPConnectWrap with oncomplete callback, matching Node.js API.
  #[nofast]
  fn connect(
    &self,
    state: &mut OpState,
    js_req: v8::Local<v8::Object>,
    #[string] address: &str,
    #[smi] port: i32,
    scope: &mut v8::PinScope,
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

    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return Ok(-1);
    }
    let sock_addr = Socket2SockAddr::from(socket_addr);
    let js_req_global = v8::Global::new(scope, js_req);
    let mut connect_req = Box::new(ConnectReqData {
      uv_req: uv_compat::new_connect(),
      js_req: js_req_global,
    });
    let req_ptr = &mut connect_req.uv_req as *mut UvConnect;
    let _ = Box::into_raw(connect_req);
    // SAFETY: tcp is valid (null-checked above); req_ptr is a valid
    // heap-allocated UvConnect; sock_addr is properly initialized.
    // connect_req is leaked and will be reclaimed in connect_cb.
    let ret = unsafe {
      uv_compat::uv_tcp_connect(
        req_ptr,
        tcp,
        sock_addr.as_ptr() as *const _,
        Some(connect_cb),
      )
    };
    if ret != 0 {
      // SAFETY: uv_tcp_connect failed synchronously; reclaim the request.
      unsafe {
        let _ = Box::from_raw(req_ptr as *mut ConnectReqData);
      }
    }
    Ok(ret)
  }

  /// Connect to an IPv6 address. uv_tcp_connect handles both v4 and v6.
  #[nofast]
  fn connect6(
    &self,
    state: &mut OpState,
    js_req: v8::Local<v8::Object>,
    #[string] address: &str,
    #[smi] port: i32,
    scope: &mut v8::PinScope,
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

    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return Ok(-1);
    }
    let sock_addr = Socket2SockAddr::from(socket_addr);
    let js_req_global = v8::Global::new(scope, js_req);
    let mut connect_req = Box::new(ConnectReqData {
      uv_req: uv_compat::new_connect(),
      js_req: js_req_global,
    });
    let req_ptr = &mut connect_req.uv_req as *mut UvConnect;
    let _ = Box::into_raw(connect_req);
    // SAFETY: same as connect() above.
    let ret = unsafe {
      uv_compat::uv_tcp_connect(
        req_ptr,
        tcp,
        sock_addr.as_ptr() as *const _,
        Some(connect_cb),
      )
    };
    if ret != 0 {
      // SAFETY: uv_tcp_connect failed synchronously; reclaim the request.
      unsafe {
        let _ = Box::from_raw(req_ptr as *mut ConnectReqData);
      }
    }
    Ok(ret)
  }

  /// Populates the output object with remote address info. Returns 0 on
  /// success, negative error code on failure. Matches Node.js API:
  /// `handle.getpeername(out)` where out gets {address, port, family}.
  #[nofast]
  fn getpeername(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return uv_compat::UV_EBADF;
    }
    // SAFETY: tcp is valid (null-checked above); storage is properly sized.
    unsafe {
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_compat::uv_tcp_getpeername(
        tcp,
        storage.as_mut_ptr() as *mut _,
        &mut len,
      );
      if ret != 0 {
        return ret;
      }
      populate_sockaddr_object(scope, out, &storage.assume_init());
      0
    }
  }

  /// Populates the output object with local address info. Returns 0 on
  /// success, negative error code on failure. Matches Node.js API:
  /// `handle.getsockname(out)` where out gets {address, port, family}.
  #[nofast]
  fn getsockname(
    &self,
    out: v8::Local<v8::Object>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let tcp = self.tcp_ptr();
    if tcp.is_null() {
      return uv_compat::UV_EBADF;
    }
    // SAFETY: tcp is valid (null-checked above); storage is properly sized.
    unsafe {
      let mut storage = std::mem::MaybeUninit::<socket2::SockAddr>::uninit();
      let mut len = std::mem::size_of::<socket2::SockAddr>() as i32;
      let ret = uv_compat::uv_tcp_getsockname(
        tcp,
        storage.as_mut_ptr() as *mut _,
        &mut len,
      );
      if ret != 0 {
        return ret;
      }
      populate_sockaddr_object(scope, out, &storage.assume_init());
      0
    }
  }
}

// -- helpers --

/// Populate a JS object with {address, port, family} from a socket2::SockAddr.
fn populate_sockaddr_object(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  sock_addr: &socket2::SockAddr,
) {
  let (address, port, family) = if let Some(addr) = sock_addr.as_socket_ipv4() {
    (addr.ip().to_string(), addr.port(), "IPv4")
  } else if let Some(addr) = sock_addr.as_socket_ipv6() {
    (addr.ip().to_string(), addr.port(), "IPv6")
  } else {
    return;
  };

  let addr_key = v8::String::new(scope, "address").unwrap();
  let addr_val = v8::String::new(scope, &address).unwrap();
  obj.set(scope, addr_key.into(), addr_val.into());

  let port_key = v8::String::new(scope, "port").unwrap();
  let port_val = v8::Integer::new(scope, port as i32);
  obj.set(scope, port_key.into(), port_val.into());

  let family_key = v8::String::new(scope, "family").unwrap();
  let family_val = v8::String::new(scope, family).unwrap();
  obj.set(scope, family_key.into(), family_val.into());
}
