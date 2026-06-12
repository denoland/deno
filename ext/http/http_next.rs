// Copyright 2018-2026 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt;
use std::fmt::Write;
use std::future::Future;
use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;
use std::time::Duration;
use std::time::SystemTime;

use bytes::Bytes;
use bytes::BytesMut;
use deno_core::AsyncMut;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufMutView;
use deno_core::BufView;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::ExternalPointer;
use deno_core::FromV8;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::convert::ByteString;
use deno_core::external;
use deno_core::op2;
use deno_core::unsync::JoinHandle;
use deno_core::unsync::spawn;
use deno_core::v8;
use deno_http_h1 as h1;
use deno_net::ops_tls::TlsStream;
use deno_net::raw::NetworkStream;
use deno_net::raw::NetworkStreamReadHalf;
use deno_net::raw::NetworkStreamWriteHalf;
use deno_websocket::ws_create_server_stream;
use fly_accept_encoding::Encoding;
use hyper::StatusCode;
use hyper::body::Incoming;
use hyper::header::ACCEPT_ENCODING;
use hyper::header::CACHE_CONTROL;
use hyper::header::CONTENT_ENCODING;
use hyper::header::CONTENT_LENGTH;
use hyper::header::CONTENT_RANGE;
use hyper::header::CONTENT_TYPE;
use hyper::header::COOKIE;
use hyper::header::HeaderMap;
use hyper::http::HeaderName;
use hyper::http::HeaderValue;
use hyper::server::conn::http2;
use hyper::service::HttpService;
use hyper::service::service_fn;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use smallvec::SmallVec;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use super::fly_accept_encoding;
use crate::LocalExecutor;
use crate::Options;
use crate::OtelInfo;
use crate::OtelInfoAttributes;
use crate::compressible::is_content_compressible;
use crate::extract_network_stream;
use crate::network_buffered_stream::NetworkBufferedStream;
use crate::network_buffered_stream::NetworkStreamPrefixCheck;
use crate::request_body::HttpRequestBody;
use crate::request_properties::HttpConnectionProperties;
use crate::request_properties::HttpListenProperties;
use crate::request_properties::HttpPropertyExtractor;
use crate::response_body::Compression;
use crate::response_body::PollFrame;
use crate::response_body::ResponseBytesInner;
use crate::response_body::ResponseStreamResult;
use crate::service::DirectResponse;
use crate::service::DirectResponseBody;
use crate::service::DirectResponseHeaders;
use crate::service::FlatResponseBody;
use crate::service::HttpRecord;
use crate::service::HttpRecordResponse;
use crate::service::HttpRequestBodyAutocloser;
use crate::service::HttpServerState;
use crate::service::NativeResponseCell;
use crate::service::ServerCallback;
use crate::service::SignallingRc;
use crate::service::handle_request;
use crate::service::http_general_trace;
#[cfg(feature = "__http_tracing")]
use crate::service::http_trace;
use crate::v8_util::v8_string_to_utf8_bytes;

type Request = hyper::Request<Incoming>;

/// All HTTP/2 connections start with this byte string.
///
/// In HTTP/2, each endpoint is required to send a connection preface as a final confirmation
/// of the protocol in use and to establish the initial settings for the HTTP/2 connection. The
/// client and server each send a different connection preface.
///
/// The client connection preface starts with a sequence of 24 octets, which in hex notation is:
///
/// 0x505249202a20485454502f322e300d0a0d0a534d0d0a0d0a
///
/// That is, the connection preface starts with the string PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n). This sequence
/// MUST be followed by a SETTINGS frame (Section 6.5), which MAY be empty.
const HTTP2_PREFIX: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// ALPN negotiation for "h2"
const TLS_ALPN_HTTP_2: &[u8] = b"h2";

/// ALPN negotiation for "http/1.1"
const TLS_ALPN_HTTP_11: &[u8] = b"http/1.1";

/// Name a trait for streams we can serve HTTP over.
trait HttpServeStream:
  tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
{
}
impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static>
  HttpServeStream for S
{
}

#[repr(transparent)]
struct RcHttpRecord(HttpRecordExternal);

#[derive(Clone)]
enum HttpRecordExternal {
  Hyper(Rc<HttpRecord>),
  Raw(Rc<RawHttpRecord>),
}

impl HttpRecordExternal {
  fn into_hyper(
    self,
    op: &'static str,
  ) -> Result<Rc<HttpRecord>, HttpNextError> {
    match self {
      Self::Hyper(record) => Ok(record),
      Self::Raw(_) => {
        Err(HttpNextError::Other(deno_error::JsErrorBox::generic(
          format!("{op} unavailable on raw HTTP/1 path"),
        )))
      }
    }
  }

  fn cancelled(&self) -> bool {
    match self {
      Self::Hyper(record) => record.cancelled(),
      Self::Raw(record) => record.cancelled(),
    }
  }

  fn complete(&self) {
    match self {
      Self::Hyper(record) => record.clone().complete(),
      Self::Raw(record) => record.complete(),
    }
  }

  fn set_flat_response_body(&self, body: FlatResponseBody) {
    match self {
      Self::Hyper(record) => record.set_flat_response_body(body),
      Self::Raw(record) => record.set_flat_response_body(body),
    }
  }

  fn set_status(&self, status: u16) {
    match self {
      Self::Hyper(record) => {
        if let Ok(code) = StatusCode::from_u16(status) {
          record.response_parts().status = code;
          record.otel_info_set_status(status);
        }
      }
      Self::Raw(record) => record.set_status(status),
    }
  }

  fn append_response_header(&self, name: Vec<u8>, value: Vec<u8>) {
    match self {
      Self::Hyper(record) => {
        let mut response_parts = record.response_parts();
        let name = HeaderName::from_bytes(&name).unwrap();
        // SAFETY: JS response headers are converted through ByteString, so
        // they are valid one-byte header values here.
        let value = unsafe {
          HeaderValue::from_maybe_shared_unchecked(Bytes::from(value))
        };
        response_parts.headers.append(name, value);
      }
      Self::Raw(record) => record.append_response_header(name, value),
    }
  }

  fn set_default_text_content_type(&self) {
    match self {
      Self::Hyper(record) => {
        record.response_parts().headers.insert(
          CONTENT_TYPE,
          HeaderValue::from_static("text/plain;charset=UTF-8"),
        );
      }
      Self::Raw(record) => record.set_default_text_content_type(),
    }
  }

  fn set_content_type(&self, value: Vec<u8>) {
    match self {
      Self::Hyper(record) => {
        // SAFETY: JS response headers are converted through ByteString, so
        // they are valid one-byte header values here.
        let value = unsafe {
          HeaderValue::from_maybe_shared_unchecked(Bytes::from(value))
        };
        record.response_parts().headers.insert(CONTENT_TYPE, value);
      }
      Self::Raw(record) => record.set_content_type(value),
    }
  }

  fn set_response_body(&self, body: ResponseBytesInner) {
    match self {
      Self::Hyper(record) => record.set_response_body(body),
      Self::Raw(_) => body.abort(),
    }
  }

  fn otel_info_set_error(&self, error: &'static str) {
    match self {
      Self::Hyper(record) => record.otel_info_set_error(error),
      Self::Raw(record) => record.otel_info_set_error(error),
    }
  }

  fn copy_span_to_otel_info(&self, span: &deno_telemetry::OtelSpan) {
    match self {
      Self::Hyper(record) => record.copy_span_to_otel_info(span),
      Self::Raw(record) => record.copy_span_to_otel_info(span),
    }
  }
}

// Register the [`HttpRecord`] as an external.
external!(RcHttpRecord, "http record");

/// Construct Rc<HttpRecord> from raw external pointer, consuming
/// refcount. You must make sure the external is deleted on the JS side.
macro_rules! take_external {
  ($external:expr, $args:tt) => {{
    let ptr = ExternalPointer::<RcHttpRecord>::from_raw($external);
    let record = ptr.unsafely_take().0;
    #[cfg(feature = "__http_tracing")]
    {
      if let HttpRecordExternal::Hyper(record) = &record {
        http_trace!(record, $args);
      }
    }
    record
  }};
}

/// Clone Rc<HttpRecord> from raw external pointer.
macro_rules! clone_external {
  ($external:expr, $args:tt) => {{
    let ptr = ExternalPointer::<RcHttpRecord>::from_raw($external);
    ptr.unsafely_deref().0.clone()
  }};
}

/// Try to clone Rc<HttpRecord> from raw external pointer.
/// Returns None if the pointer has already been consumed by take_external.
macro_rules! try_clone_external {
  ($external:expr) => {{
    let ptr = $external;
    if ptr.is_null()
      || ptr.align_offset(std::mem::align_of::<usize>()) != 0
      || std::ptr::read::<usize>(ptr as _)
        != <RcHttpRecord as deno_core::Externalizable>::external_marker()
    {
      None
    } else {
      let ptr = ExternalPointer::<RcHttpRecord>::from_raw(ptr);
      Some(ptr.unsafely_deref().0.clone())
    }
  }};
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum HttpNextError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] io::Error),
  #[class("Http")]
  #[error("{0}")]
  Hyper(#[from] hyper::Error),
  #[class(inherit)]
  #[error(transparent)]
  JoinError(
    #[from]
    #[inherit]
    tokio::task::JoinError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Canceled(
    #[from]
    #[inherit]
    deno_core::Canceled,
  ),
  #[class(generic)]
  #[error(transparent)]
  UpgradeUnavailable(#[from] crate::service::UpgradeUnavailableError),
  #[class(inherit)]
  #[error("{0}")]
  Other(
    #[from]
    #[inherit]
    deno_error::JsErrorBox,
  ),
  #[class("Http")]
  #[error("invalid HTTP status line")]
  InvalidHttpStatusLine,
  #[class("Http")]
  #[error("raw upgrade failed")]
  RawUpgradeFailed,
  #[class(inherit)]
  #[error(transparent)]
  TakeNetworkStream(
    #[from]
    #[inherit]
    deno_net::raw::TakeNetworkStreamError,
  ),
}

impl From<h1::Error> for HttpNextError {
  fn from(error: h1::Error) -> Self {
    match error {
      h1::Error::Io(error) => Self::Io(error),
      _ => Self::Other(deno_error::JsErrorBox::generic(error.to_string())),
    }
  }
}

struct RawHeader {
  name: Vec<u8>,
  value: Vec<u8>,
}

enum RawMethod {
  Get,
  Post,
  Head,
  Put,
  Delete,
  Connect,
  Options,
  Trace,
  Patch,
  Other(String),
}

impl RawMethod {
  fn from_bytes(method: &[u8]) -> Self {
    match method {
      b"GET" => Self::Get,
      b"POST" => Self::Post,
      b"HEAD" => Self::Head,
      b"PUT" => Self::Put,
      b"DELETE" => Self::Delete,
      b"CONNECT" => Self::Connect,
      b"OPTIONS" => Self::Options,
      b"TRACE" => Self::Trace,
      b"PATCH" => Self::Patch,
      method => Self::Other(String::from_utf8_lossy(method).into_owned()),
    }
  }

  fn is_connect(&self) -> bool {
    matches!(self, Self::Connect)
  }

  fn is_head(&self) -> bool {
    matches!(self, Self::Head)
  }

  fn is_options(&self) -> bool {
    matches!(self, Self::Options)
  }

  fn as_cow(&self) -> Cow<'static, str> {
    match self {
      Self::Get => Cow::Borrowed("GET"),
      Self::Post => Cow::Borrowed("POST"),
      Self::Head => Cow::Borrowed("HEAD"),
      Self::Put => Cow::Borrowed("PUT"),
      Self::Delete => Cow::Borrowed("DELETE"),
      Self::Connect => Cow::Borrowed("CONNECT"),
      Self::Options => Cow::Borrowed("OPTIONS"),
      Self::Trace => Cow::Borrowed("TRACE"),
      Self::Patch => Cow::Borrowed("PATCH"),
      Self::Other(method) => Cow::Owned(method.clone()),
    }
  }
}

struct RawRequestHeader {
  name_start: usize,
  name_len: usize,
  value_start: usize,
  value_len: usize,
}

struct RawRequestHeaders {
  bytes: Vec<u8>,
  entries: Vec<RawRequestHeader>,
  host_index: Option<usize>,
  authorization_index: Option<usize>,
  authorization_multiple: bool,
}

struct RawRequestHeaderRef<'a> {
  name: &'a [u8],
  value: &'a [u8],
}

impl RawRequestHeaders {
  fn empty() -> Self {
    Self {
      bytes: Vec::new(),
      entries: Vec::new(),
      host_index: None,
      authorization_index: None,
      authorization_multiple: false,
    }
  }

  fn from_h1(headers: &[h1::Header<'_>]) -> Self {
    let byte_len = headers
      .iter()
      .map(|header| header.name.len() + header.value.len())
      .sum();
    let mut bytes = Vec::with_capacity(byte_len);
    let mut entries = Vec::with_capacity(headers.len());
    let mut host_index = None;
    let mut authorization_index = None;
    let mut authorization_multiple = false;
    // SAFETY: the loop below copies exactly `byte_len` initialized bytes into
    // the spare capacity computed above before exposing the length.
    unsafe {
      bytes.set_len(byte_len);
    }
    let mut pos = 0usize;
    for header in headers {
      let name_start = pos;
      let name_end = name_start + header.name.len();
      bytes[name_start..name_end].copy_from_slice(header.name);
      pos = name_end;
      let value_start = pos;
      let value_end = value_start + header.value.len();
      bytes[value_start..value_end].copy_from_slice(header.value);
      pos = value_end;
      entries.push(RawRequestHeader {
        name_start,
        name_len: header.name.len(),
        value_start,
        value_len: header.value.len(),
      });
      let entry_index = entries.len() - 1;
      if host_index.is_none() && header.name.eq_ignore_ascii_case(b"host") {
        host_index = Some(entry_index);
      }
      if header.name.eq_ignore_ascii_case(b"authorization") {
        if authorization_index.is_some() {
          authorization_multiple = true;
        } else {
          authorization_index = Some(entry_index);
        }
      }
    }
    debug_assert_eq!(pos, byte_len);
    Self {
      bytes,
      entries,
      host_index,
      authorization_index,
      authorization_multiple,
    }
  }

  fn len(&self) -> usize {
    self.entries.len()
  }

  fn iter(&self) -> impl Iterator<Item = RawRequestHeaderRef<'_>> {
    self.entries.iter().map(|header| RawRequestHeaderRef {
      name: &self.bytes[header.name_start..header.name_start + header.name_len],
      value: &self.bytes
        [header.value_start..header.value_start + header.value_len],
    })
  }

  fn get(&self, name: &[u8]) -> Option<&[u8]> {
    if name == b"host" {
      if let Some(index) = self.host_index {
        let header = &self.entries[index];
        return Some(
          &self.bytes
            [header.value_start..header.value_start + header.value_len],
        );
      }
      return None;
    }
    self
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value)
  }

  fn get_single_authorization(&self) -> Option<&[u8]> {
    if self.authorization_multiple {
      return None;
    }
    let index = self.authorization_index?;
    let header = &self.entries[index];
    Some(&self.bytes[header.value_start..header.value_start + header.value_len])
  }

  /// Removes all headers with the given (lowercase) name, returning the
  /// value of the first removed header.
  fn remove(&mut self, name: &[u8]) -> Option<Vec<u8>> {
    let mut value = None;
    let mut i = 0;
    while i < self.entries.len() {
      let header = &self.entries[i];
      let header_name =
        &self.bytes[header.name_start..header.name_start + header.name_len];
      if header_name.eq_ignore_ascii_case(name) {
        if value.is_none() {
          value = Some(
            self.bytes
              [header.value_start..header.value_start + header.value_len]
              .to_vec(),
          );
        }
        self.entries.remove(i);
        if let Some(index) = self.host_index
          && index > i
        {
          self.host_index = Some(index - 1);
        }
        if let Some(index) = self.authorization_index
          && index > i
        {
          self.authorization_index = Some(index - 1);
        }
      } else {
        i += 1;
      }
    }
    value
  }

  fn get_joined(&self, name: &[u8], separator: &[u8]) -> Option<Vec<u8>> {
    let mut out = None::<Vec<u8>>;
    for header in self.iter() {
      if header.name.eq_ignore_ascii_case(name) {
        match &mut out {
          Some(out) => {
            out.extend_from_slice(separator);
            out.extend_from_slice(header.value);
          }
          None => out = Some(header.value.to_vec()),
        }
      }
    }
    out
  }
}

struct RawOtelInfoAttributes {
  method: Cow<'static, str>,
  scheme: Cow<'static, str>,
  server_address: Option<String>,
  server_port: Option<i64>,
}

impl RawOtelInfoAttributes {
  fn new(
    request_info: &HttpConnectionProperties,
    method: &RawMethod,
    path: &str,
    _headers: &RawRequestHeaders,
  ) -> Self {
    if !matches!(path.as_bytes().first(), Some(b'/' | b'*'))
      && let Some((scheme, authority, _)) = split_absolute_form_target(path)
    {
      let (address, port) = split_authority(authority);
      return Self {
        method: method.as_cow(),
        scheme: Cow::Owned(scheme.to_string()),
        server_address: (!address.is_empty()).then(|| address.to_string()),
        server_port: port.map(i64::from),
      };
    }

    Self {
      method: method.as_cow(),
      scheme: Cow::Borrowed(raw_otel_scheme(request_info.scheme)),
      server_address: None,
      server_port: None,
    }
  }

  fn into_attributes(self) -> OtelInfoAttributes {
    OtelInfoAttributes {
      http_request_method: self.method,
      network_protocol_version: "1.1",
      url_scheme: self.scheme,
      server_address: self.server_address,
      server_port: self.server_port,
      error_type: Default::default(),
      http_route: None,
      http_response_status_code: Default::default(),
    }
  }
}

fn raw_otel_scheme(scheme_prefix: &'static str) -> &'static str {
  scheme_prefix.strip_suffix("://").unwrap_or(scheme_prefix)
}

fn split_authority(authority: &str) -> (&str, Option<u16>) {
  let Some((address, port)) = authority.rsplit_once(':') else {
    return (authority, None);
  };
  let Ok(port) = port.parse() else {
    return (authority, None);
  };
  (address, Some(port))
}

fn request_header_value_separator(name: &[u8]) -> &'static [u8] {
  if name.eq_ignore_ascii_case(COOKIE.as_ref()) {
    b"; "
  } else {
    b", "
  }
}

fn raw_request_header_to_v8<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  headers: &RawRequestHeaders,
  name: &[u8],
) -> v8::Local<'scope, v8::Value> {
  if name == b"authorization"
    && let Some(value) = headers.get_single_authorization()
  {
    return v8::String::new_from_one_byte(
      scope,
      value,
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into();
  }
  let separator = request_header_value_separator(name);
  let mut first = None::<&[u8]>;
  let mut out = None::<Vec<u8>>;
  for header in headers.iter() {
    if !header.name.eq_ignore_ascii_case(name) {
      continue;
    }
    let Some(first_value) = first else {
      first = Some(header.value);
      continue;
    };
    match &mut out {
      Some(out) => {
        out.extend_from_slice(separator);
        out.extend_from_slice(header.value);
      }
      None => {
        let mut value = Vec::with_capacity(
          first_value.len() + separator.len() + header.value.len(),
        );
        value.extend_from_slice(first_value);
        value.extend_from_slice(separator);
        value.extend_from_slice(header.value);
        out = Some(value);
      }
    }
  }
  if let Some(value) = out {
    return v8::String::new_from_one_byte(
      scope,
      &value,
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into();
  }
  let Some(value) = first else {
    return v8::null(scope).into();
  };
  v8::String::new_from_one_byte(scope, value, v8::NewStringType::Normal)
    .unwrap()
    .into()
}

struct RawResponseParts {
  status: u16,
  headers: Vec<RawHeader>,
  trailers: Vec<RawHeader>,
  default_text_content_type: bool,
  content_type: Option<Vec<u8>>,
}

enum RawResponseBody {
  Flat(FlatResponseBody),
  Stream {
    body: ResponseBytesInner,
    content_length: Option<u64>,
  },
}

fn preferred_supported_compression(
  encodings: impl Iterator<
    Item = Result<(Option<Encoding>, f32), fly_accept_encoding::EncodingError>,
  >,
) -> Compression {
  match super::preferred_supported_encoding(encodings) {
    Encoding::Brotli => Compression::Brotli,
    Encoding::Gzip => Compression::GZip,
    _ => Compression::None,
  }
}

fn raw_request_compression(headers: &RawRequestHeaders) -> Compression {
  let Some(accept_encoding) = headers.get_joined(b"accept-encoding", b", ")
  else {
    return Compression::None;
  };
  let Ok(accept_encoding) = std::str::from_utf8(&accept_encoding) else {
    return Compression::None;
  };
  match accept_encoding {
    // Firefox and Chrome send this -- no need to parse.
    "gzip, deflate, br" => return Compression::Brotli,
    "gzip, deflate, br, zstd" => return Compression::Brotli,
    "gzip" => return Compression::GZip,
    "br" => return Compression::Brotli,
    _ => {}
  }

  let accepted =
    fly_accept_encoding::encodings_iter_str(std::iter::once(accept_encoding));
  preferred_supported_compression(accepted)
}

#[cfg(test)]
mod compression_tests {
  use super::*;

  fn compression_for(accept_encoding: &str) -> Compression {
    let accepted =
      fly_accept_encoding::encodings_iter_str(std::iter::once(accept_encoding));
    preferred_supported_compression(accepted)
  }

  #[test]
  fn compression_prefers_brotli_over_gzip_when_equal() {
    assert!(matches!(
      compression_for("gzip, deflate, br, zstd"),
      Compression::Brotli
    ));
    assert!(matches!(
      compression_for("zstd, gzip, deflate, br"),
      Compression::Brotli
    ));
    assert!(matches!(
      compression_for("gzip;q=1.0, br;q=1.0"),
      Compression::Brotli
    ));
  }

  #[test]
  fn compression_respects_higher_qval() {
    assert!(matches!(
      compression_for("gzip;q=1.0, br;q=0.9, zstd;q=1.0"),
      Compression::GZip
    ));
    assert!(matches!(
      compression_for("gzip;q=0.5, br;q=0.9"),
      Compression::Brotli
    ));
    assert!(matches!(
      compression_for("identity;q=1.0, gzip;q=0.9, br;q=0.9"),
      Compression::None
    ));
  }

  #[test]
  fn compression_ignores_unsupported_or_invalid_tokens() {
    assert!(matches!(
      compression_for("br, compress"),
      Compression::Brotli
    ));
    assert!(matches!(compression_for("gzip, br;q=2"), Compression::GZip));
    assert!(matches!(
      compression_for("compress, x-gzip"),
      Compression::None
    ));
  }
}

fn raw_response_header<'a>(
  headers: &'a [RawHeader],
  name: &[u8],
) -> Option<&'a [u8]> {
  headers
    .iter()
    .find(|header| header.name.eq_ignore_ascii_case(name))
    .map(|header| header.value.as_slice())
}

fn raw_response_is_compressible(inner: &RawHttpRecordInner) -> bool {
  let Some(content_type) = inner.content_type.as_deref().or_else(|| {
    raw_response_header(&inner.response_headers, CONTENT_TYPE.as_ref())
  }) else {
    return false;
  };

  let Ok(content_type) = HeaderValue::from_bytes(content_type) else {
    return false;
  };
  if !is_content_compressible(&content_type) {
    return false;
  }
  if raw_response_header(&inner.response_headers, CONTENT_ENCODING.as_ref())
    .is_some()
    || raw_response_header(&inner.response_headers, CONTENT_RANGE.as_ref())
      .is_some()
  {
    return false;
  }
  if let Some(cache_control) =
    raw_response_header(&inner.response_headers, CACHE_CONTROL.as_ref())
    && let Ok(cache_control) = std::str::from_utf8(cache_control)
    && let Some(no_transform) =
      crate::cache_control_has_no_transform(cache_control)
    && no_transform
  {
    return false;
  }
  true
}

fn ensure_raw_vary_accept_encoding(headers: &mut Vec<RawHeader>) {
  for header in headers.iter_mut() {
    if header.name.eq_ignore_ascii_case(b"vary") {
      let Ok(value) = std::str::from_utf8(&header.value) else {
        return;
      };
      if !value.to_lowercase().contains("accept-encoding") {
        let mut new_value = b"Accept-Encoding, ".to_vec();
        new_value.extend_from_slice(&header.value);
        header.value = new_value;
      }
      return;
    }
  }
  headers.push(RawHeader {
    name: b"vary".to_vec(),
    value: b"Accept-Encoding".to_vec(),
  });
}

fn weaken_raw_etag(headers: &mut [RawHeader]) {
  for header in headers {
    if header.name.eq_ignore_ascii_case(b"etag")
      && !header.value.starts_with(b"W/")
    {
      let mut value = b"W/".to_vec();
      value.extend_from_slice(&header.value);
      header.value = value;
      return;
    }
  }
}

struct RawH1ConnectionState<I> {
  conn: h1::SharedConn<I>,
  scratch: h1::SharedScratch,
}

impl<I> RawH1ConnectionState<I>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  fn poll_read_body(
    &mut self,
    cx: &mut Context<'_>,
    limit: usize,
  ) -> Poll<Result<BufView, HttpNextError>> {
    self.scratch.ensure_read_capacity(limit);
    match ready!(self.conn.poll_read_body_chunk_limited_with(
      cx,
      &mut self.scratch,
      limit,
      |chunk| BufView::from(chunk.to_vec())
    ))? {
      h1::SharedBodyChunk::Chunk(chunk) => Poll::Ready(Ok(chunk)),
      h1::SharedBodyChunk::Complete => Poll::Ready(Ok(BufView::empty())),
    }
  }

  fn poll_read_body_byob(
    &mut self,
    cx: &mut Context<'_>,
    buf: &mut [u8],
  ) -> Poll<Result<usize, HttpNextError>> {
    self.scratch.ensure_read_capacity(buf.len());
    match ready!(self.conn.poll_read_body_chunk_limited_with(
      cx,
      &mut self.scratch,
      buf.len(),
      |chunk| {
        let len = chunk.len();
        buf[..len].copy_from_slice(chunk);
        len
      }
    ))? {
      h1::SharedBodyChunk::Chunk(len) => Poll::Ready(Ok(len)),
      h1::SharedBodyChunk::Complete => Poll::Ready(Ok(0)),
    }
  }

  fn poll_write_response(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedResponseWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self
      .conn
      .poll_write_response_with(cx, &mut self.scratch, writer)
    {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_start_chunked_response(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedChunkedResponseHeadWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self.conn.poll_start_chunked_response_with(
      cx,
      &mut self.scratch,
      writer,
    ) {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_start_fixed_response(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedFixedResponseHeadWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self.conn.poll_start_fixed_response_with(
      cx,
      &mut self.scratch,
      writer,
    ) {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_write_response_chunk(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedResponseChunkWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self.conn.poll_write_response_chunk_with(
      cx,
      &mut self.scratch,
      writer,
    ) {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_write_response_body(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedResponseBodyWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self.conn.poll_write_response_body_with(cx, writer) {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_peer_closed(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Result<bool, HttpNextError>> {
    match self.conn.poll_peer_closed_with(cx, &mut self.scratch) {
      Poll::Ready(Ok(closed)) => Poll::Ready(Ok(closed)),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }

  fn poll_finish_response(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut h1::SharedResponseEndWriter<'_>,
  ) -> Poll<Result<(), HttpNextError>> {
    match self
      .conn
      .poll_finish_response_with(cx, &mut self.scratch, writer)
    {
      Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
      Poll::Pending => Poll::Pending,
    }
  }
}

type RawH1ConnectionCell<I> = Rc<RefCell<Option<RawH1ConnectionState<I>>>>;
type RawH1Io = NetworkBufferedStream<NetworkStream>;
type RawNetworkH1ConnectionCell = RawH1ConnectionCell<RawH1Io>;
type RawWebSocketUpgradeSender =
  tokio::sync::oneshot::Sender<Result<(NetworkStream, Bytes), HttpNextError>>;

struct RawUpgrade {
  conn: RawNetworkH1ConnectionCell,
  websocket_tx: RefCell<Option<RawWebSocketUpgradeSender>>,
}

enum RawRequestBody {
  Prebuffered(Vec<u8>),
  Streaming(Rc<RawH1RequestBody<RawH1Io>>),
}

struct RawHttpRecordInner {
  request_info: HttpConnectionProperties,
  client_addr: Option<Vec<u8>>,
  method: RawMethod,
  path: String,
  headers: RawRequestHeaders,
  request_body: Option<RawRequestBody>,
  upgrade: Option<Rc<RawUpgrade>>,
  response_status: u16,
  response_headers: Vec<RawHeader>,
  response_trailers: Vec<RawHeader>,
  default_text_content_type: bool,
  content_type: Option<Vec<u8>>,
  response_body: Option<RawResponseBody>,
  request_body_taken_full: bool,
  request_cancelled: bool,
  request_cancel_waker: Option<std::task::Waker>,
  response_ready: bool,
  response_ready_waker: Option<std::task::Waker>,
  response_body_finished: bool,
  response_body_waker: Option<std::task::Waker>,
  otel_info: Option<OtelInfo>,
}

struct RawHttpRecord(RefCell<RawHttpRecordInner>);

impl RawHttpRecord {
  fn new(
    request_info: HttpConnectionProperties,
    method: RawMethod,
    path: String,
    mut headers: RawRequestHeaders,
    request_body: Option<RawRequestBody>,
    upgrade: Option<Rc<RawUpgrade>>,
    request_size: u64,
  ) -> Rc<Self> {
    let client_addr = if crate::service::trust_proxy_headers() {
      headers.remove(b"x-deno-client-address")
    } else {
      None
    };
    let otel_info = deno_telemetry::OTEL_GLOBALS
      .get()
      .filter(|o| o.has_metrics())
      .map(|otel| {
        OtelInfo::new(
          otel,
          std::time::Instant::now(),
          request_size,
          RawOtelInfoAttributes::new(&request_info, &method, &path, &headers)
            .into_attributes(),
        )
      });
    Rc::new(Self(RefCell::new(RawHttpRecordInner {
      request_info,
      client_addr,
      method,
      path,
      headers,
      request_body,
      upgrade,
      response_status: 200,
      response_headers: Vec::new(),
      response_trailers: Vec::new(),
      default_text_content_type: false,
      content_type: None,
      response_body: None,
      request_body_taken_full: false,
      request_cancelled: false,
      request_cancel_waker: None,
      response_ready: false,
      response_ready_waker: None,
      response_body_finished: false,
      response_body_waker: None,
      otel_info,
    })))
  }

  fn complete(&self) {
    let mut inner = self.0.borrow_mut();
    inner.response_ready = true;
    if let Some(waker) = inner.response_ready_waker.take() {
      waker.wake();
    }
    if let Some(waker) = inner.request_cancel_waker.take() {
      waker.wake();
    }
  }

  fn cancel_request(&self) {
    let mut inner = self.0.borrow_mut();
    inner.request_cancelled = true;
    if let Some(waker) = inner.request_cancel_waker.take() {
      waker.wake();
    }
  }

  fn cancel_unfinished_request(&self) {
    let mut inner = self.0.borrow_mut();
    if inner.response_ready && inner.response_body_finished {
      return;
    }
    inner.request_cancelled = true;
    inner.response_body_finished = true;
    if let Some(waker) = inner.request_cancel_waker.take() {
      waker.wake();
    }
    if let Some(waker) = inner.response_ready_waker.take() {
      waker.wake();
    }
    if let Some(waker) = inner.response_body_waker.take() {
      waker.wake();
    }
  }

  fn cancelled(&self) -> bool {
    self.0.borrow().request_cancelled
  }

  fn request_cancelled(&self) -> impl Future<Output = bool> + '_ {
    struct Cancelled<'a>(&'a RawHttpRecord);

    impl Future for Cancelled<'_> {
      type Output = bool;

      fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
      ) -> Poll<Self::Output> {
        let mut inner = self.0.0.borrow_mut();
        if inner.request_cancelled {
          return Poll::Ready(true);
        }
        // Resolve only once the response body has been fully written, not
        // when the response is merely dispatched: in legacy-abort mode the
        // JS side aborts `Request.signal` as soon as this future resolves,
        // which would tear down anything tied to the signal (e.g. a proxied
        // `fetch` whose body is still streaming as the response).
        if inner.response_body_finished {
          return Poll::Ready(false);
        }
        inner.request_cancel_waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }

    Cancelled(self)
  }

  fn set_flat_response_body(&self, body: FlatResponseBody) {
    let mut inner = self.0.borrow_mut();
    debug_assert!(inner.response_body.is_none());
    inner.response_body = Some(RawResponseBody::Flat(body));
  }

  fn set_stream_response_body(
    &self,
    body: ResponseBytesInner,
    content_length: Option<u64>,
  ) {
    let mut inner = self.0.borrow_mut();
    debug_assert!(inner.response_body.is_none());
    inner.response_body = Some(RawResponseBody::Stream {
      body,
      content_length,
    });
  }

  fn set_stream_response_body_resource(
    &self,
    content_length: Option<u64>,
    length_for_compression: Option<usize>,
    resource: Rc<dyn Resource>,
    auto_close: bool,
  ) {
    let compression = self.prepare_stream_compression(length_for_compression);
    let content_length = (compression == Compression::None)
      .then_some(content_length)
      .flatten();
    self.set_stream_response_body(
      ResponseBytesInner::from_resource(compression, resource, auto_close),
      content_length,
    );
  }

  fn set_status(&self, status: u16) {
    let mut inner = self.0.borrow_mut();
    inner.response_status = status;
    if let Some(info) = inner.otel_info.as_mut() {
      info.attributes.http_response_status_code = Some(status as _);
      info.handle_duration_and_request_size();
    }
  }

  fn append_response_header(&self, name: Vec<u8>, value: Vec<u8>) {
    self
      .0
      .borrow_mut()
      .response_headers
      .push(RawHeader { name, value });
  }

  fn set_response_trailers(&self, trailers: Vec<RawHeader>) {
    self.0.borrow_mut().response_trailers = trailers;
  }

  fn set_default_text_content_type(&self) {
    self.0.borrow_mut().default_text_content_type = true;
  }

  fn set_content_type(&self, value: Vec<u8>) {
    self.0.borrow_mut().content_type = Some(value);
  }

  fn prepare_stream_compression(&self, length: Option<usize>) -> Compression {
    if let Some(length) = length
      && length < 64
    {
      return Compression::None;
    }

    let mut inner = self.0.borrow_mut();
    let compression = raw_request_compression(&inner.headers);
    if compression == Compression::None || !raw_response_is_compressible(&inner)
    {
      return Compression::None;
    }

    inner
      .response_headers
      .retain(|header| !header.name.eq_ignore_ascii_case(b"content-length"));
    ensure_raw_vary_accept_encoding(&mut inner.response_headers);
    weaken_raw_etag(&mut inner.response_headers);

    let encoding = match compression {
      Compression::Brotli => b"br".as_slice(),
      Compression::GZip => b"gzip".as_slice(),
      Compression::None => unreachable!(),
    };
    inner.response_headers.push(RawHeader {
      name: b"content-encoding".to_vec(),
      value: encoding.to_vec(),
    });

    compression
  }

  fn into_flat_response(
    self: Rc<Self>,
  ) -> Option<(RawResponseParts, RawResponseBody)> {
    let mut inner = self.0.borrow_mut();
    let body = inner.response_body.take();
    body.map(|body| {
      (
        RawResponseParts {
          status: inner.response_status,
          headers: std::mem::take(&mut inner.response_headers),
          trailers: std::mem::take(&mut inner.response_trailers),
          default_text_content_type: inner.default_text_content_type,
          content_type: inner.content_type.take(),
        },
        body,
      )
    })
  }

  fn take_request_body(&self) -> Option<Rc<dyn Resource>> {
    let body = self.0.borrow_mut().request_body.take()?;
    match body {
      RawRequestBody::Streaming(body) => Some(body as Rc<dyn Resource>),
      RawRequestBody::Prebuffered(body) => {
        self.0.borrow_mut().request_body =
          Some(RawRequestBody::Prebuffered(body));
        None
      }
    }
  }

  fn try_take_full_request_body(&self) -> Option<Vec<u8>> {
    let streaming_body = {
      let mut inner = self.0.borrow_mut();
      match inner.request_body.take() {
        None => {
          inner.request_body_taken_full = true;
          return Some(Vec::new());
        }
        Some(RawRequestBody::Prebuffered(body)) => {
          inner.request_body_taken_full = true;
          return Some(body);
        }
        Some(RawRequestBody::Streaming(body)) => {
          inner.request_body = Some(RawRequestBody::Streaming(body.clone()));
          body
        }
      }
    };
    let bytes = streaming_body.try_take_full()?;
    let mut inner = self.0.borrow_mut();
    inner.request_body.take();
    inner.request_body_taken_full = true;
    Some(bytes)
  }

  fn request_body_taken_full(&self) -> bool {
    self.0.borrow().request_body_taken_full
  }

  fn take_upgrade(&self) -> Option<Rc<RawUpgrade>> {
    self.0.borrow_mut().upgrade.take()
  }

  fn finish_response_body(&self, complete: bool) {
    let mut inner = self.0.borrow_mut();
    if complete {
      inner.response_body =
        Some(RawResponseBody::Flat(FlatResponseBody::Empty));
    }
    inner.response_body_finished = true;
    if let Some(waker) = inner.response_body_waker.take() {
      waker.wake();
    }
    if let Some(waker) = inner.request_cancel_waker.take() {
      waker.wake();
    }
  }

  fn add_otel_response_size(&self, size: usize) {
    let mut inner = self.0.borrow_mut();
    if let Some(info) = inner.otel_info.as_mut()
      && let Some(total) = info.response_size.as_mut()
    {
      *total += size as u64;
    }
  }

  fn otel_info_set_error(&self, error: &'static str) {
    let mut inner = self.0.borrow_mut();
    if let Some(info) = inner.otel_info.as_mut() {
      info.attributes.error_type = Some(error);
      info.handle_duration_and_request_size();
    }
  }

  fn copy_span_to_otel_info(&self, span: &deno_telemetry::OtelSpan) {
    let mut inner = self.0.borrow_mut();
    let Some(info) = inner.otel_info.as_mut() else {
      return;
    };
    let span_state = span.0.borrow();
    if let deno_telemetry::OtelSpanState::Recording(data) = &**span_state {
      for attr in &data.attributes {
        if attr.key.as_str() == "http.route" {
          info.attributes.http_route = Some(attr.value.to_string());
        }
      }
    }
  }

  fn response_body_finished(&self) -> impl Future<Output = bool> + '_ {
    struct Finished<'a>(&'a RawHttpRecord);

    impl Future for Finished<'_> {
      type Output = bool;

      fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
      ) -> Poll<Self::Output> {
        let mut inner = self.0.0.borrow_mut();
        if inner.response_body_finished {
          return Poll::Ready(matches!(
            inner.response_body,
            Some(RawResponseBody::Flat(FlatResponseBody::Empty)) | None
          ));
        }
        inner.response_body_waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }

    Finished(self)
  }
}

struct RawHttpRecordCancelGuard(Option<Rc<RawHttpRecord>>);

impl RawHttpRecordCancelGuard {
  fn new(record: Rc<RawHttpRecord>) -> Self {
    Self(Some(record))
  }

  fn disarm(&mut self) {
    if let Some(record) = self.0.take() {
      // The response (including its body, if any) has been written. Mark the
      // body finished so `request_cancelled()` resolves; flat-response paths
      // don't go through `RawResponseBodyFinishGuard`.
      record.finish_response_body(false);
    }
  }
}

impl Drop for RawHttpRecordCancelGuard {
  fn drop(&mut self) {
    if let Some(record) = self.0.take() {
      record.cancel_unfinished_request();
    }
  }
}

struct RawH1RequestBody<I> {
  conn: RawH1ConnectionCell<I>,
  size_hint: (u64, Option<u64>),
  canceled: std::cell::Cell<bool>,
}

impl<I> RawH1RequestBody<I> {
  fn new(conn: RawH1ConnectionCell<I>, length: Option<u64>) -> Self {
    Self {
      conn,
      size_hint: length.map_or((0, None), |length| (length, Some(length))),
      canceled: std::cell::Cell::new(false),
    }
  }

  fn cancel(&self) {
    self.canceled.set(true);
  }

  fn try_take_full(&self) -> Option<Vec<u8>> {
    if self.canceled.get() {
      return None;
    }
    let mut conn = self.conn.borrow_mut();
    let conn = conn.as_mut()?;
    conn.conn.try_take_full_body().ok().flatten()
  }
}

struct RawH1RequestBodyRead<I> {
  body: Rc<RawH1RequestBody<I>>,
  limit: usize,
}

struct RawH1RequestBodyReadByob<I> {
  body: Rc<RawH1RequestBody<I>>,
  buf: Option<BufMutView>,
}

impl<I> Future for RawH1RequestBodyRead<I>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  type Output = Result<BufView, HttpNextError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.body.canceled.get() {
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::new(
          "BadResource",
          "Cannot read request body as underlying resource unavailable",
        ),
      )));
    }
    let mut conn = this.body.conn.borrow_mut();
    let Some(conn) = conn.as_mut() else {
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::generic("request body is no longer readable"),
      )));
    };
    conn.scratch.ensure_read_capacity(this.limit);
    if let Poll::Ready(Ok(true)) = conn.poll_peer_closed(cx) {
      this.body.cancel();
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::new(
          "BadResource",
          "Cannot read request body as underlying resource unavailable",
        ),
      )));
    }
    conn.poll_read_body(cx, this.limit)
  }
}

impl<I> Future for RawH1RequestBodyReadByob<I>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  type Output = Result<(usize, BufMutView), HttpNextError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.body.canceled.get() {
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::new(
          "BadResource",
          "Cannot read request body as underlying resource unavailable",
        ),
      )));
    }
    let mut conn = this.body.conn.borrow_mut();
    let Some(conn) = conn.as_mut() else {
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::generic("request body is no longer readable"),
      )));
    };
    let buf_len = this.buf.as_ref().unwrap().len();
    conn.scratch.ensure_read_capacity(buf_len);
    if let Poll::Ready(Ok(true)) = conn.poll_peer_closed(cx) {
      this.body.cancel();
      return Poll::Ready(Err(HttpNextError::Other(
        deno_error::JsErrorBox::new(
          "BadResource",
          "Cannot read request body as underlying resource unavailable",
        ),
      )));
    }
    let buf = this.buf.as_mut().unwrap();
    let read = ready!(conn.poll_read_body_byob(cx, buf))?;
    let buf = this.buf.take().unwrap();
    Poll::Ready(Ok((read, buf)))
  }
}

impl<I> Resource for RawH1RequestBody<I>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + 'static,
{
  fn name(&self) -> Cow<'_, str> {
    "requestBody".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      RawH1RequestBodyRead { body: self, limit }.await.map_err(
        |err| match err {
          HttpNextError::Other(error) => error,
          _ => deno_error::JsErrorBox::new("Http", err.to_string()),
        },
      )
    })
  }

  fn read_byob(
    self: Rc<Self>,
    buf: BufMutView,
  ) -> AsyncResult<(usize, BufMutView)> {
    Box::pin(async move {
      RawH1RequestBodyReadByob {
        body: self,
        buf: Some(buf),
      }
      .await
      .map_err(|err| match err {
        HttpNextError::Other(error) => error,
        _ => deno_error::JsErrorBox::new("Http", err.to_string()),
      })
    })
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    self.size_hint
  }
}

#[op2(fast)]
#[smi]
pub fn op_http_upgrade_raw(
  state: &mut OpState,
  external: *const c_void,
) -> Result<ResourceId, HttpNextError> {
  // SAFETY: external is deleted before calling this op.
  let http = unsafe { take_external!(external, "op_http_upgrade_raw") };
  if let HttpRecordExternal::Raw(record) = http {
    let Some(upgrade) = record.take_upgrade() else {
      return Err(raw_upgrade_unavailable());
    };
    let read = Rc::new(AsyncRefCell::new(None));
    let read_cell = AsyncRefCell::borrow_sync(read.clone()).unwrap();
    let write = UpgradeStreamWriteState::RawParsing(
      BytesMut::with_capacity(
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n".len(),
      ),
      record,
      upgrade.conn.clone(),
      read_cell,
    );
    return Ok(state.resource_table.add(UpgradeStream::new(read, write)));
  }
  let http = http.into_hyper("op_http_upgrade_raw")?;

  let upgrade = http.upgrade()?;

  let read = Rc::new(AsyncRefCell::new(None));
  let read_cell = AsyncRefCell::borrow_sync(read.clone()).unwrap();

  let write = UpgradeStreamWriteState::Parsing(
    BytesMut::with_capacity(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n".len()),
    http,
    upgrade,
    read_cell,
  );

  Ok(state.resource_table.add(UpgradeStream::new(read, write)))
}

#[op2]
#[smi]
pub async fn op_http_upgrade_websocket_next(
  state: Rc<RefCell<OpState>>,
  external: *const c_void,
) -> Result<ResourceId, HttpNextError> {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_upgrade_websocket_next") };
  if let HttpRecordExternal::Raw(record) = http {
    let Some(upgrade) = record.0.borrow().upgrade.clone() else {
      return Err(raw_upgrade_unavailable());
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    *upgrade.websocket_tx.borrow_mut() = Some(tx);
    let (stream, bytes) =
      rx.await.map_err(|_| raw_h1_connection_closed())??;
    return Ok(ws_create_server_stream(
      &mut state.borrow_mut(),
      stream,
      bytes,
    ));
  }
  let upgrade = {
    let http = http.into_hyper("op_http_upgrade_websocket_next")?;
    http.upgrade()?
  };

  let upgraded = upgrade.await?;
  let (stream, bytes) = extract_network_stream(upgraded);

  Ok(ws_create_server_stream(
    &mut state.borrow_mut(),
    stream,
    bytes,
  ))
}

/// Create a server WebSocket from a TCP stream resource (e.g. taken from a
/// node:http TCPWrap handle). This lets ext/node hand off the raw stream
/// without depending on deno_websocket directly.
#[op2(fast)]
#[smi]
pub fn op_http_ws_create_from_stream_resource(
  state: &mut OpState,
  #[smi] stream_rid: ResourceId,
  #[buffer] extra_bytes: &[u8],
) -> Result<ResourceId, HttpNextError> {
  let stream = deno_net::raw::take_network_stream_resource(
    &mut state.resource_table,
    stream_rid,
  )?;
  let read_buf = Bytes::copy_from_slice(extra_bytes);
  Ok(ws_create_server_stream(state, stream, read_buf))
}

#[op2(fast)]
pub fn op_http_set_promise_complete(external: *const c_void, status: u16) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe { take_external!(external, "op_http_set_promise_complete") };
  set_promise_complete(http, status);
}

fn set_promise_complete(http: HttpRecordExternal, status: u16) {
  http.set_status(status);
  if let HttpRecordExternal::Raw(record) = &http {
    record.set_flat_response_body(FlatResponseBody::Empty);
  }
  http.complete();
}

fn set_response_status(http: &HttpRecordExternal, status: u16) {
  http.set_status(status);
}

fn raw_request_method_and_url<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  http: &RawHttpRecord,
) -> v8::Local<'scope, v8::Array> {
  let inner = http.0.borrow();
  let method: v8::Local<v8::Value> = raw_method_v8(scope, &inner.method).into();
  let (authority, path, scheme) = if inner.method.is_connect() {
    (Some(inner.path.as_str()), Cow::Borrowed(""), None)
  } else if let Some((scheme, authority, path)) =
    split_absolute_form_target(&inner.path)
  {
    (Some(authority), Cow::Borrowed(path), Some(scheme))
  } else {
    let host = inner
      .headers
      .get(b"host")
      .and_then(|value| std::str::from_utf8(value).ok())
      .filter(|host| !host.is_empty());
    let path = if matches!(inner.path.as_bytes().first(), Some(b'/' | b'*')) {
      Cow::Borrowed(inner.path.as_str())
    } else {
      Cow::Owned(format!("/{}", inner.path))
    };
    (host, path, None)
  };
  let authority: v8::Local<v8::Value> = if let Some(authority) = authority {
    v8::String::new_from_utf8(
      scope,
      authority.as_bytes(),
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into()
  } else {
    v8::undefined(scope).into()
  };
  let path: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    path.as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();
  let scheme: v8::Local<v8::Value> = if let Some(scheme) = scheme {
    v8::String::new_from_utf8(
      scope,
      scheme.as_bytes(),
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into()
  } else {
    v8::undefined(scope).into()
  };
  let vec = [method, authority, path, scheme];
  v8::Array::new_with_elements(scope, vec.as_slice())
}

fn v8_string_from_bytes<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  bytes: &[u8],
) -> v8::Local<'scope, v8::String> {
  if bytes.is_ascii() {
    return v8::String::new_from_one_byte(
      scope,
      bytes,
      v8::NewStringType::Normal,
    )
    .unwrap();
  }
  v8::String::new_from_utf8(scope, bytes, v8::NewStringType::Normal).unwrap()
}

fn raw_method_v8<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  method: &RawMethod,
) -> v8::Local<'scope, v8::String> {
  match method {
    RawMethod::Get => v8_string_from_bytes(scope, b"GET"),
    RawMethod::Post => v8_string_from_bytes(scope, b"POST"),
    RawMethod::Head => v8_string_from_bytes(scope, b"HEAD"),
    RawMethod::Put => v8_string_from_bytes(scope, b"PUT"),
    RawMethod::Delete => v8_string_from_bytes(scope, b"DELETE"),
    RawMethod::Connect => v8_string_from_bytes(scope, b"CONNECT"),
    RawMethod::Options => v8_string_from_bytes(scope, b"OPTIONS"),
    RawMethod::Trace => v8_string_from_bytes(scope, b"TRACE"),
    RawMethod::Patch => v8_string_from_bytes(scope, b"PATCH"),
    RawMethod::Other(method) => v8_string_from_bytes(scope, method.as_bytes()),
  }
}

fn push_url_from_parts(
  out: &mut String,
  scheme_prefix: &str,
  method: &str,
  authority: &str,
  path: &str,
) {
  out.push_str(scheme_prefix);
  out.push_str(authority);
  if method == "OPTIONS" && path == "*" {
    out.push('/');
  }
  out.push_str(path);
}

fn raw_request_url_v8<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  http: &RawHttpRecord,
) -> v8::Local<'scope, v8::String> {
  let inner = http.0.borrow();
  let scheme_prefix = inner.request_info.scheme.as_bytes();
  if inner.method.is_connect() {
    let mut out = SmallVec::<[u8; 256]>::with_capacity(
      scheme_prefix.len() + inner.path.len(),
    );
    out.extend_from_slice(scheme_prefix);
    out.extend_from_slice(inner.path.as_bytes());
    return v8_string_from_bytes(scope, &out);
  }

  let fallback_host = inner.request_info.fallback_host.as_ref();
  let path = inner.path.as_bytes();
  if !matches!(path.first(), Some(b'/' | b'*'))
    && let Some((scheme, authority, path)) =
      split_absolute_form_target(&inner.path)
  {
    let mut out = SmallVec::<[u8; 256]>::with_capacity(
      scheme.len() + 3 + authority.len() + path.len(),
    );
    out.extend_from_slice(scheme.as_bytes());
    out.extend_from_slice(b"://");
    out.extend_from_slice(authority.as_bytes());
    out.extend_from_slice(path.as_bytes());
    return v8_string_from_bytes(scope, &out);
  }

  let authority = inner
    .headers
    .get(b"host")
    .and_then(|value| std::str::from_utf8(value).ok())
    .filter(|host| !host.is_empty())
    .map(|host| host.as_bytes())
    .unwrap_or_else(|| fallback_host.as_bytes());
  let needs_slash = !matches!(path.first(), Some(b'/' | b'*'));
  let mut out = SmallVec::<[u8; 256]>::with_capacity(
    scheme_prefix.len() + authority.len() + path.len() + 1,
  );
  out.extend_from_slice(scheme_prefix);
  out.extend_from_slice(authority);
  if needs_slash {
    out.push(b'/');
  }
  if inner.method.is_options() && path == b"*" {
    out.push(b'/');
  }
  out.extend_from_slice(path);
  v8_string_from_bytes(scope, &out)
}

fn split_absolute_form_target(target: &str) -> Option<(&str, &str, &str)> {
  let scheme_end = target.find("://")?;
  let scheme = &target[..scheme_end];
  let rest = &target[scheme_end + 3..];
  if scheme.is_empty() || rest.is_empty() {
    return None;
  }

  let path_start = rest.find(['/', '?']).unwrap_or(rest.len());
  let authority = &rest[..path_start];
  if authority.is_empty() {
    return None;
  }

  let path = if path_start == rest.len() {
    "/"
  } else {
    &rest[path_start..]
  };
  Some((scheme, authority, path))
}

#[op2]
pub fn op_http_get_request_method<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::String>
where
  HTTP: HttpPropertyExtractor,
{
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_method") };
  match http {
    HttpRecordExternal::Raw(http) => {
      let inner = http.0.borrow();
      raw_method_v8(scope, &inner.method)
    }
    HttpRecordExternal::Hyper(http) => {
      let request_parts = http.request_parts();
      v8_string_from_bytes(scope, request_parts.method.as_str().as_bytes())
    }
  }
}

#[op2]
pub fn op_http_get_request_url<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::String>
where
  HTTP: HttpPropertyExtractor,
{
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_url") };
  let url = match http {
    HttpRecordExternal::Raw(http) => return raw_request_url_v8(scope, &http),
    HttpRecordExternal::Hyper(http) => {
      let request_info = http.request_info();
      let request_parts = http.request_parts();
      let request_properties = HTTP::request_properties(
        &request_info,
        &request_parts.uri,
        &request_parts.headers,
      );
      let scheme_prefix = if let Some(scheme) = request_parts.uri.scheme_str() {
        Cow::Owned(format!("{scheme}://"))
      } else {
        Cow::Borrowed(request_info.scheme)
      };
      let authority = request_parts
        .uri
        .authority()
        .map(|authority| Cow::Borrowed(authority.as_str()))
        .or(request_properties.authority)
        .unwrap_or_else(|| Cow::Borrowed(request_info.fallback_host.as_ref()));

      if request_parts.method == hyper::Method::CONNECT {
        let mut out =
          String::with_capacity(scheme_prefix.len() + authority.len());
        out.push_str(&scheme_prefix);
        out.push_str(&authority);
        out
      } else {
        let path = match request_parts.uri.path_and_query() {
          Some(path_and_query) => {
            let path = path_and_query.as_str();
            if matches!(path.as_bytes().first(), Some(b'/' | b'*')) {
              Cow::Borrowed(path)
            } else {
              Cow::Owned(format!("/{}", path))
            }
          }
          None => Cow::Borrowed(""),
        };
        let method = request_parts.method.as_str();
        let mut out = String::with_capacity(
          scheme_prefix.len() + authority.len() + path.len() + 1,
        );
        push_url_from_parts(
          &mut out,
          &scheme_prefix,
          method,
          &authority,
          &path,
        );
        out
      }
    }
  };
  v8_string_from_bytes(scope, url.as_bytes())
}

#[op2]
pub fn op_http_get_request_method_and_url<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::Array>
where
  HTTP: HttpPropertyExtractor,
{
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_method_and_url") };
  if let HttpRecordExternal::Raw(http) = http {
    return raw_request_method_and_url(scope, &http);
  }
  let HttpRecordExternal::Hyper(http) = http else {
    unreachable!()
  };
  let request_info = http.request_info();
  let request_parts = http.request_parts();
  let request_properties = HTTP::request_properties(
    &request_info,
    &request_parts.uri,
    &request_parts.headers,
  );

  let method: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    request_parts.method.as_str().as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();

  let scheme: v8::Local<v8::Value> = match request_parts.uri.scheme_str() {
    Some(scheme) => v8::String::new_from_utf8(
      scope,
      scheme.as_bytes(),
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into(),
    None => v8::undefined(scope).into(),
  };

  let authority: v8::Local<v8::Value> =
    if let Some(authority) = request_parts.uri.authority() {
      v8::String::new_from_utf8(
        scope,
        authority.as_str().as_ref(),
        v8::NewStringType::Normal,
      )
      .unwrap()
      .into()
    } else if let Some(authority) = request_properties.authority {
      v8::String::new_from_utf8(
        scope,
        authority.as_bytes(),
        v8::NewStringType::Normal,
      )
      .unwrap()
      .into()
    } else {
      v8::undefined(scope).into()
    };

  // Only extract the path part - we handle authority elsewhere
  let path = match request_parts.uri.path_and_query() {
    Some(path_and_query) => {
      let path = path_and_query.as_str();
      if matches!(path.as_bytes().first(), Some(b'/' | b'*')) {
        Cow::Borrowed(path)
      } else {
        Cow::Owned(format!("/{}", path))
      }
    }
    None => Cow::Borrowed(""),
  };

  let path: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    path.as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();

  let vec = [method, authority, path, scheme];
  v8::Array::new_with_elements(scope, vec.as_slice())
}

fn raw_request_remote_addr<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  http: &RawHttpRecord,
) -> v8::Local<'scope, v8::Array> {
  let inner = http.0.borrow();
  let (peer_ip, peer_port) = match &inner.client_addr {
    Some(client_addr) => {
      let addr: std::net::SocketAddr =
        std::str::from_utf8(client_addr).unwrap().parse().unwrap();
      (Rc::from(format!("{}", addr.ip())), Some(addr.port() as u32))
    }
    _ => (
      inner.request_info.peer_address.clone(),
      inner.request_info.peer_port,
    ),
  };
  let peer_ip: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    peer_ip.as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();
  let peer_port: v8::Local<v8::Value> = match peer_port {
    Some(port) => v8::Number::new(scope, port.into()).into(),
    None => v8::undefined(scope).into(),
  };

  let vec = [peer_ip, peer_port];
  v8::Array::new_with_elements(scope, vec.as_slice())
}

#[op2]
pub fn op_http_get_request_remote_addr<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::Array>
where
  HTTP: HttpPropertyExtractor,
{
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_remote_addr") };
  if let HttpRecordExternal::Raw(http) = http {
    return raw_request_remote_addr(scope, &http);
  }
  let HttpRecordExternal::Hyper(http) = http else {
    unreachable!()
  };
  let request_info = http.request_info();
  let (peer_ip, peer_port) = match &*http.client_addr() {
    Some(client_addr) => {
      let addr: std::net::SocketAddr =
        client_addr.to_str().unwrap().parse().unwrap();
      (Rc::from(format!("{}", addr.ip())), Some(addr.port() as u32))
    }
    _ => (request_info.peer_address.clone(), request_info.peer_port),
  };

  let peer_ip: v8::Local<v8::Value> = v8::String::new_from_utf8(
    scope,
    peer_ip.as_bytes(),
    v8::NewStringType::Normal,
  )
  .unwrap()
  .into();

  let peer_port: v8::Local<v8::Value> = match peer_port {
    Some(port) => v8::Number::new(scope, port.into()).into(),
    None => v8::undefined(scope).into(),
  };

  let vec = [peer_ip, peer_port];
  v8::Array::new_with_elements(scope, vec.as_slice())
}

#[op2]
pub fn op_http_get_request_header<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
  #[string(onebyte)] name: Cow<'_, [u8]>,
) -> v8::Local<'scope, v8::Value> {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_header") };
  match http {
    HttpRecordExternal::Hyper(http) => {
      let Ok(name) = HeaderName::from_bytes(&name) else {
        return v8::null(scope).into();
      };
      let separator = request_header_value_separator(name.as_ref());
      let request_parts = http.request_parts();
      let mut values = request_parts.headers.get_all(name).iter();
      let Some(first) = values.next() else {
        return v8::null(scope).into();
      };
      let Some(next) = values.next() else {
        return v8::String::new_from_one_byte(
          scope,
          first.as_bytes(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into();
      };
      let mut value = Vec::with_capacity(
        first.as_bytes().len() + separator.len() + next.as_bytes().len(),
      );
      value.extend_from_slice(first.as_bytes());
      value.extend_from_slice(separator);
      value.extend_from_slice(next.as_bytes());
      for next in values {
        value.extend_from_slice(separator);
        value.extend_from_slice(next.as_bytes());
      }
      v8::String::new_from_one_byte(scope, &value, v8::NewStringType::Normal)
        .unwrap()
        .into()
    }
    HttpRecordExternal::Raw(http) => {
      let inner = http.0.borrow();
      raw_request_header_to_v8(scope, &inner.headers, &name)
    }
  }
}

#[op2]
pub fn op_http_get_request_headers<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::Array> {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_headers") };
  if let HttpRecordExternal::Raw(http) = http {
    let inner = http.0.borrow();
    let mut vec: SmallVec<[v8::Local<v8::Value>; 32]> =
      SmallVec::with_capacity(inner.headers.len() * 2);
    let mut cookies: Option<Vec<&[u8]>> = None;
    for header in inner.headers.iter() {
      if header.name.eq_ignore_ascii_case(b"cookie") {
        if let Some(ref mut cookies) = cookies {
          cookies.push(header.value);
        } else {
          cookies = Some(vec![header.value]);
        }
        continue;
      }
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          header.name,
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          header.value,
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
    }
    if let Some(cookies) = cookies {
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          b"cookie",
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          cookies
            .join(request_header_value_separator(COOKIE.as_ref()))
            .as_ref(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
    }
    return v8::Array::new_with_elements(scope, vec.as_slice());
  }
  let HttpRecordExternal::Hyper(http) = http else {
    unreachable!()
  };
  let headers = &http.request_parts().headers;
  // Two slots for each header key/value pair
  let mut vec: SmallVec<[v8::Local<v8::Value>; 32]> =
    SmallVec::with_capacity(headers.len() * 2);

  let mut cookies: Option<Vec<&[u8]>> = None;
  for (name, value) in headers {
    if name == COOKIE {
      if let Some(ref mut cookies) = cookies {
        cookies.push(value.as_bytes());
      } else {
        cookies = Some(vec![value.as_bytes()]);
      }
    } else {
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          name.as_ref(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
      vec.push(
        v8::String::new_from_one_byte(
          scope,
          value.as_bytes(),
          v8::NewStringType::Normal,
        )
        .unwrap()
        .into(),
      );
    }
  }

  // We treat cookies specially, because we don't want them to get them
  // mangled by the `Headers` object in JS. What we do is take all cookie
  // headers and concat them into a single cookie header, separated by
  // semicolons.
  // TODO(mmastrac): This should probably happen on the JS side on-demand
  if let Some(cookies) = cookies {
    vec.push(
      v8::String::new_external_onebyte_static(scope, COOKIE.as_ref())
        .unwrap()
        .into(),
    );
    vec.push(
      v8::String::new_from_one_byte(
        scope,
        cookies
          .join(request_header_value_separator(COOKIE.as_ref()))
          .as_ref(),
        v8::NewStringType::Normal,
      )
      .unwrap()
      .into(),
    );
  }

  v8::Array::new_with_elements(scope, vec.as_slice())
}

/// Try to drain the entire request body without blocking.
/// Returns the bytes as a Uint8Array iff the whole body is
/// already buffered in hyper; returns `null` otherwise, in
/// which case the JS caller falls through to the streaming
/// `op_http_read_request_body` path. The body is left intact
/// on the `null` branch.
#[op2]
#[buffer]
pub fn op_http_try_take_full_request_body(
  external: *const c_void,
) -> Option<Vec<u8>> {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_try_take_full_request_body") };
  match http {
    HttpRecordExternal::Hyper(http) => http.try_take_full_request_body(),
    HttpRecordExternal::Raw(http) => http.try_take_full_request_body(),
  }
}

#[op2]
pub fn op_http_try_take_full_request_body_text<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  external: *const c_void,
) -> v8::Local<'scope, v8::Value> {
  let http =
    // SAFETY: op is called with external.
    unsafe {
      clone_external!(external, "op_http_try_take_full_request_body_text")
    };
  let bytes = match http {
    HttpRecordExternal::Hyper(http) => http.try_take_full_request_body(),
    HttpRecordExternal::Raw(http) => http.try_take_full_request_body(),
  };
  let Some(bytes) = bytes else {
    return v8::null(scope).into();
  };
  if bytes.is_ascii() {
    return v8::String::new_from_one_byte(
      scope,
      &bytes,
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into();
  }
  if let Ok(text) = std::str::from_utf8(&bytes) {
    return v8::String::new_from_utf8(
      scope,
      text.as_bytes(),
      v8::NewStringType::Normal,
    )
    .unwrap()
    .into();
  }
  let text = String::from_utf8_lossy(&bytes);
  v8::String::new_from_utf8(scope, text.as_bytes(), v8::NewStringType::Normal)
    .unwrap()
    .into()
}

#[op2(fast)]
#[smi]
pub fn op_http_read_request_body(
  state: Rc<RefCell<OpState>>,
  external: *const c_void,
) -> ResourceId {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_read_request_body") };
  if let HttpRecordExternal::Raw(http) = http {
    let rid = match http.take_request_body() {
      Some(body_resource) => {
        state.borrow_mut().resource_table.add_rc_dyn(body_resource)
      }
      None => ResourceId::MAX,
    };
    return rid;
  }
  let Ok(http) = http.into_hyper("op_http_read_request_body") else {
    return 0;
  };
  let rid = match http.take_request_body() {
    Some(incoming) => {
      let body_resource = Rc::new(HttpRequestBody::new(incoming));
      state.borrow_mut().resource_table.add_rc(body_resource)
    }
    _ => {
      // This should not be possible, but rather than panicking we'll return an invalid
      // resource value to JavaScript.
      ResourceId::MAX
    }
  };
  http.put_resource(HttpRequestBodyAutocloser::new(rid, state.clone()));
  rid
}

#[op2(fast)]
pub fn op_http_set_response_header(
  external: *const c_void,
  #[string(onebyte)] name: Cow<'_, [u8]>,
  #[string(onebyte)] value: Cow<'_, [u8]>,
) {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_set_response_header") };
  append_response_header(&http, name, value);
}

fn append_response_header(
  http: &HttpRecordExternal,
  name: Cow<'_, [u8]>,
  value: Cow<'_, [u8]>,
) {
  let name = match name {
    Cow::Borrowed(bytes) => bytes.to_vec(),
    Cow::Owned(bytes) => bytes,
  };
  let value = match value {
    Cow::Borrowed(bytes) => bytes.to_vec(),
    Cow::Owned(bytes) => bytes,
  };
  http.append_response_header(name, value);
}

#[op2(fast)]
pub fn op_http_set_response_headers(
  scope: &mut v8::PinScope<'_, '_>,
  external: *const c_void,
  headers: v8::Local<v8::Array>,
) {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_set_response_headers") };
  let len = headers.length();
  for i in 0..len {
    let item = headers.get_index(scope, i).unwrap();
    let pair = v8::Local::<v8::Array>::try_from(item).unwrap();
    let name = pair.get_index(scope, 0).unwrap();
    let value = pair.get_index(scope, 1).unwrap();

    let v8_name = ByteString::from_v8(scope, name).unwrap();
    let v8_value = ByteString::from_v8(scope, value).unwrap();
    http.append_response_header(v8_name.into(), v8_value.into());
  }
}

#[op2]
pub fn op_http_set_response_trailers(
  external: *const c_void,
  #[scoped] trailers: Vec<(ByteString, ByteString)>,
) {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_set_response_trailers") };
  let http = match http {
    HttpRecordExternal::Hyper(http) => http,
    HttpRecordExternal::Raw(record) => {
      record.set_response_trailers(
        trailers
          .into_iter()
          .map(|(name, value)| RawHeader {
            name: name.into(),
            value: value.into(),
          })
          .collect(),
      );
      return;
    }
  };
  let mut trailer_map: HeaderMap = HeaderMap::with_capacity(trailers.len());
  for (name, value) in trailers {
    // These are valid latin-1 strings
    let name = HeaderName::from_bytes(&name).unwrap();
    // SAFETY: These are valid latin-1 strings
    let value = unsafe { HeaderValue::from_maybe_shared_unchecked(value) };
    trailer_map.append(name, value);
  }
  *http.trailers() = Some(trailer_map);
}

fn is_request_compressible(
  length: Option<usize>,
  headers: &HeaderMap,
) -> Compression {
  if let Some(length) = length {
    // By the time we add compression headers and Accept-Encoding, it probably doesn't make sense
    // to compress stuff that's smaller than this.
    if length < 64 {
      return Compression::None;
    }
  }

  let Some(accept_encoding) = headers.get(ACCEPT_ENCODING) else {
    return Compression::None;
  };

  match accept_encoding.to_str() {
    // Firefox and Chrome send this -- no need to parse
    Ok("gzip, deflate, br") => return Compression::Brotli,
    Ok("gzip, deflate, br, zstd") => return Compression::Brotli,
    Ok("gzip") => return Compression::GZip,
    Ok("br") => return Compression::Brotli,
    _ => (),
  }

  // Fall back to the expensive parser
  let accepted = fly_accept_encoding::encodings_iter_http_1(headers);
  preferred_supported_compression(accepted)
}

fn is_response_compressible(headers: &HeaderMap) -> bool {
  if let Some(content_type) = headers.get(CONTENT_TYPE) {
    if !is_content_compressible(content_type) {
      return false;
    }
  } else {
    return false;
  }
  if headers.contains_key(CONTENT_ENCODING) {
    return false;
  }
  if headers.contains_key(CONTENT_RANGE) {
    return false;
  }
  if let Some(cache_control) = headers.get(CACHE_CONTROL)
    && let Ok(s) = std::str::from_utf8(cache_control.as_bytes())
    && let Some(no_transform) = crate::cache_control_has_no_transform(s)
    && no_transform
  {
    return false;
  }
  true
}

fn modify_compressibility_from_response(
  compression: Compression,
  headers: &mut HeaderMap,
) -> Compression {
  if compression == Compression::None {
    return Compression::None;
  }
  if !is_response_compressible(headers) {
    return Compression::None;
  }
  ensure_vary_accept_encoding(headers);
  let encoding = match compression {
    Compression::Brotli => "br",
    Compression::GZip => "gzip",
    _ => unreachable!(),
  };
  weaken_etag(headers);
  headers.remove(CONTENT_LENGTH);
  headers.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
  compression
}

/// If the user provided a ETag header for uncompressed data, we need to ensure it is a
/// weak Etag header ("W/").
fn weaken_etag(hmap: &mut HeaderMap) {
  if let Some(etag) = hmap.get_mut(hyper::header::ETAG)
    && !etag.as_bytes().starts_with(b"W/")
  {
    let mut v = Vec::with_capacity(etag.as_bytes().len() + 2);
    v.extend(b"W/");
    v.extend(etag.as_bytes());
    *etag = v.try_into().unwrap();
  }
}

fn ensure_vary_accept_encoding(hmap: &mut HeaderMap) {
  if let Some(v) = hmap.get_mut(hyper::header::VARY)
    && let Ok(s) = v.to_str()
  {
    if !s.to_lowercase().contains("accept-encoding") {
      *v = format!("Accept-Encoding, {s}").try_into().unwrap()
    }
    return;
  }
  hmap.insert(
    hyper::header::VARY,
    HeaderValue::from_static("Accept-Encoding"),
  );
}

/// Sets the appropriate response body. Use `force_instantiate_body` if you need
/// to ensure that the response is cleaned up correctly (eg: for resources).
fn set_response(
  http: Rc<HttpRecord>,
  length: Option<usize>,
  status: u16,
  force_instantiate_body: bool,
  response_fn: impl FnOnce(Compression) -> ResponseBytesInner,
) {
  // The request may have been cancelled by this point and if so, there's no need for us to
  // do all of this work to send the response.
  if !http.cancelled() {
    let compression =
      is_request_compressible(length, &http.request_parts().headers);
    let mut response_headers =
      std::cell::RefMut::map(http.response_parts(), |this| &mut this.headers);
    let compression =
      modify_compressibility_from_response(compression, &mut response_headers);
    drop(response_headers);
    http.set_response_body(response_fn(compression));

    if let Ok(code) = StatusCode::from_u16(status) {
      http.response_parts().status = code;
      http.otel_info_set_status(status);
    }
  } else if force_instantiate_body {
    response_fn(Compression::None).abort();
  }

  http.complete();
}

fn set_static_response_vec(
  http: HttpRecordExternal,
  bytes: Vec<u8>,
  status: u16,
) {
  if !http.cancelled() {
    match &http {
      HttpRecordExternal::Hyper(record) => {
        let compression = is_request_compressible(
          Some(bytes.len()),
          &record.request_parts().headers,
        );
        let mut response_headers =
          std::cell::RefMut::map(record.response_parts(), |this| {
            &mut this.headers
          });
        let compression = modify_compressibility_from_response(
          compression,
          &mut response_headers,
        );
        drop(response_headers);
        set_response_status(&http, status);
        if compression == Compression::None {
          http.set_flat_response_body(FlatResponseBody::Bytes(BufView::from(
            bytes,
          )));
        } else {
          http.set_response_body(ResponseBytesInner::from_vec(
            compression,
            bytes,
          ));
        }
      }
      HttpRecordExternal::Raw(record) => {
        let compression = record.prepare_stream_compression(Some(bytes.len()));
        set_response_status(&http, status);
        if compression == Compression::None {
          http.set_flat_response_body(FlatResponseBody::Bytes(BufView::from(
            bytes,
          )));
        } else {
          record.set_stream_response_body(
            ResponseBytesInner::from_vec(compression, bytes),
            None,
          );
        }
      }
    }
  }
  http.complete();
}

fn set_static_response_bufview(
  http: HttpRecordExternal,
  buffer: JsBuffer,
  status: u16,
) {
  if !http.cancelled() {
    match &http {
      HttpRecordExternal::Hyper(record) => {
        let compression = is_request_compressible(
          Some(buffer.len()),
          &record.request_parts().headers,
        );
        let mut response_headers =
          std::cell::RefMut::map(record.response_parts(), |this| {
            &mut this.headers
          });
        let compression = modify_compressibility_from_response(
          compression,
          &mut response_headers,
        );
        drop(response_headers);
        set_response_status(&http, status);
        let buffer = BufView::from(buffer);
        if compression == Compression::None {
          http.set_flat_response_body(FlatResponseBody::Bytes(buffer));
        } else {
          http.set_response_body(ResponseBytesInner::from_bufview(
            compression,
            buffer,
          ));
        }
      }
      HttpRecordExternal::Raw(record) => {
        let compression = record.prepare_stream_compression(Some(buffer.len()));
        set_response_status(&http, status);
        let buffer = BufView::from(buffer);
        if compression == Compression::None {
          http.set_flat_response_body(FlatResponseBody::Bytes(buffer));
        } else {
          record.set_stream_response_body(
            ResponseBytesInner::from_bufview(compression, buffer),
            None,
          );
        }
      }
    }
  }
  http.complete();
}

fn set_static_empty_response(http: HttpRecordExternal, status: u16) {
  if !http.cancelled() {
    set_response_status(&http, status);
    http.set_flat_response_body(FlatResponseBody::Empty);
  }
  http.complete();
}

#[op2(fast)]
pub fn op_http_set_response_body_static_with_default_header<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  external: *const c_void,
  body: v8::Local<'s, v8::Value>,
  status: u16,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
      take_external!(
        external,
        "op_http_set_response_body_static_with_default_header"
      )
  };
  http.set_default_text_content_type();
  set_static_response_body_from_v8(scope, http, body, status);
}

fn set_static_response_body_from_v8<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  http: HttpRecordExternal,
  body: v8::Local<'s, v8::Value>,
  status: u16,
) {
  if let Ok(text) = v8::Local::<v8::String>::try_from(body) {
    if text.length() != 0 {
      let text = v8_string_to_utf8_bytes(scope, text);
      set_static_response_vec(http, text, status);
    } else {
      set_static_empty_response(http, status);
    }
  } else if let Ok(buffer) =
    deno_core::serde_v8::from_v8::<JsBuffer>(scope, body)
  {
    if !buffer.is_empty() {
      set_static_response_bufview(http, buffer, status);
    } else {
      set_static_empty_response(http, status);
    }
  } else {
    debug_assert!(false, "expected static response body");
    set_static_empty_response(http, status);
  }
}

fn direct_response_body_from_v8<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  body: v8::Local<'s, v8::Value>,
) -> Option<DirectResponseBody> {
  if body.is_null_or_undefined() {
    Some(DirectResponseBody::Empty)
  } else if let Ok(text) = v8::Local::<v8::String>::try_from(body) {
    if text.length() == 0 {
      Some(DirectResponseBody::Empty)
    } else {
      Some(DirectResponseBody::Bytes(BufView::from(
        v8_string_to_utf8_bytes(scope, text),
      )))
    }
  } else {
    let buffer = deno_core::serde_v8::from_v8::<JsBuffer>(scope, body).ok()?;
    if buffer.is_empty() {
      Some(DirectResponseBody::Empty)
    } else {
      Some(DirectResponseBody::Bytes(BufView::from(buffer)))
    }
  }
}

fn direct_response_headers_from_v8<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  headers: v8::Local<'s, v8::Array>,
) -> Option<DirectResponseHeaders> {
  let len = headers.length();
  if len == 0 {
    return Some(DirectResponseHeaders::None);
  }

  let mut out = Vec::with_capacity(len as usize);
  for i in 0..len {
    let item = headers.get_index(scope, i)?;
    let pair = v8::Local::<v8::Array>::try_from(item).ok()?;
    let name = pair.get_index(scope, 0)?;
    let value = pair.get_index(scope, 1)?;
    let name = ByteString::from_v8(scope, name).ok()?;
    let value = ByteString::from_v8(scope, value).ok()?;
    out.push((name.into(), value.into()));
  }
  Some(DirectResponseHeaders::List(out))
}

#[op2(fast)]
pub fn op_http_new_response_native_static<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  body: v8::Local<'s, v8::Value>,
  status: u16,
  #[smi] header_kind: u32,
  #[string(onebyte)] content_type: Cow<'_, [u8]>,
) -> *const c_void {
  let Some(body) = direct_response_body_from_v8(scope, body) else {
    return std::ptr::null();
  };
  let headers = match header_kind {
    0 => DirectResponseHeaders::None,
    1 => DirectResponseHeaders::DefaultText,
    2 => DirectResponseHeaders::ContentType(content_type.into_owned()),
    _ => return std::ptr::null(),
  };
  NativeResponseCell::new(DirectResponse {
    status,
    headers,
    body,
  })
}

#[op2(fast)]
pub fn op_http_new_response_native_headers<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  body: v8::Local<'s, v8::Value>,
  status: u16,
  headers: v8::Local<'s, v8::Array>,
) -> *const c_void {
  let Some(body) = direct_response_body_from_v8(scope, body) else {
    return std::ptr::null();
  };
  let Some(headers) = direct_response_headers_from_v8(scope, headers) else {
    return std::ptr::null();
  };
  NativeResponseCell::new(DirectResponse {
    status,
    headers,
    body,
  })
}

#[op2(fast)]
pub fn op_http_drop_response_native(response: *const c_void) {
  // SAFETY: this drops the JS-owned NativeResponseCell strong reference.
  unsafe { NativeResponseCell::drop(response) };
}

#[op2(fast)]
pub fn op_http_set_response_native(
  external: *const c_void,
  response: *const c_void,
) -> bool {
  // SAFETY: response is a NativeResponseCell pointer created by
  // op_http_new_response_native_*.
  let Some(response) = (unsafe { NativeResponseCell::take(response) }) else {
    return false;
  };
  let http =
    // SAFETY: external is consumed when this op commits the response.
    unsafe { take_external!(external, "op_http_set_response_native") };
  set_direct_response(http, response);
  true
}

#[op2(fast)]
pub fn op_http_set_response_body_static_with_header<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  external: *const c_void,
  body: v8::Local<'s, v8::Value>,
  status: u16,
  #[string(onebyte)] name: Cow<'_, [u8]>,
  #[string(onebyte)] value: Cow<'_, [u8]>,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
      take_external!(external, "op_http_set_response_body_static_with_header")
    };
  append_response_header(&http, name, value);
  set_static_response_body_from_v8(scope, http, body, status);
}

#[op2(fast)]
pub fn op_http_set_response_body_static_with_content_type<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  external: *const c_void,
  body: v8::Local<'s, v8::Value>,
  status: u16,
  #[string(onebyte)] value: Cow<'_, [u8]>,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
      take_external!(
        external,
        "op_http_set_response_body_static_with_content_type"
      )
    };
  let value = match value {
    Cow::Borrowed(bytes) => bytes.to_vec(),
    Cow::Owned(bytes) => bytes,
  };
  http.set_content_type(value);
  set_static_response_body_from_v8(scope, http, body, status);
}

#[op2(fast)]
pub fn op_http_get_request_cancelled(external: *const c_void) -> bool {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_get_request_cancelled") };
  http.cancelled()
}

#[op2(fast)]
pub fn op_http_is_raw_request(external: *const c_void) -> bool {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_is_raw_request") };
  matches!(http, HttpRecordExternal::Raw(_))
}

#[op2]
pub async fn op_http_request_on_cancel(external: *const c_void) -> bool {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_request_on_cancel") };
  let http = match http {
    HttpRecordExternal::Hyper(http) => http,
    HttpRecordExternal::Raw(http) => return http.request_cancelled().await,
  };
  let (tx, rx) = tokio::sync::oneshot::channel();

  http.on_cancel(tx);
  drop(http);

  rx.await.is_ok()
}

/// Returned promise resolves when body streaming finishes.
/// Call [`op_http_close_after_finish`] when done with the external.
#[op2]
pub async fn op_http_set_response_body_resource(
  state: Rc<RefCell<OpState>>,
  external: *const c_void,
  #[smi] stream_rid: ResourceId,
  auto_close: bool,
  status: u16,
) -> Result<bool, HttpNextError> {
  let http =
    // SAFETY: op is called with external.
    unsafe { clone_external!(external, "op_http_set_response_body_resource") };
  if let HttpRecordExternal::Raw(record) = http {
    let resource = {
      let mut state = state.borrow_mut();
      if auto_close {
        state.resource_table.take_any(stream_rid)?
      } else {
        state.resource_table.get_any(stream_rid)?
      }
    };
    record.set_status(status);
    let (lower, upper) = resource.size_hint();
    let exact_length = upper.filter(|upper| *upper == lower);
    record.set_stream_response_body_resource(
      exact_length,
      exact_length.and_then(|length| usize::try_from(length).ok()),
      resource,
      auto_close,
    );
    record.complete();
    return Ok(record.response_body_finished().await);
  }
  let http = http.into_hyper("op_http_set_response_body_resource")?;

  // IMPORTANT: We might end up requiring the OpState lock in set_response if we need to drop the request
  // body resource so we _cannot_ hold the OpState lock longer than necessary.

  // If the stream is auto_close, we will hold the last ref to it until the response is complete.
  // TODO(mmastrac): We should be using the same auto-close functionality rather than removing autoclose resources.
  // It's possible things could fail elsewhere if code expects the rid to continue existing after the response has been
  // returned.
  let resource = {
    let mut state = state.borrow_mut();
    if auto_close {
      state.resource_table.take_any(stream_rid)?
    } else {
      state.resource_table.get_any(stream_rid)?
    }
  };

  *http.needs_close_after_finish() = true;

  set_response(
    http.clone(),
    resource.size_hint().1.map(|s| s as usize),
    status,
    true,
    move |compression| {
      ResponseBytesInner::from_resource(compression, resource, auto_close)
    },
  );

  Ok(http.response_body_finished().await)
}

#[op2(fast)]
pub fn op_http_close_after_finish(external: *const c_void) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe { take_external!(external, "op_http_close_after_finish") };
  if matches!(http, HttpRecordExternal::Raw(_)) {
    return;
  }
  let Ok(http) = http.into_hyper("op_http_close_after_finish") else {
    return;
  };
  http.close_after_finish();
}

#[op2(fast)]
pub fn op_http_set_response_body_text(
  external: *const c_void,
  #[string] text: String,
  status: u16,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe { take_external!(external, "op_http_set_response_body_text") };
  if !text.is_empty() {
    set_static_response_vec(http, text.into_bytes(), status);
  } else {
    set_static_empty_response(http, status);
  }
}

#[op2]
pub fn op_http_set_response_body_bytes(
  external: *const c_void,
  #[buffer] buffer: JsBuffer,
  status: u16,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe { take_external!(external, "op_http_set_response_body_bytes") };
  if !buffer.is_empty() {
    set_static_response_bufview(http, buffer, status);
  } else {
    set_static_empty_response(http, status);
  }
}

#[op2]
pub fn op_http_set_response_body_text_with_headers(
  external: *const c_void,
  #[string] text: String,
  status: u16,
  #[scoped] headers: Vec<(ByteString, ByteString)>,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
      take_external!(external, "op_http_set_response_body_text_with_headers")
    };
  for (name, value) in headers {
    http.append_response_header(name.into(), value.into());
  }
  if !text.is_empty() {
    set_static_response_vec(http, text.into_bytes(), status);
  } else {
    set_static_empty_response(http, status);
  }
}

#[op2]
pub fn op_http_set_response_body_bytes_with_headers(
  external: *const c_void,
  #[buffer] buffer: JsBuffer,
  status: u16,
  #[scoped] headers: Vec<(ByteString, ByteString)>,
) {
  let http =
    // SAFETY: external is deleted before calling this op.
    unsafe {
      take_external!(external, "op_http_set_response_body_bytes_with_headers")
    };
  for (name, value) in headers {
    http.append_response_header(name.into(), value.into());
  }
  if !buffer.is_empty() {
    set_static_response_bufview(http, buffer, status);
  } else {
    set_static_empty_response(http, status);
  }
}

struct RawParsedRequest {
  version: h1::Version,
  method: RawMethod,
  path: String,
  headers: RawRequestHeaders,
  keep_alive: bool,
  expect_continue: bool,
  has_body: bool,
  request_size: u64,
  request_body_len: Option<u64>,
  upgrade: Option<h1::UpgradeKind>,
}

fn raw_request_target_to_string(target: &[u8]) -> String {
  // `h1::Request::target` comes from `httparse::Request::path`, which is
  // already validated as UTF-8 before the h1 parser exposes it as bytes.
  // SAFETY: see above; the parser has already accepted this as UTF-8.
  unsafe { std::str::from_utf8_unchecked(target) }.to_owned()
}

fn raw_request_from_h1(
  request: h1::Request<'_>,
  store_request: bool,
) -> RawParsedRequest {
  let headers = if store_request {
    RawRequestHeaders::from_h1(request.headers)
  } else {
    RawRequestHeaders::empty()
  };

  RawParsedRequest {
    version: request.version,
    method: RawMethod::from_bytes(request.method),
    path: if store_request {
      raw_request_target_to_string(request.target)
    } else {
      String::new()
    },
    headers,
    keep_alive: request.keep_alive,
    expect_continue: request.expect_continue,
    has_body: !matches!(
      request.body,
      h1::BodyKind::Empty | h1::BodyKind::Upgrade
    ),
    request_size: match request.body {
      h1::BodyKind::ContentLength(length) => length,
      _ => 0,
    },
    request_body_len: match request.body {
      h1::BodyKind::ContentLength(length) => Some(length),
      _ => None,
    },
    upgrade: request.upgrade,
  }
}

fn h1_reason_for(status: u16) -> &'static [u8] {
  StatusCode::from_u16(status)
    .ok()
    .and_then(|status| status.canonical_reason())
    .unwrap_or("")
    .as_bytes()
}

const RAW_H1_DATE_LEN: usize = 29;

#[derive(Clone, Copy)]
struct RawH1ResponseContext {
  version: h1::Version,
  keep_alive: bool,
  head: bool,
}

#[derive(Clone, Copy)]
struct RawH1DateCache {
  next_update: SystemTime,
  pos: usize,
  value: [u8; RAW_H1_DATE_LEN],
}

thread_local! {
  static RAW_H1_DATE_CACHE: RefCell<RawH1DateCache> =
    const { RefCell::new(RawH1DateCache {
      next_update: SystemTime::UNIX_EPOCH,
      pos: 0,
      value: [0; RAW_H1_DATE_LEN],
    }) };
}

fn raw_h1_date() -> [u8; RAW_H1_DATE_LEN] {
  let now = SystemTime::now();
  RAW_H1_DATE_CACHE.with_borrow_mut(|cache| {
    if now > cache.next_update {
      cache.pos = 0;
      write!(cache, "{}", httpdate::HttpDate::from(now))
        .expect("http date exceeded raw H1 date buffer");
      assert_eq!(cache.pos, RAW_H1_DATE_LEN);
      cache.next_update = now + Duration::from_secs(1);
    }
    cache.value
  })
}

impl fmt::Write for RawH1DateCache {
  fn write_str(&mut self, value: &str) -> fmt::Result {
    let end = self.pos + value.len();
    if end > self.value.len() {
      return Err(fmt::Error);
    }
    self.value[self.pos..end].copy_from_slice(value.as_bytes());
    self.pos = end;
    Ok(())
  }
}

fn raw_response_from_direct_response(
  response: DirectResponse,
) -> (RawResponseParts, FlatResponseBody) {
  let mut parts = RawResponseParts {
    status: response.status,
    headers: Vec::new(),
    trailers: Vec::new(),
    default_text_content_type: false,
    content_type: None,
  };
  match response.headers {
    DirectResponseHeaders::None => {}
    DirectResponseHeaders::DefaultText => {
      parts.default_text_content_type = true;
    }
    DirectResponseHeaders::ContentType(value) => {
      if value.eq_ignore_ascii_case(b"text/plain;charset=UTF-8") {
        parts.default_text_content_type = true;
      } else {
        parts.content_type = Some(value);
      }
    }
    DirectResponseHeaders::List(headers) => {
      parts.headers = headers
        .into_iter()
        .map(|(name, value)| RawHeader { name, value })
        .collect();
    }
  }
  let body = match response.body {
    DirectResponseBody::Empty => FlatResponseBody::Empty,
    DirectResponseBody::Bytes(body) => FlatResponseBody::Bytes(body),
  };
  (parts, body)
}

fn set_direct_response(http: HttpRecordExternal, response: DirectResponse) {
  if !http.cancelled() {
    set_response_status(&http, response.status);
    match response.headers {
      DirectResponseHeaders::None => {}
      DirectResponseHeaders::DefaultText => {
        http.set_default_text_content_type();
      }
      DirectResponseHeaders::ContentType(value) => {
        http.set_content_type(value);
      }
      DirectResponseHeaders::List(headers) => {
        for (name, value) in headers {
          http.append_response_header(name, value);
        }
      }
    }
    match response.body {
      DirectResponseBody::Empty => {
        http.set_flat_response_body(FlatResponseBody::Empty);
      }
      DirectResponseBody::Bytes(body) => {
        http.set_flat_response_body(FlatResponseBody::Bytes(body));
      }
    }
  }
  http.complete();
}

async fn write_h1_flat_response<I>(
  conn: &mut h1::SharedConn<I>,
  scratch: &mut h1::SharedScratch,
  version: h1::Version,
  parts: RawResponseParts,
  body: FlatResponseBody,
  keep_alive: bool,
  head: bool,
) -> Result<(), h1::Error>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let date = raw_h1_date();
  let should_chunk = version == h1::Version::Http11
    && !head
    && raw_response_needs_chunked(&parts);
  if should_chunk {
    let headers = raw_response_headers(&parts, &date);
    let trailers = raw_response_trailers(&parts);
    conn
      .start_chunked_response_with_scratch(
        scratch,
        h1::ResponseHead {
          version,
          status: parts.status,
          reason: h1_reason_for(parts.status),
          headers: &headers,
          keep_alive,
        },
      )
      .await?;
    if let FlatResponseBody::Bytes(body) = &body
      && !body.is_empty()
    {
      conn
        .write_response_chunk_with_scratch(scratch, body)
        .await?;
    }
    conn
      .finish_response_with_scratch(scratch, trailers.as_slice())
      .await?;
    return Ok(());
  }
  if !head
    && version == h1::Version::Http11
    && parts.status == StatusCode::OK.as_u16()
    && parts.default_text_content_type
    && parts.headers.is_empty()
  {
    let body = match &body {
      FlatResponseBody::Empty => &[][..],
      FlatResponseBody::Bytes(body) => body.as_ref(),
    };
    if conn
      .try_write_default_text_response_with_scratch(
        scratch, &date, body, keep_alive,
      )
      .await?
    {
      return Ok(());
    }
  }
  if !head
    && version == h1::Version::Http11
    && parts.status == StatusCode::OK.as_u16()
    && !parts.default_text_content_type
    && parts.headers.is_empty()
    && let Some(content_type) = &parts.content_type
  {
    let body = match &body {
      FlatResponseBody::Empty => &[][..],
      FlatResponseBody::Bytes(body) => body.as_ref(),
    };
    if conn
      .try_write_content_type_response_with_scratch(
        scratch,
        content_type,
        &date,
        body,
        keep_alive,
      )
      .await?
    {
      return Ok(());
    }
  }

  let headers = raw_response_headers(&parts, &date);
  let response_body = match &body {
    _ if head => h1::ResponseBody::Head(Some(match &body {
      FlatResponseBody::Empty => 0,
      FlatResponseBody::Bytes(body) => body.len() as u64,
    })),
    FlatResponseBody::Empty => h1::ResponseBody::Empty,
    FlatResponseBody::Bytes(body) => h1::ResponseBody::Bytes(body.as_ref()),
  };
  conn
    .write_response_with_scratch(
      scratch,
      h1::Response {
        version,
        status: parts.status,
        reason: h1_reason_for(parts.status),
        headers: &headers,
        body: response_body,
        keep_alive,
      },
    )
    .await
}

async fn write_h1_bad_request<I>(
  conn: &mut h1::SharedConn<I>,
  scratch: &mut h1::SharedScratch,
) -> Result<(), h1::Error>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let date = raw_h1_date();
  let headers = [h1::Header {
    name: b"date",
    value: &date,
  }];
  conn
    .write_response_with_scratch(
      scratch,
      h1::Response {
        version: h1::Version::Http11,
        status: StatusCode::BAD_REQUEST.as_u16(),
        reason: h1_reason_for(StatusCode::BAD_REQUEST.as_u16()),
        headers: &headers,
        body: h1::ResponseBody::Empty,
        keep_alive: false,
      },
    )
    .await
}

fn raw_h1_connection_closed() -> HttpNextError {
  HttpNextError::Other(deno_error::JsErrorBox::generic(
    "HTTP connection closed",
  ))
}

async fn wait_raw_response_ready(
  record: &RawHttpRecord,
  body_conn: &RawNetworkH1ConnectionCell,
  request_body: Option<&Rc<RawH1RequestBody<RawH1Io>>>,
) -> Result<(), HttpNextError> {
  let poll_peer_closed = request_body.is_none();
  poll_fn(|cx| {
    {
      let mut inner = record.0.borrow_mut();
      if inner.response_ready {
        return Poll::Ready(Ok(()));
      }
      inner.response_ready_waker = Some(cx.waker().clone());
    }

    if poll_peer_closed {
      let mut conn = body_conn.borrow_mut();
      if let Some(conn) = conn.as_mut() {
        match conn.poll_peer_closed(cx) {
          Poll::Ready(Ok(true)) => {
            record.cancel_request();
            return Poll::Ready(Ok(()));
          }
          Poll::Ready(Ok(false)) | Poll::Pending => {}
          Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
        }
      }
    }

    Poll::Pending
  })
  .await
}

async fn wait_raw_response_ready_or_closed<I>(
  record: &RawHttpRecord,
  conn: &mut h1::SharedConn<I>,
  scratch: &mut h1::SharedScratch,
) -> Result<bool, HttpNextError>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let mut peer_closed = false;
  poll_fn(|cx| {
    {
      let mut inner = record.0.borrow_mut();
      if inner.response_ready {
        return Poll::Ready(Ok(peer_closed));
      }
      inner.response_ready_waker = Some(cx.waker().clone());
    }

    if !peer_closed {
      match conn.poll_peer_closed_with(cx, scratch) {
        Poll::Ready(Ok(true)) => {
          record.cancel_request();
          peer_closed = true;
        }
        Poll::Ready(Ok(false)) | Poll::Pending => {}
        Poll::Ready(Err(error)) => return Poll::Ready(Err(error.into())),
      }
    }

    Poll::Pending
  })
  .await
}

fn raw_upgrade_unavailable() -> HttpNextError {
  HttpNextError::Other(deno_error::JsErrorBox::generic(
    "HTTP upgrade is not available for this request",
  ))
}

fn take_raw_upgrade_stream(
  upgrade: &RawUpgrade,
) -> Result<(NetworkStream, Bytes), HttpNextError> {
  let Some(state) = upgrade.conn.borrow_mut().take() else {
    return Err(raw_h1_connection_closed());
  };
  let (io, h1_bytes) = state.conn.into_upgrade_parts();
  let (stream, prefix_bytes) = io.into_inner();
  let bytes = if prefix_bytes.is_empty() {
    Bytes::from(h1_bytes)
  } else if h1_bytes.is_empty() {
    prefix_bytes
  } else {
    let mut bytes = Vec::with_capacity(prefix_bytes.len() + h1_bytes.len());
    bytes.extend_from_slice(&prefix_bytes);
    bytes.extend_from_slice(&h1_bytes);
    Bytes::from(bytes)
  };
  Ok((stream, bytes))
}

async fn write_h1_flat_response_shared<I>(
  conn: RawH1ConnectionCell<I>,
  version: h1::Version,
  parts: RawResponseParts,
  body: FlatResponseBody,
  keep_alive: bool,
  head: bool,
) -> Result<(), HttpNextError>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let date = raw_h1_date();
  let should_chunk = version == h1::Version::Http11
    && !head
    && raw_response_needs_chunked(&parts);
  if should_chunk {
    let headers = raw_response_headers(&parts, &date);
    let trailers = raw_response_trailers(&parts);
    let mut head = h1::SharedChunkedResponseHeadWriter::new(h1::ResponseHead {
      version,
      status: parts.status,
      reason: h1_reason_for(parts.status),
      headers: &headers,
      keep_alive,
    });
    poll_fn(|cx| {
      let mut conn = conn.borrow_mut();
      let Some(conn) = conn.as_mut() else {
        return Poll::Ready(Err(raw_h1_connection_closed()));
      };
      conn.poll_start_chunked_response(cx, &mut head)
    })
    .await?;
    if let FlatResponseBody::Bytes(body) = &body
      && !body.is_empty()
    {
      let mut chunk = h1::SharedResponseChunkWriter::new(body);
      poll_fn(|cx| {
        let mut conn = conn.borrow_mut();
        let Some(conn) = conn.as_mut() else {
          return Poll::Ready(Err(raw_h1_connection_closed()));
        };
        conn.poll_write_response_chunk(cx, &mut chunk)
      })
      .await?;
    }
    let mut end = h1::SharedResponseEndWriter::new(trailers.as_slice());
    poll_fn(|cx| {
      let mut conn = conn.borrow_mut();
      let Some(conn) = conn.as_mut() else {
        return Poll::Ready(Err(raw_h1_connection_closed()));
      };
      conn.poll_finish_response(cx, &mut end)
    })
    .await?;
    return Ok(());
  }
  let headers = raw_response_headers(&parts, &date);
  let response_body = match &body {
    _ if head => h1::ResponseBody::Head(Some(match &body {
      FlatResponseBody::Empty => 0,
      FlatResponseBody::Bytes(body) => body.len() as u64,
    })),
    FlatResponseBody::Empty => h1::ResponseBody::Empty,
    FlatResponseBody::Bytes(body) => h1::ResponseBody::Bytes(body.as_ref()),
  };
  let mut writer = h1::SharedResponseWriter::new(h1::Response {
    version,
    status: parts.status,
    reason: h1_reason_for(parts.status),
    headers: &headers,
    body: response_body,
    keep_alive,
  });
  poll_fn(|cx| {
    let mut conn = conn.borrow_mut();
    let Some(conn) = conn.as_mut() else {
      return Poll::Ready(Err(raw_h1_connection_closed()));
    };
    conn.poll_write_response(cx, &mut writer)
  })
  .await
}

fn raw_response_headers<'a>(
  parts: &'a RawResponseParts,
  date: &'a [u8],
) -> SmallVec<[h1::Header<'a>; 16]> {
  let mut headers = SmallVec::<[h1::Header<'_>; 16]>::new();
  let has_date = parts
    .headers
    .iter()
    .any(|header| header.name.eq_ignore_ascii_case(b"date"));
  if parts.default_text_content_type {
    headers.push(h1::Header {
      name: b"content-type",
      value: b"text/plain;charset=UTF-8",
    });
  } else if let Some(content_type) = &parts.content_type {
    headers.push(h1::Header {
      name: b"content-type",
      value: content_type.as_slice(),
    });
  }
  for RawHeader { name, value } in &parts.headers {
    headers.push(h1::Header {
      name: name.as_slice(),
      value: value.as_slice(),
    });
  }
  if !has_date {
    headers.push(h1::Header {
      name: b"date",
      value: date,
    });
  }
  headers
}

fn raw_response_trailers(
  parts: &RawResponseParts,
) -> SmallVec<[h1::Header<'_>; 4]> {
  parts
    .trailers
    .iter()
    .map(|RawHeader { name, value }| h1::Header {
      name: name.as_slice(),
      value: value.as_slice(),
    })
    .collect()
}

fn raw_response_has_transfer_encoding(parts: &RawResponseParts) -> bool {
  parts
    .headers
    .iter()
    .any(|header| header.name.eq_ignore_ascii_case(b"transfer-encoding"))
}

fn raw_response_needs_chunked(parts: &RawResponseParts) -> bool {
  !parts.trailers.is_empty() || raw_response_has_transfer_encoding(parts)
}

fn raw_response_content_length(parts: &RawResponseParts) -> Option<u64> {
  parts.headers.iter().find_map(|header| {
    if !header.name.eq_ignore_ascii_case(b"content-length") {
      return None;
    }
    let mut len = 0u64;
    for byte in &header.value {
      if !byte.is_ascii_digit() {
        return None;
      }
      len = len.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
    }
    Some(len)
  })
}

fn poll_raw_response_body_frame(
  body: &mut ResponseBytesInner,
  cx: &mut Context<'_>,
) -> Poll<ResponseStreamResult> {
  match body {
    ResponseBytesInner::Done | ResponseBytesInner::Empty => {
      Poll::Ready(ResponseStreamResult::EndOfStream)
    }
    ResponseBytesInner::Bytes(..) => {
      let ResponseBytesInner::Bytes(data) =
        std::mem::replace(body, ResponseBytesInner::Done)
      else {
        unreachable!();
      };
      Poll::Ready(ResponseStreamResult::NonEmptyBuf(data))
    }
    ResponseBytesInner::UncompressedStream(stm) => Pin::new(stm).poll_frame(cx),
    ResponseBytesInner::GZipStream(stm) => {
      Pin::new(stm.as_mut()).poll_frame(cx)
    }
    ResponseBytesInner::BrotliStream(stm) => {
      Pin::new(stm.as_mut()).poll_frame(cx)
    }
  }
}

enum RawResponseBodyEvent {
  Frame(ResponseStreamResult),
  PeerClosed,
}

fn raw_response_body_is_compressed(body: &ResponseBytesInner) -> bool {
  matches!(
    body,
    ResponseBytesInner::GZipStream(_) | ResponseBytesInner::BrotliStream(_)
  )
}

fn abort_raw_response_body(body: &mut ResponseBytesInner) {
  std::mem::take(body).abort();
}

struct RawResponseBodyFinishGuard {
  record: Rc<RawHttpRecord>,
  active: bool,
}

impl RawResponseBodyFinishGuard {
  fn new(record: Rc<RawHttpRecord>) -> Self {
    Self {
      record,
      active: true,
    }
  }

  fn finish(mut self, complete: bool) {
    self.record.finish_response_body(complete);
    self.active = false;
  }
}

impl Drop for RawResponseBodyFinishGuard {
  fn drop(&mut self) {
    if self.active {
      self.record.finish_response_body(false);
    }
  }
}

async fn write_h1_stream_response<I>(
  conn: &mut h1::SharedConn<I>,
  scratch: &mut h1::SharedScratch,
  context: RawH1ResponseContext,
  parts: RawResponseParts,
  mut body: ResponseBytesInner,
  body_content_length: Option<u64>,
  record: Rc<RawHttpRecord>,
) -> Result<(), HttpNextError>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let finish = RawResponseBodyFinishGuard::new(record);
  let date = raw_h1_date();
  let headers = raw_response_headers(&parts, &date);
  let trailers = raw_response_trailers(&parts);
  let content_length = (!raw_response_body_is_compressed(&body)
    && !raw_response_has_transfer_encoding(&parts))
  .then(|| raw_response_content_length(&parts).or(body_content_length))
  .flatten();
  if context.head {
    conn
      .write_response_with_scratch(
        scratch,
        h1::Response {
          version: context.version,
          status: parts.status,
          reason: h1_reason_for(parts.status),
          headers: &headers,
          body: h1::ResponseBody::Head(content_length),
          keep_alive: context.keep_alive,
        },
      )
      .await
      .inspect_err(|_| abort_raw_response_body(&mut body))?;
    abort_raw_response_body(&mut body);
    finish.finish(false);
    return Ok(());
  }
  let response_head = h1::ResponseHead {
    version: context.version,
    status: parts.status,
    reason: h1_reason_for(parts.status),
    headers: &headers,
    keep_alive: context.keep_alive
      && (context.version == h1::Version::Http11 || content_length.is_some()),
  };
  if let Some(content_length) = content_length {
    if let Err(error) = conn
      .start_fixed_response_with_scratch(scratch, response_head, content_length)
      .await
    {
      abort_raw_response_body(&mut body);
      finish.finish(false);
      return Err(error.into());
    }
  } else {
    if let Err(error) = conn
      .start_chunked_response_with_scratch(scratch, response_head)
      .await
    {
      abort_raw_response_body(&mut body);
      finish.finish(false);
      return Err(error.into());
    }
  }
  loop {
    let event = poll_fn(|cx| {
      match conn.poll_peer_closed_with(cx, scratch) {
        Poll::Ready(Ok(true)) => {
          return Poll::Ready(Ok::<_, HttpNextError>(
            RawResponseBodyEvent::PeerClosed,
          ));
        }
        Poll::Ready(Ok(false)) | Poll::Pending => {}
        Poll::Ready(Err(error)) => return Poll::Ready(Err(error.into())),
      }
      Poll::Ready(Ok::<_, HttpNextError>(RawResponseBodyEvent::Frame(ready!(
        poll_raw_response_body_frame(&mut body, cx)
      ))))
    })
    .await?;
    match event {
      RawResponseBodyEvent::PeerClosed => {
        // The client went away mid-response; treat it as a cancellation so
        // `Request.signal` aborts, like the hyper path does.
        finish.record.cancel_request();
        abort_raw_response_body(&mut body);
        finish.finish(false);
        return Ok(());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::EndOfStream) => {
        let trailers = if content_length.is_some()
          || context.version == h1::Version::Http10
        {
          &[][..]
        } else {
          trailers.as_slice()
        };
        conn.finish_response_with_scratch(scratch, trailers).await?;
        finish.finish(true);
        return Ok(());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::NonEmptyBuf(chunk)) => {
        let result = if content_length.is_some() {
          conn.write_response_body_with_scratch(&chunk).await
        } else {
          conn
            .write_response_chunk_with_scratch(scratch, &chunk)
            .await
        };
        if let Err(error) = result {
          abort_raw_response_body(&mut body);
          finish.finish(false);
          return Err(error.into());
        }
        finish.record.add_otel_response_size(chunk.len());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::NoData) => continue,
      RawResponseBodyEvent::Frame(ResponseStreamResult::Error(error)) => {
        finish.finish(false);
        return Err(HttpNextError::Other(error));
      }
    }
  }
}

async fn write_h1_stream_response_shared<I>(
  conn: RawH1ConnectionCell<I>,
  context: RawH1ResponseContext,
  parts: RawResponseParts,
  mut body: ResponseBytesInner,
  body_content_length: Option<u64>,
  record: Rc<RawHttpRecord>,
) -> Result<(), HttpNextError>
where
  I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
  let finish = RawResponseBodyFinishGuard::new(record);
  let date = raw_h1_date();
  let headers = raw_response_headers(&parts, &date);
  let trailers = raw_response_trailers(&parts);
  let content_length = (!raw_response_body_is_compressed(&body)
    && !raw_response_has_transfer_encoding(&parts))
  .then(|| raw_response_content_length(&parts).or(body_content_length))
  .flatten();
  if context.head {
    let mut writer = h1::SharedResponseWriter::new(h1::Response {
      version: context.version,
      status: parts.status,
      reason: h1_reason_for(parts.status),
      headers: &headers,
      body: h1::ResponseBody::Head(content_length),
      keep_alive: context.keep_alive,
    });
    poll_fn(|cx| {
      let mut conn = conn.borrow_mut();
      let Some(conn) = conn.as_mut() else {
        return Poll::Ready(Err(raw_h1_connection_closed()));
      };
      conn.poll_write_response(cx, &mut writer)
    })
    .await
    .inspect_err(|_| abort_raw_response_body(&mut body))?;
    abort_raw_response_body(&mut body);
    finish.finish(false);
    return Ok(());
  }
  let response_head = h1::ResponseHead {
    version: context.version,
    status: parts.status,
    reason: h1_reason_for(parts.status),
    headers: &headers,
    keep_alive: context.keep_alive
      && (context.version == h1::Version::Http11 || content_length.is_some()),
  };
  if let Some(content_length) = content_length {
    let mut head =
      h1::SharedFixedResponseHeadWriter::new(response_head, content_length);
    poll_fn(|cx| {
      let mut conn = conn.borrow_mut();
      let Some(conn) = conn.as_mut() else {
        return Poll::Ready(Err(raw_h1_connection_closed()));
      };
      conn.poll_start_fixed_response(cx, &mut head)
    })
    .await
    .inspect_err(|_| abort_raw_response_body(&mut body))?;
  } else {
    let mut head = h1::SharedChunkedResponseHeadWriter::new(response_head);
    poll_fn(|cx| {
      let mut conn = conn.borrow_mut();
      let Some(conn) = conn.as_mut() else {
        return Poll::Ready(Err(raw_h1_connection_closed()));
      };
      conn.poll_start_chunked_response(cx, &mut head)
    })
    .await
    .inspect_err(|_| abort_raw_response_body(&mut body))?;
  }
  loop {
    let event = poll_fn(|cx| {
      {
        let mut conn = conn.borrow_mut();
        let Some(conn) = conn.as_mut() else {
          return Poll::Ready(Err(raw_h1_connection_closed()));
        };
        match conn.poll_peer_closed(cx) {
          Poll::Ready(Ok(true)) => {
            return Poll::Ready(Ok::<_, HttpNextError>(
              RawResponseBodyEvent::PeerClosed,
            ));
          }
          Poll::Ready(Ok(false)) | Poll::Pending => {}
          Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
        }
      }
      Poll::Ready(Ok::<_, HttpNextError>(RawResponseBodyEvent::Frame(ready!(
        poll_raw_response_body_frame(&mut body, cx)
      ))))
    })
    .await?;
    match event {
      RawResponseBodyEvent::PeerClosed => {
        // The client went away mid-response; treat it as a cancellation so
        // `Request.signal` aborts, like the hyper path does.
        finish.record.cancel_request();
        abort_raw_response_body(&mut body);
        finish.finish(false);
        return Ok(());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::EndOfStream) => {
        let trailers = if content_length.is_some()
          || context.version == h1::Version::Http10
        {
          &[][..]
        } else {
          trailers.as_slice()
        };
        let mut end = h1::SharedResponseEndWriter::new(trailers);
        poll_fn(|cx| {
          let mut conn = conn.borrow_mut();
          let Some(conn) = conn.as_mut() else {
            return Poll::Ready(Err(raw_h1_connection_closed()));
          };
          conn.poll_finish_response(cx, &mut end)
        })
        .await?;
        finish.finish(true);
        return Ok(());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::NonEmptyBuf(chunk)) => {
        let result = if content_length.is_some() {
          let mut writer = h1::SharedResponseBodyWriter::new(&chunk);
          poll_fn(|cx| {
            let mut conn = conn.borrow_mut();
            let Some(conn) = conn.as_mut() else {
              return Poll::Ready(Err(raw_h1_connection_closed()));
            };
            conn.poll_write_response_body(cx, &mut writer)
          })
          .await
        } else {
          let mut writer = h1::SharedResponseChunkWriter::new(&chunk);
          poll_fn(|cx| {
            let mut conn = conn.borrow_mut();
            let Some(conn) = conn.as_mut() else {
              return Poll::Ready(Err(raw_h1_connection_closed()));
            };
            conn.poll_write_response_chunk(cx, &mut writer)
          })
          .await
        };
        if let Err(error) = result {
          abort_raw_response_body(&mut body);
          finish.finish(false);
          return Err(error);
        }
        finish.record.add_otel_response_size(chunk.len());
      }
      RawResponseBodyEvent::Frame(ResponseStreamResult::NoData) => continue,
      RawResponseBodyEvent::Frame(ResponseStreamResult::Error(error)) => {
        finish.finish(false);
        return Err(HttpNextError::Other(error));
      }
    }
  }
}

async fn serve_http11_raw(
  io: RawH1Io,
  request_info: HttpConnectionProperties,
  callback: Rc<ServerCallback>,
  cancel: Rc<CancelHandle>,
  _server_state: SignallingRc<HttpServerState>,
) -> Result<(), HttpNextError> {
  let mut conn = h1::SharedConn::new(io);
  conn.set_allow_missing_host(true);
  let mut scratch = h1::SharedScratch::default();
  let store_request = !callback.raw_no_request();
  loop {
    let next_request = poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| {
        raw_request_from_h1(request, store_request)
      })
    })
    .or_cancel(cancel.clone())
    .await;
    let Some(parsed) = (match next_request {
      Ok(Ok(result)) => result,
      Ok(Err(h1::Error::Parse(_) | h1::Error::HeadTooLarge)) => {
        write_h1_bad_request(&mut conn, &mut scratch).await?;
        return Ok(());
      }
      Ok(Err(error)) => {
        return Err(error.into());
      }
      Err(_) => {
        let mut io = conn.into_inner();
        io.shutdown().await?;
        return Ok(());
      }
    }) else {
      return Ok(());
    };

    let keep_alive = parsed.keep_alive;
    let expect_continue = parsed.expect_continue;
    let has_body = parsed.has_body;
    let head = parsed.method.is_head();
    let response_context = RawH1ResponseContext {
      version: parsed.version,
      keep_alive,
      head,
    };

    if expect_continue && has_body {
      conn.write_continue().await?;
    }

    let upgrade = match parsed.upgrade {
      Some(h1::UpgradeKind::Any) => Some(()),
      Some(h1::UpgradeKind::H2c) | None => None,
    };
    if parsed.has_body
      && upgrade.is_none()
      && let Some(body) = conn.try_take_full_body()?
    {
      let record = RawHttpRecord::new(
        request_info.clone(),
        parsed.method,
        parsed.path,
        parsed.headers,
        Some(RawRequestBody::Prebuffered(body)),
        None,
        parsed.request_size,
      );
      let mut record_cancel_guard =
        RawHttpRecordCancelGuard::new(record.clone());
      let direct_response =
        dispatch_raw_to_native_response(&callback, record.clone());
      if let Some(response) = direct_response {
        let (response_parts, body) =
          raw_response_from_direct_response(response);
        let response_status = response_parts.status;
        write_h1_flat_response(
          &mut conn,
          &mut scratch,
          parsed.version,
          response_parts,
          body,
          keep_alive,
          head,
        )
        .await?;
        record_cancel_guard.disarm();
        if response_status == StatusCode::SWITCHING_PROTOCOLS.as_u16()
          || !keep_alive
          || cancel.is_canceled()
        {
          return Ok(());
        }
        continue;
      }
      if wait_raw_response_ready_or_closed(&record, &mut conn, &mut scratch)
        .await?
      {
        if let Some((_, RawResponseBody::Stream { mut body, .. })) =
          record.clone().into_flat_response()
        {
          abort_raw_response_body(&mut body);
          record.finish_response_body(false);
        }
        return Ok(());
      }
      let Some((response_parts, body)) = record.clone().into_flat_response()
      else {
        return Ok(());
      };
      let response_status = response_parts.status;
      match body {
        RawResponseBody::Flat(body) => {
          write_h1_flat_response(
            &mut conn,
            &mut scratch,
            parsed.version,
            response_parts,
            body,
            keep_alive,
            head,
          )
          .await?;
        }
        RawResponseBody::Stream {
          body,
          content_length,
        } => {
          write_h1_stream_response(
            &mut conn,
            &mut scratch,
            response_context,
            response_parts,
            body,
            content_length,
            record.clone(),
          )
          .await?;
          if parsed.version == h1::Version::Http10 {
            record_cancel_guard.disarm();
            return Ok(());
          }
        }
      }
      if response_status == StatusCode::SWITCHING_PROTOCOLS.as_u16()
        || !keep_alive
        || cancel.is_canceled()
      {
        record_cancel_guard.disarm();
        return Ok(());
      }
      record_cancel_guard.disarm();
      continue;
    }
    if parsed.has_body || upgrade.is_some() {
      let body_conn =
        Rc::new(RefCell::new(Some(RawH1ConnectionState { conn, scratch })));
      let request_body_resource = parsed.has_body.then(|| {
        Rc::new(RawH1RequestBody::new(
          body_conn.clone(),
          parsed.request_body_len,
        ))
      });
      let request_body_for_cancel = request_body_resource.clone();
      let upgrade = upgrade.map(|()| {
        Rc::new(RawUpgrade {
          conn: body_conn.clone(),
          websocket_tx: RefCell::new(None),
        })
      });
      let record = RawHttpRecord::new(
        request_info.clone(),
        parsed.method,
        parsed.path,
        parsed.headers,
        request_body_resource.map(RawRequestBody::Streaming),
        upgrade.clone(),
        parsed.request_size,
      );
      let mut record_cancel_guard =
        RawHttpRecordCancelGuard::new(record.clone());
      let direct_response = if upgrade.is_none() {
        dispatch_raw_to_native_response(&callback, record.clone())
      } else {
        dispatch_raw_to_js(&callback, record.clone());
        None
      };
      if let Some(response) = direct_response {
        let (response_parts, body) =
          raw_response_from_direct_response(response);
        let response_status = response_parts.status;
        let state = { body_conn.borrow_mut().take() };
        if let Some(state) = state {
          let mut local_conn = state.conn;
          let mut local_scratch = state.scratch;
          let keep_alive = keep_alive && !parsed.has_body;
          write_h1_flat_response(
            &mut local_conn,
            &mut local_scratch,
            parsed.version,
            response_parts,
            body,
            keep_alive,
            head,
          )
          .await?;
          conn = local_conn;
          scratch = local_scratch;
          record_cancel_guard.disarm();
          if response_status == StatusCode::SWITCHING_PROTOCOLS.as_u16()
            || !keep_alive
            || cancel.is_canceled()
          {
            if parsed.has_body {
              let _ = conn.discard_body_with_scratch(&mut scratch).await;
            }
            return Ok(());
          }
          continue;
        }
      }
      wait_raw_response_ready(
        &record,
        &body_conn,
        request_body_for_cancel.as_ref(),
      )
      .await?;
      let Some((response_parts, body)) = record.clone().into_flat_response()
      else {
        return Ok(());
      };
      let response_status = response_parts.status;
      if record.request_body_taken_full() {
        let state = { body_conn.borrow_mut().take() };
        if let Some(state) = state {
          let mut local_conn = state.conn;
          let mut local_scratch = state.scratch;
          match body {
            RawResponseBody::Flat(body) => {
              write_h1_flat_response(
                &mut local_conn,
                &mut local_scratch,
                parsed.version,
                response_parts,
                body,
                keep_alive,
                head,
              )
              .await?;
            }
            RawResponseBody::Stream {
              body,
              content_length,
            } => {
              write_h1_stream_response(
                &mut local_conn,
                &mut local_scratch,
                response_context,
                response_parts,
                body,
                content_length,
                record.clone(),
              )
              .await?;
              if parsed.version == h1::Version::Http10 {
                record_cancel_guard.disarm();
                return Ok(());
              }
            }
          }
          conn = local_conn;
          scratch = local_scratch;
          if !keep_alive || cancel.is_canceled() {
            record_cancel_guard.disarm();
            return Ok(());
          }
          record_cancel_guard.disarm();
          continue;
        }
      }
      match body {
        RawResponseBody::Flat(body) => {
          let keep_alive = keep_alive && !parsed.has_body;
          write_h1_flat_response_shared(
            body_conn.clone(),
            parsed.version,
            response_parts,
            body,
            keep_alive,
            head,
          )
          .await?;
        }
        RawResponseBody::Stream {
          body,
          content_length,
        } => {
          let response_context = RawH1ResponseContext {
            version: response_context.version,
            keep_alive: response_context.keep_alive && !parsed.has_body,
            head: response_context.head,
          };
          write_h1_stream_response_shared(
            body_conn.clone(),
            response_context,
            response_parts,
            body,
            content_length,
            record.clone(),
          )
          .await?;
          if parsed.version == h1::Version::Http10 {
            record_cancel_guard.disarm();
            return Ok(());
          }
        }
      }
      if response_status == StatusCode::SWITCHING_PROTOCOLS.as_u16()
        && let Some(upgrade) = upgrade
        && let Some(tx) = upgrade.websocket_tx.borrow_mut().take()
      {
        let result = take_raw_upgrade_stream(&upgrade);
        let _ = tx.send(result);
        record_cancel_guard.disarm();
        return Ok(());
      }
      let Some(state) = body_conn.borrow_mut().take() else {
        return Err(raw_h1_connection_closed());
      };
      conn = state.conn;
      scratch = state.scratch;
      if !keep_alive || parsed.has_body || cancel.is_canceled() {
        if parsed.has_body {
          let _ = conn.discard_body_with_scratch(&mut scratch).await;
        }
        record_cancel_guard.disarm();
        return Ok(());
      }
      record_cancel_guard.disarm();
      continue;
    }

    let record = RawHttpRecord::new(
      request_info.clone(),
      parsed.method,
      parsed.path,
      parsed.headers,
      None,
      None,
      parsed.request_size,
    );
    let mut record_cancel_guard = RawHttpRecordCancelGuard::new(record.clone());
    let direct_response =
      dispatch_raw_to_native_response(&callback, record.clone());
    if let Some(response) = direct_response {
      let (response_parts, body) = raw_response_from_direct_response(response);
      write_h1_flat_response(
        &mut conn,
        &mut scratch,
        parsed.version,
        response_parts,
        body,
        keep_alive,
        head,
      )
      .await?;
      record_cancel_guard.disarm();
      if !keep_alive || cancel.is_canceled() {
        return Ok(());
      }
      continue;
    }
    if wait_raw_response_ready_or_closed(&record, &mut conn, &mut scratch)
      .await?
    {
      if let Some((_, RawResponseBody::Stream { mut body, .. })) =
        record.clone().into_flat_response()
      {
        abort_raw_response_body(&mut body);
        record.finish_response_body(false);
      }
      return Ok(());
    }
    let Some((response_parts, body)) = record.clone().into_flat_response()
    else {
      return Ok(());
    };
    match body {
      RawResponseBody::Flat(body) => {
        write_h1_flat_response(
          &mut conn,
          &mut scratch,
          parsed.version,
          response_parts,
          body,
          keep_alive,
          head,
        )
        .await?;
      }
      RawResponseBody::Stream {
        body,
        content_length,
      } => {
        write_h1_stream_response(
          &mut conn,
          &mut scratch,
          response_context,
          response_parts,
          body,
          content_length,
          record.clone(),
        )
        .await?;
        if parsed.version == h1::Version::Http10 {
          record_cancel_guard.disarm();
          return Ok(());
        }
      }
    }
    if !keep_alive || cancel.is_canceled() {
      record_cancel_guard.disarm();
      return Ok(());
    }
    record_cancel_guard.disarm();
  }
}

fn serve_http2_unconditional(
  io: impl HttpServeStream,
  svc: impl HttpService<Incoming, ResBody = HttpRecordResponse> + 'static,
  cancel: Rc<CancelHandle>,
  http2_builder_hook: Option<
    fn(http2::Builder<LocalExecutor>) -> http2::Builder<LocalExecutor>,
  >,
) -> impl Future<Output = Result<(), hyper::Error>> + 'static {
  let mut builder = http2::Builder::new(LocalExecutor);

  if let Some(http2_builder_hook) = http2_builder_hook {
    builder = http2_builder_hook(builder);
  }

  let conn = builder.serve_connection(TokioIo::new(io), svc);
  async {
    match conn.or_abort(cancel).await {
      Err(mut conn) => {
        Pin::new(&mut conn).graceful_shutdown();
        conn.await
      }
      Ok(res) => res,
    }
  }
}

async fn serve_http2_autodetect(
  io: NetworkStream,
  svc: impl HttpService<Incoming, ResBody = HttpRecordResponse> + 'static,
  request_info: HttpConnectionProperties,
  callback: Rc<ServerCallback>,
  cancel: Rc<CancelHandle>,
  server_state: SignallingRc<HttpServerState>,
  options: Options,
) -> Result<(), HttpNextError> {
  let prefix = NetworkStreamPrefixCheck::new(io, HTTP2_PREFIX);
  let Some((matches, io)) = prefix
    .match_prefix_or_shutdown(
      std::future::pending::<()>().or_cancel(cancel.clone()),
    )
    .await?
  else {
    return Ok(());
  };
  if matches {
    serve_http2_unconditional(io, svc, cancel, options.http2_builder_hook)
      .await
      .map_err(HttpNextError::Hyper)
  } else {
    serve_http11_raw(io, request_info, callback, cancel, server_state).await
  }
}

/// Dispatch a record to the JS callback registered for the server.
/// Wraps the record as an `ExternalPointer<RcHttpRecord>` and hands
/// the raw pointer to the JS callback. The JS side eventually calls
/// `take_external!` (via `op_http_set_response_*`) which consumes the
/// refcount.
fn dispatch_to_js(callback: &ServerCallback, record: Rc<HttpRecord>) {
  let ptr =
    ExternalPointer::new(RcHttpRecord(HttpRecordExternal::Hyper(record)))
      .into_raw();
  // SAFETY: callback's isolate ptr remains valid while the
  // HttpJoinHandle is alive (the HttpJoinHandle owns the
  // ServerCallback and is stored in the resource table; on close
  // the resource is dropped synchronously while the isolate is still
  // live).
  unsafe { callback.dispatch(ptr as *mut std::ffi::c_void) };
}

fn dispatch_raw_to_js(callback: &ServerCallback, record: Rc<RawHttpRecord>) {
  let ptr = ExternalPointer::new(RcHttpRecord(HttpRecordExternal::Raw(record)))
    .into_raw();
  // SAFETY: callback's isolate ptr remains valid while the HttpJoinHandle is
  // alive.
  unsafe { callback.dispatch(ptr as *mut std::ffi::c_void) };
}

fn dispatch_raw_to_native_response(
  callback: &ServerCallback,
  record: Rc<RawHttpRecord>,
) -> Option<DirectResponse> {
  let ptr = ExternalPointer::new(RcHttpRecord(HttpRecordExternal::Raw(record)))
    .into_raw();
  // SAFETY: callback's isolate ptr remains valid while the HttpJoinHandle is
  // alive.
  let response =
    unsafe { callback.dispatch_native_response(ptr as *mut std::ffi::c_void) };
  if response.is_some() {
    // The JS native-response callback closed its InnerRequest before returning
    // this response, so no JS object should retain this external pointer.
    // SAFETY: no JS object retains this external pointer when a native response
    // is returned synchronously, so consume the strong ref created above.
    let _ = unsafe { take_external!(ptr, "native response") };
  }
  response
}

fn serve_https(
  mut io: TlsStream<TcpStream>,
  request_info: HttpConnectionProperties,
  lifetime: HttpLifetime,
  callback: Rc<ServerCallback>,
  options: Options,
) -> JoinHandle<Result<(), HttpNextError>> {
  let HttpLifetime {
    server_state,
    connection_cancel_handle,
    listen_cancel_handle,
  } = lifetime;

  let legacy_abort = !options.no_legacy_abort;
  let raw_request_info = request_info.clone();
  let raw_callback = callback.clone();
  let raw_server_state = server_state.clone();
  let svc = service_fn(move |req: Request| {
    let callback = callback.clone();
    let request_info = request_info.clone();
    let server_state = server_state.clone();
    async move {
      handle_request(
        req,
        request_info,
        server_state,
        move |record| dispatch_to_js(&callback, record),
        legacy_abort,
      )
      .await
    }
  });
  spawn(
    async move {
      let handshake = io.handshake().await?;
      // If the client specifically negotiates a protocol, we will use it. If not, we'll auto-detect
      // based on the prefix bytes
      let handshake = handshake.alpn;
      let io = NetworkStream::Tls(io);
      if Some(TLS_ALPN_HTTP_2) == handshake.as_deref() {
        serve_http2_unconditional(
          io,
          svc,
          listen_cancel_handle,
          options.http2_builder_hook,
        )
        .await
        .map_err(HttpNextError::Hyper)
      } else if Some(TLS_ALPN_HTTP_11) == handshake.as_deref() {
        serve_http11_raw(
          NetworkBufferedStream::from_io(io),
          raw_request_info,
          raw_callback,
          listen_cancel_handle,
          raw_server_state,
        )
        .await
      } else {
        Box::pin(serve_http2_autodetect(
          io,
          svc,
          raw_request_info,
          raw_callback,
          listen_cancel_handle,
          raw_server_state,
          options,
        ))
        .await
      }
    }
    .try_or_cancel(connection_cancel_handle),
  )
}

fn serve_http(
  io: NetworkStream,
  request_info: HttpConnectionProperties,
  lifetime: HttpLifetime,
  callback: Rc<ServerCallback>,
  options: Options,
  wait_for_connection: bool,
) -> JoinHandle<Result<(), HttpNextError>> {
  let HttpLifetime {
    server_state,
    connection_cancel_handle,
    listen_cancel_handle,
  } = lifetime;

  let legacy_abort = !options.no_legacy_abort;
  let raw_request_info = request_info.clone();
  let raw_callback = callback.clone();
  let raw_server_state = server_state.clone();
  let svc = service_fn(move |req: Request| {
    let callback = callback.clone();
    let request_info = request_info.clone();
    let server_state = server_state.clone();
    async move {
      handle_request(
        req,
        request_info,
        server_state,
        move |record| dispatch_to_js(&callback, record),
        legacy_abort,
      )
      .await
    }
  });
  let connection_cancel_handle_for_outer = connection_cancel_handle.clone();
  spawn(
    async move {
      let prefix = NetworkStreamPrefixCheck::new(io, HTTP2_PREFIX);
      let Some((matches, io)) = prefix
        .match_prefix_or_shutdown(
          std::future::pending::<()>().or_cancel(listen_cancel_handle.clone()),
        )
        .await?
      else {
        return Ok(());
      };
      let join_handle = if matches {
        spawn(
          async move {
            serve_http2_unconditional(
              io,
              svc,
              listen_cancel_handle,
              options.http2_builder_hook,
            )
            .await
            .map_err(HttpNextError::Hyper)
          }
          .try_or_cancel(connection_cancel_handle),
        )
      } else {
        spawn(
          async move {
            serve_http11_raw(
              io,
              raw_request_info,
              raw_callback,
              listen_cancel_handle,
              raw_server_state,
            )
            .await
          }
          .try_or_cancel(connection_cancel_handle),
        )
      };

      if wait_for_connection {
        join_handle.await?
      } else {
        Ok(())
      }
    }
    .try_or_cancel(connection_cancel_handle_for_outer),
  )
}

fn serve_http_on<HTTP>(
  connection: HTTP::Connection,
  listen_properties: &HttpListenProperties,
  lifetime: HttpLifetime,
  callback: Rc<ServerCallback>,
  options: Options,
  wait_for_connection: bool,
) -> JoinHandle<Result<(), HttpNextError>>
where
  HTTP: HttpPropertyExtractor,
{
  let connection_properties: HttpConnectionProperties =
    HTTP::connection_properties(listen_properties, &connection);

  let network_stream = HTTP::to_network_stream_from_connection(connection);
  if let NetworkStream::Tcp(tcp) = &network_stream {
    let _ = tcp.set_nodelay(true);
  }

  match network_stream {
    NetworkStream::Tls(conn) => {
      serve_https(conn, connection_properties, lifetime, callback, options)
    }
    network_stream => serve_http(
      network_stream,
      connection_properties,
      lifetime,
      callback,
      options,
      wait_for_connection,
    ),
  }
}

#[derive(Clone)]
struct HttpLifetime {
  connection_cancel_handle: Rc<CancelHandle>,
  listen_cancel_handle: Rc<CancelHandle>,
  server_state: SignallingRc<HttpServerState>,
}

struct HttpJoinHandle {
  join_handle: AsyncRefCell<Option<JoinHandle<Result<(), HttpNextError>>>>,
  connection_cancel_handle: Rc<CancelHandle>,
  listen_cancel_handle: Rc<CancelHandle>,
  server_state: SignallingRc<HttpServerState>,
}

impl HttpJoinHandle {
  fn new() -> Self {
    Self {
      join_handle: AsyncRefCell::new(None),
      connection_cancel_handle: CancelHandle::new_rc(),
      listen_cancel_handle: CancelHandle::new_rc(),
      server_state: HttpServerState::new(),
    }
  }

  fn lifetime(self: &Rc<Self>) -> HttpLifetime {
    HttpLifetime {
      connection_cancel_handle: self.connection_cancel_handle.clone(),
      listen_cancel_handle: self.listen_cancel_handle.clone(),
      server_state: self.server_state.clone(),
    }
  }

  fn connection_cancel_handle(self: &Rc<Self>) -> Rc<CancelHandle> {
    self.connection_cancel_handle.clone()
  }

  fn listen_cancel_handle(self: &Rc<Self>) -> Rc<CancelHandle> {
    self.listen_cancel_handle.clone()
  }
}

impl Resource for HttpJoinHandle {
  fn name(&self) -> Cow<'_, str> {
    "http".into()
  }

  fn close(self: Rc<Self>) {
    // During a close operation, we cancel everything
    self.connection_cancel_handle.cancel();
    self.listen_cancel_handle.cancel();
  }
}

impl Drop for HttpJoinHandle {
  fn drop(&mut self) {
    // In some cases we may be dropped without closing, so let's cancel everything on the way out
    self.connection_cancel_handle.cancel();
    self.listen_cancel_handle.cancel();
  }
}

#[op2]
pub fn op_http_serve<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  isolate: &mut v8::Isolate,
  state: Rc<RefCell<OpState>>,
  #[smi] listener_rid: ResourceId,
  callback: v8::Local<'scope, v8::Function>,
  raw_no_request: bool,
  native_callback: v8::Local<'scope, v8::Function>,
  serve_native_response_key: v8::Local<'scope, v8::Value>,
  serve_fast_status_key: v8::Local<'scope, v8::Value>,
  serve_fast_body_key: v8::Local<'scope, v8::Value>,
  serve_fast_header_kind_key: v8::Local<'scope, v8::Value>,
  serve_fast_content_type_key: v8::Local<'scope, v8::Value>,
  serve_fast_consumed_key: v8::Local<'scope, v8::Value>,
) -> Result<(ResourceId, &'static str, String, bool), HttpNextError>
where
  HTTP: HttpPropertyExtractor,
{
  let listener =
    HTTP::get_listener_for_rid(&mut state.borrow_mut(), listener_rid)?;

  let listen_properties = HTTP::listen_properties_from_listener(&listener)?;

  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle::new());
  let listen_cancel_clone = resource.listen_cancel_handle();

  let lifetime = resource.lifetime();
  let callback_global = v8::Global::new(scope, callback);
  let native_callback_global = v8::Global::new(scope, native_callback);
  let callback = Rc::new(ServerCallback::new(
    scope,
    isolate,
    callback_global,
    native_callback_global,
    v8::Global::new(scope, serve_native_response_key),
    v8::Global::new(scope, serve_fast_status_key),
    v8::Global::new(scope, serve_fast_body_key),
    v8::Global::new(scope, serve_fast_header_kind_key),
    v8::Global::new(scope, serve_fast_content_type_key),
    v8::Global::new(scope, serve_fast_consumed_key),
    raw_no_request,
    state.borrow().waker.clone(),
  ));

  let options = {
    let state = state.borrow();
    *state.borrow::<Options>()
  };

  let listen_properties_clone: HttpListenProperties = listen_properties.clone();
  let handle = spawn(async move {
    loop {
      let conn = HTTP::accept_connection_from_listener(&listener)
        .try_or_cancel(listen_cancel_clone.clone())
        .await?;
      serve_http_on::<HTTP>(
        conn,
        &listen_properties_clone,
        lifetime.clone(),
        callback.clone(),
        options,
        false,
      );
    }
    #[allow(unreachable_code, reason = "to avoid typing closure")]
    Ok::<_, HttpNextError>(())
  });

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.join_handle)
    .try_borrow_mut()
    .unwrap() = Some(handle);

  Ok((
    state.borrow_mut().resource_table.add_rc(resource),
    listen_properties.scheme,
    listen_properties.fallback_host,
    options.no_legacy_abort,
  ))
}

#[op2]
pub fn op_http_serve_on<'scope, HTTP>(
  scope: &mut v8::PinScope<'scope, '_>,
  isolate: &mut v8::Isolate,
  state: Rc<RefCell<OpState>>,
  #[smi] connection_rid: ResourceId,
  callback: v8::Local<'scope, v8::Function>,
  raw_no_request: bool,
  native_callback: v8::Local<'scope, v8::Function>,
  serve_native_response_key: v8::Local<'scope, v8::Value>,
  serve_fast_status_key: v8::Local<'scope, v8::Value>,
  serve_fast_body_key: v8::Local<'scope, v8::Value>,
  serve_fast_header_kind_key: v8::Local<'scope, v8::Value>,
  serve_fast_content_type_key: v8::Local<'scope, v8::Value>,
  serve_fast_consumed_key: v8::Local<'scope, v8::Value>,
) -> Result<(ResourceId, &'static str, String, bool), HttpNextError>
where
  HTTP: HttpPropertyExtractor,
{
  let connection =
    HTTP::get_connection_for_rid(&mut state.borrow_mut(), connection_rid)?;

  let listen_properties = HTTP::listen_properties_from_connection(&connection)?;

  let resource: Rc<HttpJoinHandle> = Rc::new(HttpJoinHandle::new());
  let callback_global = v8::Global::new(scope, callback);
  let native_callback_global = v8::Global::new(scope, native_callback);
  let callback = Rc::new(ServerCallback::new(
    scope,
    isolate,
    callback_global,
    native_callback_global,
    v8::Global::new(scope, serve_native_response_key),
    v8::Global::new(scope, serve_fast_status_key),
    v8::Global::new(scope, serve_fast_body_key),
    v8::Global::new(scope, serve_fast_header_kind_key),
    v8::Global::new(scope, serve_fast_content_type_key),
    v8::Global::new(scope, serve_fast_consumed_key),
    raw_no_request,
    state.borrow().waker.clone(),
  ));

  let options = {
    let state = state.borrow();
    *state.borrow::<Options>()
  };

  let handle = serve_http_on::<HTTP>(
    connection,
    &listen_properties,
    resource.lifetime(),
    callback,
    options,
    true,
  );

  // Set the handle after we start the future
  *RcRef::map(&resource, |this| &this.join_handle)
    .try_borrow_mut()
    .unwrap() = Some(handle);

  Ok((
    state.borrow_mut().resource_table.add_rc(resource),
    listen_properties.scheme,
    listen_properties.fallback_host,
    options.no_legacy_abort,
  ))
}

/// Wait for the server to finish accepting connections. Resolves
/// when the accept-loop spawned by `op_http_serve` has exited (either
/// from listener error or because the resource was closed).
#[op2]
pub async fn op_http_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), HttpNextError> {
  let join_handle = state
    .borrow_mut()
    .resource_table
    .get::<HttpJoinHandle>(rid)?;

  let res = RcRef::map(join_handle, |this| &this.join_handle)
    .borrow_mut()
    .await
    .take()
    .unwrap()
    .await?;

  // Filter out shutdown (ENOTCONN) errors
  if let Err(err) = res
    && !is_normal_close(&err)
  {
    return Err(err);
  }

  Ok(())
}

fn is_normal_close(err: &(dyn std::error::Error + 'static)) -> bool {
  if let Some(err) = err.downcast_ref::<HttpNextError>() {
    if let HttpNextError::Io(err) = &err {
      return is_normal_close(err);
    }

    if let HttpNextError::Other(err) = &err {
      return is_normal_close(err);
    }

    return false;
  }

  if let Some(err) = err.downcast_ref::<deno_error::JsErrorBox>() {
    if let Some(err) = err.get_inner_ref() {
      return is_normal_close(err);
    }

    return false;
  }

  if let Some(err) = err.downcast_ref::<std::io::Error>() {
    if err.kind() == io::ErrorKind::NotConnected {
      return true;
    }

    if let Some(err) = err.get_ref() {
      return is_normal_close(err);
    }

    return false;
  }

  if let Some(err) = err.downcast_ref::<deno_net::tunnel::Error>() {
    if let deno_net::tunnel::Error::QuinnConnection(err) = err {
      return is_normal_close(err);
    }

    return false;
  }

  if let Some(err) =
    err.downcast_ref::<deno_net::tunnel::quinn::ConnectionError>()
  {
    if matches!(err, deno_net::tunnel::quinn::ConnectionError::LocallyClosed) {
      return true;
    }

    return false;
  }

  false
}

/// Cancels the HTTP handle.
#[op2(fast)]
pub fn op_http_cancel(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  graceful: bool,
) -> Result<(), deno_core::error::ResourceError> {
  let join_handle = state.resource_table.get::<HttpJoinHandle>(rid)?;

  if graceful {
    // In a graceful shutdown, we close the listener and allow all the remaining connections to drain
    join_handle.listen_cancel_handle().cancel();
  } else {
    // In a forceful shutdown, we close everything
    join_handle.listen_cancel_handle().cancel();
    join_handle.connection_cancel_handle().cancel();
  }

  Ok(())
}

#[op2]
pub async fn op_http_close(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  graceful: bool,
) -> Result<(), HttpNextError> {
  let join_handle = state
    .borrow_mut()
    .resource_table
    .take::<HttpJoinHandle>(rid)?;

  if graceful {
    http_general_trace!("graceful shutdown");
    // In a graceful shutdown, we close the listener and allow all the remaining connections to drain
    join_handle.listen_cancel_handle().cancel();
    // Idle connections can still be waiting in protocol prefix detection and
    // are not represented in the active request set. Give them a turn to
    // observe the graceful listener cancellation and close with FIN before the
    // server resource is dropped.
    tokio::task::yield_now().await;
    poll_fn(|cx| join_handle.server_state.poll_complete(cx)).await;
  } else {
    http_general_trace!("forceful shutdown");
    // In a forceful shutdown, we close everything
    join_handle.listen_cancel_handle().cancel();
    join_handle.connection_cancel_handle().cancel();
    // Give streaming responses a tick to close
    tokio::task::yield_now().await;
  }

  http_general_trace!("awaiting shutdown");

  let mut join_handle = RcRef::map(&join_handle, |this| &this.join_handle)
    .borrow_mut()
    .await;
  if let Some(join_handle) = join_handle.take() {
    join_handle.await??;
  }

  Ok(())
}

enum UpgradeStreamWriteState {
  RawParsing(
    BytesMut,
    Rc<RawHttpRecord>,
    RawNetworkH1ConnectionCell,
    AsyncMut<Option<(NetworkStreamReadHalf, Bytes)>>,
  ),
  Parsing(
    BytesMut,
    Rc<HttpRecord>,
    OnUpgrade,
    AsyncMut<Option<(NetworkStreamReadHalf, Bytes)>>,
  ),
  Network(NetworkStreamWriteHalf),
  /// The upgrade was rejected with a non-101 status code.
  /// The response has been sent and the stream is now closed for writing.
  Rejected,
  Failed,
}

struct UpgradeStream {
  read: Rc<AsyncRefCell<Option<(NetworkStreamReadHalf, Bytes)>>>,
  write: AsyncRefCell<UpgradeStreamWriteState>,
  cancel_handle: CancelHandle,
  /// Set to true when the upgrade was rejected with a non-101 status.
  /// When rejected, reads return EOF and writes are silently ignored.
  rejected: std::cell::Cell<bool>,
}

impl UpgradeStream {
  pub fn new(
    read: Rc<AsyncRefCell<Option<(NetworkStreamReadHalf, Bytes)>>>,
    write: UpgradeStreamWriteState,
  ) -> Self {
    Self {
      read,
      write: AsyncRefCell::new(write),
      cancel_handle: CancelHandle::new(),
      rejected: std::cell::Cell::new(false),
    }
  }

  async fn read(
    self: Rc<Self>,
    buf: &mut [u8],
  ) -> Result<usize, std::io::Error> {
    // If the upgrade was rejected, return EOF
    if self.rejected.get() {
      return Ok(0);
    }

    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    async {
      let read = RcRef::map(self, |this| &this.read);
      let mut read = read.borrow_mut().await;
      let Some(read) = &mut *read else {
        return Err(std::io::Error::other(HttpNextError::RawUpgradeFailed));
      };
      if !read.1.is_empty() {
        let n = read.1.len().min(buf.len());
        buf[0..n].copy_from_slice(&read.1.split_to(n));
        Ok(n)
      } else {
        Pin::new(&mut read.0).read(buf).await
      }
    }
    .try_or_cancel(cancel_handle)
    .await
  }

  async fn write(self: Rc<Self>, buf: &[u8]) -> Result<usize, std::io::Error> {
    let cancel_handle = RcRef::map(self.clone(), |this| &this.cancel_handle);
    let this = self.clone();
    async {
      let wr = RcRef::map(self, |this| &this.write);
      let mut wr = wr.borrow_mut().await;
      match std::mem::replace(&mut *wr, UpgradeStreamWriteState::Failed) {
        UpgradeStreamWriteState::Failed => {
          Err(std::io::Error::other(HttpNextError::RawUpgradeFailed))
        }
        UpgradeStreamWriteState::Rejected => {
          // The upgrade was rejected and the response was already sent.
          // Silently accept writes but don't do anything with them.
          *wr = UpgradeStreamWriteState::Rejected;
          Ok(buf.len())
        }
        UpgradeStreamWriteState::RawParsing(
          mut bytes,
          http,
          conn,
          mut read_cell,
        ) => {
          let prev_len = bytes.len();
          bytes.extend_from_slice(buf);

          let mut headers = [httparse::EMPTY_HEADER; 16];
          let mut response = httparse::Response::new(&mut headers);
          match response.parse(&bytes) {
            Ok(httparse::Status::Partial) => {
              *wr = UpgradeStreamWriteState::RawParsing(
                bytes, http, conn, read_cell,
              );
              Ok(buf.len())
            }
            Ok(httparse::Status::Complete(n)) => {
              let status_code = response.code.unwrap_or(0);

              if status_code != StatusCode::SWITCHING_PROTOCOLS.as_u16() {
                http.set_status(status_code);
                for header in response.headers {
                  if header.name.is_empty() {
                    continue;
                  }
                  http.append_response_header(
                    header.name.as_bytes().to_vec(),
                    header.value.to_vec(),
                  );
                }
                if bytes.len() > n {
                  http.set_flat_response_body(FlatResponseBody::Bytes(
                    BufView::from(bytes[n..].to_vec()),
                  ));
                } else {
                  http.set_flat_response_body(FlatResponseBody::Empty);
                }
                *wr = UpgradeStreamWriteState::Rejected;
                this.rejected.set(true);
                http.complete();
                return Ok(buf.len());
              }

              let (stream, head_bytes) = take_raw_upgrade_stream(&RawUpgrade {
                conn,
                websocket_tx: RefCell::new(None),
              })
              .map_err(std::io::Error::other)?;
              let (read, mut write) = stream.into_split();

              let mut written = 0;
              while written < n {
                written +=
                  Pin::new(&mut write).write(&bytes[written..n]).await?;
              }

              let _ = read_cell.insert((read, head_bytes));

              if status_code == StatusCode::SWITCHING_PROTOCOLS.as_u16() {
                *wr = UpgradeStreamWriteState::Network(write);
              } else {
                *wr = UpgradeStreamWriteState::Rejected;
                this.rejected.set(true);
              }

              http.complete();
              Ok(n - prev_len)
            }
            Err(e) => Err(std::io::Error::other(e)),
          }
        }
        UpgradeStreamWriteState::Parsing(
          mut bytes,
          http,
          on_upgrade,
          mut read_cell,
        ) => {
          let prev_len = bytes.len();
          bytes.extend_from_slice(buf);

          let mut headers = [httparse::EMPTY_HEADER; 16];
          let mut response = httparse::Response::new(&mut headers);
          match response.parse(&bytes) {
            Ok(httparse::Status::Partial) => {
              *wr = UpgradeStreamWriteState::Parsing(
                bytes, http, on_upgrade, read_cell,
              );
              Ok(buf.len())
            }
            Ok(httparse::Status::Complete(n)) => {
              let status_code = response.code.unwrap_or(0);

              if status_code == StatusCode::SWITCHING_PROTOCOLS.as_u16() {
                let status = StatusCode::from_u16(status_code)
                  .unwrap_or(StatusCode::SWITCHING_PROTOCOLS);
                http.otel_info_set_status(status.as_u16());
                http.response_parts().status = status;

                for header in response.headers {
                  http.response_parts().headers.append(
                    HeaderName::from_bytes(header.name.as_bytes())
                      .map_err(std::io::Error::other)?,
                    HeaderValue::from_bytes(header.value)
                      .map_err(std::io::Error::other)?,
                  );
                }

                http.complete();

                let upgraded =
                  on_upgrade.await.map_err(std::io::Error::other)?;
                let (stream, bytes) = extract_network_stream(upgraded);
                let (read, write) = stream.into_split();

                let _ = read_cell.insert((read, bytes));
                *wr = UpgradeStreamWriteState::Network(write);

                Ok(n - prev_len)
              } else {
                // Upgrade rejected - send the rejection response through hyper
                http.otel_info_set_status(status_code);
                http.response_parts().status =
                  StatusCode::from_u16(status_code)
                    .unwrap_or(StatusCode::BAD_REQUEST);

                for header in response.headers {
                  http.response_parts().headers.append(
                    HeaderName::from_bytes(header.name.as_bytes())
                      .map_err(std::io::Error::other)?,
                    HeaderValue::from_bytes(header.value)
                      .map_err(std::io::Error::other)?,
                  );
                }

                // Any data after the headers is the response body
                let body = bytes.split_off(n);
                if !body.is_empty() {
                  http.set_response_body(ResponseBytesInner::Bytes(
                    BufView::from(body.freeze()),
                  ));
                }

                http.complete();

                // Mark as rejected - no upgrade will happen
                *wr = UpgradeStreamWriteState::Rejected;
                this.rejected.set(true);
                // Drop the on_upgrade future since we're not upgrading
                drop(on_upgrade);

                Ok(buf.len())
              }
            }
            Err(e) => Err(std::io::Error::other(e)),
          }
        }
        UpgradeStreamWriteState::Network(mut stream) => {
          let r = Pin::new(&mut stream).write(buf).await;
          *wr = UpgradeStreamWriteState::Network(stream);
          r
        }
      }
    }
    .try_or_cancel(cancel_handle)
    .await
  }

  async fn write_vectored(
    self: Rc<Self>,
    buf1: &[u8],
    buf2: &[u8],
  ) -> Result<usize, std::io::Error> {
    let mut wr = RcRef::map(&self, |r| &r.write).borrow_mut().await;

    match &mut *wr {
      UpgradeStreamWriteState::Failed => {
        Err(std::io::Error::other(HttpNextError::RawUpgradeFailed))
      }
      UpgradeStreamWriteState::Rejected => {
        // The upgrade was rejected; silently accept writes
        Ok(buf1.len() + buf2.len())
      }
      UpgradeStreamWriteState::RawParsing(..) => {
        drop(wr);
        self.write(if buf1.is_empty() { buf2 } else { buf1 }).await
      }
      UpgradeStreamWriteState::Parsing(..) => {
        drop(wr);
        self.write(if buf1.is_empty() { buf2 } else { buf1 }).await
      }
      UpgradeStreamWriteState::Network(stream) => {
        let bufs = [std::io::IoSlice::new(buf1), std::io::IoSlice::new(buf2)];
        stream.write_vectored(&bufs).await
      }
    }
  }
}

impl Resource for UpgradeStream {
  fn name(&self) -> Cow<'_, str> {
    "httpRawUpgradeStream".into()
  }

  deno_core::impl_readable_byob!();
  deno_core::impl_writable!();

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
  }
}

#[op2(fast)]
pub fn op_can_write_vectored(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> bool {
  state.resource_table.get::<UpgradeStream>(rid).is_ok()
}

#[op2]
#[number]
pub async fn op_raw_write_vectored(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] buf1: JsBuffer,
  #[buffer] buf2: JsBuffer,
) -> Result<usize, HttpNextError> {
  let resource: Rc<UpgradeStream> =
    state.borrow().resource_table.get::<UpgradeStream>(rid)?;
  let nwritten = resource.write_vectored(&buf1, &buf2).await?;
  Ok(nwritten)
}

#[op2(fast)]
pub fn op_http_metric_handle_otel_error(external: *const c_void) {
  // SAFETY: The external may have already been consumed by a take_external!
  // call. Gracefully skip if the pointer is no longer valid.
  let Some(http) = (unsafe { try_clone_external!(external) }) else {
    return;
  };

  http.otel_info_set_error("user");
}

#[op2(fast)]
pub fn op_http_copy_span_to_otel_info(
  external: *const c_void,
  #[cppgc] span: &deno_telemetry::OtelSpan,
) {
  // SAFETY: The external may have already been consumed by a take_external!
  // call (e.g. op_http_set_promise_complete). On some platforms (Windows),
  // the freed memory may be reused, causing the marker check to fail.
  // We gracefully skip the copy in that case rather than panicking.
  let Some(http) = (unsafe { try_clone_external!(external) }) else {
    return;
  };

  http.copy_span_to_otel_info(span);
}
