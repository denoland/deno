// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::cell::RefMut;
use std::ffi::c_void;
use std::future::poll_fn;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use bytes::BytesMut;
use deno_core::external;
use deno_core::op2;
use deno_core::serde_v8::V8Slice;
use deno_core::unsync::TaskQueue;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::ExternalPointer;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcLike;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use futures::TryFutureExt;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum StreamResourceError {
  #[class(inherit)]
  #[error(transparent)]
  Canceled(#[from] deno_core::Canceled),
  #[class(type)]
  #[error("{0}")]
  Js(String),
}

// How many buffers we'll allow in the channel before we stop allowing writes.
const BUFFER_CHANNEL_SIZE: u16 = 1024;

// How much data is in the channel before we stop allowing writes.
const BUFFER_BACKPRESSURE_LIMIT: usize = 64 * 1024;

// Optimization: prevent multiple small writes from adding overhead.
//
// If the total size of the channel is less than this value and there is more than one buffer available
// to read, we will allocate a buffer to store the entire contents of the channel and copy each value from
// the channel rather than yielding them one at a time.
const BUFFER_AGGREGATION_LIMIT: usize = 1024;

struct BoundedBufferChannelInner {
  buffers: [MaybeUninit<V8Slice<u8>>; BUFFER_CHANNEL_SIZE as _],
  ring_producer: u16,
  ring_consumer: u16,
  error: Option<StreamResourceError>,
  current_size: usize,
  // TODO(mmastrac): we can math this field instead of accounting for it
  len: usize,
  closed: bool,

  read_waker: Option<Waker>,
  write_waker: Option<Waker>,

  _unsend: PhantomData<std::sync::MutexGuard<'static, ()>>,
}

impl Default for BoundedBufferChannelInner {
  fn default() -> Self {
    Self::new()
  }
}

impl Drop for BoundedBufferChannelInner {
  fn drop(&mut self) {
    // If any buffers remain in the ring, drop them here
    self.drain(std::mem::drop);
  }
}

impl std::fmt::Debug for BoundedBufferChannelInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "[BoundedBufferChannel closed={} error={:?} ring={}->{} len={} size={}]",
      self.closed,
      self.error,
      self.ring_producer,
      self.ring_consumer,
      self.len,
      self.current_size
    ))
  }
}

impl BoundedBufferChannelInner {
  pub fn new() -> Self {
    const UNINIT: MaybeUninit<V8Slice<u8>> = MaybeUninit::uninit();
    Self {
      buffers: [UNINIT; BUFFER_CHANNEL_SIZE as _],
      ring_producer: 0,
      ring_consumer: 0,
      len: 0,
      closed: false,
      error: None,
      current_size: 0,
      read_waker: None,
      write_waker: None,
      _unsend: PhantomData,
    }
  }

  /// # Safety
  ///
  /// This doesn't check whether `ring_consumer` is valid, so you'd better make sure it is before
  /// calling this.
  #[inline(always)]
  unsafe fn next_unsafe(&mut self) -> &mut V8Slice<u8> {
    self
      .buffers
      .get_unchecked_mut(self.ring_consumer as usize)
      .assume_init_mut()
  }

  /// # Safety
  ///
  /// This doesn't check whether `ring_consumer` is valid, so you'd better make sure it is before
  /// calling this.
  #[inline(always)]
  unsafe fn take_next_unsafe(&mut self) -> V8Slice<u8> {
    let res = std::ptr::read(self.next_unsafe());
    self.ring_consumer = (self.ring_consumer + 1) % BUFFER_CHANNEL_SIZE;

    res
  }

  fn drain(&mut self, mut f: impl FnMut(V8Slice<u8>)) {
    while self.ring_producer != self.ring_consumer {
      // SAFETY: We know the ring indexes are valid
      let res = unsafe { std::ptr::read(self.next_unsafe()) };
      self.ring_consumer = (self.ring_consumer + 1) % BUFFER_CHANNEL_SIZE;
      f(res);
    }
    self.current_size = 0;
    self.ring_producer = 0;
    self.ring_consumer = 0;
    self.len = 0;
  }

  pub fn read(
    &mut self,
    limit: usize,
  ) -> Result<Option<BufView>, StreamResourceError> {
    // Empty buffers will return the error, if one exists, or None
    if self.len == 0 {
      if let Some(error) = self.error.take() {
        return Err(error);
      } else {
        return Ok(None);
      }
    }

    // If we have less than the aggregation limit AND we have more than one buffer in the channel,
    // aggregate and return everything in a single buffer.
    if limit >= BUFFER_AGGREGATION_LIMIT
      && self.current_size <= BUFFER_AGGREGATION_LIMIT
      && self.len > 1
    {
      let mut bytes = BytesMut::with_capacity(BUFFER_AGGREGATION_LIMIT);
      self.drain(|slice| {
        bytes.extend_from_slice(slice.as_ref());
      });

      // We can always write again
      if let Some(waker) = self.write_waker.take() {
        waker.wake();
      }

      return Ok(Some(BufView::from(bytes.freeze())));
    }

    // SAFETY: We know this exists
    let buf = unsafe { self.next_unsafe() };
    let buf = if buf.len() <= limit {
      self.current_size -= buf.len();
      self.len -= 1;
      // SAFETY: We know this exists
      unsafe { self.take_next_unsafe() }
    } else {
      let buf = buf.split_to(limit);
      self.current_size -= limit;
      buf
    };

    // If current_size is zero, len must be zero (and if not, len must not be)
    debug_assert!(
      !((self.current_size == 0) ^ (self.len == 0)),
      "Length accounting mismatch: {self:?}"
    );

    if self.write_waker.is_some() {
      // We may be able to write again if we have buffer and byte room in the channel
      if self.can_write() {
        if let Some(waker) = self.write_waker.take() {
          waker.wake();
        }
      }
    }

    Ok(Some(BufView::from(JsBuffer::from_parts(buf))))
  }

  pub fn write(&mut self, buffer: V8Slice<u8>) -> Result<(), V8Slice<u8>> {
    let next_producer_index = (self.ring_producer + 1) % BUFFER_CHANNEL_SIZE;
    if next_producer_index == self.ring_consumer {
      // Note that we may have been allowed to write because of a close/error condition, but the
      // underlying channel is actually closed. If this is the case, we return `Ok(())`` and just
      // drop the bytes on the floor.
      return if self.closed || self.error.is_some() {
        Ok(())
      } else {
        Err(buffer)
      };
    }

    self.current_size += buffer.len();

    // SAFETY: we know the ringbuffer bounds are correct
    unsafe {
      *self.buffers.get_unchecked_mut(self.ring_producer as usize) =
        MaybeUninit::new(buffer)
    };
    self.ring_producer = next_producer_index;
    self.len += 1;
    debug_assert!(self.ring_producer != self.ring_consumer);
    if let Some(waker) = self.read_waker.take() {
      waker.wake();
    }
    Ok(())
  }

  pub fn write_error(&mut self, error: StreamResourceError) {
    self.error = Some(error);
    if let Some(waker) = self.read_waker.take() {
      waker.wake();
    }
  }

  #[inline(always)]
  pub fn can_read(&self) -> bool {
    // Read will return if:
    //  - the stream is closed
    //  - there is an error
    //  - the stream is not empty
    self.closed
      || self.error.is_some()
      || self.ring_consumer != self.ring_producer
  }

  #[inline(always)]
  pub fn can_write(&self) -> bool {
    // Write will return if:
    //  - the stream is closed
    //  - there is an error
    //  - the stream is not full (either buffer or byte count)
    let next_producer_index = (self.ring_producer + 1) % BUFFER_CHANNEL_SIZE;
    self.closed
      || self.error.is_some()
      || (next_producer_index != self.ring_consumer
        && self.current_size < BUFFER_BACKPRESSURE_LIMIT)
  }

  pub fn poll_read_ready(&mut self, cx: &mut Context) -> Poll<()> {
    if !self.can_read() {
      self.read_waker = Some(cx.waker().clone());
      Poll::Pending
    } else {
      self.read_waker.take();
      Poll::Ready(())
    }
  }

  pub fn poll_write_ready(&mut self, cx: &mut Context) -> Poll<()> {
    if !self.can_write() {
      self.write_waker = Some(cx.waker().clone());
      Poll::Pending
    } else {
      self.write_waker.take();
      Poll::Ready(())
    }
  }

  pub fn close(&mut self) {
    self.closed = true;
    // Wake up reads and writes, since they'll both be able to proceed forever now
    if let Some(waker) = self.write_waker.take() {
      waker.wake();
    }
    if let Some(waker) = self.read_waker.take() {
      waker.wake();
    }
  }
}

#[repr(transparent)]
#[derive(Clone, Default)]
struct BoundedBufferChannel {
  inner: Rc<RefCell<BoundedBufferChannelInner>>,
}

impl BoundedBufferChannel {
  // TODO(mmastrac): in release mode we should be able to make this an UnsafeCell
  #[inline(always)]
  fn inner(&self) -> RefMut<BoundedBufferChannelInner> {
    self.inner.borrow_mut()
  }

  pub fn read(
    &self,
    limit: usize,
  ) -> Result<Option<BufView>, StreamResourceError> {
    self.inner().read(limit)
  }

  pub fn write(&self, buffer: V8Slice<u8>) -> Result<(), V8Slice<u8>> {
    self.inner().write(buffer)
  }

  pub fn write_error(&self, error: StreamResourceError) {
    self.inner().write_error(error)
  }

  pub fn can_write(&self) -> bool {
    self.inner().can_write()
  }

  pub fn poll_read_ready(&self, cx: &mut Context) -> Poll<()> {
    self.inner().poll_read_ready(cx)
  }

  pub fn poll_write_ready(&self, cx: &mut Context) -> Poll<()> {
    self.inner().poll_write_ready(cx)
  }

  pub fn closed(&self) -> bool {
    self.inner().closed
  }

  #[cfg(test)]
  pub fn byte_size(&self) -> usize {
    self.inner().current_size
  }

  pub fn close(&self) {
    self.inner().close()
  }
}

#[allow(clippy::type_complexity)]
struct ReadableStreamResource {
  read_queue: Rc<TaskQueue>,
  channel: BoundedBufferChannel,
  cancel_handle: CancelHandle,
  data: ReadableStreamResourceData,
  size_hint: (u64, Option<u64>),
}

impl ReadableStreamResource {
  pub fn cancel_handle(self: &Rc<Self>) -> impl RcLike<CancelHandle> {
    RcRef::map(self, |s| &s.cancel_handle).clone()
  }

  async fn read(
    self: Rc<Self>,
    limit: usize,
  ) -> Result<BufView, StreamResourceError> {
    let cancel_handle = self.cancel_handle();
    // Serialize all the reads using a task queue.
    let _read_permit = self.read_queue.acquire().await;
    poll_fn(|cx| self.channel.poll_read_ready(cx))
      .or_cancel(cancel_handle)
      .await?;
    self
      .channel
      .read(limit)
      .map(|buf| buf.unwrap_or_else(BufView::empty))
  }

  fn close_channel(&self) {
    // Trigger the promise in JS to cancel the stream if necessarily
    self.data.completion.complete(true);
    // Cancel any outstanding read requests
    self.cancel_handle.cancel();
    // Close the channel to wake up anyone waiting
    self.channel.close();
  }
}

impl Resource for ReadableStreamResource {
  fn name(&self) -> Cow<str> {
    Cow::Borrowed("readableStream")
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(
      ReadableStreamResource::read(self, limit)
        .map_err(deno_error::JsErrorBox::from_err),
    )
  }

  fn close(self: Rc<Self>) {
    self.close_channel();
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    self.size_hint
  }
}

impl Drop for ReadableStreamResource {
  fn drop(&mut self) {
    self.close_channel();
  }
}

// TODO(mmastrac): Move this to deno_core
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

/// Allocate a resource that wraps a ReadableStream.
#[op2(fast)]
#[smi]
pub fn op_readable_stream_resource_allocate(state: &mut OpState) -> ResourceId {
  let completion = CompletionHandle::default();
  let resource = ReadableStreamResource {
    read_queue: Default::default(),
    cancel_handle: Default::default(),
    channel: BoundedBufferChannel::default(),
    data: ReadableStreamResourceData { completion },
    size_hint: (0, None),
  };
  state.resource_table.add(resource)
}

/// Allocate a resource that wraps a ReadableStream, with a size hint.
#[op2(fast)]
#[smi]
pub fn op_readable_stream_resource_allocate_sized(
  state: &mut OpState,
  #[number] length: u64,
) -> ResourceId {
  let completion = CompletionHandle::default();
  let resource = ReadableStreamResource {
    read_queue: Default::default(),
    cancel_handle: Default::default(),
    channel: BoundedBufferChannel::default(),
    data: ReadableStreamResourceData { completion },
    size_hint: (length, Some(length)),
  };
  state.resource_table.add(resource)
}

#[op2(fast)]
pub fn op_readable_stream_resource_get_sink(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> *const c_void {
  let Ok(resource) = state.resource_table.get::<ReadableStreamResource>(rid)
  else {
    return std::ptr::null();
  };
  ExternalPointer::new(resource.channel.clone()).into_raw()
}

external!(BoundedBufferChannel, "stream resource channel");

fn get_sender(sender: *const c_void) -> BoundedBufferChannel {
  // SAFETY: We know this is a valid v8::External
  unsafe {
    ExternalPointer::<BoundedBufferChannel>::from_raw(sender)
      .unsafely_deref()
      .clone()
  }
}

fn drop_sender(sender: *const c_void) {
  // SAFETY: We know this is a valid v8::External
  unsafe {
    ExternalPointer::<BoundedBufferChannel>::from_raw(sender).unsafely_take();
  }
}

#[op2(async)]
pub fn op_readable_stream_resource_write_buf(
  sender: *const c_void,
  #[buffer] buffer: JsBuffer,
) -> impl Future<Output = bool> {
  let sender = get_sender(sender);
  async move {
    poll_fn(|cx| sender.poll_write_ready(cx)).await;
    sender.write(buffer.into_parts()).unwrap();
    !sender.closed()
  }
}

/// Write to the channel synchronously, returning 0 if the channel was closed, 1 if we wrote
/// successfully, 2 if the channel was full and we need to block.
#[op2]
pub fn op_readable_stream_resource_write_sync(
  sender: *const c_void,
  #[buffer] buffer: JsBuffer,
) -> u32 {
  let sender = get_sender(sender);
  if sender.can_write() {
    if sender.closed() {
      0
    } else {
      sender.write(buffer.into_parts()).unwrap();
      1
    }
  } else {
    2
  }
}

#[op2(fast)]
pub fn op_readable_stream_resource_write_error(
  sender: *const c_void,
  #[string] error: String,
) -> bool {
  let sender = get_sender(sender);
  // We can always write an error, no polling required
  sender.write_error(StreamResourceError::Js(error));
  !sender.closed()
}

#[op2(fast)]
#[smi]
pub fn op_readable_stream_resource_close(sender: *const c_void) {
  get_sender(sender).close();
  drop_sender(sender);
}

#[op2(async)]
pub fn op_readable_stream_resource_await_close(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> impl Future<Output = ()> {
  let completion = state
    .resource_table
    .get::<ReadableStreamResource>(rid)
    .ok()
    .map(|r| r.data.completion.clone());

  async move {
    if let Some(completion) = completion {
      completion.await;
    }
  }
}

struct ReadableStreamResourceData {
  completion: CompletionHandle,
}

impl Drop for ReadableStreamResourceData {
  fn drop(&mut self) {
    self.completion.complete(true);
  }
}

#[cfg(test)]
mod tests {
  use std::cell::OnceCell;
  use std::sync::atomic::AtomicUsize;
  use std::sync::OnceLock;
  use std::time::Duration;

  use deno_core::v8;

  use super::*;

  static V8_GLOBAL: OnceLock<()> = OnceLock::new();

  thread_local! {
    static ISOLATE: OnceCell<std::sync::Mutex<v8::OwnedIsolate>> = const { OnceCell::new() };
  }

  fn with_isolate<T>(mut f: impl FnMut(&mut v8::Isolate) -> T) -> T {
    V8_GLOBAL.get_or_init(|| {
      let platform =
        v8::new_unprotected_default_platform(0, false).make_shared();
      v8::V8::initialize_platform(platform);
      v8::V8::initialize();
    });
    ISOLATE.with(|cell| {
      let mut isolate = cell
        .get_or_init(|| {
          std::sync::Mutex::new(v8::Isolate::new(Default::default()))
        })
        .try_lock()
        .unwrap();
      f(&mut isolate)
    })
  }

  fn create_buffer(byte_length: usize) -> V8Slice<u8> {
    with_isolate(|isolate| {
      let ptr = v8::ArrayBuffer::new_backing_store(isolate, byte_length);
      // SAFETY: we just made this
      unsafe { V8Slice::from_parts(ptr.into(), 0..byte_length) }
    })
  }

  #[test]
  fn test_bounded_buffer_channel() {
    let channel = BoundedBufferChannel::default();

    for _ in 0..BUFFER_CHANNEL_SIZE - 1 {
      channel.write(create_buffer(1024)).unwrap();
    }
  }

  #[tokio::test(flavor = "current_thread")]
  async fn test_multi_task() {
    let channel = BoundedBufferChannel::default();
    let channel_send = channel.clone();

    // Fast writer
    let a = deno_core::unsync::spawn(async move {
      for _ in 0..BUFFER_CHANNEL_SIZE * 2 {
        poll_fn(|cx| channel_send.poll_write_ready(cx)).await;
        channel_send
          .write(create_buffer(BUFFER_AGGREGATION_LIMIT))
          .unwrap();
      }
    });

    // Slightly slower reader
    let b = deno_core::unsync::spawn(async move {
      for _ in 0..BUFFER_CHANNEL_SIZE * 2 {
        if cfg!(windows) {
          // windows has ~15ms resolution on sleep, so just yield so
          // this test doesn't take 30 seconds to run
          tokio::task::yield_now().await;
        } else {
          tokio::time::sleep(Duration::from_millis(1)).await;
        }
        poll_fn(|cx| channel.poll_read_ready(cx)).await;
        channel.read(BUFFER_AGGREGATION_LIMIT).unwrap();
      }
    });

    a.await.unwrap();
    b.await.unwrap();
  }

  #[tokio::test(flavor = "current_thread")]
  async fn test_multi_task_small_reads() {
    let channel = BoundedBufferChannel::default();
    let channel_send = channel.clone();

    let total_send = Rc::new(AtomicUsize::new(0));
    let total_send_task = total_send.clone();
    let total_recv = Rc::new(AtomicUsize::new(0));
    let total_recv_task = total_recv.clone();

    // Fast writer
    let a = deno_core::unsync::spawn(async move {
      for _ in 0..BUFFER_CHANNEL_SIZE * 2 {
        poll_fn(|cx| channel_send.poll_write_ready(cx)).await;
        channel_send.write(create_buffer(16)).unwrap();
        total_send_task.fetch_add(16, std::sync::atomic::Ordering::SeqCst);
      }
      // We need to close because we may get aggregated packets and we want a signal
      channel_send.close();
    });

    // Slightly slower reader
    let b = deno_core::unsync::spawn(async move {
      for _ in 0..BUFFER_CHANNEL_SIZE * 2 {
        poll_fn(|cx| channel.poll_read_ready(cx)).await;
        // We want to make sure we're aggregating at least some packets
        while channel.byte_size() <= 16 && !channel.closed() {
          tokio::time::sleep(Duration::from_millis(1)).await;
        }
        let len = channel
          .read(1024)
          .unwrap()
          .map(|b| b.len())
          .unwrap_or_default();
        total_recv_task.fetch_add(len, std::sync::atomic::Ordering::SeqCst);
      }
    });

    a.await.unwrap();
    b.await.unwrap();

    assert_eq!(
      total_send.load(std::sync::atomic::Ordering::SeqCst),
      total_recv.load(std::sync::atomic::Ordering::SeqCst)
    );
  }
}
