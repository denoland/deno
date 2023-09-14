// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use bytes::Bytes;
use deno_core::error::AnyError;
use deno_core::futures::stream::Peekable;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::RcRef;
use deno_core::Resource;
use hyper1::body::Body;
use hyper1::body::Incoming;
use hyper1::body::SizeHint;
use std::borrow::Cow;
use std::pin::Pin;
use std::rc::Rc;

/// Converts a hyper incoming body stream into a stream of [`Bytes`] that we can use to read in V8.
struct ReadFuture(Incoming);

impl Stream for ReadFuture {
  type Item = Result<Bytes, AnyError>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    let res = Pin::new(&mut self.get_mut().0).poll_frame(cx);
    match res {
      std::task::Poll::Ready(Some(Ok(frame))) => {
        if let Ok(data) = frame.into_data() {
          // Ensure that we never yield an empty frame
          if !data.is_empty() {
            return std::task::Poll::Ready(Some(Ok(data)));
          }
        }
      }
      std::task::Poll::Ready(Some(Err(e))) => {
        return std::task::Poll::Ready(Some(Err(e.into())));
      }
      std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
      _ => {}
    }
    std::task::Poll::Pending
  }
}

pub struct HttpRequestBody(AsyncRefCell<Peekable<ReadFuture>>, SizeHint);

impl HttpRequestBody {
  pub fn new(body: Incoming) -> Self {
    let size_hint = body.size_hint();
    Self(AsyncRefCell::new(ReadFuture(body).peekable()), size_hint)
  }

  async fn read(self: Rc<Self>, limit: usize) -> Result<BufView, AnyError> {
    let peekable = RcRef::map(self, |this| &this.0);
    let mut peekable = peekable.borrow_mut().await;
    match Pin::new(&mut *peekable).peek_mut().await {
      None => Ok(BufView::empty()),
      Some(Err(_)) => Err(peekable.next().await.unwrap().err().unwrap()),
      Some(Ok(bytes)) => {
        if bytes.len() <= limit {
          // We can safely take the next item since we peeked it
          return Ok(BufView::from(peekable.next().await.unwrap()?));
        }
        let ret = bytes.split_to(limit);
        Ok(BufView::from(ret))
      }
    }
  }
}

impl Resource for HttpRequestBody {
  fn name(&self) -> Cow<str> {
    "requestBody".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(HttpRequestBody::read(self, limit))
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (self.1.lower(), self.1.upper())
  }
}
