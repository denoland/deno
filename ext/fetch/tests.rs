// Copyright 2018-2026 the Deno authors. MIT license.

use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use bytes::Bytes;
use deno_tls::rustls::pki_types::PrivateKeyDer;
use fast_socks5::server::Config as Socks5Config;
use fast_socks5::server::Socks5Socket;
use http::header::ACCEPT_ENCODING;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::HeaderValue;
use http::header::RANGE;
use http::header::TRANSFER_ENCODING;
use http_body_util::BodyExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::CreateHttpClientOptions;
use super::create_http_client;
use crate::dns;

static GZIP_HELLO_FROM_SERVER: &[u8] = &[
  0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xcb, 0x48, 0xcd,
  0xc9, 0xc9, 0x57, 0x48, 0x2b, 0xca, 0xcf, 0x55, 0x28, 0x4e, 0x2d, 0x2a, 0x4b,
  0x2d, 0x02, 0x00, 0x24, 0x20, 0xa8, 0x29, 0x11, 0x00, 0x00, 0x00,
];
static BR_HELLO_FROM_SERVER: &[u8] = &[
  0x1b, 0x10, 0x00, 0xf8, 0x25, 0x00, 0x6a, 0x10, 0x42, 0x8a, 0x89, 0x97, 0x74,
  0x56,
];

static EXAMPLE_CRT: &[u8] = include_bytes!("../tls/testdata/example1_cert.der");
static EXAMPLE_KEY: &[u8] =
  include_bytes!("../tls/testdata/example1_prikey.der");

#[test]
fn test_userspace_resolver() {
  let thread_counter = Arc::new(AtomicUsize::new(0));

  let thread_counter_ref = thread_counter.clone();
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .on_thread_start(move || {
      thread_counter_ref.fetch_add(1, SeqCst);
    })
    .build()
    .unwrap();

  rt.block_on(async move {
    assert_eq!(thread_counter.load(SeqCst), 0);
    let src_addr = create_https_server(true, Ipv4Addr::LOCALHOST.into()).await;
    assert_eq!(src_addr.ip().to_string(), "127.0.0.1");
    // use `localhost` to ensure dns step happens.
    let addr = format!("localhost:{}", src_addr.port());

    let hickory = hickory_resolver::Resolver::builder_tokio().unwrap().build();

    assert_eq!(thread_counter.load(SeqCst), 0);
    run_test_client_with_resolver(
      None,
      addr.clone(),
      "https",
      http::Version::HTTP_2,
      dns::Resolver::hickory_from_resolver(hickory),
    )
    .await;
    assert_eq!(thread_counter.load(SeqCst), 0, "userspace resolver shouldn't spawn new threads.");
    run_test_client_with_resolver(
      None,
      addr.clone(),
      "https",
      http::Version::HTTP_2,
      dns::Resolver::gai(),
    )
    .await;
    assert_eq!(thread_counter.load(SeqCst), 1, "getaddrinfo is called inside spawn_blocking, so tokio spawn a new worker thread for it.");
  });
}

#[tokio::test]
async fn test_http_proxy_http11_ipv4() {
  let src_addr = create_https_server(false, Ipv4Addr::LOCALHOST.into()).await;
  let prx_addr = create_http_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "http", http::Version::HTTP_11).await;
}

#[tokio::test]
async fn test_http_proxy_h2_ipv4() {
  let src_addr = create_https_server(true, Ipv4Addr::LOCALHOST.into()).await;
  let prx_addr = create_http_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "http", http::Version::HTTP_2).await;
}

#[tokio::test]
async fn test_http_proxy_h2_ipv6() {
  let src_addr = create_https_server(true, Ipv6Addr::LOCALHOST.into()).await;
  let prx_addr = create_http_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "http", http::Version::HTTP_2).await;
}

#[tokio::test]
async fn test_https_proxy_h2_ipv4() {
  let src_addr = create_https_server(true, Ipv4Addr::LOCALHOST.into()).await;
  let prx_addr = create_https_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "https", http::Version::HTTP_2).await;
}

#[tokio::test]
async fn test_https_proxy_h2_ipv6() {
  let src_addr = create_https_server(true, Ipv6Addr::LOCALHOST.into()).await;
  let prx_addr = create_https_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "https", http::Version::HTTP_2).await;
}

#[tokio::test]
async fn test_socks_proxy_http11_ipv4() {
  let src_addr = create_https_server(false, Ipv4Addr::LOCALHOST.into()).await;
  let prx_addr = create_socks_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "socks5", http::Version::HTTP_11).await;
}

#[tokio::test]
async fn test_socks_proxy_h2_ipv4() {
  let src_addr = create_https_server(true, Ipv4Addr::LOCALHOST.into()).await;
  let prx_addr = create_socks_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "socks5", http::Version::HTTP_2).await;
}

#[tokio::test]
async fn test_socks_proxy_h2_ipv6() {
  let src_addr = create_https_server(true, Ipv6Addr::LOCALHOST.into()).await;
  let prx_addr = create_socks_proxy(src_addr).await;
  run_test_client(prx_addr, src_addr, "socks5", http::Version::HTTP_2).await;
}

async fn run_test_client_with_resolver(
  prx_addr: Option<SocketAddr>,
  src_addr: String,
  proto: &str,
  ver: http::Version,
  resolver: dns::Resolver,
) {
  let client = create_http_client(
    "fetch/test",
    CreateHttpClientOptions {
      root_cert_store: None,
      ca_certs: vec![],
      proxy: prx_addr.map(|p| deno_tls::Proxy::Http {
        url: format!("{}://{}", proto, p),
        basic_auth: None,
      }),
      unsafely_ignore_certificate_errors: Some(vec![]),
      client_cert_chain_and_key: None,
      pool_max_idle_per_host: None,
      pool_idle_timeout: None,
      dns_resolver: resolver,
      http1: true,
      http2: true,
      local_address: None,
      client_builder_hook: None,
      http2_max_header_list_size: None,
    },
  )
  .unwrap();

  let req = http::Request::builder()
    .uri(format!("https://{}/foo", src_addr))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();
  assert_eq!(resp.status(), http::StatusCode::OK);
  assert_eq!(resp.version(), ver);
  let hello = resp.collect().await.unwrap().to_bytes();
  assert_eq!(hello, "hello from server");
}

async fn run_test_client(
  prx_addr: SocketAddr,
  src_addr: SocketAddr,
  proto: &str,
  ver: http::Version,
) {
  run_test_client_with_resolver(
    Some(prx_addr),
    src_addr.to_string(),
    proto,
    ver,
    Default::default(),
  )
  .await
}

#[tokio::test]
async fn test_fetch_decompresses_gzip_response_and_sets_accept_encoding() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static(" GZip "),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("gzip,br")
  );
  assert_eq!(resp.headers().get(CONTENT_ENCODING), None);
  assert_eq!(resp.headers().get(CONTENT_LENGTH), None);
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, "hello from server");
}

#[tokio::test]
async fn test_fetch_decompresses_br_response_and_preserves_accept_encoding() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("Br"),
    BR_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(resp.headers().get(CONTENT_ENCODING), None);
  assert_eq!(resp.headers().get(CONTENT_LENGTH), None);
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, "hello from server");
}

#[tokio::test]
async fn test_fetch_empty_body_with_content_encoding_skips_decompression() {
  for encoding in ["gzip", "br"] {
    let captured_accept_encoding = Arc::new(Mutex::new(None));
    let src_addr = create_encoded_http_server(
      captured_accept_encoding.clone(),
      HeaderValue::from_static(encoding),
      b"",
    )
    .await;
    let client = create_http_test_client();

    let req = http::Request::builder()
      .uri(format!("http://{}/foo", src_addr))
      .body(crate::ReqBody::empty())
      .unwrap();
    let resp = client.send(req).await.unwrap();

    assert_eq!(
      captured_accept_encoding.lock().await.as_ref().unwrap(),
      HeaderValue::from_static("gzip,br")
    );
    assert_eq!(resp.headers().get(CONTENT_ENCODING), None);
    assert_eq!(
      resp.headers().get(CONTENT_LENGTH).unwrap(),
      HeaderValue::from_static("0")
    );
    let body = resp.collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
  }
}

#[tokio::test]
async fn test_fetch_strips_transfer_encoding_after_decompression() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr =
    create_chunked_gzip_http_server(captured_accept_encoding.clone()).await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_deref().unwrap(),
    "gzip,br"
  );
  assert_eq!(resp.headers().get(CONTENT_ENCODING), None);
  assert_eq!(resp.headers().get(CONTENT_LENGTH), None);
  assert_eq!(resp.headers().get(TRANSFER_ENCODING), None);
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, "hello from server");
}

#[tokio::test]
async fn test_fetch_accept_encoding_identity_skips_decompression() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("gzip"),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .header(ACCEPT_ENCODING, HeaderValue::from_static("IdEnTiTy"))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("IdEnTiTy")
  );
  assert_eq!(
    resp.headers().get(CONTENT_ENCODING).unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(
    resp.headers().get(CONTENT_LENGTH).unwrap(),
    HeaderValue::from_static("37")
  );
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, GZIP_HELLO_FROM_SERVER);
}

#[tokio::test]
async fn test_fetch_multi_content_encoding_preserves_raw_response() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("gzip, br"),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("gzip,br")
  );
  assert_eq!(
    resp.headers().get(CONTENT_ENCODING).unwrap(),
    HeaderValue::from_static("gzip, br")
  );
  assert_eq!(
    resp.headers().get(CONTENT_LENGTH).unwrap(),
    HeaderValue::from_static("37")
  );
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, GZIP_HELLO_FROM_SERVER);
}

#[tokio::test]
async fn test_fetch_range_request_skips_decompression() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("gzip"),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .header(RANGE, HeaderValue::from_static("bytes=0-3"))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("identity")
  );
  assert_eq!(
    resp.headers().get(CONTENT_ENCODING).unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(
    resp.headers().get(CONTENT_LENGTH).unwrap(),
    HeaderValue::from_static("37")
  );
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, GZIP_HELLO_FROM_SERVER);
}

#[tokio::test]
async fn test_fetch_range_request_with_accept_encoding_preserves_raw_response()
{
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("gzip"),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .header(RANGE, HeaderValue::from_static("bytes=0-3"))
    .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send(req).await.unwrap();

  assert_eq!(
    captured_accept_encoding.lock().await.as_ref().unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(
    resp.headers().get(CONTENT_ENCODING).unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(
    resp.headers().get(CONTENT_LENGTH).unwrap(),
    HeaderValue::from_static("37")
  );
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, GZIP_HELLO_FROM_SERVER);
}

#[tokio::test]
async fn test_fetch_send_no_decompress_preserves_raw_response() {
  let captured_accept_encoding = Arc::new(Mutex::new(None));
  let src_addr = create_encoded_http_server(
    captured_accept_encoding.clone(),
    HeaderValue::from_static("gzip"),
    GZIP_HELLO_FROM_SERVER,
  )
  .await;
  let client = create_http_test_client();

  let req = http::Request::builder()
    .uri(format!("http://{}/foo", src_addr))
    .body(crate::ReqBody::empty())
    .unwrap();
  let resp = client.send_no_decompress(req).await.unwrap();

  assert_eq!(captured_accept_encoding.lock().await.as_ref(), None);
  assert_eq!(
    resp.headers().get(CONTENT_ENCODING).unwrap(),
    HeaderValue::from_static("gzip")
  );
  assert_eq!(
    resp.headers().get(CONTENT_LENGTH).unwrap(),
    HeaderValue::from_static("37")
  );
  let body = resp.collect().await.unwrap().to_bytes();
  assert_eq!(body, GZIP_HELLO_FROM_SERVER);
}

fn create_http_test_client() -> crate::Client {
  install_default_crypto_provider();

  create_http_client(
    "fetch/test",
    CreateHttpClientOptions {
      root_cert_store: None,
      ca_certs: vec![],
      proxy: None,
      unsafely_ignore_certificate_errors: Some(vec![]),
      client_cert_chain_and_key: None,
      pool_max_idle_per_host: None,
      pool_idle_timeout: None,
      dns_resolver: Default::default(),
      http1: true,
      http2: true,
      local_address: None,
      client_builder_hook: None,
    },
  )
  .unwrap()
}

fn install_default_crypto_provider() {
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

async fn create_encoded_http_server(
  captured_accept_encoding: Arc<Mutex<Option<HeaderValue>>>,
  content_encoding: HeaderValue,
  body: &'static [u8],
) -> SocketAddr {
  let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((sock, _)) = tcp.accept().await {
      let captured_accept_encoding = captured_accept_encoding.clone();
      let content_encoding = content_encoding.clone();
      let fut = hyper::server::conn::http1::Builder::new().serve_connection(
        hyper_util::rt::TokioIo::new(sock),
        hyper::service::service_fn(move |req: http::Request<_>| {
          let captured_accept_encoding = captured_accept_encoding.clone();
          let content_encoding = content_encoding.clone();
          async move {
            *captured_accept_encoding.lock().await =
              req.headers().get(ACCEPT_ENCODING).cloned();
            Ok::<_, std::convert::Infallible>(
              http::Response::builder()
                .header(CONTENT_ENCODING, content_encoding)
                .header(CONTENT_LENGTH, body.len())
                .body(http_body_util::Full::<Bytes>::new(Bytes::from_static(
                  body,
                )))
                .unwrap(),
            )
          }
        }),
      );
      tokio::spawn(fut);
    }
  });

  addr
}

async fn create_chunked_gzip_http_server(
  captured_accept_encoding: Arc<Mutex<Option<String>>>,
) -> SocketAddr {
  let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((mut sock, _)) = tcp.accept().await {
      let captured_accept_encoding = captured_accept_encoding.clone();
      tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        let n = sock.read(&mut buf).await.unwrap();
        let req = String::from_utf8_lossy(&buf[..n]);
        let accept_encoding = req.lines().find_map(|line| {
          let (name, value) = line.split_once(':')?;
          name
            .eq_ignore_ascii_case("accept-encoding")
            .then(|| value.trim().to_string())
        });
        *captured_accept_encoding.lock().await = accept_encoding;

        sock
          .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n",
          )
          .await
          .unwrap();
        sock
          .write_all(
            format!("{:x}\r\n", GZIP_HELLO_FROM_SERVER.len()).as_bytes(),
          )
          .await
          .unwrap();
        sock.write_all(GZIP_HELLO_FROM_SERVER).await.unwrap();
        sock.write_all(b"\r\n0\r\n\r\n").await.unwrap();
      });
    }
  });

  addr
}

async fn create_https_server(allow_h2: bool, bind_addr: IpAddr) -> SocketAddr {
  install_default_crypto_provider();

  let mut tls_config = deno_tls::rustls::server::ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(
      vec![EXAMPLE_CRT.into()],
      PrivateKeyDer::try_from(EXAMPLE_KEY).unwrap(),
    )
    .unwrap();
  if allow_h2 {
    tls_config.alpn_protocols.push("h2".into());
  }
  tls_config.alpn_protocols.push("http/1.1".into());
  let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::from(tls_config));
  let src_tcp = tokio::net::TcpListener::bind((bind_addr, 0)).await.unwrap();
  let src_addr = src_tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((sock, _)) = src_tcp.accept().await {
      let conn = tls_acceptor.accept(sock).await.unwrap();
      if conn.get_ref().1.alpn_protocol() == Some(b"h2") {
        let fut = hyper::server::conn::http2::Builder::new(
          hyper_util::rt::TokioExecutor::new(),
        )
        .serve_connection(
          hyper_util::rt::TokioIo::new(conn),
          hyper::service::service_fn(|_req| async {
            Ok::<_, std::convert::Infallible>(http::Response::new(
              http_body_util::Full::<Bytes>::new("hello from server".into()),
            ))
          }),
        );
        tokio::spawn(fut);
      } else {
        let fut = hyper::server::conn::http1::Builder::new().serve_connection(
          hyper_util::rt::TokioIo::new(conn),
          hyper::service::service_fn(|_req| async {
            Ok::<_, std::convert::Infallible>(http::Response::new(
              http_body_util::Full::<Bytes>::new("hello from server".into()),
            ))
          }),
        );
        tokio::spawn(fut);
      }
    }
  });

  src_addr
}

async fn create_http_proxy(src_addr: SocketAddr) -> SocketAddr {
  let prx_tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let prx_addr = prx_tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((mut sock, _)) = prx_tcp.accept().await {
      let fut = async move {
        let mut buf = [0u8; 4096];
        let _n = sock.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..7], b"CONNECT");
        let mut dst_tcp =
          tokio::net::TcpStream::connect(src_addr).await.unwrap();
        sock.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        tokio::io::copy_bidirectional(&mut sock, &mut dst_tcp)
          .await
          .unwrap();
      };
      tokio::spawn(fut);
    }
  });

  prx_addr
}

async fn create_https_proxy(src_addr: SocketAddr) -> SocketAddr {
  let mut tls_config = deno_tls::rustls::server::ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(
      vec![EXAMPLE_CRT.into()],
      PrivateKeyDer::try_from(EXAMPLE_KEY).unwrap(),
    )
    .unwrap();
  // Set ALPN, to check our proxy connector. But we shouldn't receive anything.
  tls_config.alpn_protocols.push("h2".into());
  tls_config.alpn_protocols.push("http/1.1".into());
  let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::from(tls_config));
  let prx_tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let prx_addr = prx_tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((sock, _)) = prx_tcp.accept().await {
      let mut sock = tls_acceptor.accept(sock).await.unwrap();
      assert_eq!(sock.get_ref().1.alpn_protocol(), None);

      let fut = async move {
        let mut buf = [0u8; 4096];
        let _n = sock.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..7], b"CONNECT");
        let mut dst_tcp =
          tokio::net::TcpStream::connect(src_addr).await.unwrap();
        sock.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        tokio::io::copy_bidirectional(&mut sock, &mut dst_tcp)
          .await
          .unwrap();
      };
      tokio::spawn(fut);
    }
  });

  prx_addr
}

async fn create_socks_proxy(src_addr: SocketAddr) -> SocketAddr {
  let prx_tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let prx_addr = prx_tcp.local_addr().unwrap();

  tokio::spawn(async move {
    while let Ok((sock, _)) = prx_tcp.accept().await {
      let cfg: Socks5Config = Default::default();
      let mut socks_conn = Socks5Socket::new(sock, cfg.into())
        .upgrade_to_socks5()
        .await
        .unwrap();

      let fut = async move {
        let mut dst_tcp =
          tokio::net::TcpStream::connect(src_addr).await.unwrap();
        tokio::io::copy_bidirectional(&mut socks_conn, &mut dst_tcp)
          .await
          .unwrap();
      };
      tokio::spawn(fut);
    }
  });

  prx_addr
}
