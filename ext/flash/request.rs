// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::Stream;
use std::pin::Pin;
use tokio::sync::oneshot;

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
  // Dereferencing is safe until websocket upgrade is performed.
  pub socket: *mut Stream,
  pub keep_alive: bool,
  pub content_read: usize,
  pub content_length: Option<u64>,
  pub remaining_chunk_size: Option<usize>,
  pub te_chunked: bool,
  pub expect_continue: bool,
  pub socket_rx: oneshot::Receiver<Pin<Box<Stream>>>,
  pub owned_socket: Option<Pin<Box<Stream>>>,
}

// SAFETY: Sent from server thread to JS thread.
// See comment above for `socket`.
unsafe impl Send for Request {}

impl Request {
  #[inline(always)]
  pub fn socket<'a>(&mut self) -> &'a mut Stream {
    if let Ok(mut sock) = self.socket_rx.try_recv() {
      // SAFETY: We never move the data out of the acquired mutable reference.
      self.socket = unsafe { sock.as_mut().get_unchecked_mut() };

      // Let the struct own the socket so that it won't get dropped.
      self.owned_socket = Some(sock);
    }

    // SAFETY: Dereferencing is safe until server thread detaches socket.
    unsafe { &mut *self.socket }
  }

  #[inline(always)]
  pub fn method(&self) -> &str {
    self.inner.req.method.unwrap()
  }
}
