// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use deno_core::futures::Stream;
use tokio::sync::mpsc;

/// [MpscByteStream] is a stream of bytes that is backed by a mpsc channel. It is
/// used to bridge between the fetch task and the HTTP body stream. The stream
/// has the special property that it errors if the channel is closed before an
/// explicit EOF is sent (in the form of a [None] value on the sender).
pub struct MpscByteStream {
  receiver: mpsc::Receiver<Option<bytes::Bytes>>,
  shutdown: bool,
}

impl MpscByteStream {
  pub fn new() -> (Self, mpsc::Sender<Option<bytes::Bytes>>) {
    let (sender, receiver) = mpsc::channel::<Option<bytes::Bytes>>(1);
    let this = Self {
      receiver,
      shutdown: false,
    };
    (this, sender)
  }
}

impl Stream for MpscByteStream {
  type Item = Result<bytes::Bytes, std::io::Error>;

  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    let val = std::task::ready!(self.receiver.poll_recv(cx));
    match val {
      None if self.shutdown => Poll::Ready(None),
      None => Poll::Ready(Some(Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "channel closed",
      )))),
      Some(None) => {
        self.shutdown = true;
        Poll::Ready(None)
      }
      Some(Some(val)) => Poll::Ready(Some(Ok(val))),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use bytes::Bytes;
  use deno_core::futures::StreamExt;

  #[tokio::test]
  async fn success() {
    let (mut stream, sender) = MpscByteStream::new();

    sender.send(Some(Bytes::from("hello"))).await.unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), Bytes::from("hello"));

    sender.send(Some(Bytes::from("world"))).await.unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), Bytes::from("world"));

    sender.send(None).await.unwrap();
    drop(sender);
    assert!(stream.next().await.is_none());
  }

  #[tokio::test]
  async fn error() {
    let (mut stream, sender) = MpscByteStream::new();

    sender.send(Some(Bytes::from("hello"))).await.unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), Bytes::from("hello"));

    drop(sender);
    assert_eq!(
      stream.next().await.unwrap().unwrap_err().kind(),
      std::io::ErrorKind::UnexpectedEof
    );
  }
}
