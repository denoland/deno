use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use hyper::body::Bytes;
use mio::net::TcpStream;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

mod server;

struct ServerContext {
  rx: mpsc::Receiver<NextRequest>,
  response: HashMap<u32, NextRequest>,
}

pub struct InnerRequest {
  pub _headers: Vec<httparse::Header<'static>>,
  pub req: httparse::Request<'static, 'static>,
}

type TlsTcpStream = rustls::StreamOwned<rustls::ServerConnection, TcpStream>;

pub enum Stream {
  Tcp(TcpStream, bool),
  Tls(TlsTcpStream, bool),
}

impl Stream {
  pub fn detach_ownership(&mut self) {
    match self {
      Stream::Tcp(_, detached) => *detached = true,
      Stream::Tls(_, detached) => *detached = true,
    }
  }
}

impl Write for Stream {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self {
      Stream::Tcp(stream, _) => stream.write(buf),
      Stream::Tls(stream, _) => stream.write(buf),
    }
  }
  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    match self {
      Stream::Tcp(stream, _) => stream.flush(),
      Stream::Tls(stream, _) => stream.flush(),
    }
  }
}

impl Read for Stream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self {
      Stream::Tcp(stream, _) => stream.read(buf),
      Stream::Tls(stream, _) => stream.read(buf),
    }
  }
}

pub struct NextRequest {
  // Pointer to stream owned by the server loop thread.
  //
  // Why not Arc<Mutex<Stream>>? Performance. The stream
  // is never written to by the server loop thread.
  //
  // Dereferencing is safe until server thread finishes and
  // op_flash_listen resolves.
  pub socket: *mut Stream,
  //
  pub inner: Arc<InnerRequest>,
  //
  pub no_more_requests: bool,
  pub upgrade: bool,
}

// SAFETY: Sent from server thread to JS thread.
// See comment above for `socket`.
unsafe impl Send for NextRequest {}

#[op]
fn op_flash_respond(
  op_state: &mut OpState,
  token: u32,
  status: u16,
  js_headers: Vec<(String, String)>,
  body: StringOrBuffer,
  shutdown: bool,
) {
  let mut ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow_mut();

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

  #[inline]
  fn extend(dst: &mut Vec<u8>, data: &[u8]) {
    dst.extend_from_slice(data);
  }

  let mut dst = Vec::with_capacity(1024);
  extend(&mut dst, b"HTTP/1.1 ");
  extend(&mut dst, status.to_string().as_bytes());
  extend(&mut dst, b" ");
  let status = http::StatusCode::from_u16(status).unwrap();
  extend(
    &mut dst,
    status.canonical_reason().unwrap_or("<none>").as_bytes(),
  );
  extend(&mut dst, b"\r\n");

  for (key, value) in js_headers {
    extend(&mut dst, key.as_bytes());
    extend(&mut dst, b": ");
    extend(&mut dst, value.as_bytes());
    extend(&mut dst, b"\r\n");
  }
  extend(&mut dst, b"Content-Length: ");
  extend(&mut dst, body.len().to_string().as_bytes());
  extend(&mut dst, b"\r\n\r\n");

  extend(&mut dst, &body);
  let _ = sock.write(&dst);

  // if tx.no_more_requests && !tx.upgrade {
  //  dbg!("closing socket");
  //   sock.flush().unwrap();
  // }
}

#[op]
async fn op_flash_write_stream(
  state: Rc<RefCell<OpState>>,
  token: u32,
  data: String,
  rid: u32,
) -> Result<(), AnyError> {
  let op_state = state.borrow_mut();
  let mut ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow_mut();
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
fn op_flash_method(state: &mut OpState, token: u32) -> String {
  let ctx = state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
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
fn op_flash_path(state: &mut OpState, token: u32) -> String {
  let ctx = state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
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

// TODO(@littledivy):
// - use ByteString for headers
// - maybe use typedarray with fast api calls?
#[op]
fn op_flash_headers(state: &mut OpState, token: u32) -> Vec<(String, String)> {
  let ctx = state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
  let inner_req = &ctx.response.get(&token).unwrap().inner.req;
  inner_req
    .headers
    .iter()
    .map(|h| {
      (
        h.name.to_string(),
        String::from_utf8_lossy(h.value).to_string(),
      )
    })
    .collect()
}

#[derive(Serialize, Deserialize)]
pub struct ListenOpts {
  pub cert: String,
  pub key: String,
}

#[op]
async fn op_flash_listen(
  state: Rc<RefCell<OpState>>,
  opts: Option<ListenOpts>,
) {
  let (tx, rx) = mpsc::channel(100);
  state.borrow_mut().put(Rc::new(RefCell::new(ServerContext {
    rx,
    response: HashMap::with_capacity(1000),
  })));

  tokio::task::spawn_blocking(move || {
    crate::server::start_http(tx, opts);
  })
  .await
  .unwrap();
}

#[op]
async fn op_flash_next(op_state: Rc<RefCell<OpState>>) -> u32 {
  let ctx = {
    let state = &mut *op_state.borrow_mut();
    let ctx = state.borrow::<Rc<RefCell<ServerContext>>>();
    ctx.clone()
  };

  let ctx = &mut ctx.borrow_mut();
  let mut tokens = 0;

  if let Some(req) = ctx.rx.recv().await {
    ctx.response.insert(tokens, req);
    tokens += 1;
    while let Ok(token) = ctx.rx.try_recv() {
      ctx.response.insert(tokens, token);
      tokens += 1;
    }
  }
  tokens
}

impl hyper::body::HttpBody for NextRequest {
  type Data = Bytes;
  type Error = AnyError;

  fn poll_data(
    mut self: Pin<&mut Self>,
    _: &mut Context<'_>,
  ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
    let stream = unsafe { &mut *self.socket };
    let mut vec = vec![0u8; 64 * 1024]; // 64KB
    match stream.read(&mut vec) {
      Ok(nread) => {
        if nread == 0 {
          return Poll::Ready(None);
        }
        Poll::Ready(Some(Ok(vec[..nread].to_vec().into())))
      }
      Err(e) => Poll::Ready(Some(Err(e.into()))),
    }
  }

  fn poll_trailers(
    self: Pin<&mut Self>,
    _: &mut Context<'_>,
  ) -> Poll<
    Result<Option<hyper::HeaderMap<hyper::header::HeaderValue>>, Self::Error>,
  > {
    Poll::Ready(Ok(None))
  }
}

#[op]
async fn op_flash_upgrade_websocket(
  state: Rc<RefCell<OpState>>,
  token: u32,
) -> Result<deno_core::ResourceId, AnyError> {
  let tx = {
    let op_state = state.borrow_mut();
    let mut ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow_mut();
    ctx.response.remove(&token).unwrap()
  };
  {
    let stream = unsafe { &mut *tx.socket };
    stream.detach_ownership();
  }

  let mut request = http::Request::builder();
  let headers = request.headers_mut().unwrap();
  for header in tx.inner.req.headers.iter() {
    headers.append(
      header.name,
      http::header::HeaderValue::from_bytes(header.value)?,
    );
  }
  let request = request.body(tx)?;
  dbg!("upgrading to websocket");
  let transport = hyper::upgrade::on(request).await?;
  dbg!("upgrading to websocket");
  let ws_rid =
    deno_websocket::ws_create_server_stream(&state, transport).await?;
  Ok(ws_rid)
}

pub fn init() -> Extension {
  Extension::builder()
    .js(deno_core::include_js_files!(
      prefix "deno:ext/flash",
      "01_http.js",
    ))
    .ops(vec![
      op_flash_listen::decl(),
      op_flash_respond::decl(),
      op_flash_method::decl(),
      op_flash_path::decl(),
      op_flash_headers::decl(),
      op_flash_write_stream::decl(),
      op_flash_next::decl(),
      op_flash_upgrade_websocket::decl(),
    ])
    .build()
}
