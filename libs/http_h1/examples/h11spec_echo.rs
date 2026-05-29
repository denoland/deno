// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::print_stderr, reason = "example binary")]

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::str;

use deno_http_h1::BodyKind;
use deno_http_h1::Error as H1Error;
use deno_http_h1::Header;
use deno_http_h1::Response;
use deno_http_h1::ResponseBody;
use deno_http_h1::ResponseHead;
use deno_http_h1::SharedConn;
use deno_http_h1::SharedScratch;
use deno_http_h1::Version;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

const CAPABILITIES: &[u8] = b"echo, close, chunked, trailers, expect-100";
const DATE: &[u8] = b"Thu, 21 May 2026 12:00:00 GMT";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
  let port = env::args()
    .nth(1)
    .and_then(|value| value.parse().ok())
    .unwrap_or(8080);
  let listener = TcpListener::bind(("127.0.0.1", port)).await?;
  eprintln!("h11spec_echo listening on http://127.0.0.1:{port}");
  loop {
    let (stream, _) = listener.accept().await?;
    tokio::spawn(async move {
      if let Err(err) = serve(stream).await {
        eprintln!("connection error: {err}");
      }
    });
  }
}

async fn serve(stream: TcpStream) -> Result<(), H1Error> {
  let mut conn = SharedConn::new(stream);
  let mut scratch = SharedScratch::default();
  loop {
    let request = match std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, OwnedRequest::from_request)
    })
    .await
    {
      Ok(Some(request)) => request,
      Ok(None) => return Ok(()),
      Err(_) => {
        write_simple(&mut conn, &mut scratch, 400, false).await?;
        return Ok(());
      }
    };

    if let Some(status) = validate_request(&request) {
      write_simple(&mut conn, &mut scratch, status, false).await?;
      return Ok(());
    }

    if request.has_unknown_expect() {
      write_simple(&mut conn, &mut scratch, 417, false).await?;
      return Ok(());
    }
    if request.expect_continue && !request.suppresses_continue() {
      conn.write_continue().await?;
    }

    if request.requires_length() && !request.has_length_indicator() {
      write_simple(&mut conn, &mut scratch, 411, false).await?;
      return Ok(());
    }

    let mut body = Vec::new();
    if let Err(_err) = conn
      .read_body_to_end_with_scratch(&mut scratch, &mut body)
      .await
    {
      write_simple(&mut conn, &mut scratch, 400, false).await?;
      return Ok(());
    }

    let keep_alive =
      request.keep_alive && !request.has_token(b"connection", b"close");
    write_echo(&mut conn, &mut scratch, &request, &body, keep_alive).await?;
    if !keep_alive {
      return Ok(());
    }
  }
}

#[derive(Debug)]
struct OwnedRequest {
  method: Vec<u8>,
  target: Vec<u8>,
  version: Version,
  headers: Vec<(Vec<u8>, Vec<u8>)>,
  body: BodyKind,
  keep_alive: bool,
  expect_continue: bool,
}

impl OwnedRequest {
  fn from_request(request: deno_http_h1::Request<'_>) -> Self {
    Self {
      method: request.method.to_vec(),
      target: request.target.to_vec(),
      version: request.version,
      headers: request
        .headers
        .iter()
        .map(|header| (header.name.to_vec(), header.value.to_vec()))
        .collect(),
      body: request.body,
      keep_alive: request.keep_alive,
      expect_continue: request.expect_continue,
    }
  }

  fn header_values<'a>(
    &'a self,
    name: &[u8],
  ) -> impl Iterator<Item = &'a [u8]> + 'a {
    let name = name.to_vec();
    self
      .headers
      .iter()
      .filter(move |(header_name, _)| header_name.eq_ignore_ascii_case(&name))
      .map(|(_, value)| value.as_slice())
  }

  fn has_header(&self, name: &[u8]) -> bool {
    self.header_values(name).next().is_some()
  }

  fn first_header(&self, name: &[u8]) -> Option<&[u8]> {
    self.header_values(name).next()
  }

  fn has_token(&self, name: &[u8], token: &[u8]) -> bool {
    self.header_values(name).any(|value| {
      value
        .split(|byte| *byte == b',')
        .map(trim_ows)
        .any(|value| value.eq_ignore_ascii_case(token))
    })
  }

  fn has_unknown_expect(&self) -> bool {
    self
      .header_values(b"expect")
      .any(|value| !trim_ows(value).eq_ignore_ascii_case(b"100-continue"))
  }

  fn suppresses_continue(&self) -> bool {
    self
      .first_header(b"x-test-send-100-continue")
      .is_some_and(|value| trim_ows(value).eq_ignore_ascii_case(b"never"))
  }

  fn requires_length(&self) -> bool {
    self.method == b"POST" || self.method == b"PUT" || self.method == b"PATCH"
  }

  fn has_length_indicator(&self) -> bool {
    self.has_header(b"content-length")
      || matches!(self.body, BodyKind::Chunked | BodyKind::Upgrade)
  }
}

fn validate_request(request: &OwnedRequest) -> Option<u16> {
  if request.target.len() > 8 * 1024 {
    return Some(414);
  }
  if request.version == Version::Http11 {
    match request.header_values(b"host").count() {
      0 => return Some(400),
      1 => {}
      _ => return Some(400),
    }
    if !valid_host(request.first_header(b"host").unwrap_or_default()) {
      return Some(400);
    }
  }

  if request.target == b"*" {
    if request.method != b"OPTIONS" {
      return Some(400);
    }
  } else if request.target.starts_with(b"/")
    || request.target.starts_with(b"http://")
    || request.target.starts_with(b"https://")
  {
  } else if request.method != b"CONNECT" {
    return Some(400);
  }
  None
}

async fn write_echo(
  conn: &mut SharedConn<TcpStream>,
  scratch: &mut SharedScratch,
  request: &OwnedRequest,
  request_body: &[u8],
  keep_alive: bool,
) -> Result<(), H1Error> {
  let mut status = request
    .first_header(b"x-test-status")
    .and_then(|value| str::from_utf8(trim_ows(value)).ok())
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(200);
  if request.method == b"OPTIONS" && status == 200 {
    status = 204;
  }

  let mut response_body =
    if let Some(body) = request.first_header(b"x-test-body") {
      trim_ows(body).to_vec()
    } else {
      build_echo_body(request, request_body)
    };
  if !status_allows_body(status) {
    response_body.clear();
  }

  let mut header_bytes: Vec<(Vec<u8>, Vec<u8>)> = vec![
    (b"Date".to_vec(), DATE.to_vec()),
    (b"Server".to_vec(), b"deno_http_h1/h11spec".to_vec()),
    (b"Content-Type".to_vec(), b"application/json".to_vec()),
  ];
  if request.method == b"OPTIONS" {
    header_bytes.push((
      b"Allow".to_vec(),
      b"GET, HEAD, POST, PUT, DELETE, OPTIONS, TRACE".to_vec(),
    ));
    if request.target == b"*" {
      header_bytes
        .push((b"X-H11spec-Capabilities".to_vec(), CAPABILITIES.to_vec()));
    }
  }
  if status == 401 {
    header_bytes.push((
      b"WWW-Authenticate".to_vec(),
      b"Basic realm=\"h11spec\"".to_vec(),
    ));
  } else if status == 405 {
    header_bytes.push((b"Allow".to_vec(), b"GET, HEAD, OPTIONS".to_vec()));
  } else if status == 407 {
    header_bytes.push((
      b"Proxy-Authenticate".to_vec(),
      b"Basic realm=\"h11spec\"".to_vec(),
    ));
  } else if status == 426 {
    header_bytes.push((b"Upgrade".to_vec(), b"HTTP/2".to_vec()));
    header_bytes.push((b"Connection".to_vec(), b"Upgrade".to_vec()));
  }
  if matches!(status, 201 | 301 | 302 | 303 | 307 | 308) {
    header_bytes
      .push((b"Location".to_vec(), b"/created-or-redirected".to_vec()));
  }
  for value in request.header_values(b"x-test-header") {
    if let Some((name, value)) = parse_header_control(value) {
      header_bytes.push((name, value));
    }
  }
  let mut trailer_bytes = Vec::new();
  for value in request.header_values(b"x-test-trailer") {
    if let Some((name, value)) = parse_header_control(value) {
      trailer_bytes.push((name, value));
    }
  }
  let forced_chunked = request
    .first_header(b"x-test-chunked")
    .is_some_and(|value| trim_ows(value) == b"1")
    || !trailer_bytes.is_empty();
  if !trailer_bytes.is_empty() {
    let mut names = Vec::new();
    for (index, (name, _)) in trailer_bytes.iter().enumerate() {
      if index > 0 {
        names.extend_from_slice(b", ");
      }
      names.extend_from_slice(name);
    }
    header_bytes.push((b"Trailer".to_vec(), names));
  }

  let response_headers = header_bytes
    .iter()
    .map(|(name, value)| Header {
      name: name.as_slice(),
      value: value.as_slice(),
    })
    .collect::<Vec<_>>();
  if forced_chunked && request.method != b"HEAD" && status_allows_body(status) {
    conn
      .start_chunked_response_with_scratch(
        scratch,
        ResponseHead {
          version: request.version,
          status,
          reason: reason_for(status),
          headers: &response_headers,
          keep_alive,
        },
      )
      .await?;
    if !response_body.is_empty() {
      conn
        .write_response_chunk_with_scratch(scratch, &response_body)
        .await?;
    }
    let trailers = trailer_bytes
      .iter()
      .map(|(name, value)| Header {
        name: name.as_slice(),
        value: value.as_slice(),
      })
      .collect::<Vec<_>>();
    return conn.finish_response_with_scratch(scratch, &trailers).await;
  }
  let body = if request.method == b"HEAD" {
    ResponseBody::Head(Some(response_body.len() as u64))
  } else if response_body.is_empty() {
    ResponseBody::Empty
  } else {
    ResponseBody::Bytes(&response_body)
  };
  conn
    .write_response_with_scratch(
      scratch,
      Response {
        version: request.version,
        status,
        reason: reason_for(status),
        headers: &response_headers,
        body,
        keep_alive,
      },
    )
    .await
}

async fn write_simple(
  conn: &mut SharedConn<TcpStream>,
  scratch: &mut SharedScratch,
  status: u16,
  keep_alive: bool,
) -> Result<(), H1Error> {
  let headers = [Header {
    name: b"Date",
    value: DATE,
  }];
  conn
    .write_response_with_scratch(
      scratch,
      Response {
        version: Version::Http11,
        status,
        reason: reason_for(status),
        headers: &headers,
        body: ResponseBody::Empty,
        keep_alive,
      },
    )
    .await
}

fn build_echo_body(request: &OwnedRequest, body: &[u8]) -> Vec<u8> {
  let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
  for (name, value) in &request.headers {
    grouped
      .entry(String::from_utf8_lossy(name).into_owned())
      .or_default()
      .push(String::from_utf8_lossy(value).into_owned());
  }

  let mut out = String::new();
  out.push('{');
  out.push_str("\"method\":");
  push_json_str(&mut out, &String::from_utf8_lossy(&request.method));
  out.push_str(",\"target\":");
  push_json_str(&mut out, &String::from_utf8_lossy(&request.target));
  out.push_str(",\"headers\":{");
  let mut first_header = true;
  for (name, values) in grouped {
    if !first_header {
      out.push(',');
    }
    first_header = false;
    push_json_str(&mut out, &name);
    out.push_str(":[");
    for (index, value) in values.iter().enumerate() {
      if index > 0 {
        out.push(',');
      }
      push_json_str(&mut out, value);
    }
    out.push(']');
  }
  out.push_str("},\"body\":");
  push_json_str(&mut out, &String::from_utf8_lossy(body));
  out.push('}');
  out.into_bytes()
}

fn parse_header_control(value: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
  let value = str::from_utf8(trim_ows(value)).ok()?;
  let (name, value) = value.split_once(':')?;
  Some((
    name.trim().as_bytes().to_vec(),
    value.trim().as_bytes().to_vec(),
  ))
}

fn push_json_str(out: &mut String, value: &str) {
  out.push('"');
  for ch in value.chars() {
    match ch {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      ch if (ch as u32) < 0x20 => {
        out.push_str("\\u");
        push_hex4(out, ch as u32);
      }
      ch => out.push(ch),
    }
  }
  out.push('"');
}

fn push_hex4(out: &mut String, value: u32) {
  for shift in [12, 8, 4, 0] {
    let digit = ((value >> shift) & 0xf) as u8;
    out.push(match digit {
      0..=9 => (b'0' + digit) as char,
      _ => (b'a' + digit - 10) as char,
    });
  }
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
  while matches!(value.first(), Some(b' ' | b'\t')) {
    value = &value[1..];
  }
  while matches!(value.last(), Some(b' ' | b'\t')) {
    value = &value[..value.len() - 1];
  }
  value
}

fn valid_host(value: &[u8]) -> bool {
  let value = trim_ows(value);
  if value.is_empty() {
    return false;
  }
  let Ok(value) = str::from_utf8(value) else {
    return false;
  };
  let Some((_, port)) = value.rsplit_once(':') else {
    return true;
  };
  if value.starts_with('[') {
    return value.contains(']') && port.parse::<u16>().is_ok();
  }
  port.parse::<u16>().is_ok()
}

fn status_allows_body(status: u16) -> bool {
  !((100..200).contains(&status) || status == 204 || status == 304)
}

fn reason_for(status: u16) -> &'static [u8] {
  match status {
    100 => b"Continue",
    200 => b"OK",
    201 => b"Created",
    202 => b"Accepted",
    204 => b"No Content",
    205 => b"Reset Content",
    301 => b"Moved Permanently",
    302 => b"Found",
    303 => b"See Other",
    304 => b"Not Modified",
    307 => b"Temporary Redirect",
    308 => b"Permanent Redirect",
    400 => b"Bad Request",
    401 => b"Unauthorized",
    405 => b"Method Not Allowed",
    407 => b"Proxy Authentication Required",
    409 => b"Conflict",
    410 => b"Gone",
    411 => b"Length Required",
    413 => b"Content Too Large",
    414 => b"URI Too Long",
    415 => b"Unsupported Media Type",
    417 => b"Expectation Failed",
    422 => b"Unprocessable Content",
    426 => b"Upgrade Required",
    429 => b"Too Many Requests",
    431 => b"Request Header Fields Too Large",
    500 => b"Internal Server Error",
    502 => b"Bad Gateway",
    503 => b"Service Unavailable",
    _ => b"OK",
  }
}
