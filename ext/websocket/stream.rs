// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use bytes::Buf;
use bytes::Bytes;
use deno_net::raw::NetworkStream;
use hyper::upgrade::Upgraded;
use std::pin::Pin;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;

// TODO(bartlomieju): remove this
pub(crate) enum WsStreamKind {
  Upgraded(Upgraded),
  Network(NetworkStream),
}

pub(crate) struct WebSocketStream {
  stream: WsStreamKind,
  pre: Option<Bytes>,
}

impl WebSocketStream {
  pub fn new(stream: WsStreamKind, buffer: Option<Bytes>) -> Self {
    Self {
      stream,
      pre: buffer,
    }
  }
}

impl AsyncRead for WebSocketStream {
  // From hyper's Rewind (https://github.com/hyperium/hyper), MIT License, Copyright (c) Sean McArthur
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<std::io::Result<()>> {
    if let Some(mut prefix) = self.pre.take() {
      // If there are no remaining bytes, let the bytes get dropped.
      if !prefix.is_empty() {
        let copy_len = std::cmp::min(prefix.len(), buf.remaining());
        // TODO: There should be a way to do following two lines cleaner...
        buf.put_slice(&prefix[..copy_len]);
        prefix.advance(copy_len);
        // Put back what's left
        if !prefix.is_empty() {
          self.pre = Some(prefix);
        }

        return Poll::Ready(Ok(()));
      }
    }
    match &mut self.stream {
      WsStreamKind::Network(stream) => Pin::new(stream).poll_read(cx, buf),
      WsStreamKind::Upgraded(stream) => Pin::new(stream).poll_read(cx, buf),
    }
  }
}

impl AsyncWrite for WebSocketStream {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    match &mut self.stream {
      WsStreamKind::Network(stream) => Pin::new(stream).poll_write(cx, buf),
      WsStreamKind::Upgraded(stream) => Pin::new(stream).poll_write(cx, buf),
    }
  }

  fn poll_flush(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    match &mut self.stream {
      WsStreamKind::Network(stream) => Pin::new(stream).poll_flush(cx),
      WsStreamKind::Upgraded(stream) => Pin::new(stream).poll_flush(cx),
    }
  }

  fn poll_shutdown(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Result<(), std::io::Error>> {
    match &mut self.stream {
      WsStreamKind::Network(stream) => Pin::new(stream).poll_shutdown(cx),
      WsStreamKind::Upgraded(stream) => Pin::new(stream).poll_shutdown(cx),
    }
  }

  fn is_write_vectored(&self) -> bool {
    match &self.stream {
      WsStreamKind::Network(stream) => stream.is_write_vectored(),
      WsStreamKind::Upgraded(stream) => stream.is_write_vectored(),
    }
  }

  fn poll_write_vectored(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> std::task::Poll<Result<usize, std::io::Error>> {
    match &mut self.stream {
      WsStreamKind::Network(stream) => {
        Pin::new(stream).poll_write_vectored(cx, bufs)
      }
      WsStreamKind::Upgraded(stream) => {
        Pin::new(stream).poll_write_vectored(cx, bufs)
      }
    }
  }
}
