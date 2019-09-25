// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use futures::stream::Stream;
use futures::Async;
use futures::Poll;
use reqwest::r#async::Chunk;
use reqwest::r#async::Decoder;
use std::cmp::min;
use std::io;
use std::io::Read;
use tokio::io::AsyncRead;

/// Wraps `reqwest::Decoder` so that it can be exposed as an `AsyncRead` and integrated
/// into resources more easily.
pub struct HttpBody {
  decoder: Decoder,
  chunk: Option<Chunk>,
  pos: usize,
}

impl HttpBody {
  pub fn from(body: Decoder) -> Self {
    Self {
      decoder: body,
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
  fn poll_read(&mut self, buf: &mut [u8]) -> Poll<usize, io::Error> {
    if let Some(chunk) = self.chunk.take() {
      debug!(
        "HttpBody Fake Read buf {} chunk {} pos {}",
        buf.len(),
        chunk.len(),
        self.pos
      );
      let n = min(buf.len(), chunk.len() - self.pos);
      {
        let rest = &chunk[self.pos..];
        buf[..n].clone_from_slice(&rest[..n]);
      }
      self.pos += n;
      if self.pos == chunk.len() {
        self.pos = 0;
      } else {
        self.chunk = Some(chunk);
      }
      return Ok(Async::Ready(n));
    } else {
      assert_eq!(self.pos, 0);
    }

    let p = self.decoder.poll();
    match p {
      Err(e) => Err(
        // TODO Need to map hyper::Error into std::io::Error.
        io::Error::new(io::ErrorKind::Other, e),
      ),
      Ok(Async::NotReady) => Ok(Async::NotReady),
      Ok(Async::Ready(maybe_chunk)) => match maybe_chunk {
        None => Ok(Async::Ready(0)),
        Some(chunk) => {
          debug!(
            "HttpBody Real Read buf {} chunk {} pos {}",
            buf.len(),
            chunk.len(),
            self.pos
          );
          let n = min(buf.len(), chunk.len());
          buf[..n].clone_from_slice(&chunk[..n]);
          if buf.len() < chunk.len() {
            self.pos = n;
            self.chunk = Some(chunk);
          }
          Ok(Async::Ready(n))
        }
      },
    }
  }
}
