// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::missing_safety_doc)]

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::v8::fast_api;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use deno_core::V8_WRAPPER_OBJECT_INDEX;
use http::header::HeaderName;
use http::header::CONNECTION;
use http::header::CONTENT_LENGTH;
use http::header::EXPECT;
use http::header::TRANSFER_ENCODING;
use http::header::UPGRADE;
use http::HeaderValue;
use log::trace;
use mio::net::TcpListener;
use mio::net::TcpStream;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::future::Future;
use std::intrinsics::transmute;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::marker::PhantomPinned;
use std::mem::replace;
use std::net::SocketAddr;
use std::os::unix::prelude::AsRawFd;
use std::os::unix::prelude::FromRawFd;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

mod chunked;
enum Encoding {
  Identity,
  Gzip,
  Brotli,
}

pub struct FlashContext {
  next_server_id: u32,
  join_handles: HashMap<u32, JoinHandle<()>>,
  pub servers: HashMap<u32, ServerContext>,
}

pub struct ServerContext {
  addr: SocketAddr,
  tx: mpsc::Sender<NextRequest>,
  rx: mpsc::Receiver<NextRequest>,
  response: HashMap<u32, NextRequest>,
  close_tx: mpsc::Sender<()>,
  cancel_handle: Rc<CancelHandle>,
}

struct InnerRequest {
  _headers: Vec<httparse::Header<'static>>,
  req: httparse::Request<'static, 'static>,
  body_offset: usize,
  body_len: usize,
  buffer: Pin<Box<[u8]>>,
}

#[derive(Debug, PartialEq)]
enum ParseStatus {
  None,
  Ongoing(usize),
}

struct Stream {
  inner: TcpStream,
  detached: bool,
  read_rx: Option<mpsc::Receiver<()>>,
  read_tx: Option<mpsc::Sender<()>>,
  parse_done: ParseStatus,
  buffer: UnsafeCell<Vec<u8>>,
  read_lock: Arc<Mutex<()>>,
  _pin: PhantomPinned,
}

impl Stream {
  pub fn detach_ownership(&mut self) {
    self.detached = true;
  }

  fn reattach_ownership(&mut self) {
    self.detached = false;
  }
}

impl Write for Stream {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self.inner.write(buf)
  }
  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    self.inner.flush()
  }
}

impl Read for Stream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    self.inner.read(buf)
  }
}

struct NextRequest {
  // Pointer to stream owned by the server loop thread.
  //
  // Why not Arc<Mutex<Stream>>? Performance. The stream
  // is never written to by the server loop thread.
  //
  // Dereferencing is safe until server thread finishes and
  // op_flash_serve resolves or websocket upgrade is performed.
  socket: *mut Stream,
  inner: InnerRequest,
  keep_alive: bool,
  #[allow(dead_code)]
  upgrade: bool,
  content_read: usize,
  content_length: Option<u64>,
  remaining_chunk_size: Option<usize>,
  te_chunked: bool,
  expect_continue: bool,
}

// SAFETY: Sent from server thread to JS thread.
// See comment above for `socket`.
unsafe impl Send for NextRequest {}

#[op]
fn op_flash_respond(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
  response: StringOrBuffer,
  maybe_body: Option<ZeroCopyBuf>,
  shutdown: bool,
) {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();

  let mut close = false;
  let sock = match shutdown {
    true => {
      let tx = ctx.response.remove(&token).unwrap();
      close = !tx.keep_alive;
      unsafe { &mut *tx.socket }
    }
    // In case of a websocket upgrade or streaming response.
    false => {
      let tx = ctx.response.get(&token).unwrap();
      unsafe { &mut *tx.socket }
    }
  };

  sock.read_tx.take();
  sock.read_rx.take();

  let _ = sock.write(&response);
  if let Some(response) = maybe_body {
    let _ = sock.write(format!("{:x}", response.len()).as_bytes());
    let _ = sock.write(b"\r\n");
    let _ = sock.write(&response);
    let _ = sock.write(b"\r\n");
  }

  // server is done writing and request doesn't want to kept alive.
  if shutdown && close {
    let _ = sock.inner.shutdown(std::net::Shutdown::Both); // Typically shutdown shouldn't fail.
  }
}

#[op]
fn op_flash_respond_chuncked(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
  response: Option<ZeroCopyBuf>,
  shutdown: bool,
) {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  match response {
    Some(response) => {
      respond_chunked(ctx, token, shutdown, Some(&response));
    }
    None => {
      respond_chunked(ctx, token, shutdown, None);
    }
  }
}

pub struct RespondChunkedFast;

impl fast_api::FastFunction for RespondChunkedFast {
  fn function(&self) -> *const c_void {
    op_flash_respond_chunked_fast as *const c_void
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
    fast_api::CType::Void
  }
}

fn op_flash_respond_chunked_fast(
  recv: v8::Local<v8::Object>,
  token: u32,
  response: *const fast_api::FastApiTypedArray<u8>,
  shutdown: bool,
) {
  let ptr = unsafe {
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX)
  };
  let ctx = unsafe { &mut *(ptr as *mut ServerContext) };

  let response = unsafe { &*response };
  if let Some(response) = response.get_storage_if_aligned() {
    respond_chunked(ctx, token, shutdown, Some(response));
  } else {
    todo!();
  }
}

fn respond_chunked(
  ctx: &mut ServerContext,
  token: u32,
  shutdown: bool,
  response: Option<&[u8]>,
) {
  let sock = match shutdown {
    true => {
      let tx = ctx.response.remove(&token).unwrap();
      unsafe { &mut *tx.socket }
    }
    // In case of a websocket upgrade or streaming response.
    false => {
      let tx = ctx.response.get(&token).unwrap();
      unsafe { &mut *tx.socket }
    }
  };

  if let Some(response) = response {
    let _ = sock.write(format!("{:x}", response.len()).as_bytes());
    let _ = sock.write(b"\r\n");
    let _ = sock.write(response);
    let _ = sock.write(b"\r\n");
  }

  // The last chunk
  if shutdown {
    let _ = sock.write(b"0\r\n\r\n");
  }
  sock.reattach_ownership();
}

#[op]
async fn op_flash_respond_stream(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  data: String,
  rid: u32,
) -> Result<(), AnyError> {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let tx = ctx.response.remove(&token).unwrap();
  let sock = unsafe { &mut *tx.socket };
  let _n = sock.write(data.as_bytes()).unwrap();
  let resource = state.borrow().resource_table.get_any(rid)?;
  loop {
    let vec = vec![0u8; 64 * 1024]; // 64KB
    let buf = ZeroCopyBuf::new_temp(vec);
    let (nread, buf) = resource.clone().read_return(buf).await?;
    if nread == 0 {
      break;
    }
    let _n = sock.write(&buf[..nread])?;
  }
  Ok(())
}

#[op]
fn op_flash_method(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
) -> String {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  ctx
    .response
    .get(&token)
    .unwrap()
    .inner
    .req
    .method
    .unwrap()
    .to_string()
}

#[op]
async fn op_flash_close_server(state: Rc<RefCell<OpState>>, server_id: u32) {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  ctx.cancel_handle.cancel();
  let _ = ctx.close_tx.send(()).await;
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
    .response
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
  let mut tokens = 0;
  while let Ok(token) = ctx.rx.try_recv() {
    ctx.response.insert(tokens, token);
    tokens += 1;
  }
  tokens
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

fn op_flash_next_fast(recv: v8::Local<v8::Object>) -> u32 {
  let ptr = unsafe {
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX)
  };
  let ctx = unsafe { &mut *(ptr as *mut ServerContext) };

  next_request_sync(ctx)
}

pub struct HasBodyFast;

impl fast_api::FastFunction for HasBodyFast {
  fn function(&self) -> *const c_void {
    op_flash_has_body_fast as *const c_void
  }

  fn args(&self) -> &'static [fast_api::Type] {
    &[fast_api::Type::V8Value, fast_api::Type::Uint32]
  }

  fn return_type(&self) -> fast_api::CType {
    fast_api::CType::Bool
  }
}

fn op_flash_has_body_fast(recv: v8::Local<v8::Object>, token: u32) -> bool {
  let ptr = unsafe {
    recv.get_aligned_pointer_from_internal_field(V8_WRAPPER_OBJECT_INDEX)
  };
  let ctx = unsafe { &mut *(ptr as *mut ServerContext) };
  let resp = ctx.response.get(&token).unwrap();
  let sock = unsafe { &*resp.socket };

  sock.read_rx.is_some()
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
        let external: v8::Local<v8::External> =
          args.data().unwrap().try_into().unwrap();
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

  // hasBody
  {
    let builder = v8::FunctionTemplate::builder(
      |scope: &mut v8::HandleScope,
       args: v8::FunctionCallbackArguments,
       mut rv: v8::ReturnValue| {
        let external: v8::Local<v8::External> =
          args.data().unwrap().try_into().unwrap();
        let ctx = unsafe { &mut *(external.value() as *mut ServerContext) };
        let token = args.get(0).uint32_value(scope).unwrap();
        let resp = ctx.response.get(&token).unwrap();
        let sock = unsafe { &*resp.socket };

        rv.set_bool(sock.read_rx.is_some());
      },
    )
    .data(v8::External::new(scope, ctx as *mut _).into());

    let func = builder.build_fast(scope, &HasBodyFast, None);
    let func: v8::Local<v8::Value> = func.get_function(scope).unwrap().into();

    let key = v8::String::new(scope, "hasBody").unwrap();
    obj.set(scope, key.into(), func).unwrap();
  }

  // respondChunked
  {
    let builder = v8::FunctionTemplate::builder(
      |scope: &mut v8::HandleScope,
       args: v8::FunctionCallbackArguments,
       _: v8::ReturnValue| {
        let external: v8::Local<v8::External> =
          args.data().unwrap().try_into().unwrap();
        let ctx = unsafe { &mut *(external.value() as *mut ServerContext) };

        let token = args.get(0).uint32_value(scope).unwrap();

        let response: v8::Local<v8::ArrayBufferView> =
          args.get(1).try_into().unwrap();
        let ab = response.buffer(scope).unwrap();
        let store = ab.get_backing_store();
        let (offset, len) = (response.byte_offset(), response.byte_length());
        let response = unsafe {
          &*(&store[offset..offset + len] as *const _ as *const [u8])
        };

        let shutdown = args.get(2).boolean_value(scope);

        respond_chunked(ctx, token, shutdown, Some(response));
      },
    )
    .data(v8::External::new(scope, ctx as *mut _).into());

    let func = builder.build_fast(scope, &RespondChunkedFast, None);
    let func: v8::Local<v8::Value> = func.get_function(scope).unwrap().into();

    let key = v8::String::new(scope, "respondChunked").unwrap();
    obj.set(scope, key.into(), func).unwrap();
  }

  let value: v8::Local<v8::Value> = obj.into();
  value.into()
}

#[op]
fn op_flash_has_body_stream_0(op_state: &mut OpState, token: u32) -> bool {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&0).unwrap();
  let resp = ctx.response.get(&token).unwrap();
  let sock = unsafe { &*resp.socket };
  sock.read_rx.is_some()
}

#[op]
fn op_flash_has_body_stream(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
) -> bool {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();

  let resp = ctx.response.get(&token).unwrap();
  let sock = unsafe { &*resp.socket };
  sock.read_rx.is_some()
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
    .response
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
) -> ZeroCopyBuf {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let tx = ctx.response.get_mut(&token).unwrap();
  // SAFETY: The socket lives on the mio thread.
  let sock = unsafe { &mut *tx.socket };

  let buffer = &tx.inner.buffer[tx.inner.body_offset..tx.inner.body_len];
  // Oh there is nothing here.
  if buffer.is_empty() {
    return ZeroCopyBuf::empty();
  }

  if tx.expect_continue {
    let _ = sock.write(b"HTTP/1.1 100 Continue\r\n\r\n");
    tx.expect_continue = false;
  }

  if tx.te_chunked {
    let mut buf = vec![0; 1024];
    let mut decoder = chunked::Decoder::new(buffer, tx.remaining_chunk_size);
    if let Ok(n) = decoder.read(&mut buf) {
      tx.remaining_chunk_size = decoder.remaining_chunks_size;
      buf.truncate(n);
      return buf.into();
    } else {
      panic!("chunked read error");
    }
  }

  // if tx.inner.body_offset != 0 && tx.inner.body_offset != tx.inner.body_len {
  //   let buffer = &tx.inner.buffer[tx.inner.body_offset..tx.inner.body_len];
  //   let cursor = Cursor::new(buffer);
  //   let mut decoder = chunked::Decoder::new(cursor);

  //   let nread = decoder.read(&mut buf).expect("read error");
  //   let cursor = decoder.into_inner();
  //   let pos = cursor.position() as usize;

  //   if pos == tx.inner.body_len {
  //     tx.inner.body_offset = 0;
  //   } else {
  //     tx.inner.body_offset += pos;
  //   }
  //   return nread;
  // }

  tx.content_read += buffer.len();

  buffer.to_vec().into()
}

#[op]
async fn op_flash_read_body(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  mut buf: ZeroCopyBuf,
) -> usize {
  let ctx = unsafe {
    {
      let op_state = &mut state.borrow_mut();
      let flash_ctx = op_state.borrow_mut::<FlashContext>();
      flash_ctx.servers.get_mut(&server_id).unwrap() as *mut ServerContext
    }
    .as_mut()
    .unwrap()
  };
  let tx = ctx.response.get_mut(&token).unwrap();

  if tx.te_chunked {
    let mut first = true;
    loop {
      // SAFETY: The socket lives on the mio thread.
      let sock = unsafe { &mut *tx.socket };
      if !first {
        sock.read_rx.as_mut().unwrap().recv().await.unwrap();
      }

      first = false;
      let l = sock.read_lock.clone();
      let _lock = l.lock().unwrap();
      let mut decoder = chunked::Decoder::new(sock, tx.remaining_chunk_size);
      if let Ok(n) = decoder.read(&mut buf) {
        tx.remaining_chunk_size = decoder.remaining_chunks_size;
        return n;
      }
      tx.remaining_chunk_size = decoder.remaining_chunks_size;
    }
  }

  // SAFETY: The socket lives on the mio thread.
  let sock = unsafe { &mut *tx.socket };
  let l = sock.read_lock.clone();

  loop {
    let _lock = l.lock().unwrap();
    if tx.content_read >= tx.content_length.unwrap() as usize {
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
pub struct ListenOpts {
  cert: Option<String>,
  key: Option<String>,
  hostname: String,
  port: u16,
}

fn run_server(
  tx: mpsc::Sender<NextRequest>,
  mut close_rx: mpsc::Receiver<()>,
  addr: SocketAddr,
  maybe_cert: Option<String>,
  maybe_key: Option<String>,
) {
  let mut listener = TcpListener::bind(addr).unwrap();
  let mut poll = Poll::new().unwrap();
  let token = Token(0);
  poll
    .registry()
    .register(&mut listener, token, Interest::READABLE)
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
              let stream = Box::pin(Stream {
                inner: socket,
                detached: false,
                read_rx: None,
                read_tx: None,
                read_lock: Arc::new(Mutex::new(())),
                parse_done: ParseStatus::None,
                buffer: UnsafeCell::new(vec![0_u8; 1024]),
                _pin: PhantomPinned,
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
          let socket = unsafe {
            let mut_ref: Pin<&mut Stream> = Pin::as_mut(socket);
            Pin::get_unchecked_mut(mut_ref)
          };
          let sock_ptr = socket as *mut _;

          if socket.detached {
            poll.registry().deregister(&mut socket.inner).unwrap();
            sockets.remove(&token).unwrap();
            println!("Socket detached: {}", token.0);
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
                sockets.remove(&token);
                continue 'events;
              }
              Ok(read) => match req.parse(&buffer[..offset + read]) {
                Ok(httparse::Status::Complete(n)) => {
                  body_offset = n;
                  body_len = offset + read;
                  socket.parse_done = ParseStatus::None;
                  break;
                }
                Ok(httparse::Status::Partial) => {
                  socket.parse_done = ParseStatus::Ongoing(offset + read);
                  continue;
                }
                Err(e) => {
                  panic!("{}", e);
                }
                _ => unreachable!(),
              },
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
            req: unsafe { transmute::<httparse::Request<'_, '_>, _>(req) },
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
          let mut upgrade = false;
          #[allow(unused_variables)]
          let mut expect_continue = false;
          let mut te = false;
          let mut te_chunked = false;
          let mut content_length = None;
          for header in inner_req.req.headers.iter() {
            match HeaderName::from_bytes(header.name.as_bytes()) {
              Ok(CONNECTION) => {
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
              Ok(UPGRADE) => {
                upgrade = inner_req.req.version.unwrap() == 1;
              }
              Ok(TRANSFER_ENCODING) => {
                // https://tools.ietf.org/html/rfc7230#section-3.3.3
                debug_assert!(inner_req.req.version.unwrap() == 1);
                // Two states for Transfer-Encoding because we want to make sure Content-Length handling knows it.
                te = true;
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
              Ok(CONTENT_LENGTH) => {
                // Transfer-Encoding overrides the Content-Length.
                if te {
                  // request smuggling detected ;)
                  continue;
                }
                // TODO: Must respond with 400 and close conneciton if no TE and invalid / multiple Content-Length headers.
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
              Ok(EXPECT) => {
                // TODO: Must ignore if HTTP/1.0
                #[allow(unused_assignments)]
                {
                  expect_continue =
                    header.value.eq_ignore_ascii_case(b"100-continue");
                }
              }
              _ => {}
            }
          }
          tx.blocking_send(NextRequest {
            socket: sock_ptr,
            // SAFETY: headers backing buffer outlives the mio event loop ('static)
            inner: inner_req,
            keep_alive,
            upgrade,
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
}

#[op]
fn op_flash_serve(
  state: &mut OpState,
  opts: ListenOpts,
) -> Result<u32, AnyError> {
  let addr = SocketAddr::new(opts.hostname.parse()?, opts.port);
  let (tx, rx) = mpsc::channel(100);
  let (close_tx, close_rx) = mpsc::channel(1);
  let ctx = ServerContext {
    addr,
    tx,
    rx,
    response: HashMap::with_capacity(1000),
    close_tx,
    cancel_handle: CancelHandle::new_rc(),
  };
  let tx = ctx.tx.clone();
  let maybe_cert = opts.cert;
  let maybe_key = opts.key;
  let join_handle = tokio::task::spawn_blocking(move || {
    run_server(tx, close_rx, addr, maybe_cert, maybe_key)
  });
  let flash_ctx = state.borrow_mut::<FlashContext>();
  let server_id = flash_ctx.next_server_id;
  flash_ctx.next_server_id += 1;
  flash_ctx.join_handles.insert(server_id, join_handle);
  flash_ctx.servers.insert(server_id, ctx);
  Ok(server_id)
}

#[op]
fn op_flash_drive_server(
  state: &mut OpState,
  server_id: u32,
) -> impl Future<Output = ()> + 'static {
  let join_handle = {
    let flash_ctx = state.borrow_mut::<FlashContext>();
    flash_ctx.join_handles.remove(&server_id).unwrap()
  };
  async move {
    join_handle.await.unwrap();
  }
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
  let ctx = unsafe { &mut *ctx };
  let cancel_handle = &ctx.cancel_handle;
  let mut tokens = 0;
  while let Ok(token) = ctx.rx.try_recv() {
    ctx.response.insert(tokens, token);
    tokens += 1;
  }
  if tokens == 0 {
    if let Ok(Some(req)) = ctx.rx.recv().or_cancel(cancel_handle).await {
      ctx.response.insert(tokens, req);
      tokens += 1;
    }
  }
  tokens
}

// Syncrhonous version of op_flash_next_async. Under heavy load,
// this can collect buffered requests from rx channel and return tokens in a single batch.
//
// perf: please do not add any arguments to this op. With optimizations enabled,
// the ContextScope creation is optimized away and the op is as simple as:
//   f(info: *const v8::FunctionCallbackInfo) { let rv = ...; rv.set_uint32(op_flash_next()); }
#[op]
fn op_flash_next(op_state: &mut OpState) -> u32 {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&0).unwrap();
  next_request_sync(ctx)
}

// Syncrhonous version of op_flash_next_async. Under heavy load,
// this can collect buffered requests from rx channel and return tokens in a single batch.
#[op]
fn op_flash_next_server(op_state: &mut OpState, server_id: u32) -> u32 {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
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
    .response
    .remove(&token)
    .ok_or_else(|| type_error("request closed"))?;
  // SAFETY: Stream is owned by server thread.
  let stream = unsafe { &mut *tx.socket };
  // prevent socket from being dropped on server thread.
  // TODO(@littledivy): Box-ify, since there is no overhead.
  stream.detach_ownership();

  let fd = stream.inner.as_raw_fd();
  // SAFETY: `fd` is a valid file descriptor.
  let std_stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };
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

pub fn init() -> Extension {
  Extension::builder()
    .js(deno_core::include_js_files!(
      prefix "deno:ext/flash",
      "01_http.js",
    ))
    .ops(vec![
      op_flash_serve::decl(),
      op_flash_respond::decl(),
      op_flash_respond_chuncked::decl(),
      op_flash_method::decl(),
      op_flash_path::decl(),
      op_flash_headers::decl(),
      op_flash_respond_stream::decl(),
      op_flash_next::decl(),
      op_flash_next_server::decl(),
      op_flash_next_async::decl(),
      op_flash_read_body::decl(),
      op_flash_upgrade_websocket::decl(),
      op_flash_drive_server::decl(),
      op_flash_first_packet::decl(),
      op_flash_has_body_stream::decl(),
      op_flash_has_body_stream_0::decl(),
      op_flash_close_server::decl(),
      op_flash_make_request::decl(),
    ])
    .state(|op_state| {
      op_state.put(FlashContext {
        next_server_id: 0,
        join_handles: HashMap::default(),
        servers: HashMap::default(),
      });
      Ok(())
    })
    .build()
}
