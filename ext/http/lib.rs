// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bytes::Bytes;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_net::raw::NetworkStream;
use deno_telemetry::Histogram;
use deno_telemetry::MeterProvider;
use deno_telemetry::UpDownCounter;
use hyper::server::conn::http1;
use hyper::server::conn::http2;
use hyper_util::rt::TokioIo;
use once_cell::sync::OnceCell;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::sync::Notify;

use crate::network_buffered_stream::NetworkBufferedStream;

pub mod compressible;
mod fly_accept_encoding;
mod http_next;
mod network_buffered_stream;
mod request_body;
mod request_properties;
mod response_body;
mod service;

pub use http_next::HttpNextError;
pub use request_properties::DefaultHttpPropertyExtractor;
pub use request_properties::HttpConnectionProperties;
pub use request_properties::HttpListenProperties;
pub use request_properties::HttpPropertyExtractor;
pub use request_properties::HttpRequestProperties;
pub use service::UpgradeUnavailableError;

struct OtelCollectors {
  duration: Histogram<f64>,
  active_requests: UpDownCounter<i64>,
  request_size: Histogram<u64>,
  response_size: Histogram<u64>,
}

static OTEL_COLLECTORS: OnceCell<OtelCollectors> = OnceCell::new();

#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
  /// By passing a hook function, the caller can customize various configuration
  /// options for the HTTP/2 server.
  /// See [`http2::Builder`] for what parameters can be customized.
  ///
  /// If `None`, the default configuration provided by hyper will be used. Note
  /// that the default configuration is subject to change in future versions.
  pub http2_builder_hook:
    Option<fn(http2::Builder<LocalExecutor>) -> http2::Builder<LocalExecutor>>,
  /// By passing a hook function, the caller can customize various configuration
  /// options for the HTTP/1 server.
  /// See [`http1::Builder`] for what parameters can be customized.
  ///
  /// If `None`, the default configuration provided by hyper will be used. Note
  /// that the default configuration is subject to change in future versions.
  pub http1_builder_hook: Option<fn(http1::Builder) -> http1::Builder>,

  /// If `false`, the server will abort the request when the response is dropped.
  pub no_legacy_abort: bool,
}

#[cfg(not(feature = "default_property_extractor"))]
deno_core::extension!(
  deno_http,
  deps = [deno_web, deno_net, deno_fetch, deno_websocket],
  parameters = [ HTTP: HttpPropertyExtractor ],
  ops = [
    op_http_serve_address_override,
    op_http_websocket_accept_header,
    op_http_notify_serving,
    http_next::op_http_close_after_finish,
    http_next::op_http_get_request_header,
    http_next::op_http_get_request_headers,
    http_next::op_http_request_on_cancel,
    http_next::op_http_get_request_method_and_url<HTTP>,
    http_next::op_http_get_request_cancelled,
    http_next::op_http_read_request_body,
    http_next::op_http_serve_on<HTTP>,
    http_next::op_http_serve<HTTP>,
    http_next::op_http_set_promise_complete,
    http_next::op_http_set_response_body_bytes,
    http_next::op_http_set_response_body_resource,
    http_next::op_http_set_response_body_text,
    http_next::op_http_set_response_body_legacy,
    http_next::op_http_set_response_header,
    http_next::op_http_set_response_headers,
    http_next::op_http_set_response_trailers,
    http_next::op_http_upgrade_websocket_next,
    http_next::op_http_upgrade_raw,
    http_next::op_raw_write_vectored,
    http_next::op_can_write_vectored,
    http_next::op_http_try_wait,
    http_next::op_http_wait,
    http_next::op_http_close,
    http_next::op_http_cancel,
    http_next::op_http_metric_handle_otel_error,
  ],
  esm = ["00_serve.ts", "01_http.js", "02_websocket.ts"],
  options = {
    options: Options,
  },
  state = |state, options| {
    state.put::<Options>(options.options);
  }
);

#[cfg(feature = "default_property_extractor")]
deno_core::extension!(
  deno_http,
  deps = [deno_web, deno_net, deno_fetch, deno_websocket],
  ops = [
    op_http_serve_address_override,
    op_http_websocket_accept_header,
    op_http_notify_serving,
    http_next::op_http_close_after_finish,
    http_next::op_http_get_request_header,
    http_next::op_http_get_request_headers,
    http_next::op_http_request_on_cancel,
    http_next::op_http_get_request_method_and_url<DefaultHttpPropertyExtractor>,
    http_next::op_http_get_request_cancelled,
    http_next::op_http_read_request_body,
    http_next::op_http_serve_on<DefaultHttpPropertyExtractor>,
    http_next::op_http_serve<DefaultHttpPropertyExtractor>,
    http_next::op_http_set_promise_complete,
    http_next::op_http_set_response_body_bytes,
    http_next::op_http_set_response_body_resource,
    http_next::op_http_set_response_body_text,
    http_next::op_http_set_response_body_legacy,
    http_next::op_http_set_response_header,
    http_next::op_http_set_response_headers,
    http_next::op_http_set_response_trailers,
    http_next::op_http_upgrade_websocket_next,
    http_next::op_http_upgrade_raw,
    http_next::op_raw_write_vectored,
    http_next::op_can_write_vectored,
    http_next::op_http_try_wait,
    http_next::op_http_wait,
    http_next::op_http_close,
    http_next::op_http_cancel,
    http_next::op_http_metric_handle_otel_error,
  ],
  esm = ["00_serve.ts", "01_http.js", "02_websocket.ts"],
  options = {
    options: Options,
  },
  state = |state, options| {
    state.put::<Options>(options.options);
  }
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Canceled(#[from] deno_core::Canceled),
  #[class("Http")]
  #[error("response headers already sent")]
  ResponseHeadersAlreadySent,
  #[class("Http")]
  #[error("connection closed while sending response")]
  ConnectionClosedWhileSendingResponse,
  #[class("Http")]
  #[error("already in use")]
  AlreadyInUse,
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[class("Http")]
  #[error("no response headers")]
  NoResponseHeaders,
  #[class("Http")]
  #[error("response already completed")]
  ResponseAlreadyCompleted,
  #[class("Http")]
  #[error("cannot upgrade because request body was used")]
  UpgradeBodyUsed,
  #[class("Http")]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub enum HttpSocketAddr {
  IpSocket(std::net::SocketAddr),
  #[cfg(unix)]
  UnixSocket(tokio::net::unix::SocketAddr),
}

impl From<std::net::SocketAddr> for HttpSocketAddr {
  fn from(addr: std::net::SocketAddr) -> Self {
    Self::IpSocket(addr)
  }
}

#[cfg(unix)]
impl From<tokio::net::unix::SocketAddr> for HttpSocketAddr {
  fn from(addr: tokio::net::unix::SocketAddr) -> Self {
    Self::UnixSocket(addr)
  }
}

struct OtelInfo {
  attributes: OtelInfoAttributes,
  duration: Option<std::time::Instant>,
  request_size: Option<u64>,
  response_size: Option<u64>,
}

struct OtelInfoAttributes {
  http_request_method: Cow<'static, str>,
  network_protocol_version: &'static str,
  url_scheme: Cow<'static, str>,
  server_address: Option<String>,
  server_port: Option<i64>,
  error_type: Option<&'static str>,
  http_response_status_code: Option<i64>,
}

impl OtelInfoAttributes {
  fn method(method: &http::method::Method) -> Cow<'static, str> {
    use http::method::Method;

    match *method {
      Method::GET => Cow::Borrowed("GET"),
      Method::POST => Cow::Borrowed("POST"),
      Method::PUT => Cow::Borrowed("PUT"),
      Method::DELETE => Cow::Borrowed("DELETE"),
      Method::HEAD => Cow::Borrowed("HEAD"),
      Method::OPTIONS => Cow::Borrowed("OPTIONS"),
      Method::CONNECT => Cow::Borrowed("CONNECT"),
      Method::PATCH => Cow::Borrowed("PATCH"),
      Method::TRACE => Cow::Borrowed("TRACE"),
      _ => Cow::Owned(method.to_string()),
    }
  }

  fn version(version: http::Version) -> &'static str {
    use http::Version;

    match version {
      Version::HTTP_09 => "0.9",
      Version::HTTP_10 => "1.0",
      Version::HTTP_11 => "1.1",
      Version::HTTP_2 => "2",
      Version::HTTP_3 => "3",
      _ => unreachable!(),
    }
  }

  fn for_counter(&self) -> Vec<deno_telemetry::KeyValue> {
    let mut attributes = vec![
      deno_telemetry::KeyValue::new(
        "http.request.method",
        self.http_request_method.clone(),
      ),
      deno_telemetry::KeyValue::new("url.scheme", self.url_scheme.clone()),
    ];

    if let Some(address) = self.server_address.clone() {
      attributes.push(deno_telemetry::KeyValue::new("server.address", address));
    }
    if let Some(port) = self.server_port {
      attributes.push(deno_telemetry::KeyValue::new("server.port", port));
    }

    attributes
  }

  fn for_histogram(&self) -> Vec<deno_telemetry::KeyValue> {
    let mut histogram_attributes = vec![
      deno_telemetry::KeyValue::new(
        "http.request.method",
        self.http_request_method.clone(),
      ),
      deno_telemetry::KeyValue::new("url.scheme", self.url_scheme.clone()),
      deno_telemetry::KeyValue::new(
        "network.protocol.version",
        self.network_protocol_version,
      ),
    ];

    if let Some(address) = self.server_address.clone() {
      histogram_attributes
        .push(deno_telemetry::KeyValue::new("server.address", address));
    }
    if let Some(port) = self.server_port {
      histogram_attributes
        .push(deno_telemetry::KeyValue::new("server.port", port));
    }
    if let Some(status_code) = self.http_response_status_code {
      histogram_attributes.push(deno_telemetry::KeyValue::new(
        "http.response.status_code",
        status_code,
      ));
    }

    if let Some(error) = self.error_type {
      histogram_attributes
        .push(deno_telemetry::KeyValue::new("error.type", error));
    }

    histogram_attributes
  }
}

impl OtelInfo {
  fn new(
    otel: &deno_telemetry::OtelGlobals,
    instant: std::time::Instant,
    request_size: u64,
    attributes: OtelInfoAttributes,
  ) -> Self {
    let collectors = OTEL_COLLECTORS.get_or_init(|| {
      let meter = otel
        .meter_provider
        .meter_with_scope(otel.builtin_instrumentation_scope.clone());

      let duration = meter
        .f64_histogram("http.server.request.duration")
        .with_unit("s")
        .with_description("Duration of HTTP server requests.")
        .with_boundaries(vec![
          0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0,
          7.5, 10.0,
        ])
        .build();

      let active_requests = meter
        .i64_up_down_counter("http.server.active_requests")
        .with_unit("{request}")
        .with_description("Number of active HTTP server requests.")
        .build();

      let request_size = meter
        .u64_histogram("http.server.request.body.size")
        .with_unit("By")
        .with_description("Size of HTTP server request bodies.")
        .with_boundaries(vec![
          0.0,
          100.0,
          1000.0,
          10000.0,
          100000.0,
          1000000.0,
          10000000.0,
          100000000.0,
          1000000000.0,
        ])
        .build();

      let response_size = meter
        .u64_histogram("http.server.response.body.size")
        .with_unit("By")
        .with_description("Size of HTTP server response bodies.")
        .with_boundaries(vec![
          0.0,
          100.0,
          1000.0,
          10000.0,
          100000.0,
          1000000.0,
          10000000.0,
          100000000.0,
          1000000000.0,
        ])
        .build();

      OtelCollectors {
        duration,
        active_requests,
        request_size,
        response_size,
      }
    });

    collectors.active_requests.add(1, &attributes.for_counter());

    Self {
      attributes,
      duration: Some(instant),
      request_size: Some(request_size),
      response_size: Some(0),
    }
  }

  fn handle_duration_and_request_size(&mut self) {
    let collectors = OTEL_COLLECTORS.get().unwrap();
    let attributes = self.attributes.for_histogram();

    if let Some(duration) = self.duration.take() {
      let duration = duration.elapsed();
      collectors
        .duration
        .record(duration.as_secs_f64(), &attributes);
    }

    if let Some(request_size) = self.request_size.take() {
      let collectors = OTEL_COLLECTORS.get().unwrap();
      collectors.request_size.record(request_size, &attributes);
    }
  }
}

impl Drop for OtelInfo {
  fn drop(&mut self) {
    let collectors = OTEL_COLLECTORS.get().unwrap();

    self.handle_duration_and_request_size();

    collectors
      .active_requests
      .add(-1, &self.attributes.for_counter());

    if let Some(response_size) = self.response_size {
      collectors
        .response_size
        .record(response_size, &self.attributes.for_histogram());
    }
  }
}

#[op2]
#[string]
fn op_http_websocket_accept_header(#[string] key: String) -> String {
  let digest = aws_lc_rs::digest::digest(
    &aws_lc_rs::digest::SHA1_FOR_LEGACY_USE_ONLY,
    format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
  );
  BASE64_STANDARD.encode(digest)
}

// Needed so hyper can use non Send futures
#[derive(Clone)]
pub struct LocalExecutor;

impl<Fut> hyper::rt::Executor<Fut> for LocalExecutor
where
  Fut: Future + 'static,
  Fut::Output: 'static,
{
  fn execute(&self, fut: Fut) {
    deno_core::unsync::spawn(fut);
  }
}

fn maybe_extract_network_stream<
  T: Into<NetworkStream> + AsyncRead + AsyncWrite + Unpin + 'static,
>(
  upgraded: hyper::upgrade::Upgraded,
) -> Result<(NetworkStream, Bytes), hyper::upgrade::Upgraded> {
  let upgraded = match upgraded.downcast::<TokioIo<T>>() {
    Ok(parts) => return Ok((parts.io.into_inner().into(), parts.read_buf)),
    Err(x) => x,
  };

  match upgraded.downcast::<TokioIo<NetworkBufferedStream<T>>>() {
    Ok(parts) => {
      // Both the upgrade and the stream might have unread bytes
      let (io, stream_bytes) = parts.io.into_inner().into_inner();
      let bytes = match (stream_bytes.is_empty(), parts.read_buf.is_empty()) {
        (false, false) => Bytes::default(),
        (true, false) => parts.read_buf,
        (false, true) => stream_bytes,
        (true, true) => {
          // The upgraded bytes come first as they have already been read
          let mut v = parts.read_buf.to_vec();
          v.append(&mut stream_bytes.to_vec());
          Bytes::from(v)
        }
      };
      Ok((io.into(), bytes))
    }
    Err(x) => Err(x),
  }
}

fn extract_network_stream(
  upgraded: hyper::upgrade::Upgraded,
) -> (NetworkStream, Bytes) {
  let upgraded =
    match maybe_extract_network_stream::<tokio::net::TcpStream>(upgraded) {
      Ok(res) => return res,
      Err(x) => x,
    };
  let upgraded = match maybe_extract_network_stream::<
    deno_net::ops_tls::TlsStream<TcpStream>,
  >(upgraded)
  {
    Ok(res) => return res,
    Err(x) => x,
  };
  #[cfg(unix)]
  let upgraded =
    match maybe_extract_network_stream::<tokio::net::UnixStream>(upgraded) {
      Ok(res) => return res,
      Err(x) => x,
    };
  #[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
  let upgraded =
    match maybe_extract_network_stream::<tokio_vsock::VsockStream>(upgraded) {
      Ok(res) => return res,
      Err(x) => x,
    };
  let upgraded = match maybe_extract_network_stream::<
    deno_net::tunnel::TunnelStream,
  >(upgraded)
  {
    Ok(res) => return res,
    Err(x) => x,
  };
  let upgraded = match maybe_extract_network_stream::<NetworkStream>(upgraded) {
    Ok(res) => return res,
    Err(x) => x,
  };

  // TODO(mmastrac): HTTP/2 websockets may yield an un-downgradable type
  drop(upgraded);
  unreachable!("unexpected stream type");
}

#[op2]
#[serde]
pub fn op_http_serve_address_override() -> (u8, String, u32, bool) {
  if let Ok(val) = std::env::var("DENO_SERVE_ADDRESS") {
    return parse_serve_address(&val);
  };

  if deno_net::tunnel::get_tunnel().is_some() {
    return (4, String::new(), 0, true);
  }

  (0, String::new(), 0, false)
}

fn parse_serve_address(input: &str) -> (u8, String, u32, bool) {
  let (input, duplicate) = match input.strip_prefix("duplicate,") {
    Some(input) => (input, true),
    None => (input, false),
  };
  match input.split_once(':') {
    Some(("tcp", addr)) => {
      // TCP address
      match addr.parse::<SocketAddr>() {
        Ok(addr) => {
          let hostname = match addr {
            SocketAddr::V4(v4) => v4.ip().to_string(),
            SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
          };
          (1, hostname, addr.port() as u32, duplicate)
        }
        Err(_) => {
          log::error!("DENO_SERVE_ADDRESS: invalid TCP address: {}", addr);
          (0, String::new(), 0, false)
        }
      }
    }
    Some(("unix", addr)) => {
      // Unix socket path
      if addr.is_empty() {
        log::error!("DENO_SERVE_ADDRESS: empty unix socket path");
        return (0, String::new(), 0, duplicate);
      }
      (2, addr.to_string(), 0, duplicate)
    }
    Some(("vsock", addr)) => {
      // Vsock address
      match addr.split_once(':') {
        Some((cid, port)) => {
          let cid = if cid == "-1" {
            "-1".to_string()
          } else {
            match cid.parse::<u32>() {
              Ok(cid) => cid.to_string(),
              Err(_) => {
                log::error!("DENO_SERVE_ADDRESS: invalid vsock CID: {}", cid);
                return (0, String::new(), 0, false);
              }
            }
          };
          let port = match port.parse::<u32>() {
            Ok(port) => port,
            Err(_) => {
              log::error!("DENO_SERVE_ADDRESS: invalid vsock port: {}", port);
              return (0, String::new(), 0, false);
            }
          };
          (3, cid, port, duplicate)
        }
        None => (0, String::new(), 0, false),
      }
    }
    Some(("tunnel", _)) => (4, String::new(), 0, duplicate),
    Some((_, _)) | None => {
      log::error!("DENO_SERVE_ADDRESS: invalid address format: {}", input);
      (0, String::new(), 0, false)
    }
  }
}

pub static SERVE_NOTIFIER: Notify = Notify::const_new();

#[op2(fast)]
fn op_http_notify_serving() {
  static ONCE: AtomicBool = AtomicBool::new(false);

  if ONCE
    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    .is_ok()
  {
    SERVE_NOTIFIER.notify_one();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_serve_address() {
    assert_eq!(
      parse_serve_address("tcp:127.0.0.1:8080"),
      (1, "127.0.0.1".to_string(), 8080, false)
    );
    assert_eq!(
      parse_serve_address("tcp:[::1]:9000"),
      (1, "[::1]".to_string(), 9000, false)
    );
    assert_eq!(
      parse_serve_address("duplicate,tcp:[::1]:9000"),
      (1, "[::1]".to_string(), 9000, true)
    );

    assert_eq!(
      parse_serve_address("unix:/var/run/socket.sock"),
      (2, "/var/run/socket.sock".to_string(), 0, false)
    );
    assert_eq!(
      parse_serve_address("duplicate,unix:/var/run/socket.sock"),
      (2, "/var/run/socket.sock".to_string(), 0, true)
    );

    assert_eq!(
      parse_serve_address("vsock:1234:5678"),
      (3, "1234".to_string(), 5678, false)
    );
    assert_eq!(
      parse_serve_address("vsock:-1:5678"),
      (3, "-1".to_string(), 5678, false)
    );
    assert_eq!(
      parse_serve_address("duplicate,vsock:-1:5678"),
      (3, "-1".to_string(), 5678, true)
    );

    assert_eq!(parse_serve_address("tcp:"), (0, String::new(), 0, false));
    assert_eq!(parse_serve_address("unix:"), (0, String::new(), 0, false));
    assert_eq!(parse_serve_address("vsock:"), (0, String::new(), 0, false));
    assert_eq!(parse_serve_address("foo:"), (0, String::new(), 0, false));
    assert_eq!(parse_serve_address("bar"), (0, String::new(), 0, false));
  }
}
