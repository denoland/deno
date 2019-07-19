// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use futures_01::Async;
use hyper::body::Payload;
use hyper::Body;
use hyper::Chunk;
use std::cmp::min;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;

/// Wraps `hyper::Body` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  body: Body,
  chunk: Option<Chunk>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: Body) -> Self {
    Self {
      body,
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
    _cx: &mut Context,
    buf: &mut [u8],
  ) -> Poll<io::Result<usize>> {
    let inner = Pin::get_mut(self);
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

    let p = inner.body.poll_data();
    match p {
      Err(e) => Poll::Ready(Err(
        // TODO Need to map hyper::Error into std::io::Error.
        io::Error::new(io::ErrorKind::Other, e),
      )),
      Ok(Async::NotReady) => Poll::Pending,
      Ok(Async::Ready(maybe_chunk)) => match maybe_chunk {
        None => Poll::Ready(Ok(0)),
        Some(chunk) => {
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
      },
    }
  }
}

#[test]
fn test_body_async_read() {
  use std::str::from_utf8;
  let body = Body::from("hello world");
  let mut body = HttpBody::from(body);

  let buf = &mut [0, 0, 0, 0, 0];
  let r = body.poll_read(buf);
  assert!(r.is_ok());
  assert_eq!(r.unwrap(), Async::Ready(5));
  assert_eq!(from_utf8(buf).unwrap(), "hello");

  let r = body.poll_read(buf);
  assert!(r.is_ok());
  assert_eq!(r.unwrap(), Async::Ready(5));
  assert_eq!(from_utf8(buf).unwrap(), " worl");

  let r = body.poll_read(buf);
  assert!(r.is_ok());
  assert_eq!(r.unwrap(), Async::Ready(1));
  assert_eq!(from_utf8(&buf[0..1]).unwrap(), "d");
}
