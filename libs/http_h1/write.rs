// Copyright 2018-2026 the Deno authors. MIT license.

use crate::Header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputFull;

#[derive(Debug, Clone, Copy)]
pub struct ResponseHeader<'a> {
  pub status: u16,
  pub reason: &'a [u8],
  pub headers: &'a [Header<'a>],
  pub content_length: Option<u64>,
  pub keep_alive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseHeaderFast<'a> {
  pub date: &'a [u8],
  pub body_len: u64,
  pub body: &'a [u8],
  pub keep_alive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseContentTypeFast<'a> {
  pub content_type: &'a [u8],
  pub date: &'a [u8],
  pub body_len: u64,
  pub body: &'a [u8],
  pub keep_alive: bool,
}

pub fn write_response_head(out: &mut Vec<u8>, response: ResponseHeader<'_>) {
  write_response_head_inner(out, response, false);
}

pub fn default_text_response_len(response: ResponseHeaderFast<'_>) -> usize {
  b"HTTP/1.1 200 OK\r\ncontent-type: text/plain;charset=UTF-8\r\ncontent-length: "
    .len()
    + decimal_len(response.body_len)
    + b"\r\ndate: ".len()
    + response.date.len()
    + if response.keep_alive {
      b"\r\n\r\n".len()
    } else {
      b"\r\nconnection: close\r\n\r\n".len()
    }
    + response.body.len()
}

pub fn content_type_response_len(
  response: ResponseContentTypeFast<'_>,
) -> usize {
  b"HTTP/1.1 200 OK\r\ncontent-type: ".len()
    + response.content_type.len()
    + b"\r\ncontent-length: ".len()
    + decimal_len(response.body_len)
    + b"\r\ndate: ".len()
    + response.date.len()
    + if response.keep_alive {
      b"\r\n\r\n".len()
    } else {
      b"\r\nconnection: close\r\n\r\n".len()
    }
    + response.body.len()
}

pub fn write_default_text_response(
  out: &mut Vec<u8>,
  response: ResponseHeaderFast<'_>,
) {
  out.clear();
  out.extend_from_slice(
    b"HTTP/1.1 200 OK\r\ncontent-type: text/plain;charset=UTF-8\r\ncontent-length: ",
  );
  push_u64(out, response.body_len);
  out.extend_from_slice(b"\r\ndate: ");
  out.extend_from_slice(response.date);
  if response.keep_alive {
    out.extend_from_slice(b"\r\n\r\n");
  } else {
    out.extend_from_slice(b"\r\nconnection: close\r\n\r\n");
  }
  out.extend_from_slice(response.body);
}

pub fn write_content_type_response(
  out: &mut Vec<u8>,
  response: ResponseContentTypeFast<'_>,
) {
  out.clear();
  out.extend_from_slice(b"HTTP/1.1 200 OK\r\ncontent-type: ");
  out.extend_from_slice(response.content_type);
  out.extend_from_slice(b"\r\ncontent-length: ");
  push_u64(out, response.body_len);
  out.extend_from_slice(b"\r\ndate: ");
  out.extend_from_slice(response.date);
  if response.keep_alive {
    out.extend_from_slice(b"\r\n\r\n");
  } else {
    out.extend_from_slice(b"\r\nconnection: close\r\n\r\n");
  }
  out.extend_from_slice(response.body);
}

pub fn write_chunked_response_head(
  out: &mut Vec<u8>,
  response: ResponseHeader<'_>,
) {
  write_response_head_inner(out, response, true);
}

pub fn append_chunk(out: &mut Vec<u8>, chunk: &[u8]) {
  push_hex_usize(out, chunk.len());
  out.extend_from_slice(b"\r\n");
  out.extend_from_slice(chunk);
  out.extend_from_slice(b"\r\n");
}

pub fn append_chunked_end(out: &mut Vec<u8>, trailers: &[Header<'_>]) {
  out.extend_from_slice(b"0\r\n");
  for trailer in trailers {
    out.extend_from_slice(trailer.name);
    out.extend_from_slice(b": ");
    out.extend_from_slice(trailer.value);
    out.extend_from_slice(b"\r\n");
  }
  out.extend_from_slice(b"\r\n");
}

pub fn write_response_head_to(
  out: &mut [u8],
  response: ResponseHeader<'_>,
) -> Result<usize, OutputFull> {
  write_response_head_to_inner(out, response, false)
}

pub fn write_chunked_response_head_to(
  out: &mut [u8],
  response: ResponseHeader<'_>,
) -> Result<usize, OutputFull> {
  write_response_head_to_inner(out, response, true)
}

pub fn append_chunk_to(
  out: &mut [u8],
  chunk: &[u8],
) -> Result<usize, OutputFull> {
  let mut cursor = SliceWriter::new(out);
  cursor.push_hex_usize(chunk.len())?;
  cursor.push(b"\r\n")?;
  cursor.push(chunk)?;
  cursor.push(b"\r\n")?;
  Ok(cursor.len())
}

pub fn append_chunked_end_to(
  out: &mut [u8],
  trailers: &[Header<'_>],
) -> Result<usize, OutputFull> {
  let mut cursor = SliceWriter::new(out);
  cursor.push(b"0\r\n")?;
  for trailer in trailers {
    cursor.push(trailer.name)?;
    cursor.push(b": ")?;
    cursor.push(trailer.value)?;
    cursor.push(b"\r\n")?;
  }
  cursor.push(b"\r\n")?;
  Ok(cursor.len())
}

fn write_response_head_inner(
  out: &mut Vec<u8>,
  response: ResponseHeader<'_>,
  chunked: bool,
) {
  out.clear();
  out.extend_from_slice(b"HTTP/1.1 ");
  push_status(out, response.status);
  out.push(b' ');
  out.extend_from_slice(response.reason);
  out.extend_from_slice(b"\r\n");

  let mut has_content_length = false;
  let mut has_transfer_encoding = false;
  let mut date = None;
  let body_allowed = status_allows_body(response.status);
  for header in response.headers {
    if header.name.eq_ignore_ascii_case(b"date") {
      date = Some(header.value);
      continue;
    }
    if header.name.eq_ignore_ascii_case(b"content-length") {
      if chunked {
        continue;
      }
      has_content_length = true;
    }
    if header.name.eq_ignore_ascii_case(b"transfer-encoding") {
      if chunked {
        continue;
      }
      has_transfer_encoding = true;
    }
    out.extend_from_slice(header.name);
    out.extend_from_slice(b": ");
    out.extend_from_slice(header.value);
    out.extend_from_slice(b"\r\n");
  }

  if let Some(content_length) = response.content_length
    && body_allowed
    && !chunked
    && !has_content_length
  {
    out.extend_from_slice(b"content-length: ");
    push_u64(out, content_length);
    out.extend_from_slice(b"\r\n");
  }

  if chunked && body_allowed && !has_transfer_encoding {
    out.extend_from_slice(b"transfer-encoding: chunked\r\n");
  }

  if let Some(date) = date {
    out.extend_from_slice(b"date: ");
    out.extend_from_slice(date);
    out.extend_from_slice(b"\r\n");
  }

  if !response.keep_alive {
    out.extend_from_slice(b"connection: close\r\n");
  }
  out.extend_from_slice(b"\r\n");
}

fn write_response_head_to_inner(
  out: &mut [u8],
  response: ResponseHeader<'_>,
  chunked: bool,
) -> Result<usize, OutputFull> {
  let mut cursor = SliceWriter::new(out);
  cursor.push(b"HTTP/1.1 ")?;
  cursor.push_status(response.status)?;
  cursor.push(b" ")?;
  cursor.push(response.reason)?;
  cursor.push(b"\r\n")?;

  let mut has_content_length = false;
  let mut has_transfer_encoding = false;
  let mut date = None;
  let body_allowed = status_allows_body(response.status);
  for header in response.headers {
    if header.name.eq_ignore_ascii_case(b"date") {
      date = Some(header.value);
      continue;
    }
    if header.name.eq_ignore_ascii_case(b"content-length") {
      if chunked {
        continue;
      }
      has_content_length = true;
    }
    if header.name.eq_ignore_ascii_case(b"transfer-encoding") {
      if chunked {
        continue;
      }
      has_transfer_encoding = true;
    }
    cursor.push(header.name)?;
    cursor.push(b": ")?;
    cursor.push(header.value)?;
    cursor.push(b"\r\n")?;
  }

  if let Some(content_length) = response.content_length
    && body_allowed
    && !chunked
    && !has_content_length
  {
    cursor.push(b"content-length: ")?;
    cursor.push_u64(content_length)?;
    cursor.push(b"\r\n")?;
  }

  if chunked && body_allowed && !has_transfer_encoding {
    cursor.push(b"transfer-encoding: chunked\r\n")?;
  }

  if let Some(date) = date {
    cursor.push(b"date: ")?;
    cursor.push(date)?;
    cursor.push(b"\r\n")?;
  }

  if !response.keep_alive {
    cursor.push(b"connection: close\r\n")?;
  }
  cursor.push(b"\r\n")?;
  Ok(cursor.len())
}

pub fn status_allows_body(status: u16) -> bool {
  !((100..200).contains(&status) || status == 204 || status == 304)
}

fn push_status(out: &mut Vec<u8>, status: u16) {
  if status < 1000 {
    out.push(b'0' + ((status / 100) % 10) as u8);
    out.push(b'0' + ((status / 10) % 10) as u8);
    out.push(b'0' + (status % 10) as u8);
  } else {
    push_u64(out, status as u64);
  }
}

fn push_u64(out: &mut Vec<u8>, mut value: u64) {
  let mut buf = [0u8; 20];
  let mut cursor = buf.len();
  loop {
    cursor -= 1;
    buf[cursor] = b'0' + (value % 10) as u8;
    value /= 10;
    if value == 0 {
      break;
    }
  }
  out.extend_from_slice(&buf[cursor..]);
}

fn decimal_len(mut value: u64) -> usize {
  let mut len = 1;
  while value >= 10 {
    value /= 10;
    len += 1;
  }
  len
}

fn push_hex_usize(out: &mut Vec<u8>, mut value: usize) {
  let mut buf = [0u8; usize::BITS as usize / 4];
  let mut cursor = buf.len();
  loop {
    cursor -= 1;
    let digit = (value & 0xf) as u8;
    buf[cursor] = match digit {
      0..=9 => b'0' + digit,
      _ => b'a' + digit - 10,
    };
    value >>= 4;
    if value == 0 {
      break;
    }
  }
  out.extend_from_slice(&buf[cursor..]);
}

struct SliceWriter<'a> {
  out: &'a mut [u8],
  len: usize,
}

impl<'a> SliceWriter<'a> {
  fn new(out: &'a mut [u8]) -> Self {
    Self { out, len: 0 }
  }

  fn len(&self) -> usize {
    self.len
  }

  fn push(&mut self, bytes: &[u8]) -> Result<(), OutputFull> {
    let end = self.len.checked_add(bytes.len()).ok_or(OutputFull)?;
    if end > self.out.len() {
      return Err(OutputFull);
    }
    self.out[self.len..end].copy_from_slice(bytes);
    self.len = end;
    Ok(())
  }

  fn push_status(&mut self, status: u16) -> Result<(), OutputFull> {
    if status < 1000 {
      let bytes = [
        b'0' + ((status / 100) % 10) as u8,
        b'0' + ((status / 10) % 10) as u8,
        b'0' + (status % 10) as u8,
      ];
      self.push(&bytes)
    } else {
      self.push_u64(status as u64)
    }
  }

  fn push_u64(&mut self, mut value: u64) -> Result<(), OutputFull> {
    let mut buf = [0u8; 20];
    let mut cursor = buf.len();
    loop {
      cursor -= 1;
      buf[cursor] = b'0' + (value % 10) as u8;
      value /= 10;
      if value == 0 {
        break;
      }
    }
    self.push(&buf[cursor..])
  }

  fn push_hex_usize(&mut self, mut value: usize) -> Result<(), OutputFull> {
    let mut buf = [0u8; usize::BITS as usize / 4];
    let mut cursor = buf.len();
    loop {
      cursor -= 1;
      let digit = (value & 0xf) as u8;
      buf[cursor] = match digit {
        0..=9 => b'0' + digit,
        _ => b'a' + digit - 10,
      };
      value >>= 4;
      if value == 0 {
        break;
      }
    }
    self.push(&buf[cursor..])
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn writes_response_head() {
    let headers = [Header {
      name: b"content-type",
      value: b"text/plain",
    }];
    let mut out = Vec::new();
    write_response_head(
      &mut out,
      ResponseHeader {
        status: 200,
        reason: b"OK",
        headers: &headers,
        content_length: Some(13),
        keep_alive: true,
      },
    );
    assert_eq!(
      out,
      b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 13\r\n\r\n"
    );
  }

  #[test]
  fn writes_default_text_response_fast_with_date() {
    let mut out = Vec::new();
    let response = ResponseHeaderFast {
      date: b"Thu, 21 May 2026 12:00:00 GMT",
      body_len: 5,
      body: b"hello",
      keep_alive: true,
    };
    write_default_text_response(&mut out, response);
    assert_eq!(out.len(), default_text_response_len(response));
    assert_eq!(
      out,
      b"HTTP/1.1 200 OK\r\ncontent-type: text/plain;charset=UTF-8\r\ncontent-length: 5\r\ndate: Thu, 21 May 2026 12:00:00 GMT\r\n\r\nhello"
    );
  }

  #[test]
  fn writes_chunked_response_head() {
    let headers = [
      Header {
        name: b"content-length",
        value: b"13",
      },
      Header {
        name: b"trailer",
        value: b"x-sig",
      },
    ];
    let mut out = Vec::new();
    write_chunked_response_head(
      &mut out,
      ResponseHeader {
        status: 200,
        reason: b"OK",
        headers: &headers,
        content_length: None,
        keep_alive: true,
      },
    );
    assert_eq!(
      out,
      b"HTTP/1.1 200 OK\r\ntrailer: x-sig\r\ntransfer-encoding: chunked\r\n\r\n"
    );
  }

  #[test]
  fn writes_chunk_and_trailers() {
    let trailers = [Header {
      name: b"x-sig",
      value: b"abc",
    }];
    let mut out = Vec::new();
    append_chunk(&mut out, b"hello");
    append_chunked_end(&mut out, &trailers);
    assert_eq!(out, b"5\r\nhello\r\n0\r\nx-sig: abc\r\n\r\n");
  }

  #[test]
  fn writes_connection_close() {
    let mut out = Vec::new();
    write_response_head(
      &mut out,
      ResponseHeader {
        status: 404,
        reason: b"Not Found",
        headers: &[],
        content_length: Some(0),
        keep_alive: false,
      },
    );
    assert_eq!(
      out,
      b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
  }

  #[test]
  fn writes_response_head_to_slice() {
    let headers = [Header {
      name: b"content-type",
      value: b"text/plain",
    }];
    let mut out = [0; 128];
    let len = write_response_head_to(
      &mut out,
      ResponseHeader {
        status: 200,
        reason: b"OK",
        headers: &headers,
        content_length: Some(13),
        keep_alive: true,
      },
    )
    .unwrap();
    assert_eq!(
      &out[..len],
      b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 13\r\n\r\n"
    );
  }

  #[test]
  fn slice_writer_reports_full_output() {
    let mut out = [0; 8];
    assert_eq!(
      write_response_head_to(
        &mut out,
        ResponseHeader {
          status: 200,
          reason: b"OK",
          headers: &[],
          content_length: Some(0),
          keep_alive: true,
        },
      ),
      Err(OutputFull)
    );
  }
}
