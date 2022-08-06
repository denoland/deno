// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::missing_safety_doc)]

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ByteString;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use http::header::HeaderName;
use http::header::CONNECTION;
use http::header::CONTENT_LENGTH;
use http::header::EXPECT;
use http::header::TRANSFER_ENCODING;
use http::header::UPGRADE;
use http::HeaderValue;
use mio::net::TcpListener;
use mio::net::TcpStream;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::intrinsics::transmute;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::os::unix::prelude::AsRawFd;
use std::os::unix::prelude::FromRawFd;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use tokio::sync::mpsc;
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
  servers: HashMap<u32, ServerContext>,
}

pub struct ServerContext {
  addr: SocketAddr,
  tx: mpsc::Sender<NextRequest>,
  rx: mpsc::Receiver<NextRequest>,
  response: HashMap<u32, NextRequest>,
}

struct InnerRequest {
  _headers: Vec<httparse::Header<'static>>,
  req: httparse::Request<'static, 'static>,
  body_offset: usize,
  body_len: usize,
  buffer: [u8; 1024],
}

struct Stream {
  inner: TcpStream,
  detached: bool,
  read_rx: Option<mpsc::Receiver<bytes::Bytes>>,
  read_tx: Option<mpsc::Sender<bytes::Bytes>>,
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
  #[allow(dead_code)]
  no_more_requests: bool,
  #[allow(dead_code)]
  upgrade: bool,
  content_length: Option<u64>,
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

  let _ = sock.write(&response);
  if let Some(response) = maybe_body {
    let _ = sock.write(format!("{:x}", response.len()).as_bytes());
    let _ = sock.write(b"\r\n");
    let _ = sock.write(&response);
    let _ = sock.write(b"\r\n");
  }
  sock.reattach_ownership();
  sock.read_tx.take();
  sock.read_rx.take();
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
    let _ = sock.write(&response);
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

#[op]
fn op_flash_headers(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
) -> Vec<(ByteString, ByteString)> {
  let mut op_state = state.borrow_mut();
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let inner_req = &ctx.response.get(&token).unwrap().inner.req;
  inner_req
    .headers
    .iter()
    .map(|h| (h.name.as_bytes().into(), h.value.into()))
    .collect()
}

#[op]
fn op_flash_first_packet(
  op_state: &mut OpState,
  server_id: u32,
  token: u32,
  mut buf: ZeroCopyBuf,
) -> usize {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let tx = ctx.response.get_mut(&token).unwrap();
  // SAFETY: The socket lives on the mio thread.
  let sock = unsafe { &mut *tx.socket };
  if tx.expect_continue {
    let _ = sock.write(b"HTTP/1.1 100 Continue\r\n\r\n");
    tx.expect_continue = false;
  }
  
  let mut buffer = &tx.inner.buffer[..];
  debug_assert!(buf.len() <= buffer.len());

  let (tx, rx) = mpsc::channel(1);
  sock.read_tx = Some(tx);
  sock.read_rx = Some(rx);

  buffer.read(&mut buf).unwrap()
}

#[op]
async fn op_flash_read_body(
  state: Rc<RefCell<OpState>>,
  server_id: u32,
  token: u32,
  mut buf: ZeroCopyBuf,
) -> usize {
  let ctx = unsafe { {
    let op_state = &mut state.borrow_mut();
    let flash_ctx = op_state.borrow_mut::<FlashContext>();
    flash_ctx.servers.get_mut(&server_id).unwrap() as *mut ServerContext
  }.as_mut().unwrap() };
  let tx = ctx.response.get_mut(&token).unwrap();
  // SAFETY: The socket lives on the mio thread.
  let sock = unsafe { &mut *tx.socket };

  if tx.te_chunked {
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
    let mut decoder = chunked::Decoder::new(sock);
    return decoder.read(&mut buf).expect("read error");
  }

  let bytes = match sock.read_rx.as_mut().unwrap().recv().await {
    Some(bytes) => bytes,
    None => return 0,
  };

  bytes.as_ref().read(&mut buf).unwrap()
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
  loop {
    match poll.poll(&mut events, None) {
      Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
      Err(e) => panic!("{}", e),
      Ok(()) => (),
    }
    for event in &events {
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
              let stream = Stream {
                inner: socket,
                detached: false,
                read_rx: None,
                read_tx: None,
              };
              sockets.insert(token, stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
          }
        },
        token => {
          let mut buffer: [u8; 1024] = [0_u8; 1024];
          let socket = sockets.get_mut(&token).unwrap();
          if socket.detached {
            continue;
          }

          debug_assert!(event.is_readable());
          let sock_ptr = socket as *mut _;
          let nread = socket.read(&mut buffer);

          let mut headers = vec![httparse::EMPTY_HEADER; 40];
          let mut req = httparse::Request::new(&mut headers);
          let body_offset;
          let body_len;

          match nread {
            Ok(0) => {
              sockets.remove(&token);
              continue;
            }
            Ok(n) => {
              if let Some(tx) = &socket.read_tx {
                tx.blocking_send(bytes::Bytes::copy_from_slice(&buffer[..n])).unwrap();
                continue;
              }
              body_len = n;
              let r = req.parse(&buffer[0..n]).unwrap();
              // Just testing now, assumtion is we get complete message in a single packet, which is true in wrk benchmark.
              match r {
                httparse::Status::Complete(n) => body_offset = n,
                _ => unreachable!(),
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
          }
          let inner_req = InnerRequest {
            req: unsafe { transmute::<httparse::Request<'_, '_>, _>(req) },
            _headers: unsafe {
              transmute::<Vec<httparse::Header<'_>>, _>(headers)
            },
            buffer,
            body_offset,
            body_len,
          };
          // h1
          // https://github.com/tiny-http/tiny-http/blob/master/src/client.rs#L177
          // https://github.com/hyperium/hyper/blob/4545c3ef191ce9b5f5d250ee27c4c96f9b71d2c6/src/proto/h1/role.rs#L127
          let mut no_more_requests = inner_req.req.version.unwrap() == 1;
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
                if no_more_requests {
                  // 1.1
                  no_more_requests = !connection_has(&value, "close");
                } else {
                  // 1.0
                  no_more_requests = connection_has(&value, "keep-alive");
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
                      // TODO: invalid content length
                    }
                    continue;
                  }
                  content_length = Some(len);
                } else {
                  // TODO: invalid content length
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
            no_more_requests,
            upgrade,
            te_chunked,
            content_length,
            expect_continue
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
  let ctx = ServerContext {
    addr,
    tx,
    rx,
    response: HashMap::with_capacity(1000),
  };
  let tx = ctx.tx.clone();
  let maybe_cert = opts.cert;
  let maybe_key = opts.key;
  let join_handle = tokio::task::spawn_blocking(move || {
    run_server(tx, addr, maybe_cert, maybe_key)
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
  let mut tokens = 0;
  while let Ok(token) = ctx.rx.try_recv() {
    ctx.response.insert(tokens, token);
    tokens += 1;
  }
  if tokens == 0 {
    if let Some(req) = ctx.rx.recv().await {
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
  let mut tokens = 0;
  while let Ok(token) = ctx.rx.try_recv() {
    ctx.response.insert(tokens, token);
    tokens += 1;
  }
  tokens
}

// Syncrhonous version of op_flash_next_async. Under heavy load,
// this can collect buffered requests from rx channel and return tokens in a single batch.
#[op]
fn op_flash_next_server(op_state: &mut OpState, server_id: u32) -> u32 {
  let flash_ctx = op_state.borrow_mut::<FlashContext>();
  let ctx = flash_ctx.servers.get_mut(&server_id).unwrap();
  let mut tokens = 0;
  while let Ok(token) = ctx.rx.try_recv() {
    ctx.response.insert(tokens, token);
    tokens += 1;
  }
  tokens
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
  let tx = ctx.response.remove(&token).unwrap();
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
