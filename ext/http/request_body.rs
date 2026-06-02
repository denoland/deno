// Copyright 2018-2026 the Deno authors. MIT license.
use std::borrow::Cow;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use bytes::Bytes;
use bytes::BytesMut;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::futures::TryFutureExt;
use deno_core::futures::stream::Peekable;
use deno_core::futures::task::noop_waker;
use deno_error::JsErrorBox;
use hyper::body::Body;
use hyper::body::Frame;
use hyper::body::Incoming;
use hyper::body::SizeHint;

/// Wraps a hyper [`Incoming`] body to add a non-blocking
/// "drain everything available right now" operation
/// ([`try_take_full`]) used by the JS-side fast path on
/// `req.json()` / `.text()` / `.bytes()`.
///
/// The wrapper itself implements [`Body`], so the streaming
/// path through [`HttpRequestBody`] is unchanged. When the JS
/// handler asks for the body in one shot, the fast op tries
/// to non-blockingly drain all currently-available frames; if
/// the body is fully drained without ever returning `Pending`
/// we hand the bytes to JS synchronously and skip the
/// ReadableStream wrapper. Otherwise frames we already pulled
/// from `inner` are kept in `pending` and replayed before the
/// next poll of `inner`, so the streaming path picks up cleanly
/// without losing data.
pub struct BufferedIncoming {
  inner: Incoming,
  /// Frames we've polled out of `inner` but haven't yet emitted
  /// via `poll_frame`. Replayed before further polls.
  pending: BytesMut,
  done: bool,
}

impl BufferedIncoming {
  pub fn new(inner: Incoming) -> Self {
    Self {
      inner,
      pending: BytesMut::new(),
      done: false,
    }
  }

  /// Try to drain the entire body without ever returning
  /// [`Poll::Pending`]. Returns `Some(bytes)` iff every
  /// frame is already buffered in hyper; otherwise leaves
  /// the wrapper in a state where the streaming path can
  /// keep going (any frames we already pulled are replayed
  /// from `pending` on the next [`Body::poll_frame`]).
  pub fn try_take_full(&mut self) -> Option<Vec<u8>> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
      if self.done {
        return Some(std::mem::take(&mut self.pending).to_vec());
      }
      match Pin::new(&mut self.inner).poll_frame(&mut cx) {
        Poll::Ready(Some(Ok(frame))) => {
          if let Ok(data) = frame.into_data() {
            self.pending.extend_from_slice(&data);
          }
          // Trailers and other non-data frames are ignored,
          // matching ReadFuture's "data only" loop.
        }
        Poll::Ready(Some(Err(_))) => return None,
        Poll::Ready(None) => {
          self.done = true;
          return Some(std::mem::take(&mut self.pending).to_vec());
        }
        Poll::Pending => return None,
      }
    }
  }
}

impl Body for BufferedIncoming {
  type Data = Bytes;
  type Error = hyper::Error;

  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    let this = self.get_mut();
    if !this.pending.is_empty() {
      let chunk = this.pending.split().freeze();
      return Poll::Ready(Some(Ok(Frame::data(chunk))));
    }
    if this.done {
      return Poll::Ready(None);
    }
    match Pin::new(&mut this.inner).poll_frame(cx) {
      Poll::Ready(None) => {
        this.done = true;
        Poll::Ready(None)
      }
      other => other,
    }
  }

  fn size_hint(&self) -> SizeHint {
    self.inner.size_hint()
  }

  fn is_end_stream(&self) -> bool {
    self.done && self.pending.is_empty()
  }
}

/// Converts a hyper incoming body stream into a stream of [`Bytes`] that we can use to read in V8.
struct ReadFuture(BufferedIncoming);

impl Stream for ReadFuture {
  type Item = Result<Bytes, hyper::Error>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    // Loop until we receive a non-empty frame from Hyper
    let this = self.get_mut();
    loop {
      let res = ready!(Pin::new(&mut this.0).poll_frame(cx));
      break match res {
        Some(Ok(frame)) => {
          if let Ok(data) = frame.into_data() {
            // Ensure that we never yield an empty frame
            if !data.is_empty() {
              break Poll::Ready(Some(Ok(data)));
            }
          }
          // Loop again so we don't lose the waker
          continue;
        }
        Some(Err(e)) => Poll::Ready(Some(Err(e))),
        None => Poll::Ready(None),
      };
    }
  }
}

pub struct HttpRequestBody(AsyncRefCell<Peekable<ReadFuture>>, SizeHint);

impl HttpRequestBody {
  pub fn new(body: BufferedIncoming) -> Self {
    let size_hint = body.size_hint();
    Self(AsyncRefCell::new(ReadFuture(body).peekable()), size_hint)
  }

  async fn read(self: Rc<Self>, limit: usize) -> Result<BufView, hyper::Error> {
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
  fn name(&self) -> Cow<'_, str> {
    "requestBody".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(
      HttpRequestBody::read(self, limit)
        .map_err(|e| JsErrorBox::new("Http", e.to_string())),
    )
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (self.1.lower(), self.1.upper())
  }
}
