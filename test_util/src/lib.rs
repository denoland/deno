// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// Usage: provide a port as argument to run hyper_hello benchmark server
// otherwise this starts multiple servers on many ports for test endpoints.
use anyhow::anyhow;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use hyper::header::HeaderValue;
use hyper::server::Server;
use hyper::service::make_service_fn;
use hyper::service::service_fn;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use lazy_static::lazy_static;
use npm::CUSTOM_NPM_PACKAGE_CACHE;
use os_pipe::pipe;
use pretty_assertions::assert_eq;
use regex::Regex;
use rustls::Certificate;
use rustls::PrivateKey;
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::env;
use std::io;
use std::io::Read;
use std::io::Write;
use std::mem::replace;
use std::net::SocketAddr;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::result::Result;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_rustls::rustls;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;

pub mod assertions;
pub mod lsp;
mod npm;
pub mod pty;
mod temp_dir;

pub use temp_dir::TempDir;

const PORT: u16 = 4545;
const TEST_AUTH_TOKEN: &str = "abcdef123456789";
const TEST_BASIC_AUTH_USERNAME: &str = "testuser123";
const TEST_BASIC_AUTH_PASSWORD: &str = "testpassabc";
const REDIRECT_PORT: u16 = 4546;
const ANOTHER_REDIRECT_PORT: u16 = 4547;
const DOUBLE_REDIRECTS_PORT: u16 = 4548;
const INF_REDIRECTS_PORT: u16 = 4549;
const REDIRECT_ABSOLUTE_PORT: u16 = 4550;
const AUTH_REDIRECT_PORT: u16 = 4551;
const TLS_CLIENT_AUTH_PORT: u16 = 4552;
const BASIC_AUTH_REDIRECT_PORT: u16 = 4554;
const TLS_PORT: u16 = 4557;
const HTTPS_PORT: u16 = 5545;
const H1_ONLY_PORT: u16 = 5546;
const H2_ONLY_PORT: u16 = 5547;
const HTTPS_CLIENT_AUTH_PORT: u16 = 5552;
const WS_PORT: u16 = 4242;
const WSS_PORT: u16 = 4243;
const WS_CLOSE_PORT: u16 = 4244;

pub const PERMISSION_VARIANTS: [&str; 5] =
  ["read", "write", "env", "net", "run"];
pub const PERMISSION_DENIED_PATTERN: &str = "PermissionDenied";

lazy_static! {
  // STRIP_ANSI_RE and strip_ansi_codes are lifted from the "console" crate.
  // Copyright 2017 Armin Ronacher <armin.ronacher@active-4.com>. MIT License.
  static ref STRIP_ANSI_RE: Regex = Regex::new(
          r"[\x1b\x9b][\[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-nqry=><]"
  ).unwrap();

  static ref GUARD: Mutex<HttpServerCount> = Mutex::new(HttpServerCount::default());
}

pub fn root_path() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR")))
    .parent()
    .unwrap()
    .to_path_buf()
}

pub fn prebuilt_path() -> PathBuf {
  third_party_path().join("prebuilt")
}

pub fn tests_path() -> PathBuf {
  root_path().join("cli").join("tests")
}

pub fn testdata_path() -> PathBuf {
  tests_path().join("testdata")
}

pub fn third_party_path() -> PathBuf {
  root_path().join("third_party")
}

pub fn napi_tests_path() -> PathBuf {
  root_path().join("test_napi")
}

pub fn std_path() -> PathBuf {
  root_path().join("test_util").join("std")
}

pub fn target_dir() -> PathBuf {
  let current_exe = std::env::current_exe().unwrap();
  let target_dir = current_exe.parent().unwrap().parent().unwrap();
  target_dir.into()
}

pub fn deno_exe_path() -> PathBuf {
  // Something like /Users/rld/src/deno/target/debug/deps/deno
  let mut p = target_dir().join("deno");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

pub fn prebuilt_tool_path(tool: &str) -> PathBuf {
  let mut exe = tool.to_string();
  exe.push_str(if cfg!(windows) { ".exe" } else { "" });
  prebuilt_path().join(platform_dir_name()).join(exe)
}

pub fn platform_dir_name() -> &'static str {
  if cfg!(target_os = "linux") {
    "linux64"
  } else if cfg!(target_os = "macos") {
    "mac"
  } else if cfg!(target_os = "windows") {
    "win"
  } else {
    unreachable!()
  }
}

pub fn test_server_path() -> PathBuf {
  let mut p = target_dir().join("test_server");
  if cfg!(windows) {
    p.set_extension("exe");
  }
  p
}

fn ensure_test_server_built() {
  // if the test server doesn't exist then remind the developer to build first
  if !test_server_path().exists() {
    panic!(
      "Test server not found. Please cargo build before running the tests."
    );
  }
}

/// Benchmark server that just serves "hello world" responses.
async fn hyper_hello(port: u16) {
  println!("hyper hello");
  let addr = SocketAddr::from(([127, 0, 0, 1], port));
  let hello_svc = make_service_fn(|_| async move {
    Ok::<_, Infallible>(service_fn(move |_: Request<Body>| async move {
      Ok::<_, Infallible>(Response::new(Body::from("Hello World!")))
    }))
  });

  let server = Server::bind(&addr).serve(hello_svc);
  if let Err(e) = server.await {
    eprintln!("server error: {}", e);
  }
}

fn redirect_resp(url: String) -> Response<Body> {
  let mut redirect_resp = Response::new(Body::empty());
  *redirect_resp.status_mut() = StatusCode::MOVED_PERMANENTLY;
  redirect_resp.headers_mut().insert(
    hyper::header::LOCATION,
    HeaderValue::from_str(&url[..]).unwrap(),
  );

  redirect_resp
}

async fn redirect(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{}{}", PORT, p);

  Ok(redirect_resp(url))
}

async fn double_redirects(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{}{}", REDIRECT_PORT, p);

  Ok(redirect_resp(url))
}

async fn inf_redirects(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{}{}", INF_REDIRECTS_PORT, p);

  Ok(redirect_resp(url))
}

async fn another_redirect(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{}/subdir{}", PORT, p);

  Ok(redirect_resp(url))
}

async fn auth_redirect(req: Request<Body>) -> hyper::Result<Response<Body>> {
  if let Some(auth) = req
    .headers()
    .get("authorization")
    .map(|v| v.to_str().unwrap())
  {
    if auth.to_lowercase() == format!("bearer {}", TEST_AUTH_TOKEN) {
      let p = req.uri().path();
      assert_eq!(&p[0..1], "/");
      let url = format!("http://localhost:{}{}", PORT, p);
      return Ok(redirect_resp(url));
    }
  }

  let mut resp = Response::new(Body::empty());
  *resp.status_mut() = StatusCode::NOT_FOUND;
  Ok(resp)
}

async fn basic_auth_redirect(
  req: Request<Body>,
) -> hyper::Result<Response<Body>> {
  if let Some(auth) = req
    .headers()
    .get("authorization")
    .map(|v| v.to_str().unwrap())
  {
    let credentials =
      format!("{}:{}", TEST_BASIC_AUTH_USERNAME, TEST_BASIC_AUTH_PASSWORD);
    if auth == format!("Basic {}", base64::encode(credentials)) {
      let p = req.uri().path();
      assert_eq!(&p[0..1], "/");
      let url = format!("http://localhost:{}{}", PORT, p);
      return Ok(redirect_resp(url));
    }
  }

  let mut resp = Response::new(Body::empty());
  *resp.status_mut() = StatusCode::NOT_FOUND;
  Ok(resp)
}

async fn run_ws_server(addr: &SocketAddr) {
  let listener = TcpListener::bind(addr).await.unwrap();
  println!("ready: ws"); // Eye catcher for HttpServerCount
  while let Ok((stream, _addr)) = listener.accept().await {
    tokio::spawn(async move {
      let ws_stream_fut = accept_async(stream);

      let ws_stream = ws_stream_fut.await;
      if let Ok(ws_stream) = ws_stream {
        let (tx, rx) = ws_stream.split();
        rx.forward(tx)
          .map(|result| {
            if let Err(e) = result {
              println!("websocket server error: {:?}", e);
            }
          })
          .await;
      }
    });
  }
}

async fn run_ws_close_server(addr: &SocketAddr) {
  let listener = TcpListener::bind(addr).await.unwrap();
  while let Ok((stream, _addr)) = listener.accept().await {
    tokio::spawn(async move {
      let ws_stream_fut = accept_async(stream);

      let ws_stream = ws_stream_fut.await;
      if let Ok(mut ws_stream) = ws_stream {
        ws_stream.close(None).await.unwrap();
      }
    });
  }
}

enum SupportedHttpVersions {
  All,
  Http1Only,
  Http2Only,
}
impl Default for SupportedHttpVersions {
  fn default() -> SupportedHttpVersions {
    SupportedHttpVersions::All
  }
}

async fn get_tls_config(
  cert: &str,
  key: &str,
  ca: &str,
  http_versions: SupportedHttpVersions,
) -> io::Result<Arc<rustls::ServerConfig>> {
  let cert_path = testdata_path().join(cert);
  let key_path = testdata_path().join(key);
  let ca_path = testdata_path().join(ca);

  let cert_file = std::fs::File::open(cert_path)?;
  let key_file = std::fs::File::open(key_path)?;
  let ca_file = std::fs::File::open(ca_path)?;

  let certs: Vec<Certificate> = {
    let mut cert_reader = io::BufReader::new(cert_file);
    rustls_pemfile::certs(&mut cert_reader)
      .unwrap()
      .into_iter()
      .map(Certificate)
      .collect()
  };

  let mut ca_cert_reader = io::BufReader::new(ca_file);
  let ca_cert = rustls_pemfile::certs(&mut ca_cert_reader)
    .expect("Cannot load CA certificate")
    .remove(0);

  let mut key_reader = io::BufReader::new(key_file);
  let key = {
    let pkcs8_key = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
      .expect("Cannot load key file");
    let rsa_key = rustls_pemfile::rsa_private_keys(&mut key_reader)
      .expect("Cannot load key file");
    if !pkcs8_key.is_empty() {
      Some(pkcs8_key[0].clone())
    } else if !rsa_key.is_empty() {
      Some(rsa_key[0].clone())
    } else {
      None
    }
  };

  match key {
    Some(key) => {
      let mut root_cert_store = rustls::RootCertStore::empty();
      root_cert_store.add(&rustls::Certificate(ca_cert)).unwrap();

      // Allow (but do not require) client authentication.

      let mut config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(
          rustls::server::AllowAnyAnonymousOrAuthenticatedClient::new(
            root_cert_store,
          ),
        )
        .with_single_cert(certs, PrivateKey(key))
        .map_err(|e| anyhow!("Error setting cert: {:?}", e))
        .unwrap();

      match http_versions {
        SupportedHttpVersions::All => {
          config.alpn_protocols = vec!["h2".into(), "http/1.1".into()];
        }
        SupportedHttpVersions::Http1Only => {}
        SupportedHttpVersions::Http2Only => {
          config.alpn_protocols = vec!["h2".into()];
        }
      }

      Ok(Arc::new(config))
    }
    None => Err(io::Error::new(io::ErrorKind::Other, "Cannot find key")),
  }
}

async fn run_wss_server(addr: &SocketAddr) {
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";

  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, Default::default())
      .await
      .unwrap();
  let tls_acceptor = TlsAcceptor::from(tls_config);
  let listener = TcpListener::bind(addr).await.unwrap();
  println!("ready: wss"); // Eye catcher for HttpServerCount

  while let Ok((stream, _addr)) = listener.accept().await {
    let acceptor = tls_acceptor.clone();
    tokio::spawn(async move {
      match acceptor.accept(stream).await {
        Ok(tls_stream) => {
          let ws_stream_fut = accept_async(tls_stream);
          let ws_stream = ws_stream_fut.await;
          if let Ok(ws_stream) = ws_stream {
            let (tx, rx) = ws_stream.split();
            rx.forward(tx)
              .map(|result| {
                if let Err(e) = result {
                  println!("Websocket server error: {:?}", e);
                }
              })
              .await;
          }
        }
        Err(e) => {
          eprintln!("TLS accept error: {:?}", e);
        }
      }
    });
  }
}

/// This server responds with 'PASS' if client authentication was successful. Try it by running
/// test_server and
///   curl --key cli/tests/testdata/tls/localhost.key \
///        --cert cli/tests/testsdata/tls/localhost.crt \
///        --cacert cli/tests/testdata/tls/RootCA.crt https://localhost:4552/
async fn run_tls_client_auth_server() {
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, Default::default())
      .await
      .unwrap();
  let tls_acceptor = TlsAcceptor::from(tls_config);

  // Listen on ALL addresses that localhost can resolves to.
  let accept = |listener: tokio::net::TcpListener| {
    async {
      let result = listener.accept().await;
      Some((result, listener))
    }
    .boxed()
  };

  let host_and_port = &format!("localhost:{}", TLS_CLIENT_AUTH_PORT);

  let listeners = tokio::net::lookup_host(host_and_port)
    .await
    .expect(host_and_port)
    .inspect(|address| println!("{} -> {}", host_and_port, address))
    .map(tokio::net::TcpListener::bind)
    .collect::<futures::stream::FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(|s| s.unwrap())
    .map(|listener| futures::stream::unfold(listener, accept))
    .collect::<Vec<_>>();

  println!("ready: tls client auth"); // Eye catcher for HttpServerCount

  let mut listeners = futures::stream::select_all(listeners);

  while let Some(Ok((stream, _addr))) = listeners.next().await {
    let acceptor = tls_acceptor.clone();
    tokio::spawn(async move {
      match acceptor.accept(stream).await {
        Ok(mut tls_stream) => {
          let (_, tls_session) = tls_stream.get_mut();
          // We only need to check for the presence of client certificates
          // here. Rusttls ensures that they are valid and signed by the CA.
          let response = match tls_session.peer_certificates() {
            Some(_certs) => b"PASS",
            None => b"FAIL",
          };
          tls_stream.write_all(response).await.unwrap();
        }

        Err(e) => {
          eprintln!("TLS accept error: {:?}", e);
        }
      }
    });
  }
}

/// This server responds with 'PASS' if client authentication was successful. Try it by running
/// test_server and
///   curl --cacert cli/tests/testdata/tls/RootCA.crt https://localhost:4553/
async fn run_tls_server() {
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, Default::default())
      .await
      .unwrap();
  let tls_acceptor = TlsAcceptor::from(tls_config);

  // Listen on ALL addresses that localhost can resolves to.
  let accept = |listener: tokio::net::TcpListener| {
    async {
      let result = listener.accept().await;
      Some((result, listener))
    }
    .boxed()
  };

  let host_and_port = &format!("localhost:{}", TLS_PORT);

  let listeners = tokio::net::lookup_host(host_and_port)
    .await
    .expect(host_and_port)
    .inspect(|address| println!("{} -> {}", host_and_port, address))
    .map(tokio::net::TcpListener::bind)
    .collect::<futures::stream::FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(|s| s.unwrap())
    .map(|listener| futures::stream::unfold(listener, accept))
    .collect::<Vec<_>>();

  println!("ready: tls"); // Eye catcher for HttpServerCount

  let mut listeners = futures::stream::select_all(listeners);

  while let Some(Ok((stream, _addr))) = listeners.next().await {
    let acceptor = tls_acceptor.clone();
    tokio::spawn(async move {
      match acceptor.accept(stream).await {
        Ok(mut tls_stream) => {
          tls_stream.write_all(b"PASS").await.unwrap();
        }

        Err(e) => {
          eprintln!("TLS accept error: {:?}", e);
        }
      }
    });
  }
}

async fn absolute_redirect(
  req: Request<Body>,
) -> hyper::Result<Response<Body>> {
  let path = req.uri().path();

  if path.starts_with("/REDIRECT") {
    let url = &req.uri().path()[9..];
    println!("URL: {:?}", url);
    let redirect = redirect_resp(url.to_string());
    return Ok(redirect);
  }

  if path.starts_with("/a/b/c") {
    if let Some(x_loc) = req.headers().get("x-location") {
      let loc = x_loc.to_str().unwrap();
      return Ok(redirect_resp(loc.to_string()));
    }
  }

  let mut file_path = testdata_path();
  file_path.push(&req.uri().path()[1..]);
  if file_path.is_dir() || !file_path.exists() {
    let mut not_found_resp = Response::new(Body::empty());
    *not_found_resp.status_mut() = StatusCode::NOT_FOUND;
    return Ok(not_found_resp);
  }

  let file = tokio::fs::read(file_path).await.unwrap();
  let file_resp = custom_headers(req.uri().path(), file);
  Ok(file_resp)
}

async fn main_server(
  req: Request<Body>,
) -> Result<Response<Body>, hyper::http::Error> {
  return match (req.method(), req.uri().path()) {
    (&hyper::Method::POST, "/echo_server") => {
      let (parts, body) = req.into_parts();
      let mut response = Response::new(body);

      if let Some(status) = parts.headers.get("x-status") {
        *response.status_mut() =
          StatusCode::from_bytes(status.as_bytes()).unwrap();
      }
      if let Some(content_type) = parts.headers.get("content-type") {
        response
          .headers_mut()
          .insert("content-type", content_type.clone());
      }
      if let Some(user_agent) = parts.headers.get("user-agent") {
        response
          .headers_mut()
          .insert("user-agent", user_agent.clone());
      }
      Ok(response)
    }
    (&hyper::Method::POST, "/echo_multipart_file") => {
      let body = req.into_body();
      let bytes = &hyper::body::to_bytes(body).await.unwrap()[0..];
      let start = b"--boundary\t \r\n\
                    Content-Disposition: form-data; name=\"field_1\"\r\n\
                    \r\n\
                    value_1 \r\n\
                    \r\n--boundary\r\n\
                    Content-Disposition: form-data; name=\"file\"; \
                    filename=\"file.bin\"\r\n\
                    Content-Type: application/octet-stream\r\n\
                    \r\n";
      let end = b"\r\n--boundary--\r\n";
      let b = [start as &[u8], bytes, end].concat();

      let mut response = Response::new(Body::from(b));
      response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("multipart/form-data;boundary=boundary"),
      );
      Ok(response)
    }
    (_, "/multipart_form_data.txt") => {
      let b = "Preamble\r\n\
             --boundary\t \r\n\
             Content-Disposition: form-data; name=\"field_1\"\r\n\
             \r\n\
             value_1 \r\n\
             \r\n--boundary\r\n\
             Content-Disposition: form-data; name=\"field_2\";\
             filename=\"file.js\"\r\n\
             Content-Type: text/javascript\r\n\
             \r\n\
             console.log(\"Hi\")\
             \r\n--boundary--\r\n\
             Epilogue";
      let mut res = Response::new(Body::from(b));
      res.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("multipart/form-data;boundary=boundary"),
      );
      Ok(res)
    }
    (_, "/multipart_form_bad_content_type") => {
      let b = "Preamble\r\n\
             --boundary\t \r\n\
             Content-Disposition: form-data; name=\"field_1\"\r\n\
             \r\n\
             value_1 \r\n\
             \r\n--boundary\r\n\
             Content-Disposition: form-data; name=\"field_2\";\
             filename=\"file.js\"\r\n\
             Content-Type: text/javascript\r\n\
             \r\n\
             console.log(\"Hi\")\
             \r\n--boundary--\r\n\
             Epilogue";
      let mut res = Response::new(Body::from(b));
      res.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("multipart/form-datatststs;boundary=boundary"),
      );
      Ok(res)
    }
    (_, "/bad_redirect") => {
      let mut res = Response::new(Body::empty());
      *res.status_mut() = StatusCode::FOUND;
      Ok(res)
    }
    (_, "/x_deno_warning.js") => {
      let mut res = Response::new(Body::empty());
      *res.status_mut() = StatusCode::MOVED_PERMANENTLY;
      res
        .headers_mut()
        .insert("X-Deno-Warning", HeaderValue::from_static("foobar"));
      res.headers_mut().insert(
        "location",
        HeaderValue::from_bytes(b"/lsp/x_deno_warning_redirect.js").unwrap(),
      );
      Ok(res)
    }
    (_, "/non_ascii_redirect") => {
      let mut res = Response::new(Body::empty());
      *res.status_mut() = StatusCode::MOVED_PERMANENTLY;
      res.headers_mut().insert(
        "location",
        HeaderValue::from_bytes(b"/redirect\xae").unwrap(),
      );
      Ok(res)
    }
    (_, "/etag_script.ts") => {
      let if_none_match = req.headers().get("if-none-match");
      if if_none_match == Some(&HeaderValue::from_static("33a64df551425fcc55e"))
      {
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() = StatusCode::NOT_MODIFIED;
        resp.headers_mut().insert(
          "Content-type",
          HeaderValue::from_static("application/typescript"),
        );
        resp
          .headers_mut()
          .insert("ETag", HeaderValue::from_static("33a64df551425fcc55e"));

        Ok(resp)
      } else {
        let mut resp = Response::new(Body::from("console.log('etag')"));
        resp.headers_mut().insert(
          "Content-type",
          HeaderValue::from_static("application/typescript"),
        );
        resp
          .headers_mut()
          .insert("ETag", HeaderValue::from_static("33a64df551425fcc55e"));
        Ok(resp)
      }
    }
    (_, "/xTypeScriptTypes.js") => {
      let mut res = Response::new(Body::from("export const foo = 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      res.headers_mut().insert(
        "X-TypeScript-Types",
        HeaderValue::from_static("./xTypeScriptTypes.d.ts"),
      );
      Ok(res)
    }
    (_, "/xTypeScriptTypes.jsx") => {
      let mut res = Response::new(Body::from("export const foo = 'foo';"));
      res
        .headers_mut()
        .insert("Content-type", HeaderValue::from_static("text/jsx"));
      res.headers_mut().insert(
        "X-TypeScript-Types",
        HeaderValue::from_static("./xTypeScriptTypes.d.ts"),
      );
      Ok(res)
    }
    (_, "/xTypeScriptTypes.ts") => {
      let mut res =
        Response::new(Body::from("export const foo: string = 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      res.headers_mut().insert(
        "X-TypeScript-Types",
        HeaderValue::from_static("./xTypeScriptTypes.d.ts"),
      );
      Ok(res)
    }
    (_, "/xTypeScriptTypes.d.ts") => {
      let mut res = Response::new(Body::from("export const foo: 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/run/type_directives_redirect.js") => {
      let mut res = Response::new(Body::from("export const foo = 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      res.headers_mut().insert(
        "X-TypeScript-Types",
        HeaderValue::from_static(
          "http://localhost:4547/xTypeScriptTypesRedirect.d.ts",
        ),
      );
      Ok(res)
    }
    (_, "/run/type_headers_deno_types.foo.js") => {
      let mut res = Response::new(Body::from(
        "export function foo(text) { console.log(text); }",
      ));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      res.headers_mut().insert(
        "X-TypeScript-Types",
        HeaderValue::from_static(
          "http://localhost:4545/run/type_headers_deno_types.d.ts",
        ),
      );
      Ok(res)
    }
    (_, "/run/type_headers_deno_types.d.ts") => {
      let mut res =
        Response::new(Body::from("export function foo(text: number): void;"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/run/type_headers_deno_types.foo.d.ts") => {
      let mut res =
        Response::new(Body::from("export function foo(text: string): void;"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/subdir/xTypeScriptTypesRedirect.d.ts") => {
      let mut res = Response::new(Body::from(
        "import './xTypeScriptTypesRedirected.d.ts';",
      ));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/subdir/xTypeScriptTypesRedirected.d.ts") => {
      let mut res = Response::new(Body::from("export const foo: 'foo';"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/referenceTypes.js") => {
      let mut res = Response::new(Body::from("/// <reference types=\"./xTypeScriptTypes.d.ts\" />\r\nexport const foo = \"foo\";\r\n"));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      Ok(res)
    }
    (_, "/subdir/file_with_:_in_name.ts") => {
      let mut res = Response::new(Body::from(
        "console.log('Hello from file_with_:_in_name.ts');",
      ));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/v1/extensionless") => {
      let mut res =
        Response::new(Body::from(r#"export * from "/subdir/mod1.ts";"#));
      res.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/typescript"),
      );
      Ok(res)
    }
    (_, "/subdir/no_js_ext@1.0.0") => {
      let mut res = Response::new(Body::from(
        r#"import { printHello } from "./mod2.ts";
        printHello();
        "#,
      ));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      Ok(res)
    }
    (_, "/.well-known/deno-import-intellisense.json") => {
      let file_path =
        testdata_path().join("lsp/registries/deno-import-intellisense.json");
      if let Ok(body) = tokio::fs::read(file_path).await {
        Ok(custom_headers(
          "/.well-known/deno-import-intellisense.json",
          body,
        ))
      } else {
        Ok(Response::new(Body::empty()))
      }
    }
    (_, "/http_version") => {
      let version = format!("{:?}", req.version());
      Ok(Response::new(version.into()))
    }
    (_, "/content_length") => {
      let content_length = format!("{:?}", req.headers().get("content-length"));
      Ok(Response::new(content_length.into()))
    }
    (_, "/jsx/jsx-runtime") | (_, "/jsx/jsx-dev-runtime") => {
      let mut res = Response::new(Body::from(
        r#"export function jsx(
          _type,
          _props,
          _key,
          _source,
          _self,
        ) {}
        export const jsxs = jsx;
        export const jsxDEV = jsx;
        export const Fragment = Symbol("Fragment");
        console.log("imported", import.meta.url);
        "#,
      ));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/javascript"),
      );
      Ok(res)
    }
    (_, "/dynamic") => {
      let mut res = Response::new(Body::from(
        serde_json::to_string_pretty(&std::time::SystemTime::now()).unwrap(),
      ));
      res
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-cache"));
      Ok(res)
    }
    (_, "/dynamic_cache") => {
      let mut res = Response::new(Body::from(
        serde_json::to_string_pretty(&std::time::SystemTime::now()).unwrap(),
      ));
      res.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("public, max-age=604800, immutable"),
      );
      Ok(res)
    }
    (_, "/echo_accept") => {
      let accept = req.headers().get("accept").map(|v| v.to_str().unwrap());
      let res = Response::new(Body::from(
        serde_json::json!({ "accept": accept }).to_string(),
      ));
      Ok(res)
    }
    _ => {
      let mut file_path = testdata_path();
      file_path.push(&req.uri().path()[1..]);
      if let Ok(file) = tokio::fs::read(&file_path).await {
        let file_resp = custom_headers(req.uri().path(), file);
        return Ok(file_resp);
      }

      // serve npm registry files
      if let Some(suffix) =
        req.uri().path().strip_prefix("/npm/registry/@denotest/")
      {
        // serve all requests to /npm/registry/@deno using the file system
        // at that path
        match handle_custom_npm_registry_path(suffix) {
          Ok(Some(response)) => return Ok(response),
          Ok(None) => {} // ignore, not found
          Err(err) => {
            return Response::builder()
              .status(StatusCode::INTERNAL_SERVER_ERROR)
              .body(format!("{:#}", err).into());
          }
        }
      } else if req.uri().path().starts_with("/npm/registry/") {
        // otherwise, serve based on registry.json and tgz files
        let is_tarball = req.uri().path().ends_with(".tgz");
        if !is_tarball {
          file_path.push("registry.json");
        }
        if let Ok(file) = tokio::fs::read(&file_path).await {
          let file_resp = custom_headers(req.uri().path(), file);
          return Ok(file_resp);
        } else if should_download_npm_packages() {
          if let Err(err) =
            download_npm_registry_file(req.uri(), &file_path, is_tarball).await
          {
            return Response::builder()
              .status(StatusCode::INTERNAL_SERVER_ERROR)
              .body(format!("{:#}", err).into());
          };

          // serve the file
          if let Ok(file) = tokio::fs::read(&file_path).await {
            let file_resp = custom_headers(req.uri().path(), file);
            return Ok(file_resp);
          }
        }
      }

      Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
    }
  };
}

fn handle_custom_npm_registry_path(
  path: &str,
) -> Result<Option<Response<Body>>, anyhow::Error> {
  let parts = path
    .split('/')
    .filter(|p| !p.is_empty())
    .collect::<Vec<_>>();
  let cache = &CUSTOM_NPM_PACKAGE_CACHE;
  let package_name = format!("@denotest/{}", parts[0]);
  if parts.len() == 2 {
    if let Some(file_bytes) =
      cache.tarball_bytes(&package_name, parts[1].trim_end_matches(".tgz"))?
    {
      let file_resp = custom_headers("file.tgz", file_bytes);
      return Ok(Some(file_resp));
    }
  } else if parts.len() == 1 {
    if let Some(registry_file) = cache.registry_file(&package_name)? {
      let file_resp = custom_headers("registry.json", registry_file);
      return Ok(Some(file_resp));
    }
  }

  Ok(None)
}

fn should_download_npm_packages() -> bool {
  // when this env var is set, it will download and save npm packages
  // to the testdata/npm/registry directory
  std::env::var("DENO_TEST_UTIL_UPDATE_NPM") == Ok("1".to_string())
}

async fn download_npm_registry_file(
  uri: &hyper::Uri,
  file_path: &PathBuf,
  is_tarball: bool,
) -> Result<(), anyhow::Error> {
  let url_parts = uri
    .path()
    .strip_prefix("/npm/registry/")
    .unwrap()
    .split('/')
    .collect::<Vec<_>>();
  let package_name = if url_parts[0].starts_with('@') {
    url_parts.into_iter().take(2).collect::<Vec<_>>().join("/")
  } else {
    url_parts.into_iter().take(1).collect::<Vec<_>>().join("/")
  };
  let url = if is_tarball {
    let file_name = file_path.file_name().unwrap().to_string_lossy();
    format!(
      "https://registry.npmjs.org/{}/-/{}",
      package_name, file_name
    )
  } else {
    format!("https://registry.npmjs.org/{}", package_name)
  };
  let client = reqwest::Client::new();
  let response = client.get(url).send().await?;
  let bytes = response.bytes().await?;
  let bytes = if is_tarball {
    bytes.to_vec()
  } else {
    String::from_utf8(bytes.to_vec())
      .unwrap()
      .replace(
        &format!("https://registry.npmjs.org/{}/-/", package_name),
        &format!("http://localhost:4545/npm/registry/{}/", package_name),
      )
      .into_bytes()
  };
  std::fs::create_dir_all(file_path.parent().unwrap())?;
  std::fs::write(&file_path, bytes)?;
  Ok(())
}

/// Taken from example in https://github.com/ctz/hyper-rustls/blob/a02ef72a227dcdf102f86e905baa7415c992e8b3/examples/server.rs
struct HyperAcceptor<'a> {
  acceptor: Pin<
    Box<
      dyn Stream<Item = io::Result<tokio_rustls::server::TlsStream<TcpStream>>>
        + 'a,
    >,
  >,
}

impl hyper::server::accept::Accept for HyperAcceptor<'_> {
  type Conn = tokio_rustls::server::TlsStream<TcpStream>;
  type Error = io::Error;

  fn poll_accept(
    mut self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
    Pin::new(&mut self.acceptor).poll_next(cx)
  }
}

#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: unsafe trait must have unsafe implementation
unsafe impl std::marker::Send for HyperAcceptor<'_> {}

async fn wrap_redirect_server() {
  let redirect_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(redirect)) });
  let redirect_addr = SocketAddr::from(([127, 0, 0, 1], REDIRECT_PORT));
  let redirect_server = Server::bind(&redirect_addr).serve(redirect_svc);
  if let Err(e) = redirect_server.await {
    eprintln!("Redirect error: {:?}", e);
  }
}

async fn wrap_double_redirect_server() {
  let double_redirects_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(double_redirects))
  });
  let double_redirects_addr =
    SocketAddr::from(([127, 0, 0, 1], DOUBLE_REDIRECTS_PORT));
  let double_redirects_server =
    Server::bind(&double_redirects_addr).serve(double_redirects_svc);
  if let Err(e) = double_redirects_server.await {
    eprintln!("Double redirect error: {:?}", e);
  }
}

async fn wrap_inf_redirect_server() {
  let inf_redirects_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(inf_redirects))
  });
  let inf_redirects_addr =
    SocketAddr::from(([127, 0, 0, 1], INF_REDIRECTS_PORT));
  let inf_redirects_server =
    Server::bind(&inf_redirects_addr).serve(inf_redirects_svc);
  if let Err(e) = inf_redirects_server.await {
    eprintln!("Inf redirect error: {:?}", e);
  }
}

async fn wrap_another_redirect_server() {
  let another_redirect_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(another_redirect))
  });
  let another_redirect_addr =
    SocketAddr::from(([127, 0, 0, 1], ANOTHER_REDIRECT_PORT));
  let another_redirect_server =
    Server::bind(&another_redirect_addr).serve(another_redirect_svc);
  if let Err(e) = another_redirect_server.await {
    eprintln!("Another redirect error: {:?}", e);
  }
}

async fn wrap_auth_redirect_server() {
  let auth_redirect_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(auth_redirect))
  });
  let auth_redirect_addr =
    SocketAddr::from(([127, 0, 0, 1], AUTH_REDIRECT_PORT));
  let auth_redirect_server =
    Server::bind(&auth_redirect_addr).serve(auth_redirect_svc);
  if let Err(e) = auth_redirect_server.await {
    eprintln!("Auth redirect error: {:?}", e);
  }
}

async fn wrap_basic_auth_redirect_server() {
  let basic_auth_redirect_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(basic_auth_redirect))
  });
  let basic_auth_redirect_addr =
    SocketAddr::from(([127, 0, 0, 1], BASIC_AUTH_REDIRECT_PORT));
  let basic_auth_redirect_server =
    Server::bind(&basic_auth_redirect_addr).serve(basic_auth_redirect_svc);
  if let Err(e) = basic_auth_redirect_server.await {
    eprintln!("Basic auth redirect error: {:?}", e);
  }
}

async fn wrap_abs_redirect_server() {
  let abs_redirect_svc = make_service_fn(|_| async {
    Ok::<_, Infallible>(service_fn(absolute_redirect))
  });
  let abs_redirect_addr =
    SocketAddr::from(([127, 0, 0, 1], REDIRECT_ABSOLUTE_PORT));
  let abs_redirect_server =
    Server::bind(&abs_redirect_addr).serve(abs_redirect_svc);
  if let Err(e) = abs_redirect_server.await {
    eprintln!("Absolute redirect error: {:?}", e);
  }
}

async fn wrap_main_server() {
  let main_server_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_addr = SocketAddr::from(([127, 0, 0, 1], PORT));
  let main_server = Server::bind(&main_server_addr).serve(main_server_svc);
  if let Err(e) = main_server.await {
    eprintln!("HTTP server error: {:?}", e);
  }
}

async fn wrap_main_https_server() {
  let main_server_https_addr = SocketAddr::from(([127, 0, 0, 1], HTTPS_PORT));
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, Default::default())
      .await
      .unwrap();
  loop {
    let tcp = TcpListener::bind(&main_server_https_addr)
      .await
      .expect("Cannot bind TCP");
    println!("ready: https"); // Eye catcher for HttpServerCount
    let tls_acceptor = TlsAcceptor::from(tls_config.clone());
    // Prepare a long-running future stream to accept and serve cients.
    let incoming_tls_stream = async_stream::stream! {
      loop {
          let (socket, _) = tcp.accept().await?;
          let stream = tls_acceptor.accept(socket);
          yield stream.await;
      }
    }
    .boxed();

    let main_server_https_svc = make_service_fn(|_| async {
      Ok::<_, Infallible>(service_fn(main_server))
    });
    let main_server_https = Server::builder(HyperAcceptor {
      acceptor: incoming_tls_stream,
    })
    .serve(main_server_https_svc);

    //continue to prevent TLS error stopping the server
    if main_server_https.await.is_err() {
      continue;
    }
  }
}

async fn wrap_https_h1_only_server() {
  let main_server_https_addr = SocketAddr::from(([127, 0, 0, 1], H1_ONLY_PORT));
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config = get_tls_config(
    cert_file,
    key_file,
    ca_cert_file,
    SupportedHttpVersions::Http1Only,
  )
  .await
  .unwrap();
  loop {
    let tcp = TcpListener::bind(&main_server_https_addr)
      .await
      .expect("Cannot bind TCP");
    println!("ready: https"); // Eye catcher for HttpServerCount
    let tls_acceptor = TlsAcceptor::from(tls_config.clone());
    // Prepare a long-running future stream to accept and serve cients.
    let incoming_tls_stream = async_stream::stream! {
      loop {
          let (socket, _) = tcp.accept().await?;
          let stream = tls_acceptor.accept(socket);
          yield stream.await;
      }
    }
    .boxed();

    let main_server_https_svc = make_service_fn(|_| async {
      Ok::<_, Infallible>(service_fn(main_server))
    });
    let main_server_https = Server::builder(HyperAcceptor {
      acceptor: incoming_tls_stream,
    })
    .http1_only(true)
    .serve(main_server_https_svc);

    //continue to prevent TLS error stopping the server
    if main_server_https.await.is_err() {
      continue;
    }
  }
}

async fn wrap_https_h2_only_server() {
  let main_server_https_addr = SocketAddr::from(([127, 0, 0, 1], H2_ONLY_PORT));
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config = get_tls_config(
    cert_file,
    key_file,
    ca_cert_file,
    SupportedHttpVersions::Http2Only,
  )
  .await
  .unwrap();
  loop {
    let tcp = TcpListener::bind(&main_server_https_addr)
      .await
      .expect("Cannot bind TCP");
    println!("ready: https"); // Eye catcher for HttpServerCount
    let tls_acceptor = TlsAcceptor::from(tls_config.clone());
    // Prepare a long-running future stream to accept and serve cients.
    let incoming_tls_stream = async_stream::stream! {
      loop {
          let (socket, _) = tcp.accept().await?;
          let stream = tls_acceptor.accept(socket);
          yield stream.await;
      }
    }
    .boxed();

    let main_server_https_svc = make_service_fn(|_| async {
      Ok::<_, Infallible>(service_fn(main_server))
    });
    let main_server_https = Server::builder(HyperAcceptor {
      acceptor: incoming_tls_stream,
    })
    .http2_only(true)
    .serve(main_server_https_svc);

    //continue to prevent TLS error stopping the server
    if main_server_https.await.is_err() {
      continue;
    }
  }
}

async fn wrap_client_auth_https_server() {
  let main_server_https_addr =
    SocketAddr::from(([127, 0, 0, 1], HTTPS_CLIENT_AUTH_PORT));
  let cert_file = "tls/localhost.crt";
  let key_file = "tls/localhost.key";
  let ca_cert_file = "tls/RootCA.pem";
  let tls_config =
    get_tls_config(cert_file, key_file, ca_cert_file, Default::default())
      .await
      .unwrap();
  loop {
    let tcp = TcpListener::bind(&main_server_https_addr)
      .await
      .expect("Cannot bind TCP");
    println!("ready: https_client_auth on :{:?}", HTTPS_CLIENT_AUTH_PORT); // Eye catcher for HttpServerCount
    let tls_acceptor = TlsAcceptor::from(tls_config.clone());
    // Prepare a long-running future stream to accept and serve cients.
    let incoming_tls_stream = async_stream::stream! {
      loop {
          let (socket, _) = tcp.accept().await?;

          match tls_acceptor.accept(socket).await {
            Ok(mut tls_stream) => {
              let (_, tls_session) = tls_stream.get_mut();
              // We only need to check for the presence of client certificates
              // here. Rusttls ensures that they are valid and signed by the CA.
              match tls_session.peer_certificates() {
                Some(_certs) => { yield Ok(tls_stream); },
                None => { eprintln!("https_client_auth: no valid client certificate"); },
              };
            }

            Err(e) => {
              eprintln!("https-client-auth accept error: {:?}", e);
              yield Err(e);
            }
          }

      }
    }
    .boxed();

    let main_server_https_svc = make_service_fn(|_| async {
      Ok::<_, Infallible>(service_fn(main_server))
    });
    let main_server_https = Server::builder(HyperAcceptor {
      acceptor: incoming_tls_stream,
    })
    .serve(main_server_https_svc);

    //continue to prevent TLS error stopping the server
    if main_server_https.await.is_err() {
      continue;
    }
  }
}

// Use the single-threaded scheduler. The hyper server is used as a point of
// comparison for the (single-threaded!) benchmarks in cli/bench. We're not
// comparing apples to apples if we use the default multi-threaded scheduler.
#[tokio::main(flavor = "current_thread")]
pub async fn run_all_servers() {
  if let Some(port) = env::args().nth(1) {
    return hyper_hello(port.parse::<u16>().unwrap()).await;
  }

  let redirect_server_fut = wrap_redirect_server();
  let double_redirects_server_fut = wrap_double_redirect_server();
  let inf_redirects_server_fut = wrap_inf_redirect_server();
  let another_redirect_server_fut = wrap_another_redirect_server();
  let auth_redirect_server_fut = wrap_auth_redirect_server();
  let basic_auth_redirect_server_fut = wrap_basic_auth_redirect_server();
  let abs_redirect_server_fut = wrap_abs_redirect_server();

  let ws_addr = SocketAddr::from(([127, 0, 0, 1], WS_PORT));
  let ws_server_fut = run_ws_server(&ws_addr);
  let wss_addr = SocketAddr::from(([127, 0, 0, 1], WSS_PORT));
  let wss_server_fut = run_wss_server(&wss_addr);
  let ws_close_addr = SocketAddr::from(([127, 0, 0, 1], WS_CLOSE_PORT));
  let ws_close_server_fut = run_ws_close_server(&ws_close_addr);

  let tls_server_fut = run_tls_server();
  let tls_client_auth_server_fut = run_tls_client_auth_server();
  let client_auth_server_https_fut = wrap_client_auth_https_server();
  let main_server_fut = wrap_main_server();
  let main_server_https_fut = wrap_main_https_server();
  let h1_only_server_fut = wrap_https_h1_only_server();
  let h2_only_server_fut = wrap_https_h2_only_server();

  let mut server_fut = async {
    futures::join!(
      redirect_server_fut,
      ws_server_fut,
      wss_server_fut,
      tls_server_fut,
      tls_client_auth_server_fut,
      ws_close_server_fut,
      another_redirect_server_fut,
      auth_redirect_server_fut,
      basic_auth_redirect_server_fut,
      inf_redirects_server_fut,
      double_redirects_server_fut,
      abs_redirect_server_fut,
      main_server_fut,
      main_server_https_fut,
      client_auth_server_https_fut,
      h1_only_server_fut,
      h2_only_server_fut
    )
  }
  .boxed();

  let mut did_print_ready = false;
  futures::future::poll_fn(move |cx| {
    let poll_result = server_fut.poll_unpin(cx);
    if !replace(&mut did_print_ready, true) {
      println!("ready: server_fut"); // Eye catcher for HttpServerCount
    }
    poll_result
  })
  .await;
}

fn custom_headers(p: &str, body: Vec<u8>) -> Response<Body> {
  let mut response = Response::new(Body::from(body));

  if p.ends_with("/run/import_compression/brotli") {
    response
      .headers_mut()
      .insert("Content-Encoding", HeaderValue::from_static("br"));
    response.headers_mut().insert(
      "Content-Type",
      HeaderValue::from_static("application/javascript"),
    );
    response
      .headers_mut()
      .insert("Content-Length", HeaderValue::from_static("26"));
    return response;
  }
  if p.ends_with("/run/import_compression/gziped") {
    response
      .headers_mut()
      .insert("Content-Encoding", HeaderValue::from_static("gzip"));
    response.headers_mut().insert(
      "Content-Type",
      HeaderValue::from_static("application/javascript"),
    );
    response
      .headers_mut()
      .insert("Content-Length", HeaderValue::from_static("39"));
    return response;
  }

  if p.contains("/encoding/") {
    let charset = p
      .split_terminator('/')
      .last()
      .unwrap()
      .trim_end_matches(".ts");

    response.headers_mut().insert(
      "Content-Type",
      HeaderValue::from_str(
        &format!("application/typescript;charset={}", charset)[..],
      )
      .unwrap(),
    );
    return response;
  }

  let content_type = if p.contains(".t1.") {
    Some("text/typescript")
  } else if p.contains(".t2.") {
    Some("video/vnd.dlna.mpeg-tts")
  } else if p.contains(".t3.") {
    Some("video/mp2t")
  } else if p.contains(".t4.") {
    Some("application/x-typescript")
  } else if p.contains(".j1.") {
    Some("text/javascript")
  } else if p.contains(".j2.") {
    Some("application/ecmascript")
  } else if p.contains(".j3.") {
    Some("text/ecmascript")
  } else if p.contains(".j4.") {
    Some("application/x-javascript")
  } else if p.contains("form_urlencoded") {
    Some("application/x-www-form-urlencoded")
  } else if p.contains("unknown_ext") || p.contains("no_ext") {
    Some("text/typescript")
  } else if p.contains("mismatch_ext") || p.contains("no_js_ext") {
    Some("text/javascript")
  } else if p.ends_with(".ts") || p.ends_with(".tsx") {
    Some("application/typescript")
  } else if p.ends_with(".js") || p.ends_with(".jsx") {
    Some("application/javascript")
  } else if p.ends_with(".json") {
    Some("application/json")
  } else if p.ends_with(".wasm") {
    Some("application/wasm")
  } else if p.ends_with(".tgz") {
    Some("application/gzip")
  } else {
    None
  };

  if let Some(t) = content_type {
    response
      .headers_mut()
      .insert("Content-Type", HeaderValue::from_str(t).unwrap());
    return response;
  }

  response
}

#[derive(Default)]
struct HttpServerCount {
  count: usize,
  test_server: Option<Child>,
}

impl HttpServerCount {
  fn inc(&mut self) {
    self.count += 1;
    if self.test_server.is_none() {
      assert_eq!(self.count, 1);

      println!("test_server starting...");
      let mut test_server = Command::new(test_server_path())
        .current_dir(testdata_path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to execute test_server");
      let stdout = test_server.stdout.as_mut().unwrap();
      use std::io::{BufRead, BufReader};
      let lines = BufReader::new(stdout).lines();

      // Wait for all the servers to report being ready.
      let mut ready_count = 0;
      for maybe_line in lines {
        if let Ok(line) = maybe_line {
          if line.starts_with("ready:") {
            ready_count += 1;
          }
          if ready_count == 6 {
            break;
          }
        } else {
          panic!("{}", maybe_line.unwrap_err());
        }
      }
      self.test_server = Some(test_server);
    }
  }

  fn dec(&mut self) {
    assert!(self.count > 0);
    self.count -= 1;
    if self.count == 0 {
      let mut test_server = self.test_server.take().unwrap();
      match test_server.try_wait() {
        Ok(None) => {
          test_server.kill().expect("failed to kill test_server");
          let _ = test_server.wait();
        }
        Ok(Some(status)) => {
          panic!("test_server exited unexpectedly {}", status)
        }
        Err(e) => panic!("test_server error: {}", e),
      }
    }
  }
}

impl Drop for HttpServerCount {
  fn drop(&mut self) {
    assert_eq!(self.count, 0);
    assert!(self.test_server.is_none());
  }
}

fn lock_http_server<'a>() -> MutexGuard<'a, HttpServerCount> {
  let r = GUARD.lock();
  if let Err(poison_err) = r {
    // If panics happened, ignore it. This is for tests.
    poison_err.into_inner()
  } else {
    r.unwrap()
  }
}

pub struct HttpServerGuard {}

impl Drop for HttpServerGuard {
  fn drop(&mut self) {
    let mut g = lock_http_server();
    g.dec();
  }
}

/// Adds a reference to a shared target/debug/test_server subprocess. When the
/// last instance of the HttpServerGuard is dropped, the subprocess will be
/// killed.
pub fn http_server() -> HttpServerGuard {
  ensure_test_server_built();
  let mut g = lock_http_server();
  g.inc();
  HttpServerGuard {}
}

/// Helper function to strip ansi codes.
pub fn strip_ansi_codes(s: &str) -> std::borrow::Cow<str> {
  STRIP_ANSI_RE.replace_all(s, "")
}

pub fn run(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder.args(&cmd[1..]).stdin(Stdio::piped());

  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let status = prog.wait().expect("failed to wait on child");
  if expect_success != status.success() {
    panic!("Unexpected exit code: {:?}", status.code());
  }
}

pub fn run_collect(
  cmd: &[&str],
  input: Option<&[&str]>,
  envs: Option<Vec<(String, String)>>,
  current_dir: Option<&str>,
  expect_success: bool,
) -> (String, String) {
  let mut process_builder = Command::new(cmd[0]);
  process_builder
    .args(&cmd[1..])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(dir) = current_dir {
    process_builder.current_dir(dir);
  }
  if let Some(envs) = envs {
    process_builder.envs(envs);
  }
  let mut prog = process_builder.spawn().expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = prog.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = prog.wait_with_output().expect("failed to wait on child");
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{}>>>", stdout);
    eprintln!("stderr: <<<{}>>>", stderr);
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn run_and_collect_output(
  expect_success: bool,
  args: &str,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  run_and_collect_output_with_args(
    expect_success,
    args.split_whitespace().collect(),
    input,
    envs,
    need_http_server,
  )
}

pub fn run_and_collect_output_with_args(
  expect_success: bool,
  args: Vec<&str>,
  input: Option<Vec<&str>>,
  envs: Option<Vec<(String, String)>>,
  need_http_server: bool,
) -> (String, String) {
  let mut deno_process_builder = deno_cmd();
  deno_process_builder
    .args(args)
    .current_dir(&testdata_path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(envs) = envs {
    deno_process_builder.envs(envs);
  }
  let _http_guard = if need_http_server {
    Some(http_server())
  } else {
    None
  };
  let mut deno = deno_process_builder
    .spawn()
    .expect("failed to spawn script");
  if let Some(lines) = input {
    let stdin = deno.stdin.as_mut().expect("failed to get stdin");
    stdin
      .write_all(lines.join("\n").as_bytes())
      .expect("failed to write to stdin");
  }
  let Output {
    stdout,
    stderr,
    status,
  } = deno.wait_with_output().expect("failed to wait on child");
  let stdout = String::from_utf8(stdout).unwrap();
  let stderr = String::from_utf8(stderr).unwrap();
  if expect_success != status.success() {
    eprintln!("stdout: <<<{}>>>", stdout);
    eprintln!("stderr: <<<{}>>>", stderr);
    panic!("Unexpected exit code: {:?}", status.code());
  }
  (stdout, stderr)
}

pub fn new_deno_dir() -> TempDir {
  TempDir::new()
}

pub struct DenoCmd {
  // keep the deno dir directory alive for the duration of the command
  _deno_dir: TempDir,
  cmd: Command,
}

impl Deref for DenoCmd {
  type Target = Command;
  fn deref(&self) -> &Command {
    &self.cmd
  }
}

impl DerefMut for DenoCmd {
  fn deref_mut(&mut self) -> &mut Command {
    &mut self.cmd
  }
}

pub fn deno_cmd() -> DenoCmd {
  let deno_dir = new_deno_dir();
  deno_cmd_with_deno_dir(&deno_dir)
}

pub fn deno_cmd_with_deno_dir(deno_dir: &TempDir) -> DenoCmd {
  let exe_path = deno_exe_path();
  assert!(exe_path.exists());
  let mut cmd = Command::new(exe_path);
  cmd.env("DENO_DIR", deno_dir.path());
  DenoCmd {
    _deno_dir: deno_dir.clone(),
    cmd,
  }
}

pub fn run_powershell_script_file(
  script_file_path: &str,
  args: Vec<&str>,
) -> std::result::Result<(), i64> {
  let deno_dir = new_deno_dir();
  let mut command = Command::new("powershell.exe");

  command
    .env("DENO_DIR", deno_dir.path())
    .current_dir(testdata_path())
    .arg("-file")
    .arg(script_file_path);

  for arg in args {
    command.arg(arg);
  }

  let output = command.output().expect("failed to spawn script");
  let stdout = String::from_utf8(output.stdout).unwrap();
  let stderr = String::from_utf8(output.stderr).unwrap();
  println!("{}", stdout);
  if !output.status.success() {
    panic!(
      "{} executed with failing error code\n{}{}",
      script_file_path, stdout, stderr
    );
  }

  Ok(())
}

#[derive(Debug, Default)]
pub struct CheckOutputIntegrationTest<'a> {
  pub args: &'a str,
  pub args_vec: Vec<&'a str>,
  pub output: &'a str,
  pub input: Option<&'a str>,
  pub output_str: Option<&'a str>,
  pub exit_code: i32,
  pub http_server: bool,
  pub envs: Vec<(String, String)>,
  pub env_clear: bool,
  pub temp_cwd: bool,
}

impl<'a> CheckOutputIntegrationTest<'a> {
  pub fn run(&self) {
    let args = if self.args_vec.is_empty() {
      std::borrow::Cow::Owned(self.args.split_whitespace().collect::<Vec<_>>())
    } else {
      assert!(
        self.args.is_empty(),
        "Do not provide args when providing args_vec."
      );
      std::borrow::Cow::Borrowed(&self.args_vec)
    };
    let testdata_dir = testdata_path();
    let args = args
      .iter()
      .map(|arg| arg.replace("$TESTDATA", &testdata_dir.to_string_lossy()))
      .collect::<Vec<_>>();
    let deno_exe = deno_exe_path();
    println!("deno_exe path {}", deno_exe.display());

    let _http_server_guard = if self.http_server {
      Some(http_server())
    } else {
      None
    };

    let (mut reader, writer) = pipe().unwrap();
    let deno_dir = new_deno_dir(); // keep this alive for the test
    let mut command = deno_cmd_with_deno_dir(&deno_dir);
    let cwd = if self.temp_cwd {
      deno_dir.path()
    } else {
      testdata_dir.as_path()
    };
    println!("deno_exe args {}", args.join(" "));
    println!("deno_exe cwd {:?}", &testdata_dir);
    command.args(args.iter());
    if self.env_clear {
      command.env_clear();
    }
    command.envs(self.envs.clone());
    command.current_dir(&cwd);
    command.stdin(Stdio::piped());
    let writer_clone = writer.try_clone().unwrap();
    command.stderr(writer_clone);
    command.stdout(writer);

    let mut process = command.spawn().expect("failed to execute process");

    if let Some(input) = self.input {
      let mut p_stdin = process.stdin.take().unwrap();
      write!(p_stdin, "{}", input).unwrap();
    }

    // Very important when using pipes: This parent process is still
    // holding its copies of the write ends, and we have to close them
    // before we read, otherwise the read end will never report EOF. The
    // Command object owns the writers now, and dropping it closes them.
    drop(command);

    let mut actual = String::new();
    reader.read_to_string(&mut actual).unwrap();

    let status = process.wait().expect("failed to finish process");

    if let Some(exit_code) = status.code() {
      if self.exit_code != exit_code {
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!(
          "bad exit code, expected: {:?}, actual: {:?}",
          self.exit_code, exit_code
        );
      }
    } else {
      #[cfg(unix)]
      {
        use std::os::unix::process::ExitStatusExt;
        let signal = status.signal().unwrap();
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!(
          "process terminated by signal, expected exit code: {:?}, actual signal: {:?}",
          self.exit_code, signal
        );
      }
      #[cfg(not(unix))]
      {
        println!("OUTPUT\n{}\nOUTPUT", actual);
        panic!("process terminated without status code on non unix platform, expected exit code: {:?}", self.exit_code);
      }
    }

    actual = strip_ansi_codes(&actual).to_string();

    // deno test's output capturing flushes with a zero-width space in order to
    // synchronize the output pipes. Occassionally this zero width space
    // might end up in the output so strip it from the output comparison here.
    if args.first().map(|s| s.as_str()) == Some("test") {
      actual = actual.replace('\u{200B}', "");
    }

    let expected = if let Some(s) = self.output_str {
      s.to_owned()
    } else if self.output.is_empty() {
      String::new()
    } else {
      let output_path = testdata_dir.join(self.output);
      println!("output path {}", output_path.display());
      std::fs::read_to_string(output_path).expect("cannot read output")
    };

    if !expected.contains("[WILDCARD]") {
      assert_eq!(actual, expected)
    } else if !wildcard_match(&expected, &actual) {
      println!("OUTPUT\n{}\nOUTPUT", actual);
      println!("EXPECTED\n{}\nEXPECTED", expected);
      panic!("pattern match failed");
    }
  }
}

pub fn wildcard_match(pattern: &str, s: &str) -> bool {
  pattern_match(pattern, s, "[WILDCARD]")
}

pub fn pattern_match(pattern: &str, s: &str, wildcard: &str) -> bool {
  // Normalize line endings
  let mut s = s.replace("\r\n", "\n");
  let pattern = pattern.replace("\r\n", "\n");

  if pattern == wildcard {
    return true;
  }

  let parts = pattern.split(wildcard).collect::<Vec<&str>>();
  if parts.len() == 1 {
    return pattern == s;
  }

  if !s.starts_with(parts[0]) {
    return false;
  }

  // If the first line of the pattern is just a wildcard the newline character
  // needs to be pre-pended so it can safely match anything or nothing and
  // continue matching.
  if pattern.lines().next() == Some(wildcard) {
    s.insert(0, '\n');
  }

  let mut t = s.split_at(parts[0].len());

  for (i, part) in parts.iter().enumerate() {
    if i == 0 {
      continue;
    }
    dbg!(part, i);
    if i == parts.len() - 1 && (part.is_empty() || *part == "\n") {
      dbg!("exit 1 true", i);
      return true;
    }
    if let Some(found) = t.1.find(*part) {
      dbg!("found ", found);
      t = t.1.split_at(found + part.len());
    } else {
      dbg!("exit false ", i);
      return false;
    }
  }

  dbg!("end ", t.1.len());
  t.1.is_empty()
}

pub enum PtyData {
  Input(&'static str),
  Output(&'static str),
}

pub fn test_pty2(args: &str, data: Vec<PtyData>) {
  use std::io::BufRead;

  with_pty(&args.split_whitespace().collect::<Vec<_>>(), |console| {
    let mut buf_reader = std::io::BufReader::new(console);
    for d in data.iter() {
      match d {
        PtyData::Input(s) => {
          println!("INPUT {}", s.escape_debug());
          buf_reader.get_mut().write_text(s);

          // Because of tty echo, we should be able to read the same string back.
          assert!(s.ends_with('\n'));
          let mut echo = String::new();
          buf_reader.read_line(&mut echo).unwrap();
          println!("ECHO: {}", echo.escape_debug());

          // Windows may also echo the previous line, so only check the end
          assert_ends_with!(normalize_text(&echo), normalize_text(s));
        }
        PtyData::Output(s) => {
          let mut line = String::new();
          if s.ends_with('\n') {
            buf_reader.read_line(&mut line).unwrap();
          } else {
            // assumes the buffer won't have overlapping virtual terminal sequences
            while normalize_text(&line).len() < normalize_text(s).len() {
              let mut buf = [0; 64 * 1024];
              let bytes_read = buf_reader.read(&mut buf).unwrap();
              assert!(bytes_read > 0);
              let buf_str = std::str::from_utf8(&buf)
                .unwrap()
                .trim_end_matches(char::from(0));
              line += buf_str;
            }
          }
          println!("OUTPUT {}", line.escape_debug());
          assert_eq!(normalize_text(&line), normalize_text(s));
        }
      }
    }
  });

  // This normalization function is not comprehensive
  // and may need to updated as new scenarios emerge.
  fn normalize_text(text: &str) -> String {
    lazy_static! {
      static ref MOVE_CURSOR_RIGHT_ONE_RE: Regex =
        Regex::new(r"\x1b\[1C").unwrap();
      static ref FOUND_SEQUENCES_RE: Regex =
        Regex::new(r"(\x1b\]0;[^\x07]*\x07)*(\x08)*(\x1b\[\d+X)*").unwrap();
      static ref CARRIAGE_RETURN_RE: Regex =
        Regex::new(r"[^\n]*\r([^\n])").unwrap();
    }

    // any "move cursor right" sequences should just be a space
    let text = MOVE_CURSOR_RIGHT_ONE_RE.replace_all(text, " ");
    // replace additional virtual terminal sequences that strip ansi codes doesn't catch
    let text = FOUND_SEQUENCES_RE.replace_all(&text, "");
    // strip any ansi codes, which also strips more terminal sequences
    let text = strip_ansi_codes(&text);
    // get rid of any text that is overwritten with only a carriage return
    let text = CARRIAGE_RETURN_RE.replace_all(&text, "$1");
    // finally, trim surrounding whitespace
    text.trim().to_string()
  }
}

pub fn with_pty(deno_args: &[&str], mut action: impl FnMut(Box<dyn pty::Pty>)) {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    eprintln!("Ignoring non-tty environment.");
    return;
  }

  let deno_dir = new_deno_dir();
  let mut env_vars = std::collections::HashMap::new();
  env_vars.insert("NO_COLOR".to_string(), "1".to_string());
  env_vars.insert(
    "DENO_DIR".to_string(),
    deno_dir.path().to_string_lossy().to_string(),
  );
  let pty = pty::create_pty(
    &deno_exe_path().to_string_lossy().to_string(),
    deno_args,
    testdata_path(),
    Some(env_vars),
  );

  action(pty);
}

pub struct WrkOutput {
  pub latency: f64,
  pub requests: u64,
}

pub fn parse_wrk_output(output: &str) -> WrkOutput {
  lazy_static! {
    static ref REQUESTS_RX: Regex =
      Regex::new(r"Requests/sec:\s+(\d+)").unwrap();
    static ref LATENCY_RX: Regex =
      Regex::new(r"\s+99%(?:\s+(\d+.\d+)([a-z]+))").unwrap();
  }

  let mut requests = None;
  let mut latency = None;

  for line in output.lines() {
    if requests == None {
      if let Some(cap) = REQUESTS_RX.captures(line) {
        requests =
          Some(str::parse::<u64>(cap.get(1).unwrap().as_str()).unwrap());
      }
    }
    if latency == None {
      if let Some(cap) = LATENCY_RX.captures(line) {
        let time = cap.get(1).unwrap();
        let unit = cap.get(2).unwrap();

        latency = Some(
          str::parse::<f64>(time.as_str()).unwrap()
            * match unit.as_str() {
              "ms" => 1.0,
              "us" => 0.001,
              "s" => 1000.0,
              _ => unreachable!(),
            },
        );
      }
    }
  }

  WrkOutput {
    requests: requests.unwrap(),
    latency: latency.unwrap(),
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct StraceOutput {
  pub percent_time: f64,
  pub seconds: f64,
  pub usecs_per_call: Option<u64>,
  pub calls: u64,
  pub errors: u64,
}

pub fn parse_strace_output(output: &str) -> HashMap<String, StraceOutput> {
  let mut summary = HashMap::new();

  // Filter out non-relevant lines. See the error log at
  // https://github.com/denoland/deno/pull/3715/checks?check_run_id=397365887
  // This is checked in testdata/strace_summary2.out
  let mut lines = output.lines().filter(|line| {
    !line.is_empty()
      && !line.contains("detached ...")
      && !line.contains("unfinished ...")
      && !line.contains("????")
  });
  let count = lines.clone().count();

  if count < 4 {
    return summary;
  }

  let total_line = lines.next_back().unwrap();
  lines.next_back(); // Drop separator
  let data_lines = lines.skip(2);

  for line in data_lines {
    let syscall_fields = line.split_whitespace().collect::<Vec<_>>();
    let len = syscall_fields.len();
    let syscall_name = syscall_fields.last().unwrap();
    if (5..=6).contains(&len) {
      summary.insert(
        syscall_name.to_string(),
        StraceOutput {
          percent_time: str::parse::<f64>(syscall_fields[0]).unwrap(),
          seconds: str::parse::<f64>(syscall_fields[1]).unwrap(),
          usecs_per_call: Some(str::parse::<u64>(syscall_fields[2]).unwrap()),
          calls: str::parse::<u64>(syscall_fields[3]).unwrap(),
          errors: if syscall_fields.len() < 6 {
            0
          } else {
            str::parse::<u64>(syscall_fields[4]).unwrap()
          },
        },
      );
    }
  }

  let total_fields = total_line.split_whitespace().collect::<Vec<_>>();
  summary.insert(
    "total".to_string(),
    StraceOutput {
      percent_time: str::parse::<f64>(total_fields[0]).unwrap(),
      seconds: str::parse::<f64>(total_fields[1]).unwrap(),
      usecs_per_call: None,
      calls: str::parse::<u64>(total_fields[2]).unwrap(),
      errors: str::parse::<u64>(total_fields[3]).unwrap(),
    },
  );

  summary
}

pub fn parse_max_mem(output: &str) -> Option<u64> {
  // Takes the output from "time -v" as input and extracts the 'maximum
  // resident set size' and returns it in bytes.
  for line in output.lines() {
    if line
      .to_lowercase()
      .contains("maximum resident set size (kbytes)")
    {
      let value = line.split(": ").nth(1).unwrap();
      return Some(str::parse::<u64>(value).unwrap() * 1024);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn parse_wrk_output_1() {
    const TEXT: &str = include_str!("./testdata/wrk1.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 1837);
    assert!((wrk.latency - 6.25).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_2() {
    const TEXT: &str = include_str!("./testdata/wrk2.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 53435);
    assert!((wrk.latency - 6.22).abs() < f64::EPSILON);
  }

  #[test]
  fn parse_wrk_output_3() {
    const TEXT: &str = include_str!("./testdata/wrk3.txt");
    let wrk = parse_wrk_output(TEXT);
    assert_eq!(wrk.requests, 96037);
    assert!((wrk.latency - 6.36).abs() < f64::EPSILON);
  }

  #[test]
  fn strace_parse_1() {
    const TEXT: &str = include_str!("./testdata/strace_summary.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let munmap = strace.get("munmap").unwrap();
    assert_eq!(munmap.calls, 60);
    assert_eq!(munmap.errors, 0);

    // line with errors
    assert_eq!(strace.get("mkdir").unwrap().errors, 2);

    // last syscall line
    let prlimit = strace.get("prlimit64").unwrap();
    assert_eq!(prlimit.calls, 2);
    assert!((prlimit.percent_time - 0.0).abs() < f64::EPSILON);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 704);
    assert_eq!(strace.get("total").unwrap().errors, 5);
  }

  #[test]
  fn strace_parse_2() {
    const TEXT: &str = include_str!("./testdata/strace_summary2.out");
    let strace = parse_strace_output(TEXT);

    // first syscall line
    let futex = strace.get("futex").unwrap();
    assert_eq!(futex.calls, 449);
    assert_eq!(futex.errors, 94);

    // summary line
    assert_eq!(strace.get("total").unwrap().calls, 821);
    assert_eq!(strace.get("total").unwrap().errors, 107);
  }

  #[test]
  fn test_wildcard_match() {
    let fixtures = vec![
      ("foobarbaz", "foobarbaz", true),
      ("[WILDCARD]", "foobarbaz", true),
      ("foobar", "foobarbaz", false),
      ("foo[WILDCARD]baz", "foobarbaz", true),
      ("foo[WILDCARD]baz", "foobazbar", false),
      ("foo[WILDCARD]baz[WILDCARD]qux", "foobarbazqatqux", true),
      ("foo[WILDCARD]", "foobar", true),
      ("foo[WILDCARD]baz[WILDCARD]", "foobarbazqat", true),
      // check with different line endings
      ("foo[WILDCARD]\nbaz[WILDCARD]\n", "foobar\nbazqat\n", true),
      (
        "foo[WILDCARD]\nbaz[WILDCARD]\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\n",
        "foobar\nbazqat\r\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\nbazqat\n",
        true,
      ),
      (
        "foo[WILDCARD]\r\nbaz[WILDCARD]\r\n",
        "foobar\r\nbazqat\r\n",
        true,
      ),
    ];

    // Iterate through the fixture lists, testing each one
    for (pattern, string, expected) in fixtures {
      let actual = wildcard_match(pattern, string);
      dbg!(pattern, string, expected);
      assert_eq!(actual, expected);
    }
  }

  #[test]
  fn test_pattern_match() {
    // foo, bar, baz, qux, quux, quuz, corge, grault, garply, waldo, fred, plugh, xyzzy

    let wildcard = "[BAR]";
    assert!(pattern_match("foo[BAR]baz", "foobarbaz", wildcard));
    assert!(!pattern_match("foo[BAR]baz", "foobazbar", wildcard));

    let multiline_pattern = "[BAR]
foo:
[BAR]baz[BAR]";

    fn multi_line_builder(input: &str, leading_text: Option<&str>) -> String {
      // If there is leading text add a newline so it's on it's own line
      let head = match leading_text {
        Some(v) => format!("{}\n", v),
        None => "".to_string(),
      };
      format!(
        "{}foo:
quuz {} corge
grault",
        head, input
      )
    }

    // Validate multi-line string builder
    assert_eq!(
      "QUUX=qux
foo:
quuz BAZ corge
grault",
      multi_line_builder("BAZ", Some("QUUX=qux"))
    );

    // Correct input & leading line
    assert!(pattern_match(
      multiline_pattern,
      &multi_line_builder("baz", Some("QUX=quux")),
      wildcard
    ));

    // Correct input & no leading line
    assert!(pattern_match(
      multiline_pattern,
      &multi_line_builder("baz", None),
      wildcard
    ));

    // Incorrect input & leading line
    assert!(!pattern_match(
      multiline_pattern,
      &multi_line_builder("garply", Some("QUX=quux")),
      wildcard
    ));

    // Incorrect input & no leading line
    assert!(!pattern_match(
      multiline_pattern,
      &multi_line_builder("garply", None),
      wildcard
    ));
  }

  #[test]
  fn max_mem_parse() {
    const TEXT: &str = include_str!("./testdata/time.out");
    let size = parse_max_mem(TEXT);

    assert_eq!(size, Some(120380 * 1024));
  }
}
