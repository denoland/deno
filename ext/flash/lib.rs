use deno_core::Op;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ByteString;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use mio::net::TcpStream;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
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
  Tcp(TcpStream),
  Tls(TlsTcpStream),
}

impl Write for Stream {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    match self {
      Stream::Tcp(stream) => stream.write(buf),
      Stream::Tls(stream) => stream.write(buf),
    }
  }
  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    match self {
      Stream::Tcp(stream) => stream.flush(),
      Stream::Tls(stream) => stream.flush(),
    }
  }
}

impl Read for Stream {
  #[inline]
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match self {
      Stream::Tcp(stream) => stream.read(buf),
      Stream::Tls(stream) => stream.read(buf),
    }
  }
}

pub struct NextRequest {
  pub socket: *mut Stream,
  pub inner: Arc<InnerRequest>,
  pub no_more_requests: bool,
  pub upgrade: bool,
}

unsafe impl Send for NextRequest {}

#[op]
fn op_respond(
  op_state: &mut OpState,
  token: u32,
  status: u16,
  js_headers: Vec<(String, String)>,
  body: String,
) {
  let mut ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow_mut();
  let tx = ctx.response.remove(&token).unwrap();
  let sock = unsafe { &mut *tx.socket };

  // let mut headers = format!("HTTP/1.1 {} {}\r\n", status, "OK");

  // for (name, value) in js_headers.iter() {
  //   write!(sock, "{}: {}\r\n", name, value).unwrap();
  // }
  
  // headers.push_str("Content-length: 11\r\n\r\n");
  // headers.push_str(&body);
  // let headers = headers.as_bytes();
  let _ = sock.write(b"HTTP/1.1 200 OK\r\nContent-length: 11\r\n\r\nHello World").unwrap();

  // if tx.no_more_requests && !tx.upgrade {
  //  dbg!("closing socket");
  //   sock.flush().unwrap();
  // }
}

#[op]
async fn op_respond_stream(
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
fn op_method(state: Rc<RefCell<OpState>>, token: u32) -> String {
  let mut op_state = state.borrow_mut();
  let ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
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
fn op_path(state: Rc<RefCell<OpState>>, token: u32) -> String {
  let mut op_state = state.borrow_mut();
  let ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
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
fn op_headers(
  state: Rc<RefCell<OpState>>,
  token: u32,
) -> Vec<(String, String)> {
  let mut op_state = state.borrow();
  let ctx = op_state.borrow::<Rc<RefCell<ServerContext>>>().borrow();
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
async fn op_listen(state: Rc<RefCell<OpState>>, opts: Option<ListenOpts>) {
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
async fn op_next(op_state: Rc<RefCell<OpState>>) -> u32 {
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

pub fn init() -> Extension {
  Extension::builder()
    .js(deno_core::include_js_files!(
      prefix "deno:ext/flash",
      "01_http.js",
    ))
    .ops(vec![
      op_listen::decl(),
      op_respond::decl(),
      op_method::decl(),
      op_path::decl(),
      op_headers::decl(),
      op_respond_stream::decl(),
      op_next::decl(),
    ])
    .build()
}
