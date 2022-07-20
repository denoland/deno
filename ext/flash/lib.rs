use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RuntimeOptions;
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
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::sync::mpsc::channel;
use std::sync::Arc;
use tokio::sync::mpsc;

struct ServerContext {
  tx: mpsc::Sender<NextRequest>,
  rx: mpsc::Receiver<NextRequest>,
  response: HashMap<u32, NextRequest>,
}

struct InnerRequest {
  _headers: Vec<httparse::Header<'static>>,
  req: httparse::Request<'static, 'static>,
}

type TlsTcpStream = rustls::StreamOwned<rustls::ServerConnection, TcpStream>;

enum Stream {
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

struct NextRequest {
  socket: *mut Stream,
  inner: Arc<InnerRequest>,
  no_more_requests: bool,
  upgrade: bool,
}

unsafe impl Send for NextRequest {}

#[op]
fn op_respond(
  op_state: &mut OpState,
  token: u32,
  response: String,
  shutdown: bool,
) {
  let mut ctx = op_state.borrow_mut::<ServerContext>();

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

  let _ = sock.write(response.as_bytes());
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
  let mut op_state = state.borrow_mut();
  let ctx = op_state.borrow_mut::<ServerContext>();
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
  let ctx = op_state.borrow_mut::<ServerContext>();
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
  let ctx = op_state.borrow_mut::<ServerContext>();
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
  let mut op_state = state.borrow_mut();
  let ctx = op_state.borrow_mut::<ServerContext>();
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
  cert: String,
  key: String,
}

#[op]
fn op_listen(
  state: &mut OpState,
  opts: Option<ListenOpts>,
) -> impl Future<Output = ()> + 'static {
  let ctx = state.borrow_mut::<ServerContext>();
  let tx = ctx.tx.clone();

  async move {
    tokio::task::spawn_blocking(move || {
      let addr = "127.0.0.1:9000".parse().unwrap();
      let mut listener = TcpListener::bind(addr).unwrap();
      let mut poll = Poll::new().unwrap();
      let token = Token(0);
      poll
        .registry()
        .register(&mut listener, token, Interest::READABLE)
        .unwrap();

      let tls_context: Option<Arc<rustls::ServerConfig>> = match opts {
        Some(opts) => {
          let certificate_chain: Vec<rustls::Certificate> =
          rustls_pemfile::certs(&mut opts.cert.as_bytes()).unwrap()
              .into_iter()
              .map(rustls::Certificate)
              .collect();
          let private_key = rustls::PrivateKey({
            let pkcs8_keys = rustls_pemfile::pkcs8_private_keys(
                &mut opts.key.as_bytes(),
            )
            .expect("file contains invalid pkcs8 private key (encrypted keys are not supported)");

            if let Some(pkcs8_key) = pkcs8_keys.first() {
                pkcs8_key.clone()
            } else {
                let rsa_keys = rustls_pemfile::rsa_private_keys(&mut opts.key.as_bytes()).expect("file contains invalid rsa private key");
                rsa_keys[0].clone()
            }
        });

        let config = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certificate_chain, private_key).unwrap();
          Some(Arc::new(config))
        }
        None => None,
      };
      let mut sockets = HashMap::with_capacity(1000);
      let mut counter: usize = 1;
      let mut buffer: [u8; 1024] = [0_u8; 1024];
      let mut events = Events::with_capacity(1024);
      loop {
        poll.poll(&mut events, None).unwrap();
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
                  let stream = match tls_context {
                    Some(ref tls_conf) => {
                      let connection = rustls::ServerConnection::new(tls_conf.clone()).unwrap();
                      Stream::Tls(rustls::StreamOwned::new(connection, socket))
                    }
                    None => Stream::Tcp(socket)
                  };
                  sockets.insert(token, stream);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                  break
                }
                Err(_) => break,
              }
            },
            token => {
              let socket = sockets.get_mut(&token).unwrap();
              debug_assert!(event.is_readable());
              let sock_ptr = socket as *mut _;
              let nread = socket.read(&mut buffer);

              let mut headers = vec![httparse::EMPTY_HEADER; 40];
              let mut req = httparse::Request::new(&mut headers);
              match nread {
                Ok(0) => {
                  sockets.remove(&token);
                  continue;
                }
                Ok(n) => {
                  let r = req.parse(&buffer[0..n]).unwrap();
                  // Just testing now, assumtion is we get complete message in a single packet, which is true in wrk benchmark.
                  match r {
                    httparse::Status::Complete(_) => {}
                    _ => unreachable!(),
                  }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                  break
                }
                Err(_) => break,
              }
              let inner_req = InnerRequest {
                req: unsafe { transmute::<httparse::Request<'_, '_>, _>(req) },
                _headers: unsafe {
                  transmute::<Vec<httparse::Header<'_>>, _>(headers)
                },
              };
              // h1
              // https://github.com/tiny-http/tiny-http/blob/master/src/client.rs#L177
              // https://github.com/hyperium/hyper/blob/4545c3ef191ce9b5f5d250ee27c4c96f9b71d2c6/src/proto/h1/role.rs#L127
              let mut no_more_requests = inner_req.req.version.unwrap() == 1;
              let mut upgrade = false;
              let mut expect_continue = false;
              let mut transfer_encoding: Option<()> = None;
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
                    transfer_encoding = Some(());
                    // Handle chunked encoding
                  }
                  Ok(CONTENT_LENGTH) => {
                    // ignore if transfer_encoding is present
                    if transfer_encoding.is_some() {
                      continue;
                    }
                    // Handle content length
                  }
                  Ok(EXPECT) => {
                    expect_continue =
                      header.value.eq_ignore_ascii_case(b"100-continue");
                  }
                  _ => {}
                }
              }
              tx.blocking_send(NextRequest {
                socket: sock_ptr,
                // SAFETY: headers backing buffer outlives the mio event loop ('static)
                inner: Arc::new(inner_req),
                no_more_requests,
                upgrade,
              });
            }
          }
        }
      }
    })
    .await
    .unwrap();
  }
}

#[op]
async fn op_next(op_state: Rc<RefCell<OpState>>) -> u32 {
  let mut op_state = op_state.borrow_mut();
  let ctx = op_state.borrow_mut::<ServerContext>();
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
    .state(|op_state| {
      let (tx, rx) = mpsc::channel(100);
      op_state.put(ServerContext {
        tx,
        rx,
        response: HashMap::with_capacity(1000),
      });
      Ok(())
    })
    .build()
}
