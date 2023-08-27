// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcLike;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use futures::stream::Peekable;
use futures::Stream;
use futures::StreamExt;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

type SenderCell = RefCell<Option<Sender<Result<BufView, Error>>>>;

// This indirection allows us to more easily integrate the fast streams work at a later date
#[repr(transparent)]
struct ChannelStreamAdapter<C>(C);

impl<C> Stream for ChannelStreamAdapter<C>
where
  C: ChannelBytesRead,
{
  type Item = Result<BufView, AnyError>;
  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Self::Item>> {
    self.0.poll_recv(cx)
  }
}

pub trait ChannelBytesRead: Unpin + 'static {
  fn poll_recv(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<BufView, AnyError>>>;
}

impl ChannelBytesRead for tokio::sync::mpsc::Receiver<Result<BufView, Error>> {
  fn poll_recv(
    &mut self,
    cx: &mut Context<'_>,
  ) -> Poll<Option<Result<BufView, AnyError>>> {
    self.poll_recv(cx)
  }
}

#[allow(clippy::type_complexity)]
struct ReadableStreamResource {
  reader: AsyncRefCell<
    Peekable<ChannelStreamAdapter<Receiver<Result<BufView, Error>>>>,
  >,
  cancel_handle: CancelHandle,
  data: ReadableStreamResourceData,
}

impl ReadableStreamResource {
  pub fn cancel_handle(self: &Rc<Self>) -> impl RcLike<CancelHandle> {
    RcRef::map(self, |s| &s.cancel_handle).clone()
  }

  async fn read(self: Rc<Self>, limit: usize) -> Result<BufView, AnyError> {
    let cancel_handle = self.cancel_handle();
    let peekable = RcRef::map(self, |this| &this.reader);
    let mut peekable = peekable.borrow_mut().await;
    match Pin::new(&mut *peekable)
      .peek_mut()
      .or_cancel(cancel_handle)
      .await?
    {
      None => Ok(BufView::empty()),
      // Take the actual error since we only have a reference to it
      Some(Err(_)) => Err(peekable.next().await.unwrap().err().unwrap()),
      Some(Ok(bytes)) => {
        if bytes.len() <= limit {
          // We can safely take the next item since we peeked it
          return peekable.next().await.unwrap();
        }
        // The remainder of the bytes after we split it is still left in the peek buffer
        let ret = bytes.split_to(limit);
        Ok(ret)
      }
    }
  }
}

impl Resource for ReadableStreamResource {
  fn name(&self) -> Cow<str> {
    Cow::Borrowed("readableStream")
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(ReadableStreamResource::read(self, limit))
  }

  fn close(self: Rc<Self>) {
    self.cancel_handle.cancel();
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
  let (tx, rx) = tokio::sync::mpsc::channel(1);
  let tx = RefCell::new(Some(tx));
  let completion = CompletionHandle::default();
  let tx = Box::new(tx);
  let resource = ReadableStreamResource {
    cancel_handle: Default::default(),
    reader: AsyncRefCell::new(ChannelStreamAdapter(rx).peekable()),
    data: ReadableStreamResourceData {
      tx: Box::into_raw(tx),
      completion,
    },
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
  resource.data.tx as _
}

fn get_sender(sender: *const c_void) -> Option<Sender<Result<BufView, Error>>> {
  // SAFETY: We know this is a valid v8::External
  unsafe {
    (sender as *const SenderCell)
      .as_ref()
      .and_then(|r| r.borrow_mut().as_ref().cloned())
  }
}

fn drop_sender(sender: *const c_void) {
  // SAFETY: We know this is a valid v8::External
  unsafe {
    assert!(!sender.is_null());
    _ = Box::from_raw(sender as *mut SenderCell);
  }
}

#[op2(async)]
pub fn op_readable_stream_resource_write_buf(
  sender: *const c_void,
  #[buffer] buffer: JsBuffer,
) -> impl Future<Output = bool> {
  let sender = get_sender(sender);
  async move {
    let Some(sender) = sender else {
      return false;
    };
    sender.send(Ok(buffer.into())).await.ok().is_some()
  }
}

#[op2(async)]
pub fn op_readable_stream_resource_write_error(
  sender: *const c_void,
  #[string] error: String,
) -> impl Future<Output = bool> {
  let sender = get_sender(sender);
  async move {
    let Some(sender) = sender else {
      return false;
    };
    sender
      .send(Err(type_error(Cow::Owned(error))))
      .await
      .ok()
      .is_some()
  }
}

#[op2(fast)]
#[smi]
pub fn op_readable_stream_resource_close(sender: *const c_void) {
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
  tx: *const SenderCell,
  completion: CompletionHandle,
}

impl Drop for ReadableStreamResourceData {
  fn drop(&mut self) {
    self.completion.complete(true);
  }
}
