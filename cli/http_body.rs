// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use reqwest;
use std::cmp::min;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;

// TODO(bartlomieju): most of this stuff can be moved to `cli/ops/fetch.rs`
type ReqwestStream = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

/// Wraps `ReqwestStream` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  stream: ReqwestStream,
  chunk: Option<Bytes>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: ReqwestStream) -> Self {
    Self {
      stream: body,
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

    let p = inner.stream.poll_next_unpin(cx);
    match p {
      Poll::Ready(Some(Err(e))) => Poll::Ready(Err(
        // TODO(bartlomieju): rewrite it to use ErrBox
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
