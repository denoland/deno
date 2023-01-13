// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::Stream;
use std::pin::Pin;

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
  // Pointer to stream owned by the server loop thread.
  //
  // Dereferencing is safe until server thread finishes and
  // op_flash_serve resolves or websocket upgrade is performed.
  pub socket: *mut Stream,
  pub keep_alive: bool,
  pub content_read: usize,
  pub content_length: Option<u64>,
  pub remaining_chunk_size: Option<usize>,
  pub te_chunked: bool,
  pub expect_continue: bool,
}

// SAFETY: Sent from server thread to JS thread.
// See comment above for `socket`.
unsafe impl Send for Request {}

impl Request {
  #[inline(always)]
  pub fn socket<'a>(&self) -> &'a mut Stream {
    // SAFETY: Dereferencing is safe until server thread detaches socket or finishes.
    unsafe { &mut *self.socket }
  }

  #[inline(always)]
  pub fn method(&self) -> &str {
    self.inner.req.method.unwrap()
  }
}
