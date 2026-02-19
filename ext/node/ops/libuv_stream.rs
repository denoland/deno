// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::ffi::c_void;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::ptr;

use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::uv_compat::UvBuf;
use deno_core::uv_compat::UvConnect;
use deno_core::uv_compat::UvHandle;
use deno_core::uv_compat::UvLoop;
use deno_core::uv_compat::UvShutdown;
use deno_core::uv_compat::UvStream;
use deno_core::uv_compat::UvTcp;
use deno_core::uv_compat::UvWrite;
use deno_core::uv_compat;

use super::handle_wrap::AsyncId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
enum SocketType {
  Socket = 0,
  Server = 1,
}

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

pub struct TCP {
  handle: RefCell<*mut UvTcp>,
  socket_type: Cell<SocketType>,
  provider: i32,
  async_id: i64,
  // Owns the StreamHandleData. The raw pointer in handle.data
  // is a non-owning view into this same allocation.
  handle_data: RefCell<Option<Box<StreamHandleData>>>,
  closed: Cell<bool>,
  bytes_read: Cell<u64>,
  bytes_written: Cell<u64>,
}

unsafe impl GarbageCollected for TCP {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TCP"
  }
}

impl TCP {
  fn init_handle(&self, state: &mut OpState) {
    let loop_ptr: *mut UvLoop = &mut **state.borrow_mut::<Box<UvLoop>>();
    unsafe {
      let tcp = Box::into_raw(Box::new(uv_compat::new_tcp()));
      uv_compat::uv_tcp_init(loop_ptr, tcp);
      *self.handle.borrow_mut() = tcp;
    }
  }

  fn raw(&self) -> *mut UvTcp {
    *self.handle.borrow()
  }

  pub fn stream(&self) -> *mut UvStream {
    self.raw() as *mut UvStream
  }

  fn set_js_object(&self, obj: v8::Global<v8::Object>) {
    if let Some(ref mut data) = *self.handle_data.borrow_mut() {
      data.js_object = Some(obj);
    }
  }
}

unsafe fn context_from_loop(
  loop_ptr: *mut UvLoop,
) -> Option<v8::Local<'static, v8::Context>> {
  unsafe {
    let ctx_ptr = (*loop_ptr).data;
    if ctx_ptr.is_null() {
      return None;
    }
    Some(std::mem::transmute(std::ptr::NonNull::new_unchecked(
      ctx_ptr as *mut v8::Context,
    )))
  }
}

unsafe extern "C" fn stream_alloc_cb(
  handle: *mut UvHandle,
  _suggested_size: usize,
  buf: *mut UvBuf,
) {
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
  unsafe {
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

    let key = v8::String::new(scope, "onread").unwrap();
    let onread = this.get(scope, key.into());

    if let Some(onread) = onread {
      if let Ok(func) = v8::Local::<v8::Function>::try_from(onread) {
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
}

unsafe extern "C" fn server_connection_cb(server: *mut UvStream, status: i32) {
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

    if let Some(onconnection) = onconnection {
      if let Ok(func) = v8::Local::<v8::Function>::try_from(onconnection) {
        let status_val = v8::Integer::new(scope, status);
        func.call(scope, this.into(), &[status_val.into()]);
      }
    }
  }
}

unsafe extern "C" fn write_cb(req: *mut UvWrite, _status: i32) {
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

    if let Some(onconnect) = onconnect {
      if let Ok(func) = v8::Local::<v8::Function>::try_from(onconnect) {
        let status_val = v8::Integer::new(scope, status);
        func.call(scope, this.into(), &[status_val.into()]);
      }
    }
  }
}

unsafe extern "C" fn shutdown_cb(req: *mut UvShutdown, _status: i32) {
  unsafe {
    let _ = Box::from_raw(req);
  }
}

unsafe extern "C" fn tcp_close_cb(handle: *mut UvHandle) {
  unsafe {
    // Handle has been fully removed from the loop's data structures.
    // Now safe to free the UvTcp memory.
    let _ = Box::from_raw(handle as *mut UvTcp);
  }
}

#[op2]
impl TCP {
  #[constructor]
  #[cppgc]
  fn new(state: &mut OpState, #[smi] socket_type: i32) -> TCP {
    let async_id = state.borrow_mut::<AsyncId>().next();
    const PROVIDER_TCPWRAP: i32 = 14;
    let tcp = TCP {
      handle: RefCell::new(ptr::null_mut()),
      socket_type: Cell::new(if socket_type == 1 {
        SocketType::Server
      } else {
        SocketType::Socket
      }),
      provider: PROVIDER_TCPWRAP,
      async_id,
      handle_data: RefCell::new(None),
      closed: Cell::new(false),
      bytes_read: Cell::new(0),
      bytes_written: Cell::new(0),
    };

    tcp.init_handle(state);

    // Create handle data owned by the handle_data RefCell.
    // Store a raw (non-owning) pointer in the libuv handle for callbacks.
    let handle_data = Box::new(StreamHandleData {
      js_object: None,
      read_buf: vec![0u8; 65536],
    });
    let data_ptr =
      &*handle_data as *const StreamHandleData as *mut StreamHandleData;
    unsafe {
      (*(tcp.raw() as *mut UvHandle)).data = data_ptr as *mut c_void;
    }
    tcp.handle_data.replace(Some(handle_data));

    tcp
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

  #[nofast]
  fn set_owner(&self, #[this] this: v8::Global<v8::Object>) {
    self.set_js_object(this);
  }

  #[fast]
  fn open(&self, #[smi] fd: i32) -> i32 {
    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      let flags = libc::fcntl(fd, libc::F_GETFL);
      if flags != -1 {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
      }
      // For C libuv, use uv_tcp_open to assign an existing fd
      uv_compat::uv_tcp_open(tcp, fd)
    }
  }

  #[fast]
  fn bind(&self, #[string] address: &str, #[smi] port: i32) -> i32 {
    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return -1,
      },
      Err(_) => return -1,
    };

    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      let mut storage: libc::sockaddr_storage = std::mem::zeroed();
      let (sa, sa_len) = sockaddr_to_raw(socket_addr, &mut storage);
      uv_compat::uv_tcp_bind(tcp, sa as *const _, sa_len as u32, 0)
    }
  }

  #[fast]
  fn bind6(&self, #[string] address: &str, #[smi] port: i32) -> i32 {
    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return -1,
      },
      Err(_) => return -1,
    };

    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      let mut storage: libc::sockaddr_storage = std::mem::zeroed();
      let (sa, sa_len) = sockaddr_to_raw(socket_addr, &mut storage);
      uv_compat::uv_tcp_bind(tcp, sa as *const _, sa_len as u32, 0)
    }
  }

  #[fast]
  fn listen(&self, #[smi] backlog: i32) -> i32 {
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      uv_compat::uv_listen(stream, backlog, Some(server_connection_cb))
    }
  }

  #[fast]
  fn accept(&self, #[cppgc] client: &TCP) -> i32 {
    unsafe {
      let server_stream = self.stream();
      let client_stream = client.stream();
      if server_stream.is_null() || client_stream.is_null() {
        return -1;
      }
      uv_compat::uv_accept(server_stream, client_stream)
    }
  }

  #[fast]
  fn read_start(&self) -> i32 {
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
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      uv_compat::uv_read_stop(stream)
    }
  }

  fn write_buffer(&self, #[buffer] data: JsBuffer) -> i32 {
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      // Copy data into a WriteReq so it lives until write_cb fires.
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
      let _ = Box::into_raw(write_req); // leak; freed in write_cb
      let ret =
        uv_compat::uv_write(req_ptr, stream, &buf, 1, Some(write_cb));
      if ret != 0 {
        // Failed to queue write, reclaim the WriteReq
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
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      let req = Box::into_raw(Box::new(uv_compat::new_shutdown()));
      let ret = uv_compat::uv_shutdown(
        req,
        stream,
        Some(shutdown_cb),
      );
      if ret != 0 {
        let _ = Box::from_raw(req);
      }
      ret
    }
  }

  #[fast]
  fn set_no_delay(&self, enable: bool) -> i32 {
    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      uv_compat::uv_tcp_nodelay(tcp, enable as i32)
    }
  }

  #[fast]
  fn connect(&self, #[string] address: &str, #[smi] port: i32) -> i32 {
    let addr_str = format!("{}:{}", address, port);
    let socket_addr = match addr_str.to_socket_addrs() {
      Ok(mut addrs) => match addrs.next() {
        Some(addr) => addr,
        None => return -1,
      },
      Err(_) => return -1,
    };

    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      let mut storage: libc::sockaddr_storage = std::mem::zeroed();
      let (sa, _sa_len) = sockaddr_to_raw(socket_addr, &mut storage);
      let mut connect_req = Box::new(ConnectReq {
        uv_req: uv_compat::new_connect(),
      });
      let req_ptr = &mut connect_req.uv_req as *mut UvConnect;
      let _ = Box::into_raw(connect_req); // leak; freed in connect_cb
      let ret = uv_compat::uv_tcp_connect(
        req_ptr,
        tcp,
        sa as *const _,
        Some(connect_cb),
      );
      if ret != 0 {
        // Failed, reclaim the ConnectReq
        let _ = Box::from_raw(req_ptr as *mut ConnectReq);
      }
      ret
    }
  }

  #[serde]
  fn getpeername(&self) -> Option<SockAddr> {
    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return None;
      }
      let mut storage: libc::sockaddr_storage = std::mem::zeroed();
      let mut len =
        std::mem::size_of::<libc::sockaddr_storage>() as i32;
      let ret = uv_compat::uv_tcp_getpeername(
        tcp,
        &mut storage as *mut _ as *mut _,
        &mut len,
      );
      if ret != 0 {
        return None;
      }
      sockaddr_from_storage(&storage)
    }
  }

  #[serde]
  fn getsockname(&self) -> Option<SockAddr> {
    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return None;
      }
      let mut storage: libc::sockaddr_storage = std::mem::zeroed();
      let mut len =
        std::mem::size_of::<libc::sockaddr_storage>() as i32;
      let ret = uv_compat::uv_tcp_getsockname(
        tcp,
        &mut storage as *mut _ as *mut _,
        &mut len,
      );
      if ret != 0 {
        return None;
      }
      sockaddr_from_storage(&storage)
    }
  }

  #[fast]
  fn get_bytes_read(&self) -> f64 {
    self.bytes_read.get() as f64
  }

  #[fast]
  fn get_bytes_written(&self) -> f64 {
    self.bytes_written.get() as f64
  }

  #[fast]
  fn detach(&self) {
    if self.closed.get() {
      return;
    }
    self.closed.set(true);
    *self.handle.borrow_mut() = ptr::null_mut();
    // Drop the owned StreamHandleData since the handle's data pointer
    // has already been overwritten by consume_stream.
    self.handle_data.replace(None);
  }

  #[fast]
  fn close(&self) {
    if self.closed.get() {
      return;
    }
    self.closed.set(true);
    unsafe {
      let tcp = self.raw();
      if !tcp.is_null() {
        // Null out the handle's data pointer (non-owning).
        (*(tcp as *mut UvHandle)).data = ptr::null_mut();
        // Use uv_close (not uv_tcp_close) so the handle is properly:
        // 1. Removed from the loop's handle queue
        // 2. Pending writes cancelled with UV_ECANCELED
        // 3. Handle memory freed only in the callback (after libuv is done)
        uv_compat::uv_close(
          tcp as *mut UvHandle,
          Some(tcp_close_cb),
        );
      }
      *self.handle.borrow_mut() = ptr::null_mut();
    }
    // Drop the owned StreamHandleData (single owner).
    self.handle_data.replace(None);
  }
}

// -- helpers --

#[derive(serde::Serialize)]
struct SockAddr {
  address: String,
  port: u16,
  family: String,
}

unsafe fn sockaddr_from_storage(
  storage: &libc::sockaddr_storage,
) -> Option<SockAddr> {
  unsafe {
    match storage.ss_family as i32 {
      libc::AF_INET => {
        let sin = storage as *const _ as *const libc::sockaddr_in;
        let ip =
          std::net::Ipv4Addr::from(u32::from_be((*sin).sin_addr.s_addr));
        let port = u16::from_be((*sin).sin_port);
        Some(SockAddr {
          address: ip.to_string(),
          port,
          family: "IPv4".to_string(),
        })
      }
      libc::AF_INET6 => {
        let sin6 = storage as *const _ as *const libc::sockaddr_in6;
        let ip = std::net::Ipv6Addr::from((*sin6).sin6_addr.s6_addr);
        let port = u16::from_be((*sin6).sin6_port);
        Some(SockAddr {
          address: ip.to_string(),
          port,
          family: "IPv6".to_string(),
        })
      }
      _ => None,
    }
  }
}

unsafe fn sockaddr_to_raw(
  addr: SocketAddr,
  storage: &mut libc::sockaddr_storage,
) -> (*const libc::sockaddr, libc::socklen_t) {
  unsafe {
    match addr {
      SocketAddr::V4(ref a) => {
        let sin = storage as *mut _ as *mut libc::sockaddr_in;
        (*sin).sin_family = libc::AF_INET as libc::sa_family_t;
        (*sin).sin_port = a.port().to_be();
        (*sin).sin_addr.s_addr = u32::from_ne_bytes(a.ip().octets());
        (
          storage as *const _ as *const libc::sockaddr,
          std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
      }
      SocketAddr::V6(ref a) => {
        let sin6 = storage as *mut _ as *mut libc::sockaddr_in6;
        (*sin6).sin6_family = libc::AF_INET6 as libc::sa_family_t;
        (*sin6).sin6_port = a.port().to_be();
        (*sin6).sin6_addr.s6_addr = a.ip().octets();
        (*sin6).sin6_flowinfo = a.flowinfo();
        (*sin6).sin6_scope_id = a.scope_id();
        (
          storage as *const _ as *const libc::sockaddr,
          std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
        )
      }
    }
  }
}
