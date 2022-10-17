// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::Stream;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug)]
pub struct InnerRequest {
  /// Backing buffer for the request.
  pub buffer: Pin<Box<[u8]>>,
  /// Owned headers, we have to keep it around since its referenced in `req`.
  pub _headers: Vec<httparse::Header<'static>>,
  /// Fully parsed request.
  pub req: httparse::Request<'static, 'static>,
  pub body_offset: usize,
  pub body_len: usize,
}

#[derive(Debug)]
pub struct Request {
  pub inner: InnerRequest,
  pub socket: Arc<Stream>,
  pub parse_buffer: Vec<u8>,
  pub keep_alive: bool,
  pub content_read: usize,
  pub content_length: Option<u64>,
  pub remaining_chunk_size: Option<usize>,
  pub te_chunked: bool,
  pub expect_continue: bool,
}

impl Request {
  #[inline(always)]
  pub fn method(&self) -> &str {
    self.inner.req.method.unwrap()
  }
}
