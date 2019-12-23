// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use reqwest::Response;
use std::cmp::min;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;

/// Wraps `reqwest::Decoder` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  response: Response,
  chunk: Option<Bytes>,
  pos: usize,
}

impl HttpBody {
  pub fn from(response: Response) -> Self {
    Self {
      response,
      chunk: None,
      pos: 0,
    }
  }
}

impl AsyncRead for HttpBody {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<Result<usize, io::Error>> {
    let mut inner = self.get_mut();
    if let Some(chunk) = inner.chunk.take() {
      debug!(
        "HttpBody Fake Read buf {} chunk {} pos {}",
        buf.len(),
        chunk.len(),
        inner.pos
      );
      let n = min(buf.len(), chunk.len() - inner.pos);
      {
        let rest = &chunk[inner.pos..];
        buf[..n].clone_from_slice(&rest[..n]);
      }
      inner.pos += n;
      if inner.pos == chunk.len() {
        inner.pos = 0;
      } else {
        inner.chunk = Some(chunk);
      }
      return Poll::Ready(Ok(n));
    } else {
      assert_eq!(inner.pos, 0);
    }

    let chunk_future = &mut inner.response.chunk();
    // Safety: `chunk_future` lives only for duration of this poll. So, it doesn't move.
    let chunk_future = unsafe { Pin::new_unchecked(chunk_future) };
    match chunk_future.poll(cx) {
      Poll::Ready(Err(e)) => Poll::Ready(Err(
        // TODO Need to map hyper::Error into std::io::Error.
        io::Error::new(io::ErrorKind::Other, e),
      )),
      Poll::Ready(Ok(Some(chunk))) => {
        debug!(
          "HttpBody Real Read buf {} chunk {} pos {}",
          buf.len(),
          chunk.len(),
          inner.pos
        );
        let n = min(buf.len(), chunk.len());
        buf[..n].clone_from_slice(&chunk[..n]);
        if buf.len() < chunk.len() {
          inner.pos = n;
          inner.chunk = Some(chunk);
        }
        Poll::Ready(Ok(n))
      }
      Poll::Ready(Ok(None)) => Poll::Ready(Ok(0)),
      Poll::Pending => Poll::Pending,
    }
  }
}
