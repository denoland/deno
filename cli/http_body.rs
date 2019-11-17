// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use futures::io::AsyncRead;
use futures::stream::StreamExt;
use reqwest::r#async::Chunk;
use reqwest::r#async::Decoder;
use std::cmp::min;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

/// Wraps `reqwest::Decoder` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  decoder: futures::compat::Compat01As03<Decoder>,
  chunk: Option<Chunk>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: Decoder) -> Self {
    Self {
      decoder: futures::compat::Compat01As03::new(body),
      chunk: None,
      pos: 0,
    }
  }
}

impl Read for HttpBody {
  fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
    unimplemented!();
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

    let p = inner.decoder.poll_next_unpin(cx);
    match p {
      Poll::Ready(Some(Err(e))) => Poll::Ready(Err(
        // TODO Need to map hyper::Error into std::io::Error.
        io::Error::new(io::ErrorKind::Other, e),
      )),
      Poll::Ready(Some(Ok(chunk))) => {
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
      Poll::Ready(None) => Poll::Ready(Ok(0)),
      Poll::Pending => Poll::Pending,
    }
  }
}
