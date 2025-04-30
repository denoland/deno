// Copyright 2018-2025 the Deno authors. MIT license.
use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::v8;
use deno_core::BufMutView;
use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::V8TaskSpawner;
use deno_error::JsErrorBox;

#[derive(Default)]
struct ReadState {
  active: Cell<bool>,
  waker: RefCell<Option<Waker>>,
}

impl ReadState {
  fn new() -> Rc<Self> {
    Rc::new(Self::default())
  }

  fn activate(self: &Rc<Self>) {
    self.active.set(true);
    self.waker.borrow_mut().take();
  }

  fn deactivate(self: &Rc<Self>) {
    self.active.set(false);
    self.wake();
  }

  fn is_active(&self) -> bool {
    self.active.get()
  }

  fn set_waker(&self, waker: Waker) {
    *self.waker.borrow_mut() = Some(waker);
  }

  fn wake(&self) {
    if let Some(waker) = self.waker.borrow_mut().take() {
      waker.wake();
    }
  }
}

pub struct StreamWrap {
  handle: Rc<RefCell<Option<ResourceId>>>,
  read_state: Rc<ReadState>,
}

struct ReadFuture<F> {
  fut: F,
  state: Rc<RefCell<OpState>>,
  on_read: v8::Global<v8::Function>,
  read_state: Rc<ReadState>,
}

impl<F> Future for ReadFuture<F>
where
  F: Future<Output = Result<(usize, BufMutView), JsErrorBox>> + Unpin,
{
  type Output = Result<(), JsErrorBox>;

  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    if !self.read_state.is_active() {
      return Poll::Ready(Ok(()));
    }

    self.read_state.set_waker(cx.waker().clone());

    match Pin::new(&mut self.fut).poll(cx) {
      Poll::Ready(Ok((n, _))) => {
        let cb = self.on_read.clone();
        self
          .state
          .borrow()
          .borrow::<V8TaskSpawner>()
          .spawn(move |scope| {
            let recv = v8::undefined(scope);
            let nread = if n == 0 {
              v8::null(scope).into()
            } else {
              v8::Number::new(scope, n as f64).into()
            };
            cb.open(scope).call(scope, recv.into(), &[nread]).unwrap();
          });
        Poll::Ready(Ok(()))
      }
      Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
      Poll::Pending => Poll::Pending,
    }
  }
}

async fn uv_read_start(
  state: Rc<RefCell<OpState>>,
  buf: JsBuffer,
  handle: ResourceId,
  read_state: Rc<ReadState>,
  on_read: v8::Global<v8::Function>,
) -> Result<(), JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get_any(handle)
    .map_err(|_| JsErrorBox::from_err(ResourceError::BadResourceId))?;

  let parts = buf.into_parts();

  while read_state.is_active() {
    let buf = BufMutView::from(JsBuffer::from_parts(parts.clone()));

    let read_fut = ReadFuture {
      fut: resource.clone().read_byob(buf),
      state: state.clone(),
      on_read: on_read.clone(),
      read_state: read_state.clone(),
    };

    read_fut.await?;
    // Wait for the event loop to process the read callback.
    tokio::task::yield_now().await;
  }

  Ok(())
}

impl GarbageCollected for StreamWrap {}

#[op2]
impl StreamWrap {
  #[constructor]
  #[cppgc]
  fn new(#[smi] handle: Option<ResourceId>) -> StreamWrap {
    StreamWrap {
      handle: Rc::new(RefCell::new(handle)),
      read_state: ReadState::new(),
    }
  }

  #[fast]
  fn attach_handle(&self, #[smi] handle: ResourceId) -> Result<(), JsErrorBox> {
    self.handle.borrow_mut().replace(handle);
    self.read_state.deactivate();
    Ok(())
  }

  #[async_method]
  async fn read_start(
    &self,
    state: Rc<RefCell<OpState>>,
    #[buffer] buf: JsBuffer,
    #[global] cb: v8::Global<v8::Function>,
  ) -> Result<(), JsErrorBox> {
    let Some(handle) = *self.handle.borrow() else {
      return Ok(());
    };

    if self.read_state.is_active() {
      return Ok(());
    }

    self.read_state.activate();

    if let Err(err) =
      uv_read_start(state, buf, handle, self.read_state.clone(), cb).await
    {
      self.read_state.deactivate();
      return Err(err);
    }

    Ok(())
  }

  #[fast]
  fn read_stop(&self) {
    self.read_state.deactivate();
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use deno_core::futures::FutureExt;
  use deno_core::op2;
  use deno_core::AsyncResult;
  use deno_core::OpState;
  use deno_core::Resource;
  use deno_core::ResourceId;

  use crate::ops::js_test;
  struct TestResource {}

  impl TestResource {
    fn read(self: Rc<Self>, _data: &mut [u8]) -> AsyncResult<usize> {
      async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok(1)
      }
      .boxed_local()
    }
  }

  impl Resource for TestResource {
    deno_core::impl_readable_byob!();
  }

  #[op2(fast)]
  #[smi]
  fn op_test_resource(state: &mut OpState) -> ResourceId {
    state.resource_table.add(TestResource {})
  }

  deno_core::extension!(
    test_ext,
    ops = [op_test_resource],
    objects = [super::StreamWrap],
  );

  #[tokio::test]
  async fn test_stream_wrap() {
    js_test(
      test_ext::init_ops(),
      r#"
        const { StreamWrap, op_test_resource } = Deno.core.ops;

        const stream = new StreamWrap(op_test_resource());
        const buf = new Uint8Array(10);
        let count = 0;
        const promise = stream.readStart(buf, () => {
          count++;
        });

        queueMicrotask(() => stream.readStop(), 1000);

        (async () => {
          await promise;
          if (count == 0) {
            throw new Error("Expected at least one read");
          }
        })();
      "#,
    )
    .await;
  }
}
