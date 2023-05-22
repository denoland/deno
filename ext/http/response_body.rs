// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Waker;

use bytes::Bytes;
use bytes::BytesMut;
use deno_core::error::bad_resource;
use deno_core::error::AnyError;
use deno_core::futures::ready;
use deno_core::futures::FutureExt;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::WriteOutcome;
use flate2::write::GzEncoder;
use http::HeaderMap;
use hyper1::body::Body;
use hyper1::body::Frame;
use hyper1::body::SizeHint;
use pin_project::pin_project;

/// Simplification for nested types we use for our streams. We provide a way to convert from
/// this type into Hyper's body [`Frame`].
enum ResponseStreamResult {
  /// Stream is over.
  EndOfStream,
  /// Stream provided non-empty data.
  NonEmptyBuf(BufView),
  /// Stream is ready, but provided no data. Retry. This is a result that is like Pending, but does
  /// not register a waker and should be called again at the lowest level of this code. Generally this
  /// will only be returned from compression streams that require additional buffering.
  NoData,
  /// Stream provided trailers.
  // TODO(mmastrac): We are threading trailers through the response system to eventually support Grpc.
  #[allow(unused)]
  Trailers(HeaderMap),
  /// Stream failed.
  Error(AnyError),
}

impl From<ResponseStreamResult> for Option<Result<Frame<BufView>, AnyError>> {
  fn from(value: ResponseStreamResult) -> Self {
    match value {
      ResponseStreamResult::EndOfStream => None,
      ResponseStreamResult::NonEmptyBuf(buf) => Some(Ok(Frame::data(buf))),
      ResponseStreamResult::Error(err) => Some(Err(err)),
      ResponseStreamResult::Trailers(map) => Some(Ok(Frame::trailers(map))),
      // This result should be handled by retrying
      ResponseStreamResult::NoData => unimplemented!(),
    }
  }
}

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

  pub fn is_completed(&self) -> bool {
    self.inner.borrow().complete
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
  ) -> std::task::Poll<ResponseStreamResult>;

  fn size_hint(&self) -> SizeHint;
}

#[derive(PartialEq, Eq)]
pub enum Compression {
  None,
  GZip,
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
  /// A Brotli stream.
  BrotliStream(BrotliResponseStream),
}

impl std::fmt::Debug for ResponseBytesInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Done => f.write_str("Done"),
      Self::Empty => f.write_str("Empty"),
      Self::Bytes(..) => f.write_str("Bytes"),
      Self::UncompressedStream(..) => f.write_str("Uncompressed"),
      Self::GZipStream(..) => f.write_str("GZip"),
      Self::BrotliStream(..) => f.write_str("Brotli"),
    }
  }
}

/// This represents the union of possible response types in Deno with the stream-style [`Body`] interface
/// required by hyper. As the API requires information about request completion (including a success/fail
/// flag), we include a very lightweight [`CompletionHandle`] for interested parties to listen on.
#[derive(Debug, Default)]
pub struct ResponseBytes(
  ResponseBytesInner,
  CompletionHandle,
  Rc<RefCell<Option<HeaderMap>>>,
);

impl ResponseBytes {
  pub fn initialize(&mut self, inner: ResponseBytesInner) {
    debug_assert!(matches!(self.0, ResponseBytesInner::Empty));
    self.0 = inner;
  }

  pub fn completion_handle(&self) -> CompletionHandle {
    self.1.clone()
  }

  pub fn trailers(&self) -> Rc<RefCell<Option<HeaderMap>>> {
    self.2.clone()
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
      Self::GZipStream(..) => SizeHint::default(),
      Self::BrotliStream(..) => SizeHint::default(),
    }
  }

  fn from_stream(compression: Compression, stream: ResponseStream) -> Self {
    match compression {
      Compression::GZip => Self::GZipStream(GZipResponseStream::new(stream)),
      Compression::Brotli => {
        Self::BrotliStream(BrotliResponseStream::new(stream))
      }
      _ => Self::UncompressedStream(stream),
    }
  }

  pub fn from_v8(
    compression: Compression,
    rx: tokio::sync::mpsc::Receiver<BufView>,
  ) -> Self {
    Self::from_stream(compression, ResponseStream::V8Stream(rx))
  }

  pub fn from_resource(
    compression: Compression,
    stm: Rc<dyn Resource>,
    auto_close: bool,
  ) -> Self {
    Self::from_stream(
      compression,
      ResponseStream::Resource(ResourceBodyAdapter::new(stm, auto_close)),
    )
  }

  pub fn from_slice(compression: Compression, bytes: &[u8]) -> Self {
    match compression {
      Compression::GZip => {
        let mut writer =
          GzEncoder::new(Vec::new(), flate2::Compression::fast());
        writer.write_all(bytes).unwrap();
        Self::Bytes(BufView::from(writer.finish().unwrap()))
      }
      Compression::Brotli => {
        // quality level 6 is based on google's nginx default value for
        // on-the-fly compression
        // https://github.com/google/ngx_brotli#brotli_comp_level
        // lgwin 22 is equivalent to brotli window size of (2**22)-16 bytes
        // (~4MB)
        let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 6, 22);
        writer.write_all(bytes).unwrap();
        Self::Bytes(BufView::from(writer.into_inner()))
      }
      _ => Self::Bytes(BufView::from(bytes.to_vec())),
    }
  }

  pub fn from_vec(compression: Compression, vec: Vec<u8>) -> Self {
    match compression {
      Compression::GZip => {
        let mut writer =
          GzEncoder::new(Vec::new(), flate2::Compression::fast());
        writer.write_all(&vec).unwrap();
        Self::Bytes(BufView::from(writer.finish().unwrap()))
      }
      Compression::Brotli => {
        let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 6, 22);
        writer.write_all(&vec).unwrap();
        Self::Bytes(BufView::from(writer.into_inner()))
      }
      _ => Self::Bytes(BufView::from(vec)),
    }
  }
}

impl Body for ResponseBytes {
  type Data = BufView;
  type Error = AnyError;

  fn poll_frame(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
    let res = loop {
      let res = match &mut self.0 {
        ResponseBytesInner::Done | ResponseBytesInner::Empty => {
          if let Some(trailers) = self.2.borrow_mut().take() {
            return std::task::Poll::Ready(Some(Ok(Frame::trailers(trailers))));
          }
          unreachable!()
        }
        ResponseBytesInner::Bytes(..) => {
          let ResponseBytesInner::Bytes(data) = self.complete(true) else { unreachable!(); };
          return std::task::Poll::Ready(Some(Ok(Frame::data(data))));
        }
        ResponseBytesInner::UncompressedStream(stm) => {
          ready!(Pin::new(stm).poll_frame(cx))
        }
        ResponseBytesInner::GZipStream(stm) => {
          ready!(Pin::new(stm).poll_frame(cx))
        }
        ResponseBytesInner::BrotliStream(stm) => {
          ready!(Pin::new(stm).poll_frame(cx))
        }
      };
      // This is where we retry the NoData response
      if matches!(res, ResponseStreamResult::NoData) {
        continue;
      }
      break res;
    };

    if matches!(res, ResponseStreamResult::EndOfStream) {
      if let Some(trailers) = self.2.borrow_mut().take() {
        return std::task::Poll::Ready(Some(Ok(Frame::trailers(trailers))));
      }
      self.complete(true);
    }
    std::task::Poll::Ready(res.into())
  }

  fn is_end_stream(&self) -> bool {
    matches!(self.0, ResponseBytesInner::Done | ResponseBytesInner::Empty)
      && self.2.borrow_mut().is_none()
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
  ) -> std::task::Poll<ResponseStreamResult> {
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
  ) -> std::task::Poll<ResponseStreamResult> {
    let res = match ready!(self.future.poll_unpin(cx)) {
      Err(err) => ResponseStreamResult::Error(err),
      Ok(buf) => {
        if buf.is_empty() {
          if self.auto_close {
            self.stm.clone().close();
          }
          ResponseStreamResult::EndOfStream
        } else {
          // Re-arm the future
          self.future = self.stm.clone().read(64 * 1024);
          ResponseStreamResult::NonEmptyBuf(buf)
        }
      }
    };
    std::task::Poll::Ready(res)
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
  ) -> std::task::Poll<ResponseStreamResult> {
    let res = match ready!(self.poll_recv(cx)) {
      Some(buf) => ResponseStreamResult::NonEmptyBuf(buf),
      None => ResponseStreamResult::EndOfStream,
    };
    std::task::Poll::Ready(res)
  }

  fn size_hint(&self) -> SizeHint {
    SizeHint::default()
  }
}

#[derive(Copy, Clone, Debug)]
enum GZipState {
  Header,
  Streaming,
  Flushing,
  Trailer,
  EndOfStream,
}

#[pin_project]
pub struct GZipResponseStream {
  stm: flate2::Compress,
  crc: flate2::Crc,
  next_buf: Option<BytesMut>,
  partial: Option<BufView>,
  #[pin]
  underlying: ResponseStream,
  state: GZipState,
}

impl GZipResponseStream {
  pub fn new(underlying: ResponseStream) -> Self {
    Self {
      stm: flate2::Compress::new(flate2::Compression::fast(), false),
      crc: flate2::Crc::new(),
      next_buf: None,
      partial: None,
      state: GZipState::Header,
      underlying,
    }
  }
}

#[derive(Copy, Clone, Debug)]
enum BrotliState {
  Streaming,
}

#[pin_project]
pub struct BrotliResponseStream {
  state: BrotliState,
  #[pin]
  underlying: ResponseStream,
}

impl BrotliResponseStream {
  pub fn new(underlying: ResponseStream) -> Self {
    Self {
      state: BrotliState::Streaming,
      underlying,
    }
  }
}

impl PollFrame for BrotliResponseStream {
  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<ResponseStreamResult> {
    let this = self.get_mut();
    let state = &mut this.state;
    let frame = match *state {
      BrotliState::Streaming => {
        ready!(Pin::new(&mut this.underlying).poll_frame(cx))
      }
    };

    let res = match frame {
      ResponseStreamResult::NonEmptyBuf(buf) => {
        let mut writer = brotli::CompressorWriter::new(
          Vec::new(),
          4096, /* buffer size */
          6,
          22,
        );
        writer.write_all(buf.as_ref()).unwrap();

        ResponseStreamResult::NonEmptyBuf(BufView::from(writer.into_inner()))
      }
      _ => frame,
    };

    std::task::Poll::Ready(res)
  }

  fn size_hint(&self) -> SizeHint {
    SizeHint::default()
  }
}

/// This is a minimal GZip header suitable for serving data from a webserver. We don't need to provide
/// most of the information. We're skipping header name, CRC, etc, and providing a null timestamp.
///
/// We're using compression level 1, as higher levels don't produce significant size differences. This
/// is probably the reason why nginx's default gzip compression level is also 1:
///
/// https://nginx.org/en/docs/http/ngx_http_gzip_module.html#gzip_comp_level
static GZIP_HEADER: Bytes =
  Bytes::from_static(&[0x1f, 0x8b, 0x08, 0, 0, 0, 0, 0, 0x01, 0xff]);

impl PollFrame for GZipResponseStream {
  fn poll_frame(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<ResponseStreamResult> {
    let this = self.get_mut();
    let state = &mut this.state;
    let orig_state = *state;
    let frame = match *state {
      GZipState::EndOfStream => {
        return std::task::Poll::Ready(ResponseStreamResult::EndOfStream)
      }
      GZipState::Header => {
        *state = GZipState::Streaming;
        return std::task::Poll::Ready(ResponseStreamResult::NonEmptyBuf(
          BufView::from(GZIP_HEADER.clone()),
        ));
      }
      GZipState::Trailer => {
        *state = GZipState::EndOfStream;
        let mut v = Vec::with_capacity(8);
        v.extend(&this.crc.sum().to_le_bytes());
        v.extend(&this.crc.amount().to_le_bytes());
        return std::task::Poll::Ready(ResponseStreamResult::NonEmptyBuf(
          BufView::from(v),
        ));
      }
      GZipState::Streaming => {
        if let Some(partial) = this.partial.take() {
          ResponseStreamResult::NonEmptyBuf(partial)
        } else {
          ready!(Pin::new(&mut this.underlying).poll_frame(cx))
        }
      }
      GZipState::Flushing => ResponseStreamResult::EndOfStream,
    };

    let stm = &mut this.stm;

    // Ideally we could use MaybeUninit here, but flate2 requires &[u8]. We should also try
    // to dynamically adjust this buffer.
    let mut buf = this
      .next_buf
      .take()
      .unwrap_or_else(|| BytesMut::zeroed(64 * 1024));

    let start_in = stm.total_in();
    let start_out = stm.total_out();
    let res = match frame {
      // Short-circuit these and just return
      x @ (ResponseStreamResult::NoData
      | ResponseStreamResult::Error(..)
      | ResponseStreamResult::Trailers(..)) => {
        return std::task::Poll::Ready(x)
      }
      ResponseStreamResult::EndOfStream => {
        *state = GZipState::Flushing;
        stm.compress(&[], &mut buf, flate2::FlushCompress::Finish)
      }
      ResponseStreamResult::NonEmptyBuf(mut input) => {
        let res = stm.compress(&input, &mut buf, flate2::FlushCompress::None);
        let len_in = (stm.total_in() - start_in) as usize;
        debug_assert!(len_in <= input.len());
        this.crc.update(&input[..len_in]);
        if len_in < input.len() {
          input.advance_cursor(len_in);
          this.partial = Some(input);
        }
        res
      }
    };
    let len = stm.total_out() - start_out;
    let res = match res {
      Err(err) => ResponseStreamResult::Error(err.into()),
      Ok(flate2::Status::BufError) => {
        // This should not happen
        unreachable!("old={orig_state:?} new={state:?} buf_len={}", buf.len());
      }
      Ok(flate2::Status::Ok) => {
        if len == 0 {
          this.next_buf = Some(buf);
          ResponseStreamResult::NoData
        } else {
          buf.truncate(len as usize);
          ResponseStreamResult::NonEmptyBuf(BufView::from(buf.freeze()))
        }
      }
      Ok(flate2::Status::StreamEnd) => {
        *state = GZipState::Trailer;
        if len == 0 {
          this.next_buf = Some(buf);
          ResponseStreamResult::NoData
        } else {
          buf.truncate(len as usize);
          ResponseStreamResult::NonEmptyBuf(BufView::from(buf.freeze()))
        }
      }
    };

    std::task::Poll::Ready(res)
  }

  fn size_hint(&self) -> SizeHint {
    SizeHint::default()
  }
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

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::futures::future::poll_fn;
  use std::hash::Hasher;
  use std::io::Read;
  use std::io::Write;

  fn zeros() -> Vec<u8> {
    vec![0; 1024 * 1024]
  }

  fn hard_to_gzip_data() -> Vec<u8> {
    const SIZE: usize = 1024 * 1024;
    let mut v = Vec::with_capacity(SIZE);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for i in 0..SIZE {
      hasher.write_usize(i);
      v.push(hasher.finish() as u8);
    }
    v
  }

  fn already_gzipped_data() -> Vec<u8> {
    let mut v = Vec::with_capacity(1024 * 1024);
    let mut gz =
      flate2::GzBuilder::new().write(&mut v, flate2::Compression::best());
    gz.write_all(&hard_to_gzip_data()).unwrap();
    _ = gz.finish().unwrap();
    v
  }

  fn chunk(v: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
    // Chunk the data into 10k
    let mut out = vec![];
    for v in v.chunks(10 * 1024) {
      out.push(v.to_vec());
    }
    out.into_iter()
  }

  fn random(mut v: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
    let mut out = vec![];
    loop {
      if v.is_empty() {
        break;
      }
      let rand = (rand::random::<usize>() % v.len()) + 1;
      let new = v.split_off(rand);
      out.push(v);
      v = new;
    }
    // Print the lengths of the vectors if we actually fail this test at some point
    let lengths = out.iter().map(|v| v.len()).collect::<Vec<_>>();
    eprintln!("Lengths = {:?}", lengths);
    out.into_iter()
  }

  fn front_load(mut v: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
    // Chunk the data at 90%
    let offset = (v.len() * 90) / 100;
    let v2 = v.split_off(offset);
    vec![v, v2].into_iter()
  }

  fn front_load_but_one(mut v: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
    let offset = v.len() - 1;
    let v2 = v.split_off(offset);
    vec![v, v2].into_iter()
  }

  fn back_load(mut v: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
    // Chunk the data at 10%
    let offset = (v.len() * 10) / 100;
    let v2 = v.split_off(offset);
    vec![v, v2].into_iter()
  }

  async fn test(i: impl Iterator<Item = Vec<u8>> + Send + 'static) {
    let v = i.collect::<Vec<_>>();
    let mut expected: Vec<u8> = vec![];
    for v in &v {
      expected.extend(v);
    }
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let underlying = ResponseStream::V8Stream(rx);
    let mut resp = GZipResponseStream::new(underlying);
    let handle = tokio::task::spawn(async move {
      for chunk in v {
        tx.send(chunk.into()).await.ok().unwrap();
      }
    });
    // Limit how many times we'll loop
    const LIMIT: usize = 1000;
    let mut v: Vec<u8> = vec![];
    for i in 0..=LIMIT {
      assert_ne!(i, LIMIT);
      let frame = poll_fn(|cx| Pin::new(&mut resp).poll_frame(cx)).await;
      if matches!(frame, ResponseStreamResult::EndOfStream) {
        break;
      }
      if matches!(frame, ResponseStreamResult::NoData) {
        continue;
      }
      let ResponseStreamResult::NonEmptyBuf(buf) = frame else {
        panic!("Unexpected stream type");
      };
      assert_ne!(buf.len(), 0);
      v.extend(&*buf);
    }

    let mut gz = flate2::read::GzDecoder::new(&*v);
    let mut v = vec![];
    gz.read_to_end(&mut v).unwrap();

    assert_eq!(v, expected);

    handle.await.unwrap();
  }

  #[tokio::test]
  async fn test_simple() {
    test(vec![b"hello world".to_vec()].into_iter()).await
  }

  #[tokio::test]
  async fn test_empty() {
    test(vec![].into_iter()).await
  }

  #[tokio::test]
  async fn test_simple_zeros() {
    test(vec![vec![0; 0x10000]].into_iter()).await
  }

  macro_rules! test {
    ($vec:ident) => {
      mod $vec {
        #[tokio::test]
        async fn chunk() {
          let iter = super::chunk(super::$vec());
          super::test(iter).await;
        }

        #[tokio::test]
        async fn front_load() {
          let iter = super::front_load(super::$vec());
          super::test(iter).await;
        }

        #[tokio::test]
        async fn front_load_but_one() {
          let iter = super::front_load_but_one(super::$vec());
          super::test(iter).await;
        }

        #[tokio::test]
        async fn back_load() {
          let iter = super::back_load(super::$vec());
          super::test(iter).await;
        }

        #[tokio::test]
        async fn random() {
          let iter = super::random(super::$vec());
          super::test(iter).await;
        }
      }
    };
  }

  test!(zeros);
  test!(hard_to_gzip_data);
  test!(already_gzipped_data);
}
