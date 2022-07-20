use http::header::HeaderName;
use http::header::CONNECTION;
use http::header::CONTENT_LENGTH;
use http::header::EXPECT;
use http::header::TRANSFER_ENCODING;
use http::header::UPGRADE;
use http::HeaderValue;
use mio::net::TcpListener;

use crate::InnerRequest;
use crate::ListenOpts;
use crate::NextRequest;
use crate::Stream;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use std::collections::HashMap;
use std::intrinsics::transmute;
use std::io::Read;
use std::mem::forget;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn start_http(tx: mpsc::Sender<NextRequest>, opts: Option<ListenOpts>) {
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
        rustls_pemfile::certs(&mut opts.cert.as_bytes())
          .unwrap()
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
          let rsa_keys =
            rustls_pemfile::rsa_private_keys(&mut opts.key.as_bytes())
              .expect("file contains invalid rsa private key");
          rsa_keys[0].clone()
        }
      });

      let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .unwrap();
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
                  let connection =
                    rustls::ServerConnection::new(tls_conf.clone()).unwrap();
                  Stream::Tls(
                    rustls::StreamOwned::new(connection, socket),
                    false,
                  )
                }
                None => Stream::Tcp(socket, false),
              };
              sockets.insert(token, stream);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
          }
        },
        token => {
          let socket = sockets.get_mut(&token).unwrap();
          match socket {
            Stream::Tcp(_, true) | Stream::Tls(_, true) => continue,
            _ => {}
          }
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
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
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
}
