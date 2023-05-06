// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Waker;

use deno_core::error::bad_resource;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::WriteOutcome;
use hyper1::body::Body;
use hyper1::body::Frame;
use hyper1::body::SizeHint;

#[derive(Clone, Debug, Default)]
pub struct CompletionHandle {
  inner: Rc<RefCell<CompletionHandleInner>>,
}

#[derive(Debug, Default)]
struct CompletionHandleInner {
  complete: bool,
  success: bool,
  waker: Option<Waker>,
}

impl CompletionHandle {
  pub fn complete(&self, success: bool) {
    let mut mut_self = self.inner.borrow_mut();
    mut_self.complete = true;
    mut_self.success = success;
    if let Some(waker) = mut_self.waker.take() {
      drop(mut_self);
      waker.wake();
    }
  }
}

impl Future for CompletionHandle {
  type Output = bool;

  fn poll(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let mut mut_self = self.inner.borrow_mut();
    if mut_self.complete {
      return std::task::Poll::Ready(mut_self.success);
    }

    mut_self.waker = Some(cx.waker().clone());
    std::task::Poll::Pending
  }
}

trait PollFrame: Unpin {
  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<BufView>, AnyError>>>;

  fn size_hint(&self) -> SizeHint;
}

pub enum Compression {
  None,
  GZip,
  Deflate,
  Brotli,
}

pub enum ResponseStream {
  /// A resource stream, piped in fast mode.
  Resource(ResourceBodyAdapter),
  /// A JS-backed stream, written in JS and transported via pipe.
  V8Stream(tokio::sync::mpsc::Receiver<BufView>),
}

#[derive(Default)]
pub enum ResponseBytesInner {
  /// An empty stream.
  #[default]
  Empty,
  /// A completed stream.
  Done,
  /// A static buffer of bytes, sent in one fell swoop.
  Bytes(BufView),
  /// An uncompressed stream.
  UncompressedStream(ResponseStream),
  /// A GZip stream.
  GZipStream(GZipResponseStream),
}

impl std::fmt::Debug for ResponseBytesInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Done => f.write_str("Done"),
      Self::Empty => f.write_str("Empty"),
      Self::Bytes(..) => f.write_str("Bytes"),
      Self::UncompressedStream(..) => f.write_str("Uncompressed"),
      Self::GZipStream(..) => f.write_str("GZip"),
    }
  }
}

/// This represents the union of possible response types in Deno with the stream-style [`Body`] interface
/// required by hyper. As the API requires information about request completion (including a success/fail
/// flag), we include a very lightweight [`CompletionHandle`] for interested parties to listen on.
#[derive(Debug, Default)]
pub struct ResponseBytes(ResponseBytesInner, CompletionHandle);

impl ResponseBytes {
  pub fn initialize(&mut self, inner: ResponseBytesInner) {
    debug_assert!(matches!(self.0, ResponseBytesInner::Empty));
    self.0 = inner;
  }

  pub fn completion_handle(&self) -> CompletionHandle {
    self.1.clone()
  }

  fn complete(&mut self, success: bool) -> ResponseBytesInner {
    if matches!(self.0, ResponseBytesInner::Done) {
      return ResponseBytesInner::Done;
    }

    let current = std::mem::replace(&mut self.0, ResponseBytesInner::Done);
    self.1.complete(success);
    current
  }
}

impl ResponseBytesInner {
  pub fn size_hint(&self) -> SizeHint {
    match self {
      Self::Done => SizeHint::with_exact(0),
      Self::Empty => SizeHint::with_exact(0),
      Self::Bytes(bytes) => SizeHint::with_exact(bytes.len() as u64),
      Self::UncompressedStream(res) => res.size_hint(),
      // Self::GZipStream(res) => SizeHint::
    }
  }

  pub fn from_v8(compression: Compression, rx: tokio::sync::mpsc::Receiver<BufView>) -> Self {
    Self::UncompressedStream(ResponseStream::V8Stream(rx))
  }

  pub fn from_resource(compression: Compression, stm: Rc<dyn Resource>, auto_close: bool) -> Self {
    Self::UncompressedStream(ResponseStream::Resource(
      ResourceBodyAdapter::new(stm, auto_close),
    ))
  }

  pub fn from_slice(compression: Compression, bytes: &[u8]) -> Self {
    Self::Bytes(BufView::from(bytes.to_vec()))
  }

  pub fn from_vec(compression: Compression, vec: Vec<u8>) -> Self {
    Self::Bytes(BufView::from(vec))
  }
}

impl Body for ResponseBytes {
  type Data = BufView;
  type Error = AnyError;

  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    match &mut self.0 {
      ResponseBytesInner::Done | ResponseBytesInner::Empty => {
        unreachable!()
      }
      ResponseBytesInner::Bytes(..) => {
        if let ResponseBytesInner::Bytes(data) = self.complete(true) {
          std::task::Poll::Ready(Some(Ok(Frame::data(data))))
        } else {
          unreachable!()
        }
      }
      ResponseBytesInner::UncompressedStream(stm) => {
        match Pin::new(stm).poll_frame(cx) {
          x @ std::task::Poll::Ready(None) => {
            self.complete(true);
            x
          }
          x @ _ => x,
        }
      },
      ResponseBytesInner::GZipStream(stm) => {

      }
    }
  }

  fn is_end_stream(&self) -> bool {
    matches!(self.0, ResponseBytesInner::Done | ResponseBytesInner::Empty)
  }

  fn size_hint(&self) -> SizeHint {
    // The size hint currently only used in the case where it is exact bounds in hyper, but we'll pass it through
    // anyways just in case hyper needs it.
    self.0.size_hint()
  }
}

impl Drop for ResponseBytes {
  fn drop(&mut self) {
    // We won't actually poll_frame for Empty responses so this is where we return success
    self.complete(matches!(self.0, ResponseBytesInner::Empty));
  }
}

pub struct ResourceBodyAdapter {
  auto_close: bool,
  stm: Rc<dyn Resource>,
  future: AsyncResult<BufView>,
}

impl ResourceBodyAdapter {
  pub fn new(stm: Rc<dyn Resource>, auto_close: bool) -> Self {
    let future = stm.clone().read(64 * 1024);
    ResourceBodyAdapter {
      auto_close,
      stm,
      future,
    }
  }
}

impl PollFrame for ResponseStream {
  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<BufView>, AnyError>>> {
    match &mut *self {
      ResponseStream::Resource(res) => Pin::new(res).poll_frame(cx),
      ResponseStream::V8Stream(res) => Pin::new(res).poll_frame(cx),
    }
  }

  fn size_hint(&self) -> SizeHint {
    match self {
      ResponseStream::Resource(res) => res.size_hint(),
      ResponseStream::V8Stream(res) => res.size_hint(),
    }
  }
}

impl PollFrame for ResourceBodyAdapter {
  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<BufView>, AnyError>>> {
    match self.future.poll_unpin(cx) {
      std::task::Poll::Pending => std::task::Poll::Pending,
      std::task::Poll::Ready(Err(err)) => {
        std::task::Poll::Ready(Some(Err(err)))
      }
      std::task::Poll::Ready(Ok(buf)) => {
        if buf.is_empty() {
          if self.auto_close {
            self.stm.clone().close();
          }
          return std::task::Poll::Ready(None);
        }
        // Re-arm the future
        self.future = self.stm.clone().read(64 * 1024);
        std::task::Poll::Ready(Some(Ok(Frame::data(buf))))
      }
    }
  }

  fn size_hint(&self) -> SizeHint {
    let hint = self.stm.size_hint();
    let mut size_hint = SizeHint::new();
    size_hint.set_lower(hint.0);
    if let Some(upper) = hint.1 {
      size_hint.set_upper(upper)
    }
    size_hint
  }
}

impl PollFrame for tokio::sync::mpsc::Receiver<BufView> {
  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<BufView>, AnyError>>> {
    match self.poll_recv(cx) {
      std::task::Poll::Pending => std::task::Poll::Pending,
      std::task::Poll::Ready(Some(buf)) => {
        std::task::Poll::Ready(Some(Ok(Frame::data(buf))))
      }
      std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
    }
  }

  fn size_hint(&self) -> SizeHint {
    SizeHint::default()
  }
}

struct GZipResponseStream {
  stm: async_compression::tokio::bufread::GzipEncoder<ResponseStream>,
}

/// A response body object that can be passed to V8. This body will feed byte buffers to a channel which
/// feed's hyper's HTTP response.
pub struct V8StreamHttpResponseBody(
  AsyncRefCell<Option<tokio::sync::mpsc::Sender<BufView>>>,
  CancelHandle,
);

impl V8StreamHttpResponseBody {
  pub fn new(sender: tokio::sync::mpsc::Sender<BufView>) -> Self {
    Self(AsyncRefCell::new(Some(sender)), CancelHandle::default())
  }
}

impl Resource for V8StreamHttpResponseBody {
  fn name(&self) -> Cow<str> {
    "responseBody".into()
  }

  fn write(
    self: Rc<Self>,
    buf: BufView,
  ) -> AsyncResult<deno_core::WriteOutcome> {
    let cancel_handle = RcRef::map(&self, |this| &this.1);
    Box::pin(
      async move {
        let nwritten = buf.len();

        let res = RcRef::map(self, |this| &this.0).borrow().await;
        if let Some(tx) = res.as_ref() {
          tx.send(buf)
            .await
            .map_err(|_| bad_resource("failed to write"))?;
          Ok(WriteOutcome::Full { nwritten })
        } else {
          Err(bad_resource("failed to write"))
        }
      }
      .try_or_cancel(cancel_handle),
    )
  }

  fn close(self: Rc<Self>) {
    self.1.cancel();
  }
}
