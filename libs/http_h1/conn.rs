// Copyright 2018-2026 the Deno authors. MIT license.

use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::ReadBuf as TokioReadBuf;

use crate::BodyKind;
use crate::BodyStatus;
use crate::CoreRequest;
use crate::CoreUpgradeKind;
use crate::Header;
use crate::ParseError;
use crate::Protocol;
use crate::ProtocolError;
use crate::ReadBuf;
use crate::RequestStatus;
use crate::ResponseContentTypeFast;
use crate::ResponseHeader;
use crate::ResponseHeaderFast;
use crate::Version;
use crate::append_chunk;
use crate::append_chunked_end;
use crate::content_type_response_len;
use crate::default_text_response_len;
use crate::status_allows_body;
use crate::write_chunked_response_head;
use crate::write_content_type_response;
use crate::write_default_text_response;
use crate::write_response_head;

const DEFAULT_READ_CAPACITY: usize = 1024;
const DEFAULT_WRITE_CAPACITY: usize = 512;
const MAX_HEAD_BYTES: usize = 64 * 1024;
// Chunked responses accumulate the head, body chunks and terminator into a
// single per-connection buffer (`SharedScratch::write_buf`) so a small response
// is emitted in one write instead of three. Flush once the buffer reaches this
// size to bound memory for large/streaming bodies, and shrink the buffer back
// to `DEFAULT_WRITE_CAPACITY` after each response so idle connections don't
// retain a large allocation.
const CHUNKED_FLUSH_THRESHOLD: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error("invalid HTTP/1 request: {0:?}")]
  Parse(ParseError),
  #[error("request head too large")]
  HeadTooLarge,
  #[error("response stream is already active")]
  ResponseStreamActive,
  #[error("response stream is not active")]
  ResponseStreamInactive,
  #[error("response body exceeds content-length")]
  ResponseBodyTooLong,
  #[error("response body shorter than content-length")]
  ResponseBodyTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeKind {
  Any,
  H2c,
}

#[derive(Debug)]
pub struct Request<'a> {
  pub method: &'a [u8],
  pub target: &'a [u8],
  pub version: Version,
  pub headers: &'a [Header<'a>],
  pub body: BodyKind,
  pub keep_alive: bool,
  pub expect_continue: bool,
  pub upgrade: Option<UpgradeKind>,
}

#[derive(Debug, Clone, Copy)]
pub enum ResponseBody<'a> {
  Empty,
  Head(Option<u64>),
  Bytes(&'a [u8]),
}

#[derive(Debug, Clone, Copy)]
pub struct Response<'a> {
  pub version: Version,
  pub status: u16,
  pub reason: &'a [u8],
  pub headers: &'a [Header<'a>],
  pub body: ResponseBody<'a>,
  pub keep_alive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseHead<'a> {
  pub version: Version,
  pub status: u16,
  pub reason: &'a [u8],
  pub headers: &'a [Header<'a>],
  pub keep_alive: bool,
}

#[derive(Debug)]
pub struct SharedResponseWriter<'a> {
  response: Response<'a>,
  content_length: Option<u64>,
  body: &'a [u8],
  head_written: usize,
  body_written: usize,
}

impl<'a> SharedResponseWriter<'a> {
  pub fn new(response: Response<'a>) -> Self {
    let body_len = match response.body {
      ResponseBody::Empty => Some(0),
      ResponseBody::Head(content_length) => content_length,
      ResponseBody::Bytes(bytes) => Some(bytes.len() as u64),
    };
    let content_length = status_allows_body(response.status)
      .then_some(body_len)
      .flatten();
    let body = match response.body {
      ResponseBody::Bytes(bytes) if status_allows_body(response.status) => {
        bytes
      }
      _ => &[],
    };
    Self {
      response,
      content_length,
      body,
      head_written: 0,
      body_written: 0,
    }
  }
}

#[derive(Debug)]
pub struct SharedChunkedResponseHeadWriter<'a> {
  response: ResponseHead<'a>,
  written: usize,
}

impl<'a> SharedChunkedResponseHeadWriter<'a> {
  pub fn new(response: ResponseHead<'a>) -> Self {
    Self {
      response,
      written: 0,
    }
  }
}

#[derive(Debug)]
pub struct SharedFixedResponseHeadWriter<'a> {
  response: ResponseHead<'a>,
  content_length: u64,
  written: usize,
}

impl<'a> SharedFixedResponseHeadWriter<'a> {
  pub fn new(response: ResponseHead<'a>, content_length: u64) -> Self {
    Self {
      response,
      content_length,
      written: 0,
    }
  }
}

#[derive(Debug)]
pub struct SharedResponseChunkWriter<'a> {
  chunk: &'a [u8],
  // Used by the (non-buffered) Fixed / CloseDelimited body paths.
  body_written: usize,
  // For the buffered chunked path: whether this chunk's framing has already
  // been appended to `write_buf` (so a re-poll after a partial flush does not
  // append it again).
  buffered: bool,
}

impl<'a> SharedResponseChunkWriter<'a> {
  pub fn new(chunk: &'a [u8]) -> Self {
    Self {
      chunk,
      body_written: 0,
      buffered: false,
    }
  }
}

#[derive(Debug)]
pub struct SharedResponseBodyWriter<'a> {
  body: &'a [u8],
  written: usize,
}

impl<'a> SharedResponseBodyWriter<'a> {
  pub fn new(body: &'a [u8]) -> Self {
    Self { body, written: 0 }
  }
}

#[derive(Debug)]
pub struct SharedResponseEndWriter<'a> {
  trailers: &'a [Header<'a>],
  // For the buffered chunked path: whether the terminating chunk has already
  // been appended to `write_buf`.
  buffered: bool,
}

impl<'a> SharedResponseEndWriter<'a> {
  pub fn new(trailers: &'a [Header<'a>]) -> Self {
    Self {
      trailers,
      buffered: false,
    }
  }
}

#[derive(Debug)]
pub struct UpgradeParts<I> {
  pub io: I,
  pub read_buf: ReadBuf,
}

#[derive(Debug)]
pub struct SharedScratch {
  read_buf: Vec<u8>,
  write_buf: Vec<u8>,
  // How many leading bytes of `write_buf` have already been flushed to the
  // socket. Used to resume a partial flush of the buffered chunked response
  // without re-sending bytes.
  write_flushed: usize,
}

impl SharedScratch {
  pub fn new(read_capacity: usize, write_capacity: usize) -> Self {
    Self {
      read_buf: vec![0; read_capacity],
      write_buf: Vec::with_capacity(write_capacity),
      write_flushed: 0,
    }
  }

  pub fn ensure_read_capacity(&mut self, capacity: usize) {
    if self.read_buf.len() < capacity {
      self.read_buf.resize(capacity, 0);
    }
  }
}

impl Default for SharedScratch {
  fn default() -> Self {
    Self::new(DEFAULT_READ_CAPACITY, DEFAULT_WRITE_CAPACITY)
  }
}

#[derive(Debug)]
pub struct Conn<I> {
  io: I,
  protocol: Protocol,
  read_buf: ReadBuf,
  write_buf: Vec<u8>,
  pending_head_consume: usize,
  pending_body_consume: usize,
  response_state: ResponseState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseState {
  Idle,
  Chunked,
  Fixed { remaining: u64 },
  CloseDelimited,
  NoBody,
}

#[derive(Debug)]
pub struct SharedConn<I> {
  io: I,
  protocol: Protocol,
  buffered: Vec<u8>,
  response_state: ResponseState,
}

#[derive(Debug, Clone, Copy)]
pub enum SharedBodyChunk<R> {
  Chunk(R),
  Complete,
}

impl<I> Conn<I> {
  pub fn new(io: I) -> Self {
    Self {
      io,
      protocol: Protocol::new(),
      read_buf: ReadBuf::with_capacity(DEFAULT_READ_CAPACITY),
      write_buf: Vec::with_capacity(DEFAULT_WRITE_CAPACITY),
      pending_head_consume: 0,
      pending_body_consume: 0,
      response_state: ResponseState::Idle,
    }
  }

  pub fn into_upgrade_parts(self) -> UpgradeParts<I> {
    UpgradeParts {
      io: self.io,
      read_buf: self.read_buf,
    }
  }
}

impl<I> SharedConn<I> {
  pub fn new(io: I) -> Self {
    Self {
      io,
      protocol: Protocol::new(),
      buffered: Vec::new(),
      response_state: ResponseState::Idle,
    }
  }

  pub fn set_allow_missing_host(&mut self, allow: bool) {
    self.protocol.set_allow_missing_host(allow);
  }

  pub fn into_inner(self) -> I {
    self.io
  }

  pub fn into_upgrade_parts(self) -> (I, Vec<u8>) {
    (self.io, self.buffered)
  }

  pub fn try_take_full_body(&mut self) -> Result<Option<Vec<u8>>, Error> {
    if let Some(remaining) = self.protocol.content_length_remaining() {
      let Ok(remaining) = usize::try_from(remaining) else {
        return Ok(None);
      };
      if self.buffered.len() < remaining {
        return Ok(None);
      }
      let body = self.buffered[..remaining].to_vec();
      self.buffered.drain(..remaining);
      self.protocol.finish_body();
      return Ok(Some(body));
    }

    let mut protocol = self.protocol;
    let mut cursor = 0usize;
    let mut body_len = 0usize;

    loop {
      let status =
        body_status_from_buf(&mut protocol, &self.buffered[cursor..])?;
      match status {
        ConnBodyStatus::Chunk { len, consumed, .. } => {
          body_len += len;
          cursor += consumed;
        }
        ConnBodyStatus::Complete { consumed } => {
          cursor += consumed;
          break;
        }
        ConnBodyStatus::Partial { .. } => return Ok(None),
      }
    }

    let consumed_total = cursor;
    let final_protocol = protocol;
    let mut protocol = self.protocol;
    let mut cursor = 0usize;
    let mut body = Vec::with_capacity(body_len);

    while cursor < consumed_total {
      let status =
        body_status_from_buf(&mut protocol, &self.buffered[cursor..])?;
      match status {
        ConnBodyStatus::Chunk {
          offset,
          len,
          consumed: chunk_consumed,
        } => {
          body.extend_from_slice(
            &self.buffered[cursor + offset..cursor + offset + len],
          );
          let next = cursor + chunk_consumed;
          if next >= consumed_total {
            break;
          }
          cursor = next;
        }
        ConnBodyStatus::Complete { .. } => break,
        ConnBodyStatus::Partial { .. } => return Ok(None),
      }
    }

    self.protocol = final_protocol;
    self.buffered.drain(..consumed_total);
    Ok(Some(body))
  }
}

impl<I> SharedConn<I>
where
  I: AsyncWrite + Unpin,
{
  pub async fn try_write_default_text_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    date: &[u8],
    body: &[u8],
    keep_alive: bool,
  ) -> Result<bool, Error> {
    if self.response_state != ResponseState::Idle {
      return Err(Error::ResponseStreamActive);
    }
    let response = ResponseHeaderFast {
      version: Version::Http11,
      date,
      body_len: body.len() as u64,
      body,
      keep_alive,
    };
    if default_text_response_len(response) > scratch.write_buf.capacity() {
      return Ok(false);
    }
    write_default_text_response(&mut scratch.write_buf, response);
    self.io.write_all(&scratch.write_buf).await?;
    Ok(true)
  }

  pub async fn try_write_content_type_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    content_type: &[u8],
    date: &[u8],
    body: &[u8],
    keep_alive: bool,
  ) -> Result<bool, Error> {
    if self.response_state != ResponseState::Idle {
      return Err(Error::ResponseStreamActive);
    }
    let response = ResponseContentTypeFast {
      version: Version::Http11,
      content_type,
      date,
      body_len: body.len() as u64,
      body,
      keep_alive,
    };
    if content_type_response_len(response) > scratch.write_buf.capacity() {
      return Ok(false);
    }
    write_content_type_response(&mut scratch.write_buf, response);
    self.io.write_all(&scratch.write_buf).await?;
    Ok(true)
  }
}

impl<I> Conn<I>
where
  I: AsyncRead + AsyncWrite + Unpin,
{
  pub async fn next_request<'a>(
    &'a mut self,
    headers: &'a mut [Header<'a>],
  ) -> Result<Option<Request<'a>>, Error> {
    self.finish_previous_request().await?;

    let head_end = loop {
      if let Some(head_end) = find_double_crlf(self.read_buf.filled()) {
        break head_end;
      }
      if self.read_buf.len() >= MAX_HEAD_BYTES {
        return Err(Error::HeadTooLarge);
      }
      if self.read_more().await? == 0 {
        return Ok(None);
      }
    };
    if head_end > MAX_HEAD_BYTES {
      return Err(Error::HeadTooLarge);
    }

    let RequestStatus::Complete { request, consumed } = self
      .protocol
      .next_request(self.read_buf.filled(), headers)
      .map_err(protocol_error)?
    else {
      return Err(Error::Parse(ParseError::Invalid));
    };
    let request = request_from_core(&request);
    self.pending_head_consume = consumed;
    Ok(Some(request))
  }

  pub async fn read_body_to_end(
    &mut self,
    out: &mut Vec<u8>,
  ) -> Result<(), Error> {
    while self.read_body_chunk_into(out).await? {}
    Ok(())
  }

  pub async fn read_body_chunk_into(
    &mut self,
    out: &mut Vec<u8>,
  ) -> Result<bool, Error> {
    let Some(chunk) = self.read_body_chunk().await? else {
      return Ok(false);
    };
    out.extend_from_slice(chunk);
    Ok(true)
  }

  pub async fn read_body_chunk(&mut self) -> Result<Option<&[u8]>, Error> {
    self.consume_pending_head();
    self.consume_pending_body_chunk();
    loop {
      match self.body_status()? {
        ConnBodyStatus::Chunk {
          offset,
          len,
          consumed,
        } => {
          self.pending_body_consume = consumed;
          return Ok(Some(&self.read_buf.filled()[offset..offset + len]));
        }
        ConnBodyStatus::Complete { consumed } => {
          self.read_buf.consume(consumed);
          return Ok(None);
        }
        ConnBodyStatus::Partial { consumed } => {
          self.read_buf.consume(consumed);
        }
      }
      if self.read_more().await? == 0 {
        match self.body_status()? {
          ConnBodyStatus::Chunk {
            offset,
            len,
            consumed,
          } => {
            self.pending_body_consume = consumed;
            return Ok(Some(&self.read_buf.filled()[offset..offset + len]));
          }
          ConnBodyStatus::Complete { consumed } => {
            self.read_buf.consume(consumed);
            return Ok(None);
          }
          ConnBodyStatus::Partial { .. } => return Err(unexpected_eof()),
        }
      }
    }
  }

  pub async fn write_response(
    &mut self,
    response: Response<'_>,
  ) -> Result<(), Error> {
    if self.response_state != ResponseState::Idle {
      return Err(Error::ResponseStreamActive);
    }
    let body_len = match response.body {
      ResponseBody::Empty => Some(0),
      ResponseBody::Head(content_length) => content_length,
      ResponseBody::Bytes(bytes) => Some(bytes.len() as u64),
    };
    let content_length = status_allows_body(response.status)
      .then_some(body_len)
      .flatten();
    write_response_head(
      &mut self.write_buf,
      ResponseHeader {
        version: response.version,
        status: response.status,
        reason: response.reason,
        headers: response.headers,
        content_length,
        keep_alive: response.keep_alive,
      },
    );
    if let ResponseBody::Bytes(bytes) = response.body
      && status_allows_body(response.status)
    {
      self.write_buf.extend_from_slice(bytes);
    }
    self.io.write_all(&self.write_buf).await?;
    Ok(())
  }

  pub async fn start_chunked_response(
    &mut self,
    response: ResponseHead<'_>,
  ) -> Result<(), Error> {
    if self.response_state != ResponseState::Idle {
      return Err(Error::ResponseStreamActive);
    }
    write_chunked_response_head(
      &mut self.write_buf,
      ResponseHeader {
        version: response.version,
        status: response.status,
        reason: response.reason,
        headers: response.headers,
        content_length: None,
        keep_alive: response.keep_alive,
      },
    );
    self.io.write_all(&self.write_buf).await?;
    self.response_state = if status_allows_body(response.status) {
      if response.version == Version::Http11 {
        ResponseState::Chunked
      } else {
        ResponseState::CloseDelimited
      }
    } else {
      ResponseState::NoBody
    };
    Ok(())
  }

  pub async fn start_fixed_response(
    &mut self,
    response: ResponseHead<'_>,
    content_length: u64,
  ) -> Result<(), Error> {
    if self.response_state != ResponseState::Idle {
      return Err(Error::ResponseStreamActive);
    }
    write_response_head(
      &mut self.write_buf,
      ResponseHeader {
        version: response.version,
        status: response.status,
        reason: response.reason,
        headers: response.headers,
        content_length: Some(content_length),
        keep_alive: response.keep_alive,
      },
    );
    self.io.write_all(&self.write_buf).await?;
    self.response_state = if status_allows_body(response.status) {
      ResponseState::Fixed {
        remaining: content_length,
      }
    } else {
      ResponseState::NoBody
    };
    Ok(())
  }

  pub async fn write_response_chunk(
    &mut self,
    chunk: &[u8],
  ) -> Result<(), Error> {
    match self.response_state {
      ResponseState::Idle => return Err(Error::ResponseStreamInactive),
      ResponseState::NoBody => return Ok(()),
      ResponseState::Fixed { remaining } => {
        let len = chunk.len() as u64;
        if len > remaining {
          return Err(Error::ResponseBodyTooLong);
        }
        self.io.write_all(chunk).await?;
        if let ResponseState::Fixed { remaining } = &mut self.response_state {
          *remaining -= len;
        }
        return Ok(());
      }
      ResponseState::CloseDelimited => {
        self.io.write_all(chunk).await?;
        return Ok(());
      }
      ResponseState::Chunked => {}
    }
    if chunk.is_empty() {
      return Ok(());
    }
    self.write_buf.clear();
    append_chunk(&mut self.write_buf, chunk);
    self.io.write_all(&self.write_buf).await?;
    Ok(())
  }

  pub async fn finish_response(
    &mut self,
    trailers: &[Header<'_>],
  ) -> Result<(), Error> {
    match self.response_state {
      ResponseState::Idle => return Err(Error::ResponseStreamInactive),
      ResponseState::NoBody => {
        self.response_state = ResponseState::Idle;
        return Ok(());
      }
      ResponseState::CloseDelimited => {
        self.response_state = ResponseState::Idle;
        return Ok(());
      }
      ResponseState::Fixed { remaining: 0 } => {
        self.response_state = ResponseState::Idle;
        return Ok(());
      }
      ResponseState::Fixed { .. } => return Err(Error::ResponseBodyTooShort),
      ResponseState::Chunked => {}
    }
    self.write_buf.clear();
    append_chunked_end(&mut self.write_buf, trailers);
    self.io.write_all(&self.write_buf).await?;
    self.response_state = ResponseState::Idle;
    Ok(())
  }

  pub async fn write_continue(&mut self) -> Result<(), Error> {
    self.io.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
    Ok(())
  }

  async fn finish_previous_request(&mut self) -> Result<(), Error> {
    self.consume_pending_head();
    self.consume_pending_body_chunk();
    loop {
      match self.body_status()? {
        ConnBodyStatus::Chunk { consumed, .. } => {
          self.read_buf.consume(consumed);
          continue;
        }
        ConnBodyStatus::Complete { consumed } => {
          self.read_buf.consume(consumed);
          return Ok(());
        }
        ConnBodyStatus::Partial { consumed } => {
          self.read_buf.consume(consumed);
        }
      }
      if self.read_more().await? == 0 {
        match self.body_status()? {
          ConnBodyStatus::Chunk { consumed, .. } => {
            self.read_buf.consume(consumed);
            continue;
          }
          ConnBodyStatus::Complete { consumed } => {
            self.read_buf.consume(consumed);
            return Ok(());
          }
          ConnBodyStatus::Partial { .. } => return Err(unexpected_eof()),
        }
      }
    }
  }

  fn body_status(&mut self) -> Result<ConnBodyStatus, Error> {
    let status = self
      .protocol
      .body_chunk(self.read_buf.filled())
      .map_err(protocol_error)?;
    Ok(match status {
      BodyStatus::Chunk { bytes, consumed } => ConnBodyStatus::Chunk {
        offset: consumed - bytes.len(),
        len: bytes.len(),
        consumed,
      },
      BodyStatus::Complete { consumed } => {
        ConnBodyStatus::Complete { consumed }
      }
      BodyStatus::Partial { consumed } => ConnBodyStatus::Partial { consumed },
    })
  }

  fn consume_pending_head(&mut self) {
    if self.pending_head_consume == 0 {
      return;
    }
    self.read_buf.consume(self.pending_head_consume);
    self.pending_head_consume = 0;
  }

  fn consume_pending_body_chunk(&mut self) {
    if self.pending_body_consume == 0 {
      return;
    }
    self.read_buf.consume(self.pending_body_consume);
    self.pending_body_consume = 0;
  }

  async fn read_more(&mut self) -> Result<usize, Error> {
    poll_read_more(&mut self.io, &mut self.read_buf).await
  }
}

impl<I> SharedConn<I>
where
  I: AsyncRead + AsyncWrite + Unpin,
{
  pub fn poll_next_request_with<R, F>(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    mut callback: F,
  ) -> Poll<Result<Option<R>, Error>>
  where
    F: for<'a> FnMut(Request<'a>) -> R,
  {
    loop {
      if let Some(head_end) = find_double_crlf(&self.buffered) {
        if head_end > MAX_HEAD_BYTES {
          return Poll::Ready(Err(Error::HeadTooLarge));
        }
        let mut headers =
          [const { std::mem::MaybeUninit::uninit() }; crate::MAX_HEADERS];
        let mut parse_headers =
          [const { std::mem::MaybeUninit::uninit() }; crate::MAX_HEADERS];
        let RequestStatus::Complete { request, consumed } = self
          .protocol
          .next_request_uninit_all(
            &self.buffered,
            &mut headers,
            &mut parse_headers,
          )
          .map_err(protocol_error)?
        else {
          return Poll::Ready(Err(Error::Parse(ParseError::Invalid)));
        };
        let request = request_from_core(&request);
        let result = callback(request);
        self.buffered.drain(..consumed);
        return Poll::Ready(Ok(Some(result)));
      }

      if self.buffered.len() >= MAX_HEAD_BYTES {
        return Poll::Ready(Err(Error::HeadTooLarge));
      }

      let read = ready!(poll_read_into_scratch(&mut self.io, cx, scratch))?;
      if read == 0 {
        return Poll::Ready(Ok(None));
      }

      if self.buffered.is_empty() {
        let scratch_head_end = find_double_crlf(&scratch.read_buf[..read]);
        if let Some(head_end) = scratch_head_end
          && head_end > MAX_HEAD_BYTES
        {
          return Poll::Ready(Err(Error::HeadTooLarge));
        }
        let mut headers =
          [const { std::mem::MaybeUninit::uninit() }; crate::MAX_HEADERS];
        let mut parse_headers =
          [const { std::mem::MaybeUninit::uninit() }; crate::MAX_HEADERS];
        let status = self
          .protocol
          .next_request_uninit_all(
            &scratch.read_buf[..read],
            &mut headers,
            &mut parse_headers,
          )
          .map_err(protocol_error)?;
        if let RequestStatus::Complete { request, consumed } = status {
          let request = request_from_core(&request);
          let result = callback(request);
          if consumed < read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[consumed..read]);
          }
          return Poll::Ready(Ok(Some(result)));
        }
        if scratch_head_end.is_some() {
          return Poll::Ready(Err(Error::Parse(ParseError::Invalid)));
        }
      }

      self.buffered.extend_from_slice(&scratch.read_buf[..read]);
    }
  }

  pub fn poll_read_body_chunk_with<R, F>(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    callback: F,
  ) -> Poll<Result<SharedBodyChunk<R>, Error>>
  where
    F: for<'a> FnMut(&'a [u8]) -> R,
  {
    self.poll_read_body_chunk_limited_with(cx, scratch, usize::MAX, callback)
  }

  pub fn poll_read_body_chunk_limited_with<R, F>(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    limit: usize,
    mut callback: F,
  ) -> Poll<Result<SharedBodyChunk<R>, Error>>
  where
    F: for<'a> FnMut(&'a [u8]) -> R,
  {
    let limit = limit.max(1);
    loop {
      if self.buffered.is_empty()
        && let ConnBodyStatus::Complete { .. } =
          body_status_from_buf(&mut self.protocol, &[])?
      {
        return Poll::Ready(Ok(SharedBodyChunk::Complete));
      }

      if !self.buffered.is_empty() {
        let read = self.buffered.len().min(limit);
        match body_status_from_buf(&mut self.protocol, &self.buffered[..read])?
        {
          ConnBodyStatus::Chunk {
            offset,
            len,
            consumed,
          } => {
            let result = callback(&self.buffered[offset..offset + len]);
            self.buffered.drain(..consumed);
            return Poll::Ready(Ok(SharedBodyChunk::Chunk(result)));
          }
          ConnBodyStatus::Complete { consumed } => {
            self.buffered.drain(..consumed);
            return Poll::Ready(Ok(SharedBodyChunk::Complete));
          }
          ConnBodyStatus::Partial { consumed } => {
            if consumed != 0 {
              self.buffered.drain(..consumed);
            }
          }
        }
      }

      let read = ready!(poll_read_into_scratch(&mut self.io, cx, scratch))?;
      if read == 0 {
        match body_status_from_buf(&mut self.protocol, &[])? {
          ConnBodyStatus::Complete { .. } => {
            return Poll::Ready(Ok(SharedBodyChunk::Complete));
          }
          ConnBodyStatus::Chunk { .. } | ConnBodyStatus::Partial { .. } => {
            return Poll::Ready(Err(unexpected_eof()));
          }
        }
      }

      if !self.buffered.is_empty() {
        self.buffered.extend_from_slice(&scratch.read_buf[..read]);
        continue;
      }

      let parse_read = read.min(limit);
      match body_status_from_buf(
        &mut self.protocol,
        &scratch.read_buf[..parse_read],
      )? {
        ConnBodyStatus::Chunk {
          offset,
          len,
          consumed,
        } => {
          let result = callback(&scratch.read_buf[offset..offset + len]);
          if consumed < read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[consumed..read]);
          }
          return Poll::Ready(Ok(SharedBodyChunk::Chunk(result)));
        }
        ConnBodyStatus::Complete { consumed } => {
          if consumed < parse_read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[consumed..parse_read]);
          }
          if parse_read < read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[parse_read..read]);
          }
          return Poll::Ready(Ok(SharedBodyChunk::Complete));
        }
        ConnBodyStatus::Partial { consumed } => {
          if consumed < parse_read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[consumed..parse_read]);
          }
          if parse_read < read {
            self
              .buffered
              .extend_from_slice(&scratch.read_buf[parse_read..read]);
          }
        }
      }
    }
  }

  pub async fn read_body_to_end_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    out: &mut Vec<u8>,
  ) -> Result<(), Error> {
    loop {
      match std::future::poll_fn(|cx| {
        self.poll_read_body_chunk_with(cx, scratch, |chunk| chunk.to_vec())
      })
      .await?
      {
        SharedBodyChunk::Chunk(chunk) => out.extend_from_slice(&chunk),
        SharedBodyChunk::Complete => return Ok(()),
      }
    }
  }

  pub async fn discard_body_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
  ) -> Result<(), Error> {
    loop {
      match std::future::poll_fn(|cx| {
        self.poll_read_body_chunk_with(cx, scratch, |_| ())
      })
      .await?
      {
        SharedBodyChunk::Chunk(()) => {}
        SharedBodyChunk::Complete => return Ok(()),
      }
    }
  }

  pub async fn write_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    response: Response<'_>,
  ) -> Result<(), Error> {
    let mut writer = SharedResponseWriter::new(response);
    std::future::poll_fn(|cx| {
      self.poll_write_response_with(cx, scratch, &mut writer)
    })
    .await
  }

  pub async fn write_continue(&mut self) -> Result<(), Error> {
    self.io.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
    Ok(())
  }

  pub fn poll_write_response_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    writer: &mut SharedResponseWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    if self.response_state != ResponseState::Idle {
      return Poll::Ready(Err(Error::ResponseStreamActive));
    }
    scratch.write_buf.clear();
    write_response_head(
      &mut scratch.write_buf,
      ResponseHeader {
        version: writer.response.version,
        status: writer.response.status,
        reason: writer.response.reason,
        headers: writer.response.headers,
        content_length: writer.content_length,
        keep_alive: writer.response.keep_alive,
      },
    );
    let head_len = scratch.write_buf.len();
    if writer.body_written == 0
      && writer.body.len() <= scratch.write_buf.capacity() - head_len
    {
      scratch.write_buf.extend_from_slice(writer.body);
      while writer.head_written < scratch.write_buf.len() {
        let written = ready!(
          Pin::new(&mut self.io)
            .poll_write(cx, &scratch.write_buf[writer.head_written..])
        )?;
        if written == 0 {
          return Poll::Ready(Err(Error::Io(io::Error::new(
            io::ErrorKind::WriteZero,
            "failed to write response",
          ))));
        }
        writer.head_written += written;
      }
      writer.body_written = writer.body.len();
      return Poll::Ready(Ok(()));
    }
    while writer.head_written < scratch.write_buf.len() {
      let written = ready!(
        Pin::new(&mut self.io)
          .poll_write(cx, &scratch.write_buf[writer.head_written..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response head",
        ))));
      }
      writer.head_written += written;
    }
    while writer.body_written < writer.body.len() {
      let written = ready!(
        Pin::new(&mut self.io)
          .poll_write(cx, &writer.body[writer.body_written..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response body",
        ))));
      }
      writer.body_written += written;
    }
    Poll::Ready(Ok(()))
  }

  pub async fn start_chunked_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    response: ResponseHead<'_>,
  ) -> Result<(), Error> {
    let mut writer = SharedChunkedResponseHeadWriter::new(response);
    std::future::poll_fn(|cx| {
      self.poll_start_chunked_response_with(cx, scratch, &mut writer)
    })
    .await
  }

  pub fn poll_start_chunked_response_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    writer: &mut SharedChunkedResponseHeadWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    if self.response_state != ResponseState::Idle {
      return Poll::Ready(Err(Error::ResponseStreamActive));
    }

    if writer.written == 0 {
      scratch.write_buf.clear();
      scratch.write_flushed = 0;
      write_chunked_response_head(
        &mut scratch.write_buf,
        ResponseHeader {
          version: writer.response.version,
          status: writer.response.status,
          reason: writer.response.reason,
          headers: writer.response.headers,
          content_length: None,
          keep_alive: writer.response.keep_alive,
        },
      );
    }
    let next_state = if status_allows_body(writer.response.status) {
      if writer.response.version == Version::Http11 {
        ResponseState::Chunked
      } else {
        ResponseState::CloseDelimited
      }
    } else {
      ResponseState::NoBody
    };
    // For chunked (HTTP/1.1) responses, leave the head buffered in `write_buf`
    // so it coalesces with the first body chunk and the terminator into a
    // single write. CloseDelimited (HTTP/1.0) writes its body directly to the
    // socket, and NoBody has no terminator to flush behind it, so both must
    // flush the head now to preserve ordering.
    if next_state == ResponseState::Chunked {
      self.response_state = next_state;
      return Poll::Ready(Ok(()));
    }
    while writer.written < scratch.write_buf.len() {
      let written = ready!(
        Pin::new(&mut self.io)
          .poll_write(cx, &scratch.write_buf[writer.written..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response head",
        ))));
      }
      writer.written += written;
    }
    scratch.write_flushed = scratch.write_buf.len();
    self.response_state = next_state;
    Poll::Ready(Ok(()))
  }

  pub async fn start_fixed_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    response: ResponseHead<'_>,
    content_length: u64,
  ) -> Result<(), Error> {
    let mut writer =
      SharedFixedResponseHeadWriter::new(response, content_length);
    std::future::poll_fn(|cx| {
      self.poll_start_fixed_response_with(cx, scratch, &mut writer)
    })
    .await
  }

  pub fn poll_start_fixed_response_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    writer: &mut SharedFixedResponseHeadWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    if self.response_state != ResponseState::Idle {
      return Poll::Ready(Err(Error::ResponseStreamActive));
    }

    scratch.write_buf.clear();
    write_response_head(
      &mut scratch.write_buf,
      ResponseHeader {
        version: writer.response.version,
        status: writer.response.status,
        reason: writer.response.reason,
        headers: writer.response.headers,
        content_length: Some(writer.content_length),
        keep_alive: writer.response.keep_alive,
      },
    );
    while writer.written < scratch.write_buf.len() {
      let written = ready!(
        Pin::new(&mut self.io)
          .poll_write(cx, &scratch.write_buf[writer.written..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response head",
        ))));
      }
      writer.written += written;
    }
    scratch.write_flushed = scratch.write_buf.len();
    self.response_state = if status_allows_body(writer.response.status) {
      ResponseState::Fixed {
        remaining: writer.content_length,
      }
    } else {
      ResponseState::NoBody
    };
    Poll::Ready(Ok(()))
  }

  pub async fn write_response_chunk_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    chunk: &[u8],
  ) -> Result<(), Error> {
    let mut writer = SharedResponseChunkWriter::new(chunk);
    std::future::poll_fn(|cx| {
      self.poll_write_response_chunk_with(cx, scratch, &mut writer)
    })
    .await
  }

  pub fn poll_write_response_chunk_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    writer: &mut SharedResponseChunkWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    match self.response_state {
      ResponseState::Idle => {
        return Poll::Ready(Err(Error::ResponseStreamInactive));
      }
      ResponseState::NoBody => return Poll::Ready(Ok(())),
      ResponseState::Fixed { remaining } => {
        let pending = writer.chunk.len() - writer.body_written;
        if pending as u64 > remaining {
          return Poll::Ready(Err(Error::ResponseBodyTooLong));
        }
        while writer.body_written < writer.chunk.len() {
          let written = ready!(
            Pin::new(&mut self.io)
              .poll_write(cx, &writer.chunk[writer.body_written..])
          )?;
          if written == 0 {
            return Poll::Ready(Err(Error::Io(io::Error::new(
              io::ErrorKind::WriteZero,
              "failed to write response body",
            ))));
          }
          writer.body_written += written;
          if let ResponseState::Fixed { remaining } = &mut self.response_state {
            *remaining -= written as u64;
          }
        }
        return Poll::Ready(Ok(()));
      }
      ResponseState::CloseDelimited => {
        while writer.body_written < writer.chunk.len() {
          let written = ready!(
            Pin::new(&mut self.io)
              .poll_write(cx, &writer.chunk[writer.body_written..])
          )?;
          if written == 0 {
            return Poll::Ready(Err(Error::Io(io::Error::new(
              io::ErrorKind::WriteZero,
              "failed to write response body",
            ))));
          }
          writer.body_written += written;
        }
        return Poll::Ready(Ok(()));
      }
      ResponseState::Chunked => {}
    }
    if writer.chunk.is_empty() {
      return Poll::Ready(Ok(()));
    }

    // Append this chunk's framing (size prefix + data + CRLF) after the
    // still-buffered head and any earlier chunks, so a small response coalesces
    // into a single write. The buffer is flushed by `poll_finish_response_with`
    // (terminator), by the driver when the body source would block, or here
    // once it grows past the threshold.
    if !writer.buffered {
      append_chunk_prefix(&mut scratch.write_buf, writer.chunk.len());
      scratch.write_buf.extend_from_slice(writer.chunk);
      scratch.write_buf.extend_from_slice(b"\r\n");
      writer.buffered = true;
    }
    if scratch.write_buf.len() >= CHUNKED_FLUSH_THRESHOLD {
      return self.poll_flush_write_buf(cx, scratch);
    }
    Poll::Ready(Ok(()))
  }

  /// Drains the buffered (but not yet sent) bytes of a chunked response in
  /// `scratch.write_buf` to the socket, resuming from `scratch.write_flushed`
  /// on a partial write. On completion the buffer is cleared so it can be
  /// reused for the next chunk / response.
  pub fn poll_flush_write_buf(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
  ) -> Poll<Result<(), Error>> {
    while scratch.write_flushed < scratch.write_buf.len() {
      let written = ready!(
        Pin::new(&mut self.io)
          .poll_write(cx, &scratch.write_buf[scratch.write_flushed..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response chunk",
        ))));
      }
      scratch.write_flushed += written;
    }
    scratch.write_buf.clear();
    scratch.write_flushed = 0;
    Poll::Ready(Ok(()))
  }

  pub async fn write_response_body_with_scratch(
    &mut self,
    body: &[u8],
  ) -> Result<(), Error> {
    let mut writer = SharedResponseBodyWriter::new(body);
    std::future::poll_fn(|cx| {
      self.poll_write_response_body_with(cx, &mut writer)
    })
    .await
  }

  pub fn poll_write_response_body_with(
    &mut self,
    cx: &mut Context<'_>,
    writer: &mut SharedResponseBodyWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    match self.response_state {
      ResponseState::Idle => {
        return Poll::Ready(Err(Error::ResponseStreamInactive));
      }
      ResponseState::NoBody => return Poll::Ready(Ok(())),
      ResponseState::Chunked => {
        return Poll::Ready(Err(Error::ResponseStreamInactive));
      }
      ResponseState::Fixed { remaining } => {
        let pending = writer.body.len() - writer.written;
        if pending as u64 > remaining {
          return Poll::Ready(Err(Error::ResponseBodyTooLong));
        }
      }
      ResponseState::CloseDelimited => {}
    }
    while writer.written < writer.body.len() {
      let written = ready!(
        Pin::new(&mut self.io).poll_write(cx, &writer.body[writer.written..])
      )?;
      if written == 0 {
        return Poll::Ready(Err(Error::Io(io::Error::new(
          io::ErrorKind::WriteZero,
          "failed to write response body",
        ))));
      }
      writer.written += written;
      if let ResponseState::Fixed { remaining } = &mut self.response_state {
        *remaining -= written as u64;
      }
    }
    Poll::Ready(Ok(()))
  }

  pub fn poll_peer_closed_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
  ) -> Poll<Result<bool, Error>> {
    let read = ready!(poll_read_into_scratch(&mut self.io, cx, scratch))?;
    if read != 0 {
      self.buffered.extend_from_slice(&scratch.read_buf[..read]);
      cx.waker().wake_by_ref();
    }
    Poll::Ready(Ok(read == 0))
  }

  pub async fn finish_response_with_scratch(
    &mut self,
    scratch: &mut SharedScratch,
    trailers: &[Header<'_>],
  ) -> Result<(), Error> {
    let mut writer = SharedResponseEndWriter::new(trailers);
    std::future::poll_fn(|cx| {
      self.poll_finish_response_with(cx, scratch, &mut writer)
    })
    .await
  }

  pub fn poll_finish_response_with(
    &mut self,
    cx: &mut Context<'_>,
    scratch: &mut SharedScratch,
    writer: &mut SharedResponseEndWriter<'_>,
  ) -> Poll<Result<(), Error>> {
    match self.response_state {
      ResponseState::Idle => {
        return Poll::Ready(Err(Error::ResponseStreamInactive));
      }
      ResponseState::NoBody => {
        self.response_state = ResponseState::Idle;
        return Poll::Ready(Ok(()));
      }
      ResponseState::CloseDelimited => {
        self.response_state = ResponseState::Idle;
        return Poll::Ready(Ok(()));
      }
      ResponseState::Fixed { remaining: 0 } => {
        self.response_state = ResponseState::Idle;
        return Poll::Ready(Ok(()));
      }
      ResponseState::Fixed { .. } => {
        return Poll::Ready(Err(Error::ResponseBodyTooShort));
      }
      ResponseState::Chunked => {}
    }

    // Append the terminating chunk (and any trailers) after the still-buffered
    // head and body chunks, then flush everything in one write.
    if !writer.buffered {
      append_chunked_end(&mut scratch.write_buf, writer.trailers);
      writer.buffered = true;
    }
    ready!(self.poll_flush_write_buf(cx, scratch))?;
    // Don't let a connection that served one large response retain an
    // oversized buffer for the rest of its (idle) life.
    if scratch.write_buf.capacity() > DEFAULT_WRITE_CAPACITY {
      scratch.write_buf.shrink_to(DEFAULT_WRITE_CAPACITY);
    }
    self.response_state = ResponseState::Idle;
    Poll::Ready(Ok(()))
  }
}

enum ConnBodyStatus {
  Chunk {
    offset: usize,
    len: usize,
    consumed: usize,
  },
  Complete {
    consumed: usize,
  },
  Partial {
    consumed: usize,
  },
}

fn protocol_error(error: ProtocolError) -> Error {
  match error {
    ProtocolError::Parse(error) => Error::Parse(error),
    ProtocolError::HeadTooLarge => Error::HeadTooLarge,
  }
}

fn unexpected_eof() -> Error {
  Error::Io(io::Error::new(
    io::ErrorKind::UnexpectedEof,
    "unexpected EOF while reading HTTP/1 body",
  ))
}

fn body_status_from_buf(
  protocol: &mut Protocol,
  buf: &[u8],
) -> Result<ConnBodyStatus, Error> {
  let status = protocol.body_chunk(buf).map_err(protocol_error)?;
  Ok(match status {
    BodyStatus::Chunk { bytes, consumed } => ConnBodyStatus::Chunk {
      offset: consumed - bytes.len(),
      len: bytes.len(),
      consumed,
    },
    BodyStatus::Complete { consumed } => ConnBodyStatus::Complete { consumed },
    BodyStatus::Partial { consumed } => ConnBodyStatus::Partial { consumed },
  })
}

fn request_from_core<'a>(request: &CoreRequest<'a>) -> Request<'a> {
  Request {
    method: request.method,
    target: request.target,
    version: request.version,
    headers: request.headers,
    body: request.body,
    keep_alive: request.keep_alive,
    expect_continue: request.expect_continue,
    upgrade: request.upgrade.map(upgrade_kind_from_core),
  }
}

fn upgrade_kind_from_core(upgrade: CoreUpgradeKind) -> UpgradeKind {
  match upgrade {
    CoreUpgradeKind::Any => UpgradeKind::Any,
    CoreUpgradeKind::H2c => UpgradeKind::H2c,
  }
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
  // This is only a readiness gate. `Protocol`/httparse remains authoritative
  // for the actual request-head parse and consumed byte count.
  let mut cursor = 0;
  while let Some(offset) = memchr::memchr(b'\n', &buf[cursor..]) {
    let i = cursor + offset;
    if i > 0 && buf[i - 1] == b'\n' {
      return Some(i - 1);
    }
    if i >= 3 && &buf[i - 3..=i] == b"\r\n\r\n" {
      return Some(i - 3);
    }
    cursor = i + 1;
  }
  None
}

fn append_chunk_prefix(out: &mut Vec<u8>, mut len: usize) {
  let mut buf = [0u8; 16];
  let mut cursor = buf.len();
  loop {
    cursor -= 1;
    let digit = (len & 0xf) as u8;
    buf[cursor] = if digit < 10 {
      b'0' + digit
    } else {
      b'a' + (digit - 10)
    };
    len >>= 4;
    if len == 0 {
      break;
    }
  }
  out.extend_from_slice(&buf[cursor..]);
  out.extend_from_slice(b"\r\n");
}

async fn poll_read_more<I>(
  io: &mut I,
  read_buf: &mut ReadBuf,
) -> Result<usize, Error>
where
  I: AsyncRead + Unpin,
{
  std::future::poll_fn(|cx| {
    let spare = read_buf.spare_capacity_mut(1024);
    let mut tokio_buf = TokioReadBuf::uninit(spare);
    let before = tokio_buf.filled().len();
    ready!(Pin::new(&mut *io).poll_read(cx, &mut tokio_buf))?;
    let read = tokio_buf.filled().len() - before;
    // SAFETY: Tokio guarantees filled bytes were initialized by poll_read.
    unsafe { read_buf.advance_filled(read) };
    Poll::Ready(Ok(read))
  })
  .await
}

fn poll_read_into_scratch<I>(
  io: &mut I,
  cx: &mut Context<'_>,
  scratch: &mut SharedScratch,
) -> Poll<Result<usize, io::Error>>
where
  I: AsyncRead + Unpin,
{
  let mut tokio_buf = TokioReadBuf::new(&mut scratch.read_buf);
  let before = tokio_buf.filled().len();
  ready!(Pin::new(io).poll_read(cx, &mut tokio_buf))?;
  Poll::Ready(Ok(tokio_buf.filled().len() - before))
}

#[cfg(test)]
mod tests {
  use std::collections::VecDeque;
  use std::error::Error as StdError;
  use std::time::Duration;

  use tokio::io::AsyncReadExt;

  use super::*;

  type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

  struct FragmentIo {
    fragments: VecDeque<Vec<u8>>,
    written: Vec<u8>,
  }

  impl FragmentIo {
    fn new(fragments: &[&[u8]]) -> Self {
      Self {
        fragments: fragments.iter().map(|fragment| fragment.to_vec()).collect(),
        written: Vec::new(),
      }
    }
  }

  impl AsyncRead for FragmentIo {
    fn poll_read(
      mut self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      buf: &mut TokioReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
      let Some(front) = self.fragments.front_mut() else {
        return Poll::Ready(Ok(()));
      };
      let len = front.len().min(buf.remaining());
      buf.put_slice(&front[..len]);
      front.drain(..len);
      if front.is_empty() {
        self.fragments.pop_front();
      }
      Poll::Ready(Ok(()))
    }
  }

  impl AsyncWrite for FragmentIo {
    fn poll_write(
      mut self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      buf: &[u8],
    ) -> Poll<io::Result<usize>> {
      self.written.extend_from_slice(buf);
      Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Ready(Ok(()))
    }
  }

  // A write sink that accepts at most `max_write` bytes per `poll_write` and
  // returns `Pending` on every `pending_every`-th call (0 = never). Used to
  // drive the buffered chunked response path through partial writes and
  // re-polls, which the duplex/`FragmentIo`-backed tests never do (they always
  // accept the whole buffer in a single `poll_write`).
  struct ShortWriteIo {
    written: Vec<u8>,
    max_write: usize,
    pending_every: usize,
    calls: usize,
  }

  impl ShortWriteIo {
    fn new(max_write: usize, pending_every: usize) -> Self {
      Self {
        written: Vec::new(),
        max_write: max_write.max(1),
        pending_every,
        calls: 0,
      }
    }
  }

  impl AsyncRead for ShortWriteIo {
    fn poll_read(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
      _buf: &mut TokioReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Ready(Ok(()))
    }
  }

  impl AsyncWrite for ShortWriteIo {
    fn poll_write(
      mut self: Pin<&mut Self>,
      cx: &mut Context<'_>,
      buf: &[u8],
    ) -> Poll<io::Result<usize>> {
      self.calls += 1;
      if self.pending_every != 0
        && self.calls.is_multiple_of(self.pending_every)
      {
        // Wake immediately so the executor re-polls: this is what forces the
        // flush to resume from `write_flushed` (and the chunk writer to be
        // re-polled with `buffered` already set) without re-sending bytes.
        cx.waker().wake_by_ref();
        return Poll::Pending;
      }
      let n = buf.len().min(self.max_write);
      self.written.extend_from_slice(&buf[..n]);
      Poll::Ready(Ok(n))
    }

    fn poll_flush(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
      self: Pin<&mut Self>,
      _cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
      Poll::Ready(Ok(()))
    }
  }

  async fn read_shared_body_from_fragments(
    fragments: &[&[u8]],
  ) -> TestResult<Vec<u8>> {
    let io = FragmentIo::new(fragments);
    let mut conn = SharedConn::new(io);
    let mut scratch = SharedScratch::default();
    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::Chunked);

    let mut body = Vec::new();
    while let SharedBodyChunk::Chunk(chunk) = std::future::poll_fn(|cx| {
      conn.poll_read_body_chunk_with(cx, &mut scratch, |chunk| chunk.to_vec())
    })
    .await?
    {
      body.extend_from_slice(&chunk);
    }
    Ok(body)
  }

  async fn echo_once(input: &[u8]) -> TestResult<Vec<u8>> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let input = input.to_vec();
    let client_task = async move {
      let mut client = client;
      client.write_all(&input).await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      let expect_continue = match conn.next_request(&mut headers).await? {
        Some(request) => request.expect_continue,
        None => return Ok(()),
      };
      if expect_continue {
        conn.write_continue().await?;
      }
      let mut body = Vec::new();
      conn.read_body_to_end(&mut body).await?;
      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Bytes(&body),
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, _) = tokio::join!(client_task, server_task);
    Ok(response?)
  }

  async fn shared_echo_once(input: &[u8]) -> TestResult<Vec<u8>> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    let input = input.to_vec();
    let client_task = async move {
      let mut client = client;
      client.write_all(&input).await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let Some(_request_body) = std::future::poll_fn(|cx| {
        conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
      })
      .await?
      else {
        return Ok(());
      };
      let mut body = Vec::new();
      while let SharedBodyChunk::Chunk(chunk) = std::future::poll_fn(|cx| {
        conn.poll_read_body_chunk_with(cx, &mut scratch, |chunk| chunk.to_vec())
      })
      .await?
      {
        body.extend_from_slice(&chunk);
      }
      let mut writer = SharedResponseWriter::new(Response {
        version: Version::Http11,
        status: 200,
        reason: b"OK",
        headers: &[],
        body: ResponseBody::Bytes(&body),
        keep_alive: false,
      });
      std::future::poll_fn(|cx| {
        conn.poll_write_response_with(cx, &mut scratch, &mut writer)
      })
      .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    Ok(response?)
  }

  #[tokio::test]
  async fn shared_conn_try_takes_full_content_length_body() -> TestResult<()> {
    let (mut client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    client
      .write_all(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhelloGET /next HTTP/1.1\r\nHost: example.com\r\n\r\n",
      )
      .await?;

    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::ContentLength(5));
    assert_eq!(conn.try_take_full_body()?.as_deref(), Some(&b"hello"[..]));

    let method = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| {
        request.method.to_vec()
      })
    })
    .await?
    .unwrap();
    assert_eq!(method, b"GET");
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_content_length_body_respects_read_limit()
  -> TestResult<()> {
    const BODY_LEN: usize = 128 * 1024;
    const READ_LIMIT: usize = 64 * 1024;

    let (mut client, server) = tokio::io::duplex(256 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch =
      SharedScratch::new(BODY_LEN + 1024, DEFAULT_WRITE_CAPACITY);
    let body = vec![b'a'; BODY_LEN];
    let request = format!(
      "POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: {BODY_LEN}\r\n\r\n"
    );
    client.write_all(request.as_bytes()).await?;
    client.write_all(&body).await?;

    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::ContentLength(BODY_LEN as u64));

    let mut received = Vec::new();
    while let SharedBodyChunk::Chunk(chunk) = std::future::poll_fn(|cx| {
      conn.poll_read_body_chunk_limited_with(
        cx,
        &mut scratch,
        READ_LIMIT,
        |chunk| chunk.to_vec(),
      )
    })
    .await?
    {
      assert!(chunk.len() <= READ_LIMIT);
      received.extend_from_slice(&chunk);
    }
    assert_eq!(received, body);
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_try_takes_full_chunked_body() -> TestResult<()> {
    let (mut client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    client
      .write_all(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nhel\r\n2\r\nlo\r\n0\r\n\r\n",
      )
      .await?;

    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::Chunked);
    assert_eq!(conn.try_take_full_body()?.as_deref(), Some(&b"hello"[..]));
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_chunked_body_split_inter_chunk_crlf() -> TestResult<()> {
    let body = read_shared_body_from_fragments(&[
      b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r",
      b"\n3\r\ndef\r\n0\r\n\r\n",
    ])
    .await?;
    assert_eq!(body, b"abcdef");
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_chunked_body_split_chunk_size_line() -> TestResult<()> {
    let body = read_shared_body_from_fragments(&[
      b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n1",
      b"a\r\nabcdefghijklmnopqrstuvwxyz\r\n0\r\n\r\n",
    ])
    .await?;
    assert_eq!(body, b"abcdefghijklmnopqrstuvwxyz");
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_try_take_full_body_preserves_partial_body()
  -> TestResult<()> {
    let (mut client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    client
      .write_all(
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhe",
      )
      .await?;

    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::ContentLength(5));
    assert!(conn.try_take_full_body()?.is_none());

    let chunk = std::future::poll_fn(|cx| {
      conn.poll_read_body_chunk_with(cx, &mut scratch, |chunk| chunk.to_vec())
    })
    .await?;
    assert!(matches!(chunk, SharedBodyChunk::Chunk(chunk) if chunk == b"he"));
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_try_take_full_body_reports_oversized_trailers()
  -> TestResult<()> {
    let (mut client, server) = tokio::io::duplex(128 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::new(128 * 1024, DEFAULT_WRITE_CAPACITY);
    let mut request =
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX: ".to_vec();
    request.extend(std::iter::repeat_n(b'a', 64 * 1024));
    request.extend_from_slice(b"\r\n\r\n");
    client.write_all(&request).await?;

    let body_kind = std::future::poll_fn(|cx| {
      conn.poll_next_request_with(cx, &mut scratch, |request| request.body)
    })
    .await?
    .unwrap();
    assert_eq!(body_kind, BodyKind::Chunked);
    assert!(matches!(
      conn.try_take_full_body(),
      Err(Error::HeadTooLarge)
    ));
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_writes_streaming_chunked_response() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    let client_task = async move {
      let mut client = client;
      client
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      assert!(
        std::future::poll_fn(|cx| {
          conn.poll_next_request_with(cx, &mut scratch, |request| {
            request.keep_alive
          })
        })
        .await?
        .is_some()
      );
      let mut head = SharedChunkedResponseHeadWriter::new(ResponseHead {
        version: Version::Http11,
        status: 200,
        reason: b"OK",
        headers: &[],
        keep_alive: false,
      });
      std::future::poll_fn(|cx| {
        conn.poll_start_chunked_response_with(cx, &mut scratch, &mut head)
      })
      .await?;
      let mut first = SharedResponseChunkWriter::new(b"abc");
      std::future::poll_fn(|cx| {
        conn.poll_write_response_chunk_with(cx, &mut scratch, &mut first)
      })
      .await?;
      let mut second = SharedResponseChunkWriter::new(b"def");
      std::future::poll_fn(|cx| {
        conn.poll_write_response_chunk_with(cx, &mut scratch, &mut second)
      })
      .await?;
      let trailers = [Header {
        name: b"x-trailer",
        value: b"ok",
      }];
      let mut end = SharedResponseEndWriter::new(&trailers);
      std::future::poll_fn(|cx| {
        conn.poll_finish_response_with(cx, &mut scratch, &mut end)
      })
      .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\nx-trailer: ok\r\n\r\n"
    );
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_chunked_response_coalesces_across_short_writes()
  -> TestResult<()> {
    // Flush the entire buffered head + chunks + terminator one byte at a time,
    // with periodic `Pending`, so the coalesced write is delivered across many
    // partial `poll_write`s. Exercises `poll_flush_write_buf`'s resume from
    // `write_flushed` and the `buffered` re-poll guards. The output must still
    // be byte-identical to the single-write case.
    let mut conn = SharedConn::new(ShortWriteIo::new(1, 4));
    let mut scratch = SharedScratch::default();

    let mut head = SharedChunkedResponseHeadWriter::new(ResponseHead {
      version: Version::Http11,
      status: 200,
      reason: b"OK",
      headers: &[],
      keep_alive: false,
    });
    std::future::poll_fn(|cx| {
      conn.poll_start_chunked_response_with(cx, &mut scratch, &mut head)
    })
    .await?;

    for chunk in [b"abc".as_slice(), b"def".as_slice()] {
      let mut writer = SharedResponseChunkWriter::new(chunk);
      std::future::poll_fn(|cx| {
        conn.poll_write_response_chunk_with(cx, &mut scratch, &mut writer)
      })
      .await?;
    }

    let trailers = [Header {
      name: b"x-trailer",
      value: b"ok",
    }];
    let mut end = SharedResponseEndWriter::new(&trailers);
    std::future::poll_fn(|cx| {
      conn.poll_finish_response_with(cx, &mut scratch, &mut end)
    })
    .await?;

    assert_eq!(
      conn.into_inner().written,
      b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\nx-trailer: ok\r\n\r\n"
    );
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_chunked_threshold_flush_survives_short_writes()
  -> TestResult<()> {
    // A chunk larger than `CHUNKED_FLUSH_THRESHOLD` makes
    // `poll_write_response_chunk_with` flush mid-stream. Under short writes that
    // flush returns `Pending` and the chunk writer is re-polled many times; the
    // `buffered` guard must keep it from appending (and thus re-sending) the
    // chunk on each re-poll.
    let mut conn = SharedConn::new(ShortWriteIo::new(1000, 3));
    let mut scratch = SharedScratch::default();

    let mut head = SharedChunkedResponseHeadWriter::new(ResponseHead {
      version: Version::Http11,
      status: 200,
      reason: b"OK",
      headers: &[],
      keep_alive: false,
    });
    std::future::poll_fn(|cx| {
      conn.poll_start_chunked_response_with(cx, &mut scratch, &mut head)
    })
    .await?;

    let big = vec![b'x'; CHUNKED_FLUSH_THRESHOLD + 1024];
    let mut writer = SharedResponseChunkWriter::new(&big);
    std::future::poll_fn(|cx| {
      conn.poll_write_response_chunk_with(cx, &mut scratch, &mut writer)
    })
    .await?;

    let mut end = SharedResponseEndWriter::new(&[]);
    std::future::poll_fn(|cx| {
      conn.poll_finish_response_with(cx, &mut scratch, &mut end)
    })
    .await?;

    let mut expected = Vec::new();
    expected.extend_from_slice(
      b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
    );
    expected.extend_from_slice(format!("{:x}\r\n", big.len()).as_bytes());
    expected.extend_from_slice(&big);
    expected.extend_from_slice(b"\r\n0\r\n\r\n");
    assert_eq!(conn.into_inner().written, expected);
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_fixed_streaming_response_enforces_exact_length()
  -> TestResult<()> {
    let io = FragmentIo::new(&[]);
    let mut conn = SharedConn::new(io);
    let mut scratch = SharedScratch::default();
    conn
      .start_fixed_response_with_scratch(
        &mut scratch,
        ResponseHead {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          keep_alive: true,
        },
        3,
      )
      .await?;
    conn.write_response_body_with_scratch(b"abc").await?;
    conn.finish_response_with_scratch(&mut scratch, &[]).await?;
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_fixed_streaming_response_rejects_overflow()
  -> TestResult<()> {
    let io = FragmentIo::new(&[]);
    let mut conn = SharedConn::new(io);
    let mut scratch = SharedScratch::default();
    conn
      .start_fixed_response_with_scratch(
        &mut scratch,
        ResponseHead {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          keep_alive: true,
        },
        3,
      )
      .await?;
    let err = conn
      .write_response_body_with_scratch(b"abcd")
      .await
      .unwrap_err();
    assert!(matches!(err, Error::ResponseBodyTooLong));
    let io = conn.into_inner();
    assert!(!io.written.ends_with(b"abcd"));
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_fixed_streaming_response_rejects_underflow()
  -> TestResult<()> {
    let io = FragmentIo::new(&[]);
    let mut conn = SharedConn::new(io);
    let mut scratch = SharedScratch::default();
    conn
      .start_fixed_response_with_scratch(
        &mut scratch,
        ResponseHead {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          keep_alive: true,
        },
        5,
      )
      .await?;
    conn.write_response_body_with_scratch(b"abc").await?;
    let err = conn
      .finish_response_with_scratch(&mut scratch, &[])
      .await
      .unwrap_err();
    assert!(matches!(err, Error::ResponseBodyTooShort));
    Ok(())
  }

  #[tokio::test]
  async fn shared_streaming_response_state_is_enforced() -> TestResult<()> {
    let (_client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = SharedConn::new(server);
    let mut scratch = SharedScratch::default();
    let mut chunk = SharedResponseChunkWriter::new(b"abc");
    let err = std::future::poll_fn(|cx| {
      conn.poll_write_response_chunk_with(cx, &mut scratch, &mut chunk)
    })
    .await
    .unwrap_err();
    assert!(matches!(err, Error::ResponseStreamInactive));
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_echoes_content_length_body() -> TestResult<()> {
    let response = shared_echo_once(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhello",
    )
    .await?;
    assert_eq!(
      response,
      b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello"
    );
    Ok(())
  }

  #[tokio::test]
  async fn shared_conn_echoes_chunked_body() -> TestResult<()> {
    let response = shared_echo_once(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
    )
    .await?;
    assert_eq!(
      response,
      b"HTTP/1.1 200 OK\r\ncontent-length: 12\r\nconnection: close\r\n\r\nHellO world1"
    );
    Ok(())
  }

  #[tokio::test]
  async fn echoes_content_length_body() -> TestResult<()> {
    let response = echo_once(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhello",
    )
    .await?;
    assert_eq!(
      response,
      b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello"
    );
    Ok(())
  }

  #[tokio::test]
  async fn echoes_chunked_body() -> TestResult<()> {
    let response = echo_once(
      b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
    )
    .await?;
    assert_eq!(
      response,
      b"HTTP/1.1 200 OK\r\ncontent-length: 12\r\nconnection: close\r\n\r\nHellO world1"
    );
    Ok(())
  }

  #[tokio::test]
  async fn streams_content_length_body_across_reads() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(
          b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhe",
        )
        .await?;
      tokio::time::sleep(Duration::from_millis(20)).await;
      client.write_all(b"llo").await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      let request = conn.next_request(&mut headers).await?.unwrap();
      assert_eq!(request.body, BodyKind::ContentLength(5));

      let mut body = Vec::new();
      assert!(conn.read_body_chunk_into(&mut body).await?);
      assert_eq!(body, b"he");
      assert!(conn.read_body_chunk_into(&mut body).await?);
      assert_eq!(body, b"hello");
      assert!(!conn.read_body_chunk_into(&mut body).await?);

      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Bytes(&body),
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello"
    );
    Ok(())
  }

  #[tokio::test]
  async fn reads_request_body_incrementally() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(
          b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n",
        )
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      assert!(conn.next_request(&mut headers).await?.is_some());
      let mut body = Vec::new();
      let mut chunk_count = 0;
      while conn.read_body_chunk_into(&mut body).await? {
        chunk_count += 1;
      }
      assert_eq!(body, b"abcdef");
      assert!(chunk_count >= 2);
      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Empty,
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    Ok(())
  }

  #[tokio::test]
  async fn streams_chunked_body_across_fragmented_writes() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(
          b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n3\r\na",
        )
        .await?;
      tokio::time::sleep(Duration::from_millis(20)).await;
      client.write_all(b"bc\r\n3\r\n").await?;
      tokio::time::sleep(Duration::from_millis(20)).await;
      client
        .write_all(b"def\r\n0\r\nX-Trailer: ignored\r\n\r\n")
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      let request = conn.next_request(&mut headers).await?.unwrap();
      assert_eq!(request.body, BodyKind::Chunked);

      let mut body = Vec::new();
      assert!(conn.read_body_chunk_into(&mut body).await?);
      assert_eq!(body, b"a");
      while conn.read_body_chunk_into(&mut body).await? {}
      assert_eq!(body, b"abcdef");

      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Bytes(&body),
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ncontent-length: 6\r\nconnection: close\r\n\r\nabcdef"
    );
    Ok(())
  }

  #[tokio::test]
  async fn streams_chunked_body_one_byte_at_a_time() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(
          b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        )
        .await?;
      for byte in b"3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n" {
        client.write_all(&[*byte]).await?;
        tokio::task::yield_now().await;
      }
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      let request = conn.next_request(&mut headers).await?.unwrap();
      assert_eq!(request.body, BodyKind::Chunked);

      let mut body = Vec::new();
      while conn.read_body_chunk_into(&mut body).await? {}
      assert_eq!(body, b"abcdef");

      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Bytes(&body),
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ncontent-length: 6\r\nconnection: close\r\n\r\nabcdef"
    );
    Ok(())
  }

  #[tokio::test]
  async fn next_request_drains_unread_streamed_body() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(
          b"POST /one HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhelloGET /two HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n",
        )
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      {
        let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
        let first = conn.next_request(&mut headers).await?.unwrap();
        assert_eq!(first.target, b"/one");
      }

      let target = {
        let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
        let second = conn.next_request(&mut headers).await?.unwrap();
        assert_eq!(second.target, b"/two");
        second.target.to_vec()
      };
      conn
        .write_response(Response {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          body: ResponseBody::Bytes(&target),
          keep_alive: false,
        })
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ncontent-length: 4\r\nconnection: close\r\n\r\n/two"
    );
    Ok(())
  }

  #[tokio::test]
  async fn writes_streaming_chunked_response() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      assert!(conn.next_request(&mut headers).await?.is_some());
      let response_headers = [Header {
        name: b"trailer",
        value: b"x-sig",
      }];
      conn
        .start_chunked_response(ResponseHead {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &response_headers,
          keep_alive: false,
        })
        .await?;
      conn.write_response_chunk(b"hello").await?;
      conn.write_response_chunk(b" world").await?;
      let trailers = [Header {
        name: b"x-sig",
        value: b"abc",
      }];
      conn.finish_response(&trailers).await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ntrailer: x-sig\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\nx-sig: abc\r\n\r\n"
    );
    Ok(())
  }

  #[tokio::test]
  async fn streaming_response_state_is_enforced() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      assert!(conn.next_request(&mut headers).await?.is_some());
      assert!(matches!(
        conn.write_response_chunk(b"early").await,
        Err(Error::ResponseStreamInactive)
      ));
      assert!(matches!(
        conn.finish_response(&[]).await,
        Err(Error::ResponseStreamInactive)
      ));

      conn
        .start_chunked_response(ResponseHead {
          version: Version::Http11,
          status: 200,
          reason: b"OK",
          headers: &[],
          keep_alive: false,
        })
        .await?;
      assert!(matches!(
        conn
          .start_chunked_response(ResponseHead {
            version: Version::Http11,
            status: 200,
            reason: b"OK",
            headers: &[],
            keep_alive: false,
          })
          .await,
        Err(Error::ResponseStreamActive)
      ));
      assert!(matches!(
        conn
          .write_response(Response {
            version: Version::Http11,
            status: 200,
            reason: b"OK",
            headers: &[],
            body: ResponseBody::Empty,
            keep_alive: false,
          })
          .await,
        Err(Error::ResponseStreamActive)
      ));
      conn.write_response_chunk(b"ok").await?;
      conn.finish_response(&[]).await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n2\r\nok\r\n0\r\n\r\n"
    );
    Ok(())
  }

  #[tokio::test]
  async fn streaming_no_body_status_ignores_body_chunks() -> TestResult<()> {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let mut conn = Conn::new(server);
    let client_task = async move {
      let mut client = client;
      client
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await?;
      let mut response = Vec::new();
      client.read_to_end(&mut response).await?;
      Ok::<_, io::Error>(response)
    };
    let server_task = async move {
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      assert!(conn.next_request(&mut headers).await?.is_some());
      conn
        .start_chunked_response(ResponseHead {
          version: Version::Http11,
          status: 204,
          reason: b"No Content",
          headers: &[],
          keep_alive: false,
        })
        .await?;
      conn.write_response_chunk(b"must-not-write").await?;
      conn
        .finish_response(&[Header {
          name: b"x-ignored",
          value: b"ignored",
        }])
        .await?;
      Ok::<_, Error>(())
    };
    let (response, server_result) = tokio::join!(client_task, server_task);
    server_result?;
    assert_eq!(
      response?,
      b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n"
    );
    Ok(())
  }

  async fn h1spec_echo(input: &'static [u8]) -> TestResult<Option<Vec<u8>>> {
    let (mut client, server) = tokio::io::duplex(64 * 1024);
    let server = tokio::spawn(async move {
      let mut conn = Conn::new(server);
      let mut headers = [Header::EMPTY; crate::MAX_HEADERS];
      match conn.next_request(&mut headers).await {
        Ok(Some(request)) => {
          let expect_continue = request.expect_continue;
          if expect_continue {
            conn.write_continue().await?;
          }
          let mut body = Vec::new();
          if conn.read_body_to_end(&mut body).await.is_err() {
            conn
              .write_response(Response {
                version: Version::Http11,
                status: 400,
                reason: b"Bad Request",
                headers: &[],
                body: ResponseBody::Empty,
                keep_alive: false,
              })
              .await?;
            return Ok(());
          }
          conn
            .write_response(Response {
              version: Version::Http11,
              status: 200,
              reason: b"OK",
              headers: &[],
              body: ResponseBody::Bytes(&body),
              keep_alive: false,
            })
            .await?;
        }
        Ok(None) => {}
        Err(_) => {
          conn
            .write_response(Response {
              version: Version::Http11,
              status: 400,
              reason: b"Bad Request",
              headers: &[],
              body: ResponseBody::Empty,
              keep_alive: false,
            })
            .await?;
        }
      }
      Ok::<_, Error>(())
    });
    client.write_all(input).await?;
    let mut response = vec![0; 4096];
    let read = tokio::time::timeout(
      std::time::Duration::from_millis(100),
      client.read(&mut response),
    )
    .await;
    server.abort();
    match read {
      Ok(Ok(read)) if read > 0 => {
        response.truncate(read);
        Ok(Some(response))
      }
      Ok(Ok(_)) | Err(_) => Ok(None),
      Ok(Err(err)) => Err(err.into()),
    }
  }

  fn status(response: &[u8]) -> TestResult<u16> {
    let status = std::str::from_utf8(&response[..12])?
      .split(' ')
      .nth(1)
      .ok_or("missing status code")?
      .parse()?;
    Ok(status)
  }

  fn body(response: &[u8]) -> TestResult<&[u8]> {
    let index = response
      .windows(4)
      .position(|window| window == b"\r\n\r\n")
      .ok_or("missing header terminator")?;
    Ok(&response[index + 4..])
  }

  #[tokio::test]
  async fn h1spec_black_box_non_timeout_subset() -> TestResult<()> {
    type H1SpecCase<'a> = (
      &'a str,
      &'a [u8],
      std::ops::RangeInclusive<u16>,
      Option<&'a [u8]>,
    );
    let cases: &[H1SpecCase<'_>] = &[
      (
        "Request without HTTP version",
        b"GET / \r\n\r\n",
        400..=599,
        None,
      ),
      (
        "Request with Expect header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nExpect: 100-continue\r\n\r\n",
        100..=100,
        None,
      ),
      (
        "Valid GET request",
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
        200..=299,
        Some(b""),
      ),
      (
        "Valid GET request with edge cases",
        b"GET / HTTP/1.1\r\nhoSt:\texample.com\r\nempty:\r\n\r\n",
        200..=299,
        Some(b""),
      ),
      (
        "Invalid header characters",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Invalid[]: test\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Missing Host header",
        b"GET / HTTP/1.1\r\nContent-Length: 5\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Multiple Host headers",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nHost: example.org\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Overflowing negative Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: -123456789123456789123456789\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Negative Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: -1234\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Non-numeric Content-Length header",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Length: abc\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Empty header value",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Empty-Header: \r\n\r\n",
        200..=299,
        Some(b""),
      ),
      (
        "Header containing invalid control character",
        b"GET / HTTP/1.1\r\nHost: example.com\r\nX-Bad-Control-Char: test\x07\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Invalid HTTP version",
        b"GET / HTTP/9.9\r\nHost: example.com\r\n\r\n",
        400..=599,
        None,
      ),
      (
        "Invalid prefix of request",
        b"Extra lineGET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
        400..=599,
        None,
      ),
      (
        "Invalid line ending",
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\rSome-Header: Test\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Valid POST request with body",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: 5\r\n\r\nhello",
        200..=299,
        Some(b"hello"),
      ),
      (
        "Chunked Transfer-Encoding",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
        200..=299,
        Some(b"HellO world1"),
      ),
      (
        "Chunked trailer with invalid field-name",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\nBad Name: value\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Chunked trailer with invalid field-value",
        b"POST / HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\nX-Bad: value\x07\r\n\r\n",
        400..=499,
        None,
      ),
      (
        "Conflicting Transfer-Encoding and Content-Length in varying case",
        b"POST / HTTP/1.1\r\nHost: example.com\r\ncontent-LengtH: 5\r\nTransFer-Encoding: chunked\r\n\r\nc\r\nHellO world1\r\n0\r\n\r\n",
        400..=499,
        None,
      ),
    ];

    for (description, input, expected_status, expected_body) in cases {
      let response = h1spec_echo(input)
        .await?
        .ok_or_else(|| format!("{description}: no response"))?;
      assert!(
        expected_status.contains(&status(&response)?),
        "{description}: response={}",
        String::from_utf8_lossy(&response),
      );
      if let Some(expected_body) = expected_body {
        assert_eq!(
          body(&response)?,
          *expected_body,
          "{description}: response={}",
          String::from_utf8_lossy(&response),
        );
      }
    }
    Ok(())
  }

  #[tokio::test]
  async fn h1spec_black_box_fragment_timeout_cases() -> TestResult<()> {
    let cases: &[(&str, &[u8])] = &[
      ("Fragmented method", b"G"),
      ("Fragmented URL 1", b"GET "),
      ("Fragmented URL 2", b"GET /hello"),
      ("Fragmented URL 3", b"GET /hello "),
      ("Fragmented HTTP version", b"GET /hello HTTP"),
      ("Fragmented request line", b"GET /hello HTTP/1.1"),
      (
        "Fragmented request line newline 1",
        b"GET /hello HTTP/1.1\r",
      ),
      (
        "Fragmented request line newline 2",
        b"GET /hello HTTP/1.1\r\n",
      ),
      ("Fragmented field name", b"GET /hello HTTP/1.1\r\nHos"),
      ("Fragmented field value 1", b"GET /hello HTTP/1.1\r\nHost:"),
      ("Fragmented field value 2", b"GET /hello HTTP/1.1\r\nHost: "),
      (
        "Fragmented field value 3",
        b"GET /hello HTTP/1.1\r\nHost: localhost",
      ),
      (
        "Fragmented field value 4",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r",
      ),
      (
        "Fragmented request",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r\n",
      ),
      (
        "Fragmented request termination",
        b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r",
      ),
    ];

    for (description, input) in cases {
      assert!(
        h1spec_echo(input).await?.is_none(),
        "{description}: expected no response"
      );
    }
    Ok(())
  }
}
