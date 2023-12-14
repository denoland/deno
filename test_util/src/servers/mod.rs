// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Usage: provide a port as argument to run hyper_hello benchmark server
// otherwise this starts multiple servers on many ports for test endpoints.
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use denokv_proto::datapath::AtomicWrite;
use denokv_proto::datapath::AtomicWriteOutput;
use denokv_proto::datapath::AtomicWriteStatus;
use denokv_proto::datapath::ReadRangeOutput;
use denokv_proto::datapath::SnapshotRead;
use denokv_proto::datapath::SnapshotReadOutput;
use denokv_proto::datapath::SnapshotReadStatus;
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
use pretty_assertions::assert_eq;
use prost::Message;
use std::collections::HashMap;
use std::convert::Infallible;
use std::env;
use std::io;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::SocketAddrV6;
use std::path::PathBuf;
use std::pin::Pin;
use std::result::Result;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

mod grpc;
mod registry;
mod ws;

use super::https::get_tls_listener_stream;
use super::https::SupportedHttpVersions;
use super::npm::CUSTOM_NPM_PACKAGE_CACHE;
use super::std_path;
use super::testdata_path;

const PORT: u16 = 4545;
const TEST_AUTH_TOKEN: &str = "abcdef123456789";
const TEST_BASIC_AUTH_USERNAME: &str = "testuser123";
const TEST_BASIC_AUTH_PASSWORD: &str = "testpassabc";
const KV_DATABASE_ID: &str = "11111111-1111-1111-1111-111111111111";
const KV_ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const KV_DATABASE_TOKEN: &str = "MOCKMOCKMOCKMOCKMOCKMOCKMOCK";
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
const H1_ONLY_TLS_PORT: u16 = 5546;
const H2_ONLY_TLS_PORT: u16 = 5547;
const H1_ONLY_PORT: u16 = 5548;
const H2_ONLY_PORT: u16 = 5549;
const HTTPS_CLIENT_AUTH_PORT: u16 = 5552;
const WS_PORT: u16 = 4242;
const WSS_PORT: u16 = 4243;
const WSS2_PORT: u16 = 4249;
const WS_CLOSE_PORT: u16 = 4244;
const WS_PING_PORT: u16 = 4245;
const H2_GRPC_PORT: u16 = 4246;
const H2S_GRPC_PORT: u16 = 4247;
const REGISTRY_SERVER_PORT: u16 = 4250;

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

  let ws_server_fut = ws::run_ws_server(WS_PORT);
  let ws_ping_server_fut = ws::run_ws_ping_server(WS_PING_PORT);
  let wss_server_fut = ws::run_wss_server(WSS_PORT);
  let ws_close_server_fut = ws::run_ws_close_server(WS_CLOSE_PORT);
  let wss2_server_fut = ws::run_wss2_server(WSS2_PORT);

  let tls_server_fut = run_tls_server();
  let tls_client_auth_server_fut = run_tls_client_auth_server();
  let client_auth_server_https_fut = wrap_client_auth_https_server();
  let main_server_fut = wrap_main_server();
  let main_server_ipv6_fut = wrap_main_ipv6_server();
  let main_server_https_fut = wrap_main_https_server();
  let h1_only_server_tls_fut = wrap_https_h1_only_tls_server();
  let h2_only_server_tls_fut = wrap_https_h2_only_tls_server();
  let h1_only_server_fut = wrap_http_h1_only_server();
  let h2_only_server_fut = wrap_http_h2_only_server();
  let h2_grpc_server_fut = grpc::h2_grpc_server(H2_GRPC_PORT, H2S_GRPC_PORT);

  let registry_server_fut = registry::registry_server(REGISTRY_SERVER_PORT);

  let server_fut = async {
    futures::join!(
      redirect_server_fut,
      ws_server_fut,
      ws_ping_server_fut,
      wss_server_fut,
      wss2_server_fut,
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
      main_server_ipv6_fut,
      main_server_https_fut,
      client_auth_server_https_fut,
      h1_only_server_tls_fut,
      h2_only_server_tls_fut,
      h1_only_server_fut,
      h2_only_server_fut,
      h2_grpc_server_fut,
      registry_server_fut,
    )
  }
  .boxed_local();

  server_fut.await;
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
    eprintln!("server error: {e}");
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
  let url = format!("http://localhost:{PORT}{p}");

  Ok(redirect_resp(url))
}

async fn double_redirects(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{REDIRECT_PORT}{p}");

  Ok(redirect_resp(url))
}

async fn inf_redirects(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{INF_REDIRECTS_PORT}{p}");

  Ok(redirect_resp(url))
}

async fn another_redirect(req: Request<Body>) -> hyper::Result<Response<Body>> {
  let p = req.uri().path();
  assert_eq!(&p[0..1], "/");
  let url = format!("http://localhost:{PORT}/subdir{p}");

  Ok(redirect_resp(url))
}

async fn auth_redirect(req: Request<Body>) -> hyper::Result<Response<Body>> {
  if let Some(auth) = req
    .headers()
    .get("authorization")
    .map(|v| v.to_str().unwrap())
  {
    if auth.to_lowercase() == format!("bearer {TEST_AUTH_TOKEN}") {
      let p = req.uri().path();
      assert_eq!(&p[0..1], "/");
      let url = format!("http://localhost:{PORT}{p}");
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
      format!("{TEST_BASIC_AUTH_USERNAME}:{TEST_BASIC_AUTH_PASSWORD}");
    if auth == format!("Basic {}", BASE64_STANDARD.encode(credentials)) {
      let p = req.uri().path();
      assert_eq!(&p[0..1], "/");
      let url = format!("http://localhost:{PORT}{p}");
      return Ok(redirect_resp(url));
    }
  }

  let mut resp = Response::new(Body::empty());
  *resp.status_mut() = StatusCode::NOT_FOUND;
  Ok(resp)
}

/// Returns a [`Stream`] of [`TcpStream`]s accepted from the given port.
async fn get_tcp_listener_stream(
  name: &'static str,
  port: u16,
) -> impl Stream<Item = Result<TcpStream, std::io::Error>> + Unpin + Send {
  let host_and_port = &format!("localhost:{port}");

  // Listen on ALL addresses that localhost can resolves to.
  let accept = |listener: tokio::net::TcpListener| {
    async {
      let result = listener.accept().await;
      Some((result.map(|r| r.0), listener))
    }
    .boxed()
  };

  let mut addresses = vec![];
  let listeners = tokio::net::lookup_host(host_and_port)
    .await
    .expect(host_and_port)
    .inspect(|address| addresses.push(*address))
    .map(tokio::net::TcpListener::bind)
    .collect::<futures::stream::FuturesUnordered<_>>()
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .map(|s| s.unwrap())
    .map(|listener| futures::stream::unfold(listener, accept))
    .collect::<Vec<_>>();

  // Eye catcher for HttpServerCount
  println!("ready: {name} on {:?}", addresses);

  futures::stream::select_all(listeners)
}

/// This server responds with 'PASS' if client authentication was successful. Try it by running
/// test_server and
///   curl --key cli/tests/testdata/tls/localhost.key \
///        --cert cli/tests/testsdata/tls/localhost.crt \
///        --cacert cli/tests/testdata/tls/RootCA.crt https://localhost:4552/
async fn run_tls_client_auth_server() {
  let mut tls = get_tls_listener_stream(
    "tls client auth",
    TLS_CLIENT_AUTH_PORT,
    Default::default(),
  )
  .await;
  while let Some(Ok(mut tls_stream)) = tls.next().await {
    tokio::spawn(async move {
      let Ok(handshake) = tls_stream.handshake().await else {
        eprintln!("Failed to handshake");
        return;
      };
      // We only need to check for the presence of client certificates
      // here. Rusttls ensures that they are valid and signed by the CA.
      let response = match handshake.has_peer_certificates {
        true => b"PASS",
        false => b"FAIL",
      };
      tls_stream.write_all(response).await.unwrap();
    });
  }
}

/// This server responds with 'PASS' if client authentication was successful. Try it by running
/// test_server and
///   curl --cacert cli/tests/testdata/tls/RootCA.crt https://localhost:4553/
async fn run_tls_server() {
  let mut tls =
    get_tls_listener_stream("tls", TLS_PORT, Default::default()).await;
  while let Some(Ok(mut tls_stream)) = tls.next().await {
    tokio::spawn(async move {
      tls_stream.write_all(b"PASS").await.unwrap();
    });
  }
}

async fn absolute_redirect(
  req: Request<Body>,
) -> hyper::Result<Response<Body>> {
  let path = req.uri().path();

  if path == "/" {
    // We have to manually extract query params here,
    // as `req.uri()` returns `PathAndQuery` only,
    // and we cannot use `Url::parse(req.uri()).query_pairs()`,
    // as it requires url to have a proper base.
    let query_params: HashMap<_, _> = req
      .uri()
      .query()
      .unwrap_or_default()
      .split('&')
      .filter_map(|s| {
        s.split_once('=').map(|t| (t.0.to_owned(), t.1.to_owned()))
      })
      .collect();

    if let Some(url) = query_params.get("redirect_to") {
      println!("URL: {url:?}");
      let redirect = redirect_resp(url.to_owned());
      return Ok(redirect);
    }
  }

  if path.starts_with("/REDIRECT") {
    let url = &req.uri().path()[9..];
    println!("URL: {url:?}");
    let redirect = redirect_resp(url.to_string());
    return Ok(redirect);
  }

  if path.starts_with("/a/b/c") {
    if let Some(x_loc) = req.headers().get("x-location") {
      let loc = x_loc.to_str().unwrap();
      return Ok(redirect_resp(loc.to_string()));
    }
  }

  let file_path = testdata_path().join(&req.uri().path()[1..]);
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
    (_, "/echo_server") => {
      let (parts, body) = req.into_parts();
      let mut response = Response::new(body);

      if let Some(status) = parts.headers.get("x-status") {
        *response.status_mut() =
          StatusCode::from_bytes(status.as_bytes()).unwrap();
      }
      response.headers_mut().extend(parts.headers);
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
    (_, "/server_error") => {
      let mut res = Response::new(Body::empty());
      *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
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
    (_, "/dynamic_module.ts") => {
      let mut res = Response::new(Body::from(format!(
        r#"export const time = {};"#,
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
      )));
      res.headers_mut().insert(
        "Content-type",
        HeaderValue::from_static("application/typescript"),
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
    (_, "/search_params") => {
      let query = req.uri().query().map(|s| s.to_string());
      let res = Response::new(Body::from(query.unwrap_or_default()));
      Ok(res)
    }
    (&hyper::Method::POST, "/kv_remote_authorize") => {
      if req
        .headers()
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default()
        != format!("Bearer {}", KV_ACCESS_TOKEN)
      {
        return Ok(
          Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap(),
        );
      }

      Ok(
        Response::builder()
          .header("content-type", "application/json")
          .body(Body::from(
            serde_json::json!({
              "version": 1,
              "databaseId": KV_DATABASE_ID,
              "endpoints": [
                {
                  "url": format!("http://localhost:{}/kv_blackhole", PORT),
                  "consistency": "strong",
                }
              ],
              "token": KV_DATABASE_TOKEN,
              "expiresAt": "2099-01-01T00:00:00Z",
            })
            .to_string(),
          ))
          .unwrap(),
      )
    }
    (&hyper::Method::POST, "/kv_remote_authorize_invalid_format") => {
      if req
        .headers()
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default()
        != format!("Bearer {}", KV_ACCESS_TOKEN)
      {
        return Ok(
          Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap(),
        );
      }

      Ok(
        Response::builder()
          .header("content-type", "application/json")
          .body(Body::from(
            serde_json::json!({
              "version": 1,
              "databaseId": KV_DATABASE_ID,
            })
            .to_string(),
          ))
          .unwrap(),
      )
    }
    (&hyper::Method::POST, "/kv_remote_authorize_invalid_version") => {
      if req
        .headers()
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default()
        != format!("Bearer {}", KV_ACCESS_TOKEN)
      {
        return Ok(
          Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap(),
        );
      }

      Ok(
        Response::builder()
          .header("content-type", "application/json")
          .body(Body::from(
            serde_json::json!({
              "version": 1000,
              "databaseId": KV_DATABASE_ID,
              "endpoints": [
                {
                  "url": format!("http://localhost:{}/kv_blackhole", PORT),
                  "consistency": "strong",
                }
              ],
              "token": KV_DATABASE_TOKEN,
              "expiresAt": "2099-01-01T00:00:00Z",
            })
            .to_string(),
          ))
          .unwrap(),
      )
    }
    (&hyper::Method::POST, "/kv_blackhole/snapshot_read") => {
      if req
        .headers()
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default()
        != format!("Bearer {}", KV_DATABASE_TOKEN)
      {
        return Ok(
          Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap(),
        );
      }

      let body = hyper::body::to_bytes(req.into_body())
        .await
        .unwrap_or_default();
      let Ok(body): Result<SnapshotRead, _> = prost::Message::decode(&body[..])
      else {
        return Ok(
          Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap(),
        );
      };
      if body.ranges.is_empty() {
        return Ok(
          Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap(),
        );
      }
      Ok(
        Response::builder()
          .body(Body::from(
            SnapshotReadOutput {
              ranges: body
                .ranges
                .iter()
                .map(|_| ReadRangeOutput { values: vec![] })
                .collect(),
              read_disabled: false,
              read_is_strongly_consistent: true,
              status: SnapshotReadStatus::SrSuccess.into(),
            }
            .encode_to_vec(),
          ))
          .unwrap(),
      )
    }
    (&hyper::Method::POST, "/kv_blackhole/atomic_write") => {
      if req
        .headers()
        .get("authorization")
        .and_then(|x| x.to_str().ok())
        .unwrap_or_default()
        != format!("Bearer {}", KV_DATABASE_TOKEN)
      {
        return Ok(
          Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap(),
        );
      }

      let body = hyper::body::to_bytes(req.into_body())
        .await
        .unwrap_or_default();
      let Ok(_body): Result<AtomicWrite, _> = prost::Message::decode(&body[..])
      else {
        return Ok(
          Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap(),
        );
      };
      Ok(
        Response::builder()
          .body(Body::from(
            AtomicWriteOutput {
              status: AtomicWriteStatus::AwSuccess.into(),
              versionstamp: vec![0u8; 10],
              failed_checks: vec![],
            }
            .encode_to_vec(),
          ))
          .unwrap(),
      )
    }
    (&hyper::Method::GET, "/upgrade/sleep/release-latest.txt") => {
      tokio::time::sleep(Duration::from_secs(95)).await;
      return Ok(
        Response::builder()
          .status(StatusCode::OK)
          .body(Body::from("99999.99.99"))
          .unwrap(),
      );
    }
    (&hyper::Method::GET, "/upgrade/sleep/canary-latest.txt") => {
      tokio::time::sleep(Duration::from_secs(95)).await;
      return Ok(
        Response::builder()
          .status(StatusCode::OK)
          .body(Body::from("bda3850f84f24b71e02512c1ba2d6bf2e3daa2fd"))
          .unwrap(),
      );
    }
    (&hyper::Method::GET, "/release-latest.txt") => {
      return Ok(
        Response::builder()
          .status(StatusCode::OK)
          // use a deno version that will never happen
          .body(Body::from("99999.99.99"))
          .unwrap(),
      );
    }
    (&hyper::Method::GET, "/canary-latest.txt") => {
      return Ok(
        Response::builder()
          .status(StatusCode::OK)
          .body(Body::from("bda3850f84f24b71e02512c1ba2d6bf2e3daa2fd"))
          .unwrap(),
      );
    }
    _ => {
      let mut file_path = testdata_path().to_path_buf();
      file_path.push(&req.uri().path()[1..].replace("%2f", "/"));
      if let Ok(file) = tokio::fs::read(&file_path).await {
        let file_resp = custom_headers(req.uri().path(), file);
        return Ok(file_resp);
      }

      // serve npm registry files
      if let Some(suffix) = req
        .uri()
        .path()
        .strip_prefix("/npm/registry/@denotest/")
        .or_else(|| req.uri().path().strip_prefix("/npm/registry/@denotest%2f"))
      {
        // serve all requests to /npm/registry/@deno using the file system
        // at that path
        match handle_custom_npm_registry_path(suffix) {
          Ok(Some(response)) => return Ok(response),
          Ok(None) => {} // ignore, not found
          Err(err) => {
            return Response::builder()
              .status(StatusCode::INTERNAL_SERVER_ERROR)
              .body(format!("{err:#}").into());
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
              .body(format!("{err:#}").into());
          };

          // serve the file
          if let Ok(file) = tokio::fs::read(&file_path).await {
            let file_resp = custom_headers(req.uri().path(), file);
            return Ok(file_resp);
          }
        }
      } else if let Some(suffix) = req.uri().path().strip_prefix("/deno_std/") {
        let file_path = std_path().join(suffix);
        if let Ok(file) = tokio::fs::read(&file_path).await {
          let file_resp = custom_headers(req.uri().path(), file);
          return Ok(file_resp);
        }
      } else if let Some(suffix) = req.uri().path().strip_prefix("/sleep/") {
        let duration = suffix.parse::<u64>().unwrap();
        tokio::time::sleep(Duration::from_millis(duration)).await;
        return Response::builder()
          .status(StatusCode::OK)
          .header("content-type", "application/typescript")
          .body(Body::empty());
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
    format!("https://registry.npmjs.org/{package_name}/-/{file_name}")
  } else {
    format!("https://registry.npmjs.org/{package_name}")
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
        &format!("https://registry.npmjs.org/{package_name}/-/"),
        &format!("http://localhost:4545/npm/registry/{package_name}/"),
      )
      .into_bytes()
  };
  std::fs::create_dir_all(file_path.parent().unwrap())?;
  std::fs::write(file_path, bytes)?;
  Ok(())
}

/// Taken from example in https://github.com/ctz/hyper-rustls/blob/a02ef72a227dcdf102f86e905baa7415c992e8b3/examples/server.rs
struct HyperAcceptor<'a> {
  acceptor: Pin<
    Box<dyn Stream<Item = io::Result<rustls_tokio_stream::TlsStream>> + 'a>,
  >,
}

impl hyper::server::accept::Accept for HyperAcceptor<'_> {
  type Conn = rustls_tokio_stream::TlsStream;
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
    eprintln!("Redirect error: {e:?}");
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
    eprintln!("Double redirect error: {e:?}");
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
    eprintln!("Inf redirect error: {e:?}");
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
    eprintln!("Another redirect error: {e:?}");
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
    eprintln!("Auth redirect error: {e:?}");
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
    eprintln!("Basic auth redirect error: {e:?}");
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
    eprintln!("Absolute redirect error: {e:?}");
  }
}

async fn wrap_main_server() {
  let main_server_addr = SocketAddr::from(([127, 0, 0, 1], PORT));
  wrap_main_server_for_addr(&main_server_addr).await
}

// necessary because on Windows the npm binary will resolve localhost to ::1
async fn wrap_main_ipv6_server() {
  let ipv6_loopback = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
  let main_server_addr =
    SocketAddr::V6(SocketAddrV6::new(ipv6_loopback, PORT, 0, 0));
  wrap_main_server_for_addr(&main_server_addr).await
}

async fn wrap_main_server_for_addr(main_server_addr: &SocketAddr) {
  let main_server_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server = Server::bind(main_server_addr).serve(main_server_svc);
  if let Err(e) = main_server.await {
    eprintln!("HTTP server error: {e:?}");
  }
}

async fn wrap_main_https_server() {
  let tls =
    get_tls_listener_stream("https", HTTPS_PORT, Default::default()).await;
  let main_server_https_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_https = Server::builder(HyperAcceptor {
    acceptor: tls.boxed_local(),
  })
  .serve(main_server_https_svc);
  let _ = main_server_https.await;
}

async fn wrap_https_h1_only_tls_server() {
  let tls = get_tls_listener_stream(
    "https (h1 only)",
    H1_ONLY_TLS_PORT,
    SupportedHttpVersions::Http1Only,
  )
  .await;

  let main_server_https_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_https = Server::builder(HyperAcceptor {
    acceptor: tls.boxed_local(),
  })
  .http1_only(true)
  .serve(main_server_https_svc);

  let _ = main_server_https.await;
}

async fn wrap_https_h2_only_tls_server() {
  let tls = get_tls_listener_stream(
    "https (h2 only)",
    H2_ONLY_TLS_PORT,
    SupportedHttpVersions::Http2Only,
  )
  .await;

  let main_server_https_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_https = Server::builder(HyperAcceptor {
    acceptor: tls.boxed_local(),
  })
  .http2_only(true)
  .serve(main_server_https_svc);

  let _ = main_server_https.await;
}

async fn wrap_http_h1_only_server() {
  let main_server_http_addr = SocketAddr::from(([127, 0, 0, 1], H1_ONLY_PORT));

  let main_server_http_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_http = Server::bind(&main_server_http_addr)
    .http1_only(true)
    .serve(main_server_http_svc);
  let _ = main_server_http.await;
}

async fn wrap_http_h2_only_server() {
  let main_server_http_addr = SocketAddr::from(([127, 0, 0, 1], H2_ONLY_PORT));

  let main_server_http_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_http = Server::bind(&main_server_http_addr)
    .http2_only(true)
    .serve(main_server_http_svc);
  let _ = main_server_http.await;
}

async fn wrap_client_auth_https_server() {
  let mut tls = get_tls_listener_stream(
    "https_client_auth",
    HTTPS_CLIENT_AUTH_PORT,
    Default::default(),
  )
  .await;

  let tls = async_stream::stream! {
    while let Some(Ok(mut tls)) = tls.next().await {
      let handshake = tls.handshake().await?;
      // We only need to check for the presence of client certificates
      // here. Rusttls ensures that they are valid and signed by the CA.
      match handshake.has_peer_certificates {
        true => { yield Ok(tls); },
        false => { eprintln!("https_client_auth: no valid client certificate"); },
      };
    }
  };

  let main_server_https_svc =
    make_service_fn(|_| async { Ok::<_, Infallible>(service_fn(main_server)) });
  let main_server_https = Server::builder(HyperAcceptor {
    acceptor: tls.boxed_local(),
  })
  .serve(main_server_https_svc);

  let _ = main_server_https.await;
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
        &format!("application/typescript;charset={charset}")[..],
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
