// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

use bytes::Bytes;
use reqwest::Response;
use std::cmp::min;
use std::io;

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

  pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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
      return Ok(n);
    } else {
      assert_eq!(self.pos, 0);
    }

    match self.response.chunk().await {
      Ok(Some(chunk)) => {
        debug!(
          "HttpBody Real Read buf {} chunk {} pos {}",
          buf.len(),
          chunk.len(),
          self.pos
        );
        let n = min(buf.len(), chunk.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        if buf.len() < chunk.len() {
          self.pos = n;
          self.chunk = Some(chunk);
        }
        Ok(n)
      }
      Ok(None) => Ok(0),
      Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
  }
}
