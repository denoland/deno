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
use libuvrust::UvBuf;
use libuvrust::stream::UvStream;
use libuvrust::stream::UvWrite;
use libuvrust::tcp::UvTcp;
use libuvrust::uv_loop::UvLoop;

use super::handle_wrap::AsyncId;

const UV_HANDLE_BOUND: u32 = 0x00000004;
const UV_HANDLE_WRITABLE: u32 = 0x00000008;
const UV_HANDLE_READABLE: u32 = 0x00000010;

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
    let loop_ptr = &mut *state.uv_loop as *mut UvLoop;
    unsafe {
      let tcp = Box::into_raw(Box::new(UvTcp::new()));
      libuvrust::tcp::uv_tcp_init(loop_ptr, tcp);
      *self.handle.borrow_mut() = tcp;
    }
  }

  fn raw(&self) -> *mut UvTcp {
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

unsafe fn context_from_loop(
  loop_ptr: *mut libuvrust::uv_loop::UvLoop,
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

unsafe fn stream_alloc_cb(
  handle: *mut libuvrust::handle::UvHandle,
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
    (*buf).base = (*data).read_buf.as_mut_ptr();
    (*buf).len = (*data).read_buf.len();
  }
}

unsafe fn stream_read_cb(
  stream: *mut UvStream,
  nread: isize,
  _buf: *const UvBuf,
) {
  unsafe {
    let data = (*stream).handle.data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => return,
    };

    let context = match context_from_loop((*stream).handle.loop_ptr) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this = v8::Local::new(scope, js_obj);

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

unsafe fn server_connection_cb(server: *mut UvStream, status: i32) {
  unsafe {
    let data = (*server).handle.data as *mut StreamHandleData;
    if data.is_null() {
      return;
    }
    let js_obj = match (*data).js_object {
      Some(ref obj) => obj,
      None => return,
    };

    let context = match context_from_loop((*server).handle.loop_ptr) {
      Some(c) => c,
      None => return,
    };
    v8::callback_scope!(unsafe let scope, context);
    v8::tc_scope!(let scope, scope);

    let this = v8::Local::new(scope, js_obj);

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

unsafe fn write_cb(req: *mut UvWrite, _status: i32) {
  unsafe {
    // req is the first field of WriteReq (#[repr(C)]),
    // so the pointer is the same as the WriteReq pointer.
    let _ = Box::from_raw(req as *mut WriteReq);
  }
}

unsafe fn tcp_close_cb(handle: *mut libuvrust::handle::UvHandle) {
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
      (*(tcp.raw())).stream.handle.data = data_ptr as *mut c_void;
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
      let stream = tcp as *mut UvStream;
      (*stream).io_watcher.fd = fd;
      let flags = libc::fcntl(fd, libc::F_GETFL);
      if flags != -1 {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
      }
      (*stream).handle.flags |=
        UV_HANDLE_BOUND | UV_HANDLE_READABLE | UV_HANDLE_WRITABLE;
      0
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
      libuvrust::tcp::uv_tcp_bind(tcp, sa, sa_len, 0)
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
      libuvrust::tcp::uv_tcp_bind(tcp, sa, sa_len, 0)
    }
  }

  #[fast]
  fn listen(&self, #[smi] backlog: i32) -> i32 {
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      libuvrust::stream::uv_listen(stream, backlog, Some(server_connection_cb))
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
      libuvrust::stream::uv_accept(server_stream, client_stream)
    }
  }

  #[fast]
  fn read_start(&self) -> i32 {
    unsafe {
      let stream = self.stream();
      if stream.is_null() {
        return -1;
      }
      libuvrust::stream::uv_read_start(
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
      libuvrust::stream::uv_read_stop(stream)
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
        uv_req: UvWrite::new(),
        _data: data_vec,
      });
      let buf = UvBuf {
        base: write_req._data.as_mut_ptr(),
        len: data_len,
      };
      let req_ptr = &mut write_req.uv_req as *mut UvWrite;
      Box::into_raw(write_req); // leak; freed in write_cb
      let ret =
        libuvrust::stream::uv_write(req_ptr, stream, &buf, 1, Some(write_cb));
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
  fn set_no_delay(&self, enable: bool) -> i32 {
    unsafe {
      let tcp = self.raw();
      if tcp.is_null() {
        return -1;
      }
      libuvrust::tcp::uv_tcp_nodelay(tcp, enable as i32)
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
  fn close(&self) {
    if self.closed.get() {
      return;
    }
    self.closed.set(true);
    unsafe {
      let tcp = self.raw();
      if !tcp.is_null() {
        // Null out the handle's data pointer (non-owning).
        (*tcp).stream.handle.data = ptr::null_mut();
        // Use uv_close (not uv_tcp_close) so the handle is properly:
        // 1. Removed from the loop's handle queue
        // 2. Pending writes cancelled with UV_ECANCELED
        // 3. Handle memory freed only in the callback (after libuv is done)
        libuvrust::uv_loop::uv_close(
          tcp as *mut libuvrust::handle::UvHandle,
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
