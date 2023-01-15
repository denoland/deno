// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// False positive lint for explicit drops.
// https://github.com/rust-lang/rust-clippy/issues/6446
#![allow(clippy::await_holding_lock)]
// https://github.com/rust-lang/rust-clippy/issues/6353
#![allow(clippy::await_holding_refcell_ref)]

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::v8::fast_api;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use deno_core::V8_WRAPPER_OBJECT_INDEX;
use deno_tls::load_certs;
use deno_tls::load_private_keys;
use http::header::HeaderName;
use http::header::CONNECTION;
use http::header::CONTENT_LENGTH;
use http::header::EXPECT;
use http::header::TRANSFER_ENCODING;
use http::HeaderValue;
use log::trace;
use mio::net::TcpListener;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use serde::Deserialize;
use serde::Serialize;
use socket2::Socket;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::future::Future;
use std::intrinsics::transmute;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::mem::replace;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

mod chunked;
mod request;
#[cfg(unix)]
mod sendfile;
mod socket;

use request::InnerRequest;
use request::Request;
use socket::InnerStream;
use socket::Stream;

pub struct FlashContext {
  next_server_id: u32,
  join_handles: HashMap<u32, JoinHandle<Result<(), AnyError>>>,
  pub servers: HashMap<u32, ServerContext>,
}

pub struct ServerContext {
  _addr: SocketAddr,
  tx: mpsc::Sender<Request>,
  rx: mpsc::Receiver<Request>,
  requests: HashMap<u32, Request>,
  next_token: u32,
  listening_rx: Option<mpsc::Receiver<u16>>,
  close_tx: mpsc::Sender<()>,
  cancel_handle: Rc<CancelHandle>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ParseStatus {
  None,
  Ongoing(usize),
}

#[op]
fn op_flash_respond(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
  response: StringOrBuffer,
  shutdown: bool,
) -> u32 {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  flash_respond(ctx, token, shutdown, &response)
}

#[op(fast)]
fn op_try_flash_respond_chunked(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
  response: &[u8],
  shutdown: bool,
) -> u32 {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let tx = ctx.requests.get(&token).unwrap();
  let sock = tx.socket();

  // TODO(@littledivy): Use writev when `UnixIoSlice` lands.
  // https://github.com/denoland/deno/pull/15629
  let h = format!("{:x}\r\n", response.len());

  let concat = [h.as_bytes(), response, b"\r\n"].concat();
  let expected = sock.try_write(&concat);
  if expected != concat.len() {
    if expected > 2 {
      return expected as u32;
    }
    return expected as u32;
  }

  if shutdown {
    // Best case: We've written everything and the stream is done too.
    let _ = ctx.requests.remove(&token).unwrap();
  }
  0
}

#[op]
async fn op_flash_respond_async(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  response: StringOrBuffer,
  shutdown: bool,
) -> Result<(), AnyError> {
  trace!("op_flash_respond_async");

  let mut close = false;
  let sock = {
    let mut op_state = state.borrow_mut();
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();

    match shutdown {
      true => {
        let tx = ctx.requests.remove(&token).unwrap();
        close = !tx.keep_alive;
        tx.socket()
      }
      // In case of a websocket upgrade or streaming response.
      false => {
        let tx = ctx.requests.get(&token).unwrap();
        tx.socket()
      }
    }
  };

  sock
    .with_async_stream(|stream| {
      Box::pin(async move {
        Ok(tokio::io::AsyncWriteExt::write(stream, &response).await?)
      })
    })
    .await?;
  // server is done writing and request doesn't want to kept alive.
  if shutdown && close {
    sock.shutdown();
  }
  Ok(())
}

#[op]
async fn op_flash_respond_chunked(
  op_state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  response: Option<ZeroCopyBuf>,
  shutdown: bool,
  nwritten: u32,
) -> Result<(), AnyError> {
  let mut op_state = op_state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let sock = match shutdown {
    true => {
      let tx = ctx.requests.remove(&token).unwrap();
      tx.socket()
    }
    // In case of a websocket upgrade or streaming response.
    false => {
      let tx = ctx.requests.get(&token).unwrap();
      tx.socket()
    }
  };

  drop(op_state);
  sock
    .with_async_stream(|stream| {
      Box::pin(async move {
        use tokio::io::AsyncWriteExt;
        // TODO(@littledivy): Use writev when `UnixIoSlice` lands.
        // https://github.com/denoland/deno/pull/15629
        macro_rules! write_whats_not_written {
          ($e:expr) => {
            let e = $e;
            let n = nwritten as usize;
            if n < e.len() {
              stream.write_all(&e[n..]).await?;
            }
          };
        }
        if let Some(response) = response {
          let h = format!("{:x}\r\n", response.len());
          write_whats_not_written!(h.as_bytes());
          write_whats_not_written!(&response);
          write_whats_not_written!(b"\r\n");
        }

        // The last chunk
        if shutdown {
          write_whats_not_written!(b"0\r\n\r\n");
        }

        Ok(())
      })
    })
    .await?;
  Ok(())
}

#[op]
async fn op_flash_write_resource(
  op_state: Rc<RefCell<OpState>>,
  response: StringOrBuffer,
  server_id: u32,
  token: u32,
  resource_id: deno_core::ResourceId,
  auto_close: bool,
) -> Result<(), AnyError> {
  let (resource, sock) = {
    let op_state = &mut op_state.borrow_mut();
    let resource = if auto_close {
      op_state.resource_table.take_any(resource_id)?
    } else {
      op_state.resource_table.get_any(resource_id)?
    };
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
    (resource, ctx.requests.remove(&token).unwrap().socket())
  };

  let _ = sock.write(&response);

  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    if let InnerStream::Tcp(stream_handle) = &sock.inner {
      let stream_handle = stream_handle.as_raw_fd();
      if let Some(fd) = resource.clone().backing_fd() {
        // SAFETY: all-zero byte-pattern is a valid value for libc::stat.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: call to libc::fstat.
        if unsafe { libc::fstat(fd, &mut stat) } >= 0 {
          let _ = sock.write(
            format!("Content-Length: {}\r\n\r\n", stat.st_size).as_bytes(),
          );
          let tx = sendfile::SendFile {
            io: (fd, stream_handle),
            written: 0,
          };
          tx.await?;
          return Ok(());
        }
      }
    }
  }

  sock
    .with_async_stream(|stream| {
      Box::pin(async move {
        use tokio::io::AsyncWriteExt;
        stream
          .write_all(b"Transfer-Encoding: chunked\r\n\r\n")
          .await?;
        loop {
          let view = resource.clone().read(64 * 1024).await?; // 64KB
          if view.is_empty() {
            stream.write_all(b"0\r\n\r\n").await?;
            break;
          }
          // TODO(@littledivy): use vectored writes.
          stream
            .write_all(format!("{:x}\r\n", view.len()).as_bytes())
            .await?;
          stream.write_all(&view).await?;
          stream.write_all(b"\r\n").await?;
        }
        resource.close();
        Ok(())
      })
    })
    .await?;
  Ok(())
}

pub struct RespondFast;

impl fast_api::FastFunction for RespondFast {
  fn function(&self) -> *const c_void {
    op_flash_respond_fast as *const c_void
  }

  fn args(&self) -> &'static [fast_api::Type] {
    &[
      fast_api::Type::V8Value,
      fast_api::Type::Uint32,
      fast_api::Type::TypedArray(fast_api::CType::Uint8),
      fast_api::Type::Bool,
    ]
  }

  fn return_type(&self) -> fast_api::CType {
    fast_api::CType::Uint32
  }
}

fn flash_respond(
  ctx: &mut ServerContext,
  token: u32,
  shutdown: bool,
  response: &[u8],
) -> u32 {
  let tx = ctx.requests.get(&token).unwrap();
  let sock = tx.socket();

  sock.read_tx.take();
  sock.read_rx.take();

  let nwritten = sock.try_write(response);

  if shutdown && nwritten == response.len() {
    if !tx.keep_alive {
      sock.shutdown();
    }
    ctx.requests.remove(&token).unwrap();
  }

  nwritten as u32
}

unsafe fn op_flash_respond_fast(
  recv: v8::Local<v8::Object>,
  token: u32,
  response: *const fast_api::FastApiTypedArray<u8>,
  shutdown: bool,
) -> u32 {
  let ptr =
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX);
  let ctx = &mut *(ptr as *mut ServerContext);

  let response = &*response;
  if let Some(response) = response.get_storage_if_aligned() {
    flash_respond(ctx, token, shutdown, response)
  } else {
    todo!();
  }
}

macro_rules! get_request {
  ($op_state: ident, $token: ident) => {
    get_request!($op_state, 0, $token)
  };
  ($op_state: ident, $server_id: expr, $token: ident) => {{
    let flash_ctx = $op_state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&$server_id).unwrap();
    ctx.requests.get_mut(&$token).unwrap()
  }};
}

#[repr(u32)]
pub enum Method {
  GET = 0,
  HEAD,
  CONNECT,
  PUT,
  DELETE,
  OPTIONS,
  TRACE,
  POST,
  PATCH,
}

#[inline]
fn get_method(req: &mut Request) -> u32 {
  let method = match req.method() {
    "GET" => Method::GET,
    "POST" => Method::POST,
    "PUT" => Method::PUT,
    "DELETE" => Method::DELETE,
    "OPTIONS" => Method::OPTIONS,
    "HEAD" => Method::HEAD,
    "PATCH" => Method::PATCH,
    "TRACE" => Method::TRACE,
    "CONNECT" => Method::CONNECT,
    _ => Method::GET,
  };
  method as u32
}

#[op]
fn op_flash_method(state: &mut OpState, server_id: u32, token: u32) -> u32 {
  let req = get_request!(state, server_id, token);
  get_method(req)
}

#[op]
async fn op_flash_close_server(state: Rc<RefCell<OpState>>, server_id: u32) {
  let close_tx = {
    let mut op_state = state.borrow_mut();
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
    ctx.cancel_handle.cancel();
    ctx.close_tx.clone()
  };
  let _ = close_tx.send(()).await;
}

#[op]
fn op_flash_path(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
) -> String {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  ctx
    .requests
    .get(&token)
    .unwrap()
    .inner
    .req
    .path
    .unwrap()
    .to_string()
}

#[inline]
fn next_request_sync(ctx: &mut ServerContext) -> u32 {
  let offset = ctx.next_token;

  while let Ok(token) = ctx.rx.try_recv() {
    ctx.requests.insert(ctx.next_token, token);
    ctx.next_token += 1;
  }

  ctx.next_token - offset
}

pub struct NextRequestFast;

impl fast_api::FastFunction for NextRequestFast {
  fn function(&self) -> *const c_void {
    op_flash_next_fast as *const c_void
  }

  fn args(&self) -> &'static [fast_api::Type] {
    &[fast_api::Type::V8Value]
  }

  fn return_type(&self) -> fast_api::CType {
    fast_api::CType::Uint32
  }
}

unsafe fn op_flash_next_fast(recv: v8::Local<v8::Object>) -> u32 {
  let ptr =
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX);
  let ctx = &mut *(ptr as *mut ServerContext);
  next_request_sync(ctx)
}

pub struct GetMethodFast;

impl fast_api::FastFunction for GetMethodFast {
  fn function(&self) -> *const c_void {
    op_flash_get_method_fast as *const c_void
  }

  fn args(&self) -> &'static [fast_api::Type] {
    &[fast_api::Type::V8Value, fast_api::Type::Uint32]
  }

  fn return_type(&self) -> fast_api::CType {
    fast_api::CType::Uint32
  }
}

unsafe fn op_flash_get_method_fast(
  recv: v8::Local<v8::Object>,
  token: u32,
) -> u32 {
  let ptr =
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX);
  let ctx = &mut *(ptr as *mut ServerContext);
  let req = ctx.requests.get_mut(&token).unwrap();
  get_method(req)
}

// Fast calls
#[op(v8)]
fn op_flash_make_request<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut OpState,
) -> serde_v8::Value<'scope> {
  let object_template = v8::ObjectTemplate::new(scope);
  assert!(object_template
    .set_internal_field_count((V8_WRAPPER_OBJECT_INDEX + 1) as usize));
  let obj = object_template.new_instance(scope).unwrap();
  let ctx = {
    let flash_ctx = state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&0).unwrap();
    ctx as *mut ServerContext
  };
  obj.set_aligned_pointer_in_internal_field(V8_WRAPPER_OBJECT_INDEX, ctx as _);

  // nextRequest
  {
    let builder = v8::FunctionTemplate::builder(
      |_: &mut v8::HandleScope,
       args: v8::FunctionCallbackArguments,
       mut rv: v8::ReturnValue| {
        let external: v8::Local<v8::External> = args.data().try_into().unwrap();
        // SAFETY: This external is guaranteed to be a pointer to a ServerContext
        let ctx = unsafe { &mut *(external.value() as *mut ServerContext) };
        rv.set_uint32(next_request_sync(ctx));
      },
    )
    .data(v8::External::new(scope, ctx as *mut _).into());

    let func = builder.build_fast(scope, &NextRequestFast, None);
    let func: v8::Local<v8::Value> = func.get_function(scope).unwrap().into();

    let key = v8::String::new(scope, "nextRequest").unwrap();
    obj.set(scope, key.into(), func).unwrap();
  }

  // getMethod
  {
    let builder = v8::FunctionTemplate::builder(
      |scope: &mut v8::HandleScope,
       args: v8::FunctionCallbackArguments,
       mut rv: v8::ReturnValue| {
        let external: v8::Local<v8::External> = args.data().try_into().unwrap();
        // SAFETY: This external is guaranteed to be a pointer to a ServerContext
        let ctx = unsafe { &mut *(external.value() as *mut ServerContext) };
        let token = args.get(0).uint32_value(scope).unwrap();
        let req = ctx.requests.get_mut(&token).unwrap();
        rv.set_uint32(get_method(req));
      },
    )
    .data(v8::External::new(scope, ctx as *mut _).into());

    let func = builder.build_fast(scope, &GetMethodFast, None);
    let func: v8::Local<v8::Value> = func.get_function(scope).unwrap().into();

    let key = v8::String::new(scope, "getMethod").unwrap();
    obj.set(scope, key.into(), func).unwrap();
  }

  // respond
  {
    let builder = v8::FunctionTemplate::builder(
      |scope: &mut v8::HandleScope,
       args: v8::FunctionCallbackArguments,
       mut rv: v8::ReturnValue| {
        let external: v8::Local<v8::External> = args.data().try_into().unwrap();
        // SAFETY: This external is guaranteed to be a pointer to a ServerContext
        let ctx = unsafe { &mut *(external.value() as *mut ServerContext) };

        let token = args.get(0).uint32_value(scope).unwrap();

        let response: v8::Local<v8::ArrayBufferView> =
          args.get(1).try_into().unwrap();
        let ab = response.buffer(scope).unwrap();
        let store = ab.get_backing_store();
        let (offset, len) = (response.byte_offset(), response.byte_length());
        // SAFETY: v8::SharedRef<v8::BackingStore> is similar to Arc<[u8]>,
        // it points to a fixed continuous slice of bytes on the heap.
        // We assume it's initialized and thus safe to read (though may not contain meaningful data)
        let response = unsafe {
          &*(&store[offset..offset + len] as *const _ as *const [u8])
        };

        let shutdown = args.get(2).boolean_value(scope);

        rv.set_uint32(flash_respond(ctx, token, shutdown, response));
      },
    )
    .data(v8::External::new(scope, ctx as *mut _).into());

    let func = builder.build_fast(scope, &RespondFast, None);
    let func: v8::Local<v8::Value> = func.get_function(scope).unwrap().into();

    let key = v8::String::new(scope, "respond").unwrap();
    obj.set(scope, key.into(), func).unwrap();
  }

  let value: v8::Local<v8::Value> = obj.into();
  value.into()
}

#[inline]
fn has_body_stream(req: &Request) -> bool {
  let sock = req.socket();
  sock.read_rx.is_some()
}

#[op]
fn op_flash_has_body_stream(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
) -> bool {
  let req = get_request!(op_state, server_id, token);
  has_body_stream(req)
}

#[op]
fn op_flash_headers(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
) -> Result<Vec<(ByteString, ByteString)>, AnyError> {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx
    .servers
    .get_mut(&server_id)
    .ok_or_else(|| type_error("server closed"))?;
  let inner_req = &ctx
    .requests
    .get(&token)
    .ok_or_else(|| type_error("request closed"))?
    .inner
    .req;
  Ok(
    inner_req
      .headers
      .iter()
      .map(|h| (h.name.as_bytes().into(), h.value.into()))
      .collect(),
  )
}

// Remember the first packet we read? It probably also has some body data. This op quickly copies it into
// a buffer and sets up channels for streaming the rest.
#[op]
fn op_flash_first_packet(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
) -> Result<Option<ZeroCopyBuf>, AnyError> {
  let tx = get_request!(op_state, server_id, token);
  let sock = tx.socket();

  if !tx.te_chunked && tx.content_length.is_none() {
    return Ok(None);
  }

  if tx.expect_continue {
    let _ = sock.write(b"HTTP/1.1 100 Continue\r\n\r\n");
    tx.expect_continue = false;
  }

  let buffer = &tx.inner.buffer[tx.inner.body_offset..tx.inner.body_len];
  // Oh there is nothing here.
  if buffer.is_empty() {
    return Ok(Some(ZeroCopyBuf::empty()));
  }

  if tx.te_chunked {
    let mut buf = vec![0; 1024];
    let mut offset = 0;
    let mut decoder = chunked::Decoder::new(
      std::io::Cursor::new(buffer),
      tx.remaining_chunk_size,
    );

    loop {
      match decoder.read(&mut buf[offset..]) {
        Ok(n) => {
          tx.remaining_chunk_size = decoder.remaining_chunks_size;
          offset += n;

          if n == 0 {
            tx.te_chunked = false;
            buf.truncate(offset);
            return Ok(Some(buf.into()));
          }

          if offset < buf.len()
            && decoder.source.position() < buffer.len() as u64
          {
            continue;
          }

          buf.truncate(offset);
          return Ok(Some(buf.into()));
        }
        Err(e) => {
          return Err(type_error(format!("{}", e)));
        }
      }
    }
  }

  tx.content_length
    .ok_or_else(|| type_error("no content-length"))?;
  tx.content_read += buffer.len();

  Ok(Some(buffer.to_vec().into()))
}

#[op]
async fn op_flash_read_body(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  mut buf: ZeroCopyBuf,
) -> usize {
  // SAFETY: we cannot hold op_state borrow across the await point. The JS caller
  // is responsible for ensuring this is not called concurrently.
  let ctx = unsafe {
    {
      let op_state = &mut state.borrow_mut();
      let flash_ctx = op_state.borrow_mut::<FlashContext>();
      flash_ctx.servers.get_mut(&server_id).unwrap() as *mut ServerContext
    }
    .as_mut()
    .unwrap()
  };
  let tx = ctx.requests.get_mut(&token).unwrap();

  if tx.te_chunked {
    let mut decoder =
      chunked::Decoder::new(tx.socket(), tx.remaining_chunk_size);
    loop {
      let sock = tx.socket();

      let _lock = sock.read_lock.lock().unwrap();
      match decoder.read(&mut buf) {
        Ok(n) => {
          tx.remaining_chunk_size = decoder.remaining_chunks_size;
          return n;
        }
        Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
          panic!("chunked read error: {}", e);
        }
        Err(_) => {
          drop(_lock);
          sock.read_rx.as_mut().unwrap().recv().await.unwrap();
        }
      }
    }
  }

  if let Some(content_length) = tx.content_length {
    let sock = tx.socket();
    let l = sock.read_lock.clone();

    loop {
      let _lock = l.lock().unwrap();
      if tx.content_read >= content_length as usize {
        return 0;
      }
      match sock.read(&mut buf) {
        Ok(n) => {
          tx.content_read += n;
          return n;
        }
        _ => {
          drop(_lock);
          sock.read_rx.as_mut().unwrap().recv().await.unwrap();
        }
      }
    }
  }

  0
}

// https://github.com/hyperium/hyper/blob/0c8ee93d7f557afc63ca2a5686d19071813ab2b7/src/headers.rs#L67
#[inline]
fn from_digits(bytes: &[u8]) -> Option<u64> {
  // cannot use FromStr for u64, since it allows a signed prefix
  let mut result = 0u64;
  const RADIX: u64 = 10;
  if bytes.is_empty() {
    return None;
  }
  for &b in bytes {
    // can't use char::to_digit, since we haven't verified these bytes
    // are utf-8.
    match b {
      b'0'..=b'9' => {
        result = result.checked_mul(RADIX)?;
        result = result.checked_add((b - b'0') as u64)?;
      }
      _ => {
        return None;
      }
    }
  }
  Some(result)
}

#[inline]
fn connection_has(value: &HeaderValue, needle: &str) -> bool {
  if let Ok(s) = value.to_str() {
    for val in s.split(',') {
      if val.trim().eq_ignore_ascii_case(needle) {
        return true;
      }
    }
  }
  false
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenOpts {
  cert: Option<String>,
  key: Option<String>,
  hostname: String,
  port: u16,
  reuseport: bool,
}

fn run_server(
  tx: mpsc::Sender<Request>,
  listening_tx: mpsc::Sender<u16>,
  mut close_rx: mpsc::Receiver<()>,
  addr: SocketAddr,
  maybe_cert: Option<String>,
  maybe_key: Option<String>,
  reuseport: bool,
) -> Result<(), AnyError> {
  let domain = if addr.is_ipv4() {
    socket2::Domain::IPV4
  } else {
    socket2::Domain::IPV6
  };
  let socket = Socket::new(domain, socket2::Type::STREAM, None)?;

  #[cfg(not(windows))]
  socket.set_reuse_address(true)?;
  if reuseport {
    #[cfg(target_os = "linux")]
    socket.set_reuse_port(true)?;
  }

  let socket_addr = socket2::SockAddr::from(addr);
  socket.bind(&socket_addr)?;
  socket.listen(128)?;
  socket.set_nonblocking(true)?;
  let std_listener: std::net::TcpListener = socket.into();
  let mut listener = TcpListener::from_std(std_listener);

  let mut poll = Poll::new()?;
  let token = Token(0);
  poll
    .registry()
    .register(&mut listener, token, Interest::READABLE)
    .unwrap();

  let tls_context: Option<Arc<rustls::ServerConfig>> = {
    if let Some(cert) = maybe_cert {
      let key = maybe_key.unwrap();
      let certificate_chain: Vec<rustls::Certificate> =
        load_certs(&mut BufReader::new(cert.as_bytes()))?;
      let private_key = load_private_keys(key.as_bytes())?.remove(0);

      let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .expect("invalid key or certificate");
      Some(Arc::new(config))
    } else {
      None
    }
  };

  listening_tx
    .blocking_send(listener.local_addr().unwrap().port())
    .unwrap();
  let mut sockets = HashMap::with_capacity(1000);
  let mut counter: usize = 1;
  let mut events = Events::with_capacity(1024);
  'outer: loop {
    let result = close_rx.try_recv();
    if result.is_ok() {
      break 'outer;
    }
    // FIXME(bartlomieju): how does Tokio handle it? I just put random 100ms
    // timeout here to handle close signal.
    match poll.poll(&mut events, Some(Duration::from_millis(100))) {
      Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
      Err(e) => panic!("{}", e),
      Ok(()) => (),
    }
    'events: for event in &events {
      if close_rx.try_recv().is_ok() {
        break 'outer;
      }
      let token = event.token();
      match token {
        Token(0) => loop {
          match listener.accept() {
            Ok((mut socket, _)) => {
              counter += 1;
              let token = Token(counter);
              poll
                .registry()
                .register(&mut socket, token, Interest::READABLE)
                .unwrap();

              let socket = match tls_context {
                Some(ref tls_conf) => {
                  let connection =
                    rustls::ServerConnection::new(tls_conf.clone()).unwrap();
                  InnerStream::Tls(Box::new(rustls::StreamOwned::new(
                    connection, socket,
                  )))
                }
                None => InnerStream::Tcp(socket),
              };
              let stream = Box::pin(Stream {
                inner: socket,
                detached: false,
                read_rx: None,
                read_tx: None,
                read_lock: Arc::new(Mutex::new(())),
                parse_done: ParseStatus::None,
                buffer: UnsafeCell::new(vec![0_u8; 1024]),
              });

              trace!("New connection: {}", token.0);
              sockets.insert(token, stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
          }
        },
        token => {
          let socket = sockets.get_mut(&token).unwrap();
          // SAFETY: guarantee that we will never move the data out of the mutable reference.
          let socket = unsafe {
            let mut_ref: Pin<&mut Stream> = Pin::as_mut(socket);
            Pin::get_unchecked_mut(mut_ref)
          };
          let sock_ptr = socket as *mut _;

          if socket.detached {
            match &mut socket.inner {
              InnerStream::Tcp(ref mut socket) => {
                poll.registry().deregister(socket).unwrap();
              }
              InnerStream::Tls(_) => {
                todo!("upgrade tls not implemented");
              }
            }

            let boxed = sockets.remove(&token).unwrap();
            std::mem::forget(boxed);
            trace!("Socket detached: {}", token.0);
            continue;
          }

          debug_assert!(event.is_readable());

          trace!("Socket readable: {}", token.0);
          if let Some(tx) = &socket.read_tx {
            {
              let _l = socket.read_lock.lock().unwrap();
            }
            trace!("Sending readiness notification: {}", token.0);
            let _ = tx.blocking_send(());

            continue;
          }

          let mut headers = vec![httparse::EMPTY_HEADER; 40];
          let mut req = httparse::Request::new(&mut headers);
          let body_offset;
          let body_len;
          loop {
            // SAFETY: It is safe for the read buf to be mutable here.
            let buffer = unsafe { &mut *socket.buffer.get() };
            let offset = match socket.parse_done {
              ParseStatus::None => 0,
              ParseStatus::Ongoing(offset) => offset,
            };
            if offset >= buffer.len() {
              buffer.resize(offset * 2, 0);
            }
            let nread = socket.read(&mut buffer[offset..]);

            match nread {
              Ok(0) => {
                trace!("Socket closed: {}", token.0);
                // FIXME: don't remove while JS is writing!
                // sockets.remove(&token);
                continue 'events;
              }
              Ok(read) => {
                match req.parse(&buffer[..offset + read]) {
                  Ok(httparse::Status::Complete(n)) => {
                    body_offset = n;
                    body_len = offset + read;
                    socket.parse_done = ParseStatus::None;
                    // On Windows, We must keep calling socket.read() until it fails with WouldBlock.
                    //
                    // Mio tries to emulate edge triggered events on Windows.
                    // AFAICT it only rearms the event on WouldBlock, but it doesn't when a partial read happens.
                    // https://github.com/denoland/deno/issues/15549
                    #[cfg(target_os = "windows")]
                    match &mut socket.inner {
                      InnerStream::Tcp(ref mut socket) => {
                        poll
                          .registry()
                          .reregister(socket, token, Interest::READABLE)
                          .unwrap();
                      }
                      InnerStream::Tls(ref mut socket) => {
                        poll
                          .registry()
                          .reregister(
                            &mut socket.sock,
                            token,
                            Interest::READABLE,
                          )
                          .unwrap();
                      }
                    };
                    break;
                  }
                  Ok(httparse::Status::Partial) => {
                    socket.parse_done = ParseStatus::Ongoing(offset + read);
                    continue;
                  }
                  Err(_) => {
                    let _ = socket.write(b"HTTP/1.1 400 Bad Request\r\n\r\n");
                    continue 'events;
                  }
                }
              }
              Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                break 'events
              }
              Err(_) => break 'events,
            }
          }

          debug_assert_eq!(socket.parse_done, ParseStatus::None);
          if let Some(method) = &req.method {
            if method == &"POST" || method == &"PUT" {
              let (tx, rx) = mpsc::channel(100);
              socket.read_tx = Some(tx);
              socket.read_rx = Some(rx);
            }
          }

          // SAFETY: It is safe for the read buf to be mutable here.
          let buffer = unsafe { &mut *socket.buffer.get() };
          let inner_req = InnerRequest {
            // SAFETY: backing buffer is pinned and lives as long as the request.
            req: unsafe { transmute::<httparse::Request<'_, '_>, _>(req) },
            // SAFETY: backing buffer is pinned and lives as long as the request.
            _headers: unsafe {
              transmute::<Vec<httparse::Header<'_>>, _>(headers)
            },
            buffer: Pin::new(
              replace(buffer, vec![0_u8; 1024]).into_boxed_slice(),
            ),
            body_offset,
            body_len,
          };
          // h1
          // https://github.com/tiny-http/tiny-http/blob/master/src/client.rs#L177
          // https://github.com/hyperium/hyper/blob/4545c3ef191ce9b5f5d250ee27c4c96f9b71d2c6/src/proto/h1/role.rs#L127
          let mut keep_alive = inner_req.req.version.unwrap() == 1;
          let mut expect_continue = false;
          let mut te = false;
          let mut te_chunked = false;
          let mut content_length = None;
          for header in inner_req.req.headers.iter() {
            match HeaderName::from_bytes(header.name.as_bytes()) {
              Ok(CONNECTION) => {
                // SAFETY: illegal bytes are validated by httparse.
                let value = unsafe {
                  HeaderValue::from_maybe_shared_unchecked(header.value)
                };
                if keep_alive {
                  // 1.1
                  keep_alive = !connection_has(&value, "close");
                } else {
                  // 1.0
                  keep_alive = connection_has(&value, "keep-alive");
                }
              }
              Ok(TRANSFER_ENCODING) => {
                // https://tools.ietf.org/html/rfc7230#section-3.3.3
                debug_assert!(inner_req.req.version.unwrap() == 1);
                // Two states for Transfer-Encoding because we want to make sure Content-Length handling knows it.
                te = true;
                content_length = None;
                // SAFETY: illegal bytes are validated by httparse.
                let value = unsafe {
                  HeaderValue::from_maybe_shared_unchecked(header.value)
                };
                if let Ok(Some(encoding)) =
                  value.to_str().map(|s| s.rsplit(',').next())
                {
                  // Chunked must always be the last encoding
                  if encoding.trim().eq_ignore_ascii_case("chunked") {
                    te_chunked = true;
                  }
                }
              }
              // Transfer-Encoding overrides the Content-Length.
              Ok(CONTENT_LENGTH) if !te => {
                if let Some(len) = from_digits(header.value) {
                  if let Some(prev) = content_length {
                    if prev != len {
                      let _ = socket.write(b"HTTP/1.1 400 Bad Request\r\n\r\n");
                      continue 'events;
                    }
                    continue;
                  }
                  content_length = Some(len);
                } else {
                  let _ = socket.write(b"HTTP/1.1 400 Bad Request\r\n\r\n");
                  continue 'events;
                }
              }
              Ok(EXPECT) if inner_req.req.version.unwrap() != 0 => {
                expect_continue =
                  header.value.eq_ignore_ascii_case(b"100-continue");
              }
              _ => {}
            }
          }

          // There is Transfer-Encoding but its not chunked.
          if te && !te_chunked {
            let _ = socket.write(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            continue 'events;
          }

          tx.blocking_send(Request {
            socket: sock_ptr,
            // SAFETY: headers backing buffer outlives the mio event loop ('static)
            inner: inner_req,
            keep_alive,
            te_chunked,
            remaining_chunk_size: None,
            content_read: 0,
            content_length,
            expect_continue,
          })
          .ok();
        }
      }
    }
  }

  Ok(())
}

fn make_addr_port_pair(hostname: &str, port: u16) -> (&str, u16) {
  // Default to localhost if given just the port. Example: ":80"
  if hostname.is_empty() {
    return ("0.0.0.0", port);
  }

  // If this looks like an ipv6 IP address. Example: "[2001:db8::1]"
  // Then we remove the brackets.
  let addr = hostname.trim_start_matches('[').trim_end_matches(']');
  (addr, port)
}

/// Resolve network address *synchronously*.
pub fn resolve_addr_sync(
  hostname: &str,
  port: u16,
) -> Result<impl Iterator<Item = SocketAddr>, AnyError> {
  let addr_port_pair = make_addr_port_pair(hostname, port);
  let result = addr_port_pair.to_socket_addrs()?;
  Ok(result)
}

fn flash_serve<P>(
  state: &mut OpState,
  opts: ListenOpts,
) -> Result<u32, AnyError>
where
  P: FlashPermissions + 'static,
{
  state
    .borrow_mut::<P>()
    .check_net(&(&opts.hostname, Some(opts.port)), "Deno.serve()")?;

  let addr = resolve_addr_sync(&opts.hostname, opts.port)?
    .next()
    .ok_or_else(|| generic_error("No resolved address found"))?;
  let (tx, rx) = mpsc::channel(100);
  let (close_tx, close_rx) = mpsc::channel(1);
  let (listening_tx, listening_rx) = mpsc::channel(1);
  let ctx = ServerContext {
    _addr: addr,
    tx,
    rx,
    requests: HashMap::with_capacity(1000),
    next_token: 0,
    close_tx,
    listening_rx: Some(listening_rx),
    cancel_handle: CancelHandle::new_rc(),
  };
  let tx = ctx.tx.clone();
  let maybe_cert = opts.cert;
  let maybe_key = opts.key;
  let reuseport = opts.reuseport;
  let join_handle = tokio::task::spawn_blocking(move || {
    run_server(
      tx,
      listening_tx,
      close_rx,
      addr,
      maybe_cert,
      maybe_key,
      reuseport,
    )
  });
  let flash_ctx = state.borrow_mut::<FlashContext>();
  let server_id = flash_ctx.next_server_id;
  flash_ctx.next_server_id += 1;
  flash_ctx.join_handles.insert(server_id, join_handle);
  flash_ctx.servers.insert(server_id, ctx);
  Ok(server_id)
}

#[op]
fn op_flash_serve<P>(
  state: &mut OpState,
  opts: ListenOpts,
) -> Result<u32, AnyError>
where
  P: FlashPermissions + 'static,
{
  check_unstable(state, "Deno.serve");
  flash_serve::<P>(state, opts)
}

#[op]
fn op_node_unstable_flash_serve<P>(
  state: &mut OpState,
  opts: ListenOpts,
) -> Result<u32, AnyError>
where
  P: FlashPermissions + 'static,
{
  flash_serve::<P>(state, opts)
}

#[op]
fn op_flash_wait_for_listening(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
) -> Result<impl Future<Output = Result<u16, AnyError>> + 'static, AnyError> {
  let mut listening_rx = {
    let mut state = state.borrow_mut();
    let flash_ctx = state.borrow_mut::<FlashContext>();
    let server_ctx = flash_ctx
      .servers
      .get_mut(&server_id)
      .ok_or_else(|| type_error("server not found"))?;
    server_ctx.listening_rx.take().unwrap()
  };
  Ok(async move {
    if let Some(port) = listening_rx.recv().await {
      Ok(port)
    } else {
      Err(generic_error("This error will be discarded"))
    }
  })
}

#[op]
fn op_flash_drive_server(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
) -> Result<impl Future<Output = Result<(), AnyError>> + 'static, AnyError> {
  let join_handle = {
    let mut state = state.borrow_mut();
    let flash_ctx = state.borrow_mut::<FlashContext>();
    flash_ctx
      .join_handles
      .remove(&server_id)
      .ok_or_else(|| type_error("server not found"))?
  };
  Ok(async move {
    join_handle
      .await
      .map_err(|_| type_error("server join error"))??;
    Ok(())
  })
}

// Asychronous version of op_flash_next. This can be a bottleneck under
// heavy load, it should be used as a fallback if there are no buffered
// requests i.e `op_flash_next() == 0`.
#[op]
async fn op_flash_next_async(
  op_state: Rc<RefCell<OpState>>,
  server_id: u32,
) -> u32 {
  let ctx = {
    let mut op_state = op_state.borrow_mut();
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
    ctx as *mut ServerContext
  };
  // SAFETY: we cannot hold op_state borrow across the await point. The JS caller
  // is responsible for ensuring this is not called concurrently.
  let ctx = unsafe { &mut *ctx };
  let cancel_handle = &ctx.cancel_handle;

  if let Ok(Some(req)) = ctx.rx.recv().or_cancel(cancel_handle).await {
    ctx.requests.insert(ctx.next_token, req);
    ctx.next_token += 1;
    return 1;
  }

  0
}

// Synchronous version of op_flash_next_async. Under heavy load,
// this can collect buffered requests from rx channel and return tokens in a single batch.
//
// perf: please do not add any arguments to this op. With optimizations enabled,
// the ContextScope creation is optimized away and the op is as simple as:
//   f(info: *const v8::FunctionCallbackInfo) { let rv = ...; rv.set_uint32(op_flash_next()); }
#[op]
fn op_flash_next(state: &mut OpState) -> u32 {
  let flash_ctx = state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&0).unwrap();
  next_request_sync(ctx)
}

// Syncrhonous version of op_flash_next_async. Under heavy load,
// this can collect buffered requests from rx channel and return tokens in a single batch.
#[op]
fn op_flash_next_server(state: &mut OpState, server_id: u32) -> u32 {
  let flash_ctx = state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  next_request_sync(ctx)
}

// Wrapper type for tokio::net::TcpStream that implements
// deno_websocket::UpgradedStream
struct UpgradedStream(tokio::net::TcpStream);
impl tokio::io::AsyncRead for UpgradedStream {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut tokio::io::ReadBuf,
  ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
  }
}

impl tokio::io::AsyncWrite for UpgradedStream {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
  }
  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_flush(cx)
  }
  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
  }
}

impl deno_websocket::Upgraded for UpgradedStream {}

#[inline]
pub fn detach_socket(
  ctx: &mut ServerContext,
  token: u32,
) -> Result<tokio::net::TcpStream, AnyError> {
  // Two main 'hacks' to get this working:
  //   * make server thread forget about the socket. `detach_ownership` prevents the socket from being
  //      dropped on the server thread.
  //   * conversion from mio::net::TcpStream -> tokio::net::TcpStream.  There is no public API so we
  //      use raw fds.
  let tx = ctx
    .requests
    .remove(&token)
    .ok_or_else(|| type_error("request closed"))?;
  let stream = tx.socket();
  // prevent socket from being dropped on server thread.
  // TODO(@littledivy): Box-ify, since there is no overhead.
  stream.detach_ownership();

  #[cfg(unix)]
  let std_stream = {
    use std::os::unix::prelude::AsRawFd;
    use std::os::unix::prelude::FromRawFd;
    let fd = match stream.inner {
      InnerStream::Tcp(ref tcp) => tcp.as_raw_fd(),
      _ => todo!(),
    };
    // SAFETY: `fd` is a valid file descriptor.
    unsafe { std::net::TcpStream::from_raw_fd(fd) }
  };
  #[cfg(windows)]
  let std_stream = {
    use std::os::windows::prelude::AsRawSocket;
    use std::os::windows::prelude::FromRawSocket;
    let fd = match stream.inner {
      InnerStream::Tcp(ref tcp) => tcp.as_raw_socket(),
      _ => todo!(),
    };
    // SAFETY: `fd` is a valid file descriptor.
    unsafe { std::net::TcpStream::from_raw_socket(fd) }
  };
  let stream = tokio::net::TcpStream::from_std(std_stream)?;
  Ok(stream)
}

#[op]
async fn op_flash_upgrade_websocket(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
) -> Result<deno_core::ResourceId, AnyError> {
  let stream = {
    let op_state = &mut state.borrow_mut();
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    detach_socket(flash_ctx.servers.get_mut(&server_id).unwrap(), token)?
  };
  deno_websocket::ws_create_server_stream(
    &state,
    Box::pin(UpgradedStream(stream)),
  )
  .await
}

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

pub trait FlashPermissions {
  fn check_net<T: AsRef<str>>(
    &mut self,
    _host: &(T, Option<u16>),
    _api_name: &str,
  ) -> Result<(), AnyError>;
}

pub fn init<P: FlashPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec![
      "deno_web",
      "deno_net",
      "deno_fetch",
      "deno_websocket",
      "deno_http",
    ])
    .js(deno_core::include_js_files!(
      prefix "deno:ext/flash",
      "01_http.js",
    ))
    .ops(vec![
      op_flash_serve::decl::<P>(),
      op_node_unstable_flash_serve::decl::<P>(),
      op_flash_respond::decl(),
      op_flash_respond_async::decl(),
      op_flash_respond_chunked::decl(),
      op_flash_method::decl(),
      op_flash_path::decl(),
      op_flash_headers::decl(),
      op_flash_next::decl(),
      op_flash_next_server::decl(),
      op_flash_next_async::decl(),
      op_flash_read_body::decl(),
      op_flash_upgrade_websocket::decl(),
      op_flash_drive_server::decl(),
      op_flash_wait_for_listening::decl(),
      op_flash_first_packet::decl(),
      op_flash_has_body_stream::decl(),
      op_flash_close_server::decl(),
      op_flash_make_request::decl(),
      op_flash_write_resource::decl(),
      op_try_flash_respond_chunked::decl(),
    ])
    .state(move |op_state| {
      op_state.put(Unstable(unstable));
      op_state.put(FlashContext {
        next_server_id: 0,
        join_handles: HashMap::default(),
        servers: HashMap::default(),
      });
      Ok(())
    })
    .build()
}
