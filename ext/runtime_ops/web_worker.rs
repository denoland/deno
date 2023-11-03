// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod sync_fetch;

use crate::worker_host::WebWorkerType;
use deno_core::error::AnyError;
use deno_core::op2;

use deno_core::CancelFuture;
use deno_core::OpState;
use deno_web::JsMessageData;
use std::cell::RefCell;
use std::rc::Rc;

use self::sync_fetch::op_worker_sync_fetch;

#[async_trait::async_trait(?Send)]
pub trait WebWorkerHandle: Clone {
  fn post_message(
    &self,
    _state: &mut OpState,
    _data: JsMessageData,
  ) -> Result<(), AnyError> {
    unimplemented!()
  }

  async fn recv_message(
    &self,
    _state: Rc<RefCell<OpState>>,
  ) -> Result<Option<JsMessageData>, AnyError> {
    unimplemented!()
  }

  fn terminate(&mut self) {
    unimplemented!()
  }

  fn worker_type(&self) -> WebWorkerType {
    unimplemented!()
  }
}

deno_core::extension!(
  deno_web_worker,
  parameters = [P: WebWorkerHandle],
  ops = [
    op_worker_post_message<P>,
    op_worker_recv_message<P>,
    // Notify host that guest worker closes.
    op_worker_close<P>,
    op_worker_get_type<P>,
    op_worker_sync_fetch<P>,
  ],
);

#[op2]
fn op_worker_post_message<W>(
  state: &mut OpState,
  #[serde] data: JsMessageData,
) -> Result<(), AnyError>
where
  W: WebWorkerHandle + 'static,
{
  let handle = state.borrow::<W>().clone();
  handle.post_message(state, data)?;
  Ok(())
}

#[op2(async(lazy), fast)]
#[serde]
async fn op_worker_recv_message<W>(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<JsMessageData>, AnyError>
where
  W: WebWorkerHandle + 'static,
{
  let handle = {
    let state = state.borrow();
    state.borrow::<W>().clone()
  };
  handle.recv_message(state.clone()).await
}

#[op2(fast)]
fn op_worker_close<W>(state: &mut OpState)
where
  W: WebWorkerHandle + 'static,
{
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<W>().clone();

  handle.terminate();
}

#[op2]
#[serde]
fn op_worker_get_type<W>(state: &mut OpState) -> WebWorkerType
where
  W: WebWorkerHandle + 'static,
{
  let handle = state.borrow::<W>().clone();
  handle.worker_type()
}
