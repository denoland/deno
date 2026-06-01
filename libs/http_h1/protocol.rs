// Copyright 2018-2026 the Deno authors. MIT license.

use std::mem::MaybeUninit;

use crate::BodyKind;
use crate::Header;
use crate::ParseError;
use crate::RequestHead;
use crate::Version;
use crate::parse_request_head;
use crate::parse_request_head_uninit;
use crate::parse_request_head_uninit_all_with_options;
use crate::parse_request_head_uninit_with_options;

const MAX_CHUNK_LINE_BYTES: usize = 4096;
const MAX_TRAILER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
  Parse(ParseError),
  HeadTooLarge,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CoreRequest<'a> {
  pub method: &'a [u8],
  pub target: &'a [u8],
  pub version: Version,
  pub headers: &'a [Header<'a>],
  pub body: BodyKind,
  pub keep_alive: bool,
  pub expect_continue: bool,
  pub upgrade: Option<CoreUpgradeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreUpgradeKind {
  Any,
  H2c,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RequestStatus<'a> {
  Complete {
    request: CoreRequest<'a>,
    consumed: usize,
  },
  Partial,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BodyStatus<'a> {
  Chunk { bytes: &'a [u8], consumed: usize },
  Complete { consumed: usize },
  Partial { consumed: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Protocol {
  request_body: BodyKind,
  chunk_remaining: usize,
  chunk_needs_crlf: bool,
  chunk_waiting_trailers: bool,
  allow_missing_host: bool,
}

impl Default for Protocol {
  fn default() -> Self {
    Self::new()
  }
}

impl Protocol {
  pub const fn new() -> Self {
    Self {
      request_body: BodyKind::Empty,
      chunk_remaining: 0,
      chunk_needs_crlf: false,
      chunk_waiting_trailers: false,
      allow_missing_host: false,
    }
  }

  pub fn set_allow_missing_host(&mut self, allow: bool) {
    self.allow_missing_host = allow;
  }

  pub fn next_request<'a>(
    &mut self,
    input: &'a [u8],
    headers: &'a mut [Header<'a>],
  ) -> Result<RequestStatus<'a>, ProtocolError> {
    let Some(head) =
      parse_request_head(input, headers).map_err(ProtocolError::Parse)?
    else {
      return Ok(RequestStatus::Partial);
    };
    self.request_body = head.body_kind;
    self.chunk_remaining = 0;
    self.chunk_needs_crlf = false;
    self.chunk_waiting_trailers = false;
    Ok(RequestStatus::Complete {
      consumed: head.consumed,
      request: request_from_head(&head),
    })
  }

  pub fn next_request_uninit<'a>(
    &mut self,
    input: &'a [u8],
    headers: &'a mut [Header<'a>],
    parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
  ) -> Result<RequestStatus<'a>, ProtocolError> {
    let Some(head) = if self.allow_missing_host {
      parse_request_head_uninit_with_options(
        input,
        headers,
        parse_headers,
        true,
      )
    } else {
      parse_request_head_uninit(input, headers, parse_headers)
    }
    .map_err(ProtocolError::Parse)?
    else {
      return Ok(RequestStatus::Partial);
    };
    self.request_body = head.body_kind;
    self.chunk_remaining = 0;
    self.chunk_needs_crlf = false;
    self.chunk_waiting_trailers = false;
    Ok(RequestStatus::Complete {
      consumed: head.consumed,
      request: request_from_head(&head),
    })
  }

  pub fn next_request_uninit_all<'a>(
    &mut self,
    input: &'a [u8],
    headers: &'a mut [MaybeUninit<Header<'a>>],
    parse_headers: &mut [MaybeUninit<httparse::Header<'a>>],
  ) -> Result<RequestStatus<'a>, ProtocolError> {
    let Some(head) = parse_request_head_uninit_all_with_options(
      input,
      headers,
      parse_headers,
      self.allow_missing_host,
    )
    .map_err(ProtocolError::Parse)?
    else {
      return Ok(RequestStatus::Partial);
    };
    self.request_body = head.body_kind;
    self.chunk_remaining = 0;
    self.chunk_needs_crlf = false;
    self.chunk_waiting_trailers = false;
    Ok(RequestStatus::Complete {
      consumed: head.consumed,
      request: request_from_head(&head),
    })
  }

  pub fn body_chunk<'a>(
    &mut self,
    input: &'a [u8],
  ) -> Result<BodyStatus<'a>, ProtocolError> {
    match self.request_body {
      BodyKind::Empty | BodyKind::Upgrade => {
        Ok(BodyStatus::Complete { consumed: 0 })
      }
      BodyKind::ContentLength(remaining) => {
        if remaining == 0 {
          self.request_body = BodyKind::Empty;
          return Ok(BodyStatus::Complete { consumed: 0 });
        }
        if input.is_empty() {
          return Ok(BodyStatus::Partial { consumed: 0 });
        }
        let take = remaining.min(input.len() as u64) as usize;
        let remaining = remaining - take as u64;
        self.request_body = if remaining == 0 {
          BodyKind::Empty
        } else {
          BodyKind::ContentLength(remaining)
        };
        Ok(BodyStatus::Chunk {
          bytes: &input[..take],
          consumed: take,
        })
      }
      BodyKind::Chunked => self.chunked_body_chunk(input),
    }
  }

  pub fn content_length_remaining(&self) -> Option<u64> {
    match self.request_body {
      BodyKind::ContentLength(remaining) => Some(remaining),
      BodyKind::Empty | BodyKind::Chunked | BodyKind::Upgrade => None,
    }
  }

  pub fn finish_body(&mut self) {
    self.request_body = BodyKind::Empty;
    self.chunk_remaining = 0;
    self.chunk_needs_crlf = false;
    self.chunk_waiting_trailers = false;
  }

  fn chunked_body_chunk<'a>(
    &mut self,
    input: &'a [u8],
  ) -> Result<BodyStatus<'a>, ProtocolError> {
    if self.chunk_waiting_trailers {
      let trailers = match parse_trailers(input)? {
        TrailerStatus::Complete { consumed } => consumed,
        TrailerStatus::Partial => {
          return Ok(BodyStatus::Partial { consumed: 0 });
        }
      };
      self.chunk_waiting_trailers = false;
      self.request_body = BodyKind::Empty;
      return Ok(BodyStatus::Complete { consumed: trailers });
    }

    let mut cursor = 0usize;
    if self.chunk_needs_crlf {
      if input.len() < 2 {
        return Ok(BodyStatus::Partial { consumed: 0 });
      }
      if &input[..2] != b"\r\n" {
        return Err(ProtocolError::Parse(ParseError::Invalid));
      }
      cursor = 2;
      self.chunk_needs_crlf = false;
    }

    if self.chunk_remaining == 0 {
      let Some(line_end) = find_crlf(&input[cursor..]) else {
        if input.len().saturating_sub(cursor) > MAX_CHUNK_LINE_BYTES {
          return Err(ProtocolError::Parse(ParseError::Invalid));
        }
        return Ok(BodyStatus::Partial { consumed: cursor });
      };
      let line_end = cursor + line_end;
      if line_end - cursor > MAX_CHUNK_LINE_BYTES {
        return Err(ProtocolError::Parse(ParseError::Invalid));
      }
      let size = parse_chunk_size(&input[cursor..line_end])
        .map_err(ProtocolError::Parse)?;
      cursor = line_end + 2;
      if size == 0 {
        let trailers = match parse_trailers(&input[cursor..])? {
          TrailerStatus::Complete { consumed } => consumed,
          TrailerStatus::Partial => {
            self.chunk_waiting_trailers = true;
            return Ok(BodyStatus::Partial { consumed: cursor });
          }
        };
        let consumed = cursor + trailers;
        self.request_body = BodyKind::Empty;
        return Ok(BodyStatus::Complete { consumed });
      }
      self.chunk_remaining = size;
    }

    if cursor == input.len() {
      return Ok(BodyStatus::Partial { consumed: cursor });
    }
    let available = input.len() - cursor;
    let take = self.chunk_remaining.min(available);
    self.chunk_remaining -= take;
    if self.chunk_remaining == 0 {
      self.chunk_needs_crlf = true;
    }
    Ok(BodyStatus::Chunk {
      bytes: &input[cursor..cursor + take],
      consumed: cursor + take,
    })
  }
}

fn request_from_head<'a>(head: &RequestHead<'a>) -> CoreRequest<'a> {
  CoreRequest {
    method: head.method,
    target: head.target,
    version: head.version,
    headers: head.headers,
    body: head.body_kind,
    keep_alive: head.keep_alive,
    expect_continue: head.expect_continue,
    upgrade: upgrade_kind(head),
  }
}

fn upgrade_kind(head: &RequestHead<'_>) -> Option<CoreUpgradeKind> {
  if !matches!(head.body_kind, BodyKind::Upgrade) {
    return None;
  }
  let upgrade = head
    .headers
    .iter()
    .find(|header| header.name.eq_ignore_ascii_case(b"upgrade"))
    .map(|header| header.value);
  if upgrade.is_some_and(|value| value.eq_ignore_ascii_case(b"h2c")) {
    Some(CoreUpgradeKind::H2c)
  } else {
    Some(CoreUpgradeKind::Any)
  }
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
  buf.windows(2).position(|window| window == b"\r\n")
}

fn parse_chunk_size(line: &[u8]) -> Result<usize, ParseError> {
  if line.iter().any(|byte| matches!(*byte, 0..=0x1f | 0x7f)) {
    return Err(ParseError::Invalid);
  }
  let size = trim_ows(line.split(|byte| *byte == b';').next().unwrap());
  if size.is_empty() {
    return Err(ParseError::Invalid);
  }
  let mut out = 0usize;
  for byte in size {
    let digit = match byte {
      b'0'..=b'9' => byte - b'0',
      b'a'..=b'f' => byte - b'a' + 10,
      b'A'..=b'F' => byte - b'A' + 10,
      _ => return Err(ParseError::Invalid),
    };
    out = out
      .checked_mul(16)
      .and_then(|out| out.checked_add(digit as usize))
      .ok_or(ParseError::Invalid)?;
  }
  Ok(out)
}

enum TrailerStatus {
  Complete { consumed: usize },
  Partial,
}

fn parse_trailers(input: &[u8]) -> Result<TrailerStatus, ProtocolError> {
  let mut cursor = 0usize;
  loop {
    let Some(line_end) = find_crlf(&input[cursor..]) else {
      if input.len().saturating_sub(cursor) > MAX_TRAILER_BYTES {
        return Err(ProtocolError::HeadTooLarge);
      }
      return Ok(TrailerStatus::Partial);
    };
    let line_end = cursor + line_end;
    if line_end == cursor {
      return Ok(TrailerStatus::Complete {
        consumed: cursor + 2,
      });
    }
    if line_end + 2 > MAX_TRAILER_BYTES {
      return Err(ProtocolError::HeadTooLarge);
    }
    validate_trailer_line(&input[cursor..line_end])?;
    cursor = line_end + 2;
  }
}

fn validate_trailer_line(line: &[u8]) -> Result<(), ProtocolError> {
  let Some(colon) = line.iter().position(|byte| *byte == b':') else {
    return Err(ProtocolError::Parse(ParseError::Invalid));
  };
  if !valid_field_name(&line[..colon]) || !valid_field_value(&line[colon + 1..])
  {
    return Err(ProtocolError::Parse(ParseError::Invalid));
  }
  Ok(())
}

fn valid_field_name(name: &[u8]) -> bool {
  !name.is_empty()
    && name.iter().all(|byte| {
      matches!(
        *byte,
        b'!' | b'#'
          | b'$'
          | b'%'
          | b'&'
          | b'\''
          | b'*'
          | b'+'
          | b'-'
          | b'.'
          | b'^'
          | b'_'
          | b'`'
          | b'|'
          | b'~'
          | b'0'..=b'9'
          | b'A'..=b'Z'
          | b'a'..=b'z'
      )
    })
}

fn valid_field_value(value: &[u8]) -> bool {
  value
    .iter()
    .all(|byte| matches!(*byte, b'\t' | b' '..=0x7e | 0x80..=0xff))
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::MAX_HEADERS;

  #[test]
  fn parses_request_head_without_allocating() {
    let mut protocol = Protocol::new();
    let mut headers = [Header::EMPTY; MAX_HEADERS];
    let status = protocol
      .next_request(
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
        &mut headers,
      )
      .unwrap();
    let RequestStatus::Complete { request, consumed } = status else {
      panic!("expected complete request");
    };
    assert_eq!(request.method, b"GET");
    assert_eq!(request.target, b"/");
    assert_eq!(request.body, BodyKind::Empty);
    assert_eq!(consumed, 37);
  }

  #[test]
  fn streams_content_length_body_from_loop_buffer() {
    let mut protocol = Protocol::new();
    let mut headers = [Header::EMPTY; MAX_HEADERS];
    let input = b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhelloGET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let RequestStatus::Complete { consumed, .. } =
      protocol.next_request(input, &mut headers).unwrap()
    else {
      panic!("expected complete request");
    };
    let BodyStatus::Chunk {
      bytes,
      consumed: body_consumed,
    } = protocol.body_chunk(&input[consumed..]).unwrap()
    else {
      panic!("expected body chunk");
    };
    assert_eq!(bytes, b"hello");
    assert_eq!(body_consumed, 5);
    assert_eq!(
      protocol
        .body_chunk(&input[consumed + body_consumed..])
        .unwrap(),
      BodyStatus::Complete { consumed: 0 }
    );
  }

  #[test]
  fn streams_chunked_body_from_loop_buffer() {
    let mut protocol = Protocol::new();
    let mut headers = [Header::EMPTY; MAX_HEADERS];
    let input = b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\nX-Sig: abc\r\n\r\nGET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let RequestStatus::Complete { consumed, .. } =
      protocol.next_request(input, &mut headers).unwrap()
    else {
      panic!("expected complete request");
    };
    let mut cursor = consumed;
    let BodyStatus::Chunk {
      bytes,
      consumed: body_consumed,
    } = protocol.body_chunk(&input[cursor..]).unwrap()
    else {
      panic!("expected first body chunk");
    };
    assert_eq!(bytes, b"abc");
    cursor += body_consumed;
    let BodyStatus::Chunk {
      bytes,
      consumed: body_consumed,
    } = protocol.body_chunk(&input[cursor..]).unwrap()
    else {
      panic!("expected second body chunk");
    };
    assert_eq!(bytes, b"def");
    cursor += body_consumed;
    let BodyStatus::Complete { consumed } =
      protocol.body_chunk(&input[cursor..]).unwrap()
    else {
      panic!("expected body completion");
    };
    cursor += consumed;
    assert!(input[cursor..].starts_with(b"GET / HTTP/1.1"));
  }

  #[test]
  fn chunked_zero_chunk_can_wait_for_trailers_after_data_chunk() {
    let mut protocol = Protocol::new();
    let mut headers = [Header::EMPTY; MAX_HEADERS];
    let input =
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na";
    let RequestStatus::Complete { consumed, .. } =
      protocol.next_request(input, &mut headers).unwrap()
    else {
      panic!("expected complete request");
    };
    let BodyStatus::Chunk {
      bytes,
      consumed: body_consumed,
    } = protocol.body_chunk(&input[consumed..]).unwrap()
    else {
      panic!("expected body chunk");
    };
    assert_eq!(bytes, b"a");
    assert_eq!(body_consumed, 4);

    assert_eq!(
      protocol.body_chunk(b"\r\n0\r\n").unwrap(),
      BodyStatus::Partial { consumed: 5 }
    );
    assert_eq!(
      protocol.body_chunk(b"X-Trailer: yes\r\n").unwrap(),
      BodyStatus::Partial { consumed: 0 }
    );
    assert_eq!(
      protocol.body_chunk(b"X-Trailer: yes\r\n\r\n").unwrap(),
      BodyStatus::Complete { consumed: 18 }
    );
  }

  #[test]
  fn reports_partial_body_without_allocating() {
    let mut protocol = Protocol::new();
    let mut headers = [Header::EMPTY; MAX_HEADERS];
    let input =
      b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhe";
    let RequestStatus::Complete { consumed, .. } =
      protocol.next_request(input, &mut headers).unwrap()
    else {
      panic!("expected complete request");
    };
    let BodyStatus::Chunk {
      bytes,
      consumed: body_consumed,
    } = protocol.body_chunk(&input[consumed..]).unwrap()
    else {
      panic!("expected body chunk");
    };
    assert_eq!(bytes, b"he");
    assert_eq!(body_consumed, 2);
    assert_eq!(
      protocol.body_chunk(&[]).unwrap(),
      BodyStatus::Partial { consumed: 0 },
    );
  }

  #[test]
  fn parses_hyper_chunk_size_cases() {
    for (line, expected) in [
      (b"1".as_slice(), 1),
      (b"01", 1),
      (b"0", 0),
      (b"00", 0),
      (b"A", 10),
      (b"a", 10),
      (b"Ff", 255),
      (b"Ff   ", 255),
      (b"1;extension", 1),
      (b"a;ext name=value", 10),
      (b"1;extension;extension2", 1),
      (b"1;;;  ;", 1),
      (b"2; extension...", 2),
      (b"3   ; extension=123", 3),
      (b"3   ;", 3),
      (b"3   ;   ", 3),
    ] {
      assert_eq!(parse_chunk_size(line), Ok(expected), "{line:?}");
    }
  }

  #[test]
  fn rejects_hyper_invalid_chunk_size_cases() {
    for line in [
      b"\r\n\r\n".as_slice(),
      b"\r\n",
      b"X",
      b"1X",
      b"-",
      b"-1",
      b"1 invalid extension",
      b"1 A",
      b"1;reject\nnewlines",
      b"f0000000000000003",
    ] {
      assert!(
        parse_chunk_size(line).is_err(),
        "{line:?} should be rejected"
      );
    }
  }

  #[test]
  fn rejects_chunked_body_with_missing_zero_chunk_digit() {
    let mut protocol = Protocol {
      request_body: BodyKind::Chunked,
      ..Protocol::new()
    };
    assert_eq!(
      protocol.body_chunk(b"1\r\nZ\r\n").unwrap(),
      BodyStatus::Chunk {
        bytes: b"Z",
        consumed: 4,
      },
    );
    assert_eq!(
      protocol.body_chunk(b"\r\n\r\n"),
      Err(ProtocolError::Parse(ParseError::Invalid)),
    );
  }

  #[test]
  fn reports_partial_for_incomplete_chunk_size_line() {
    let mut protocol = Protocol {
      request_body: BodyKind::Chunked,
      ..Protocol::new()
    };
    assert_eq!(
      protocol.body_chunk(b"1;no CRLF").unwrap(),
      BodyStatus::Partial { consumed: 0 },
    );
  }

  #[test]
  fn rejects_complete_oversized_chunk_size_line() {
    let mut protocol = Protocol {
      request_body: BodyKind::Chunked,
      ..Protocol::new()
    };
    let mut input = Vec::with_capacity(MAX_CHUNK_LINE_BYTES + 3);
    input.push(b'1');
    input.extend(std::iter::repeat_n(b'a', MAX_CHUNK_LINE_BYTES));
    input.extend_from_slice(b"\r\n");

    assert_eq!(
      protocol.body_chunk(&input),
      Err(ProtocolError::Parse(ParseError::Invalid)),
    );
  }
}
