// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::anyhow::Error;
use deno_core::error::type_error;
use deno_core::op2;
use deno_core::BufView;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceBuilder;
use deno_core::ResourceBuilderImpl;
use deno_core::ResourceId;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Waker;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

type SenderCell = RefCell<Option<Sender<Result<BufView, Error>>>>;

static READABLE_STREAM_RESOURCE: ResourceBuilder<
  Receiver<Result<BufView, Error>>,
  ReadableStreamResourceData,
> = ResourceBuilderImpl::new_with_data("readableStream")
  .with_read_channel()
  .build();

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

fn sender_closed() -> Error {
  type_error("sender closed")
}

/// Allocate a resource that wraps a ReadableStream.
#[op2(fast)]
#[smi]
pub fn op_readable_stream_resource_allocate(state: &mut OpState) -> ResourceId {
  let (tx, rx) = tokio::sync::mpsc::channel(1);
  let tx = RefCell::new(Some(tx));
  let completion = CompletionHandle::default();
  let tx = Box::new(tx);
  state
    .resource_table
    .add_rc_dyn(READABLE_STREAM_RESOURCE.build_with_data(
      rx,
      ReadableStreamResourceData {
        tx: Box::into_raw(tx),
        completion,
      },
    ))
}

#[op2(fast)]
pub fn op_readable_stream_resource_get_sink(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> *const c_void {
  let Ok(resource) = state.resource_table.get_any(rid) else {
    return std::ptr::null();
  };
  let Some(data) = READABLE_STREAM_RESOURCE.data(&resource) else {
    return std::ptr::null();
  };
  data.tx as _
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
) -> impl Future<Output = Result<(), Error>> {
  let sender = get_sender(sender);
  async move {
    let sender = sender.ok_or_else(sender_closed)?;
    sender
      .send(Ok(buffer.into()))
      .await
      .map_err(|_| sender_closed())?;
    Ok(())
  }
}

#[op2(async)]
pub fn op_readable_stream_resource_write_error(
  sender: *const c_void,
  #[string] error: String,
) -> impl Future<Output = Result<(), Error>> {
  let sender = get_sender(sender);
  async move {
    let sender = sender.ok_or_else(sender_closed)?;
    sender
      .send(Err(type_error(Cow::Owned(error))))
      .await
      .map_err(|_| sender_closed())?;
    Ok(())
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
  let completion = state.resource_table.get_any(rid).ok().and_then(|d| {
    READABLE_STREAM_RESOURCE
      .data(&d)
      .map(|d| d.completion.clone())
  });

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
