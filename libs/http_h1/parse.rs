// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;

pub const MAX_HEADERS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
  Http10,
  Http11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
  Empty,
  ContentLength(u64),
  Chunked,
  Upgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header<'a> {
  pub name: &'a [u8],
  pub value: &'a [u8],
}

impl Header<'_> {
  pub const EMPTY: Self = Self {
    name: &[],
    value: &[],
  };
}

#[derive(Debug, PartialEq, Eq)]
pub struct RequestHead<'a> {
  pub method: &'a [u8],
  pub target: &'a [u8],
  pub version: Version,
  pub headers: &'a [Header<'a>],
  pub body_kind: BodyKind,
  pub keep_alive: bool,
  pub expect_continue: bool,
  pub consumed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
  TooManyHeaders,
  Invalid,
  MissingMethod,
  MissingTarget,
  MissingVersion,
  MissingHost,
  MultipleHost,
  InvalidHeaderValue,
  InvalidContentLength,
  ConflictingContentLength,
  UnsupportedTransferEncoding,
}

pub fn parse_request_head<'a>(
  buf: &'a [u8],
  headers_out: &'a mut [Header<'a>],
) -> Result<Option<RequestHead<'a>>, ParseError> {
  let mut headers = [const { MaybeUninit::uninit() }; MAX_HEADERS];
  let parsed = parse_request_head_uninit(buf, headers_out, &mut headers)?;
  Ok(parsed)
}

pub fn parse_request_head_uninit<'a>(
  buf: &'a [u8],
  headers_out: &'a mut [Header<'a>],
  parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
) -> Result<Option<RequestHead<'a>>, ParseError> {
  parse_request_head_uninit_with_options(buf, headers_out, parse_headers, false)
}

pub fn parse_request_head_uninit_with_options<'a>(
  buf: &'a [u8],
  headers_out: &'a mut [Header<'a>],
  parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
  allow_missing_host: bool,
) -> Result<Option<RequestHead<'a>>, ParseError> {
  let mut request = parse_with_header_scratch(buf, parse_headers)?;
  let Some(consumed) = request.0 else {
    return Ok(None);
  };
  finish_request_head(
    buf,
    headers_out,
    &mut request.1,
    consumed,
    allow_missing_host,
  )
}

pub fn parse_request_head_uninit_all<'a>(
  buf: &'a [u8],
  headers_out: &'a mut [MaybeUninit<Header<'a>>],
  parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
) -> Result<Option<RequestHead<'a>>, ParseError> {
  parse_request_head_uninit_all_with_options(
    buf,
    headers_out,
    parse_headers,
    false,
  )
}

pub fn parse_request_head_uninit_all_with_options<'a>(
  buf: &'a [u8],
  headers_out: &'a mut [MaybeUninit<Header<'a>>],
  parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
  allow_missing_host: bool,
) -> Result<Option<RequestHead<'a>>, ParseError> {
  let mut request = parse_with_header_scratch(buf, parse_headers)?;
  let Some(consumed) = request.0 else {
    return Ok(None);
  };
  finish_request_head_uninit(
    buf,
    headers_out,
    &mut request.1,
    consumed,
    allow_missing_host,
  )
}

fn parse_with_header_scratch<'a, 'headers>(
  buf: &'a [u8],
  parse_headers: &'headers mut [MaybeUninit<httparse::Header<'a>>],
) -> Result<(Option<usize>, httparse::Request<'headers, 'a>), ParseError> {
  let mut request = httparse::Request {
    method: None,
    path: None,
    version: None,
    headers: &mut [],
  };
  let consumed = match request.parse_with_uninit_headers(buf, parse_headers) {
    Ok(httparse::Status::Complete(consumed)) => consumed,
    Ok(httparse::Status::Partial) => return Ok((None, request)),
    Err(httparse::Error::TooManyHeaders) => {
      return Err(ParseError::TooManyHeaders);
    }
    Err(_) => return Err(ParseError::Invalid),
  };
  Ok((Some(consumed), request))
}

fn finish_request_head<'a>(
  _buf: &'a [u8],
  headers_out: &'a mut [Header<'a>],
  request: &mut httparse::Request<'_, 'a>,
  consumed: usize,
  allow_missing_host: bool,
) -> Result<Option<RequestHead<'a>>, ParseError> {
  let method = request.method.ok_or(ParseError::MissingMethod)?.as_bytes();
  let target = request.path.ok_or(ParseError::MissingTarget)?.as_bytes();
  let version = match request.version.ok_or(ParseError::MissingVersion)? {
    0 => Version::Http10,
    1 => Version::Http11,
    _ => return Err(ParseError::Invalid),
  };
  if !valid_request_target(method, target) {
    return Err(ParseError::Invalid);
  }

  if request.headers.len() > headers_out.len() {
    return Err(ParseError::TooManyHeaders);
  }
  let mut header_info = HeaderInfo::default();
  let header_len = request.headers.len();
  for (index, header) in request.headers.iter().enumerate() {
    if !valid_header_value(header.value) {
      return Err(ParseError::InvalidHeaderValue);
    }
    let value = trim_ows(header.value);
    header_info.observe(header.name.as_bytes(), value);
    headers_out[index] = Header {
      name: header.name.as_bytes(),
      value,
    };
  }
  let headers_out = &headers_out[..header_len];

  let keep_alive = header_info.keep_alive(version);
  let body_kind = header_info.body_kind(method, version)?;
  header_info.validate_host(version, body_kind, allow_missing_host)?;
  let expect_continue = header_info.expect_continue;

  Ok(Some(RequestHead {
    method,
    target,
    version,
    headers: headers_out,
    body_kind,
    keep_alive,
    expect_continue,
    consumed,
  }))
}

fn finish_request_head_uninit<'a>(
  _buf: &'a [u8],
  headers_out: &'a mut [MaybeUninit<Header<'a>>],
  request: &mut httparse::Request<'_, 'a>,
  consumed: usize,
  allow_missing_host: bool,
) -> Result<Option<RequestHead<'a>>, ParseError> {
  let method = request.method.ok_or(ParseError::MissingMethod)?.as_bytes();
  let target = request.path.ok_or(ParseError::MissingTarget)?.as_bytes();
  let version = match request.version.ok_or(ParseError::MissingVersion)? {
    0 => Version::Http10,
    1 => Version::Http11,
    _ => return Err(ParseError::Invalid),
  };
  if !valid_request_target(method, target) {
    return Err(ParseError::Invalid);
  }

  if request.headers.len() > headers_out.len() {
    return Err(ParseError::TooManyHeaders);
  }
  let mut header_info = HeaderInfo::default();
  let header_len = request.headers.len();
  for (index, header) in request.headers.iter().enumerate() {
    if !valid_header_value(header.value) {
      return Err(ParseError::InvalidHeaderValue);
    }
    let value = trim_ows(header.value);
    header_info.observe(header.name.as_bytes(), value);
    headers_out[index].write(Header {
      name: header.name.as_bytes(),
      value,
    });
  }
  // SAFETY: the loop above initialized exactly `header_len` entries, and
  // `Header` does not require drop glue.
  let headers_out = unsafe {
    std::slice::from_raw_parts(
      headers_out.as_ptr() as *const Header<'a>,
      header_len,
    )
  };

  let keep_alive = header_info.keep_alive(version);
  let body_kind = header_info.body_kind(method, version)?;
  header_info.validate_host(version, body_kind, allow_missing_host)?;
  let expect_continue = header_info.expect_continue;

  Ok(Some(RequestHead {
    method,
    target,
    version,
    headers: headers_out,
    body_kind,
    keep_alive,
    expect_continue,
    consumed,
  }))
}

#[derive(Default)]
struct HeaderInfo {
  host_count: usize,
  connection_close: bool,
  connection_keep_alive: bool,
  connection_upgrade: bool,
  has_upgrade: bool,
  has_transfer_encoding: bool,
  transfer_encoding_invalid: bool,
  transfer_encoding_saw_chunked: bool,
  transfer_encoding_last_was_chunked: bool,
  content_length: Option<u64>,
  content_length_error: Option<ParseError>,
  expect_continue: bool,
}

impl HeaderInfo {
  fn observe(&mut self, name: &[u8], value: &[u8]) {
    if eq_ignore_ascii_case(name, b"host") {
      self.host_count += 1;
    } else if eq_ignore_ascii_case(name, b"connection") {
      self.observe_connection(value);
    } else if eq_ignore_ascii_case(name, b"upgrade") {
      self.has_upgrade = true;
    } else if eq_ignore_ascii_case(name, b"transfer-encoding") {
      self.observe_transfer_encoding(value);
    } else if eq_ignore_ascii_case(name, b"content-length") {
      self.observe_content_length(value);
    } else if eq_ignore_ascii_case(name, b"expect") {
      self.expect_continue = eq_ignore_ascii_case(value, b"100-continue");
    }
  }

  fn observe_connection(&mut self, value: &[u8]) {
    for token in comma_tokens(value) {
      if eq_ignore_ascii_case(token, b"close") {
        self.connection_close = true;
      } else if eq_ignore_ascii_case(token, b"keep-alive") {
        self.connection_keep_alive = true;
      } else if eq_ignore_ascii_case(token, b"upgrade") {
        self.connection_upgrade = true;
      }
    }
  }

  fn observe_transfer_encoding(&mut self, value: &[u8]) {
    self.has_transfer_encoding = true;
    for token in comma_tokens(value) {
      if token.is_empty() || eq_ignore_ascii_case(token, b"identity") {
        self.transfer_encoding_invalid = true;
        self.transfer_encoding_last_was_chunked = false;
        continue;
      }
      let chunked = eq_ignore_ascii_case(token, b"chunked");
      self.transfer_encoding_last_was_chunked = chunked;
      if chunked {
        if self.transfer_encoding_saw_chunked {
          self.transfer_encoding_invalid = true;
        }
        self.transfer_encoding_saw_chunked = true;
      }
    }
  }

  fn observe_content_length(&mut self, value: &[u8]) {
    if self.content_length_error.is_some() {
      return;
    }
    let len = match parse_content_length(value) {
      Ok(len) => len,
      Err(err) => {
        self.content_length_error = Some(err);
        return;
      }
    };
    match self.content_length {
      Some(existing) if existing != len => {
        self.content_length_error = Some(ParseError::ConflictingContentLength);
      }
      Some(_) => {}
      None => self.content_length = Some(len),
    }
  }

  fn keep_alive(&self, version: Version) -> bool {
    if self.connection_close {
      return false;
    }
    match version {
      Version::Http11 => true,
      Version::Http10 => self.connection_keep_alive,
    }
  }

  fn validate_host(
    &self,
    version: Version,
    body_kind: BodyKind,
    allow_missing_host: bool,
  ) -> Result<(), ParseError> {
    if version != Version::Http11 {
      return Ok(());
    }
    match self.host_count {
      0 if allow_missing_host => Ok(()),
      0 if body_kind == BodyKind::Empty => Ok(()),
      0 => Err(ParseError::MissingHost),
      1 => Ok(()),
      _ => Err(ParseError::MultipleHost),
    }
  }

  fn body_kind(
    &self,
    method: &[u8],
    version: Version,
  ) -> Result<BodyKind, ParseError> {
    if self.connection_upgrade && self.has_upgrade {
      if self.has_transfer_encoding
        || self.content_length.is_some()
        || self.content_length_error.is_some()
      {
        return Err(ParseError::ConflictingContentLength);
      }
      return Ok(BodyKind::Upgrade);
    }

    if self.has_transfer_encoding {
      if self.content_length.is_some() || self.content_length_error.is_some() {
        return Err(ParseError::ConflictingContentLength);
      }
      if version == Version::Http10 {
        return Err(ParseError::UnsupportedTransferEncoding);
      }
      if !self.transfer_encoding_invalid
        && self.transfer_encoding_saw_chunked
        && self.transfer_encoding_last_was_chunked
      {
        return Ok(BodyKind::Chunked);
      }
      return Err(ParseError::UnsupportedTransferEncoding);
    }

    if let Some(error) = self.content_length_error {
      return Err(error);
    }

    if method == b"CONNECT" {
      return Ok(BodyKind::Upgrade);
    }

    Ok(match self.content_length {
      Some(0) | None => BodyKind::Empty,
      Some(len) => BodyKind::ContentLength(len),
    })
  }
}

fn comma_tokens(value: &[u8]) -> impl Iterator<Item = &[u8]> {
  value.split(|byte| *byte == b',').map(trim_ows)
}

fn parse_content_length(value: &[u8]) -> Result<u64, ParseError> {
  if value.contains(&b',') {
    return Err(ParseError::InvalidContentLength);
  }
  let value = trim_ows(value);
  if value.is_empty() {
    return Err(ParseError::InvalidContentLength);
  }
  let mut len = 0u64;
  for byte in value {
    if !byte.is_ascii_digit() {
      return Err(ParseError::InvalidContentLength);
    }
    len = len
      .checked_mul(10)
      .and_then(|len| len.checked_add((byte - b'0') as u64))
      .ok_or(ParseError::InvalidContentLength)?;
  }
  Ok(len)
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

fn valid_header_value(value: &[u8]) -> bool {
  value
    .iter()
    .all(|byte| matches!(*byte, b'\t' | b' '..=0x7e | 0x80..=0xff))
}

fn valid_request_target(method: &[u8], target: &[u8]) -> bool {
  if method == b"CONNECT" {
    return !target.is_empty()
      && !matches!(target[0], b'/' | b'*')
      && !is_absolute_form(target);
  }

  if target == b"*" {
    return method == b"OPTIONS";
  }

  matches!(target.first(), Some(b'/')) || is_absolute_form(target)
}

fn is_absolute_form(target: &[u8]) -> bool {
  let Some(scheme_end) = target.windows(3).position(|window| window == b"://")
  else {
    return false;
  };
  if scheme_end == 0 || scheme_end + 3 == target.len() {
    return false;
  }
  let scheme = &target[..scheme_end];
  if !scheme[0].is_ascii_alphabetic() {
    return false;
  }
  scheme[1..].iter().all(|byte| {
    byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'-' | b'.')
  })
}

fn eq_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
  left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse<'a>(
    input: &'a [u8],
    headers: &'a mut [Header<'a>],
  ) -> RequestHead<'a> {
    parse_request_head(input, headers).unwrap().unwrap()
  }

  #[test]
  fn parses_basic_get() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", &mut headers);
    assert_eq!(request.method, b"GET");
    assert_eq!(request.target, b"/");
    assert_eq!(request.version, Version::Http11);
    assert_eq!(request.body_kind, BodyKind::Empty);
    assert!(request.keep_alive);
    assert_eq!(request.consumed, 27);
    assert_eq!(request.headers[0].name, b"Host");
    assert_eq!(request.headers[0].value, b"x");
  }

  #[test]
  fn returns_partial() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(b"GET / HTTP/1.1\r\n", &mut headers).unwrap(),
      None
    );
  }

  #[test]
  fn parses_http10_keep_alive() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"GET / HTTP/1.0\r\nConnection: foo, keep-alive, bar\r\n\r\n",
      &mut headers,
    );
    assert!(request.keep_alive);
  }

  #[test]
  fn parses_http10_close_by_default() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(b"GET / HTTP/1.0\r\n\r\n", &mut headers);
    assert!(!request.keep_alive);
  }

  #[test]
  fn parses_http11_keep_alive_by_default() {
    let mut headers = [Header::EMPTY; 8];
    let request =
      parse(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n", &mut headers);
    assert!(request.keep_alive);
  }

  #[test]
  fn parses_close_token() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: keep-alive, close\r\n\r\n",
      &mut headers,
    );
    assert!(!request.keep_alive);
  }

  #[test]
  fn parses_duplicate_equal_content_length() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello",
      &mut headers,
    );
    assert_eq!(request.body_kind, BodyKind::ContentLength(5));
  }

  #[test]
  fn rejects_content_length_list() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5, 5\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::InvalidContentLength)
    );
  }

  #[test]
  fn rejects_conflicting_content_length() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::ConflictingContentLength)
    );
  }

  #[test]
  fn parses_chunked() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: gzip, chunked\r\n\r\n",
      &mut headers,
    );
    assert_eq!(request.body_kind, BodyKind::Chunked);
  }

  #[test]
  fn rejects_upgrade_with_content_length() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nContent-Length: 5\r\n\r\nhello",
        &mut headers,
      ),
      Err(ParseError::ConflictingContentLength)
    );
  }

  #[test]
  fn parses_chunked_transfer_encoding_across_header_lines() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n",
      &mut headers,
    );
    assert_eq!(request.body_kind, BodyKind::Chunked);
  }

  #[test]
  fn rejects_transfer_encoding_that_does_not_end_in_chunked() {
    for input in [
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: gzip\r\n\r\n".as_slice(),
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked, gzip\r\n\r\n",
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: gzip\r\n\r\n",
    ] {
      let mut headers = [Header::EMPTY; 8];
      assert_eq!(
        parse_request_head(input, &mut headers),
        Err(ParseError::UnsupportedTransferEncoding)
      );
    }
  }

  #[test]
  fn rejects_http10_transfer_encoding() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"POST / HTTP/1.0\r\nTransfer-Encoding: chunked\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::UnsupportedTransferEncoding)
    );
  }

  #[test]
  fn rejects_duplicate_chunked_transfer_encoding() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked, chunked\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::UnsupportedTransferEncoding)
    );
  }

  #[test]
  fn rejects_duplicate_chunked_transfer_encoding_lines() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::UnsupportedTransferEncoding)
    );
  }

  #[test]
  fn detects_upgrade() {
    let mut headers = [Header::EMPTY; 8];
    let request = parse(
      b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: Upgrade, HTTP2-Settings\r\nUpgrade: h2c\r\n\r\n",
      &mut headers,
    );
    assert_eq!(request.body_kind, BodyKind::Upgrade);
  }

  #[test]
  fn rejects_missing_http11_host() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"GET / HTTP/1.1\r\nContent-Length: 5\r\n\r\n",
        &mut headers
      ),
      Err(ParseError::MissingHost)
    );
  }

  #[test]
  fn rejects_multiple_http11_hosts() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"GET / HTTP/1.1\r\nHost: example.com\r\nHost: example.org\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::MultipleHost)
    );
  }

  #[test]
  fn rejects_invalid_header_value_control_character() {
    let mut headers = [Header::EMPTY; 8];
    assert_eq!(
      parse_request_head(
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Bad: test\x07\r\n\r\n",
        &mut headers,
      ),
      Err(ParseError::Invalid)
    );
  }

  #[test]
  fn validates_request_target_form() {
    for input in [
      b"OPTIONS * HTTP/1.1\r\nHost: example.com\r\n\r\n".as_slice(),
      b"GET http://example.com/path HTTP/1.1\r\nHost: ignored\r\n\r\n",
      b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n",
    ] {
      let mut headers = [Header::EMPTY; 8];
      assert!(matches!(
        parse_request_head(input, &mut headers),
        Ok(Some(_))
      ));
    }

    for input in [
      b"GET * HTTP/1.1\r\nHost: example.com\r\n\r\n".as_slice(),
      b"GET path HTTP/1.1\r\nHost: example.com\r\n\r\n",
      b"GET example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n",
      b"CONNECT /path HTTP/1.1\r\nHost: example.com\r\n\r\n",
      b"CONNECT http://example.com HTTP/1.1\r\nHost: example.com\r\n\r\n",
    ] {
      let mut headers = [Header::EMPTY; 8];
      assert_eq!(
        parse_request_head(input, &mut headers),
        Err(ParseError::Invalid)
      );
    }
  }

  #[derive(Debug, Clone, Copy)]
  enum Expected {
    Partial,
    Ok,
    Err,
  }

  #[test]
  fn h1spec_request_head_subset() {
    // These mirror the request-head classifications from h1spec, but they are
    // not a substitute for the black-box server test. The full suite belongs
    // at the connection/server layer once this crate owns body echoing.
    let cases: &[(&str, &[u8], Expected)] = &[
      ("Fragmented method", b"G", Expected::Partial),
      ("Fragmented URL 1", b"GET ", Expected::Partial),
      ("Fragmented URL 2", b"GET /hello", Expected::Partial),
      ("Fragmented URL 3", b"GET /hello ", Expected::Partial),
      ("Fragmented HTTP version", b"GET /hello HTTP", Expected::Partial),
      (
        "Fragmented request line",
        b"GET /hello HTTP/1.1",
        Expected::Partial,
      ),
      (
        "Fragmented request line newline 1",
        b"GET /hello HTTP/1.1\r",
        Expected::Partial,
      ),
      (
        "Fragmented request line newline 2",
        b"GET /hello HTTP/1.1\r\n",
        Expected::Partial,
      ),
      (
        "Fragmented field name",
        b"GET /hello HTTP/1.1\r\nHos",
        Expected::Partial,
      ),
      (
        "Fragmented field value 1",
        b"GET /hello HTTP/1.1\r\nHost:",
        Expected::Partial,
      ),
      (
        "Fragmented field value 2",
        b"GET /hello HTTP/1.1\r\nHost: ",
        Expected::Partial,
      ),
      (
        "Fragmented field value 3",
        b"GET /hello HTTP/1.1\r\nHost: localhost",
        Expected::Partial,
      ),
      (
        "Fragmented field value 4",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r",
        Expected::Partial,
      ),
      (
        "Fragmented request",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r\n",
        Expected::Partial,
      ),
      (
        "Fragmented request termination",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r",
        Expected::Partial,
      ),
      ("Request without HTTP version", b"GET / \r\n\r\n", Expected::Err),
      (
        "Request with Expect header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nExpect: 100-continue\r\n\r\n",
        Expected::Ok,
      ),
      (
        "Valid GET request",
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
        Expected::Ok,
      ),
      (
        "Valid GET request with edge cases",
        b"GET / HTTP/1.1\r\nhoSt:\texample.com\r\nempty:\r\n\r\n",
        Expected::Ok,
      ),
      (
        "Invalid header characters",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Invalid[]: test\r\n\r\n",
        Expected::Err,
      ),
      (
        "Missing Host header",
        b"GET / HTTP/1.1\r\nContent-Length: 5\r\n\r\n",
        Expected::Err,
      ),
      (
        "Multiple Host headers",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nHost: example.org\r\n\r\n",
        Expected::Err,
      ),
      (
        "Overflowing negative Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: -123456789123456789123456789\r\n\r\n",
        Expected::Err,
      ),
      (
        "Negative Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: -1234\r\n\r\n",
        Expected::Err,
      ),
      (
        "Non-numeric Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: abc\r\n\r\n",
        Expected::Err,
      ),
      (
        "Empty header value",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Empty-Header: \r\n\r\n",
        Expected::Ok,
      ),
      (
        "Header containing invalid control character",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Bad-Control-Char: test\x07\r\n\r\n",
        Expected::Err,
      ),
      (
        "Invalid HTTP version",
        b"GET / HTTP/9.9\r\nHost: example.com\r\n\r\n",
        Expected::Err,
      ),
      (
        "Invalid prefix of request",
        b"Extra lineGET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
        Expected::Err,
      ),
      (
        "Invalid line ending",
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\rSome-Header: Test\r\n\r\n",
        Expected::Err,
      ),
      (
        "Valid POST request with body",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhello",
        Expected::Ok,
      ),
      (
        "Chunked Transfer-Encoding",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
        Expected::Ok,
      ),
      (
        "Conflicting Transfer-Encoding and Content-Length in varying case",
        b"POST / HTTP/1.1\r\nHost: example.com\r\ncontent-LengtH: 5\r\nTransFer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
        Expected::Err,
      ),
    ];

    for (description, input, expected) in cases {
      let mut headers = [Header::EMPTY; MAX_HEADERS];
      let result = parse_request_head(input, &mut headers);
      match expected {
        Expected::Partial => {
          assert_eq!(result, Ok(None), "{description}");
        }
        Expected::Ok => {
          assert!(matches!(result, Ok(Some(_))), "{description}: {result:?}");
        }
        Expected::Err => {
          assert!(result.is_err(), "{description}: {result:?}");
        }
      }
    }
  }
}
