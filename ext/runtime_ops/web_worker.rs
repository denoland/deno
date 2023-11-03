// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod sync_fetch;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::AnyError;
use deno_core::op2;

use deno_core::CancelFuture;
use deno_core::OpState;
use deno_web::JsMessageData;
use std::cell::RefCell;
use std::rc::Rc;

use self::sync_fetch::op_worker_sync_fetch;

deno_core::extension!(
  deno_web_worker,
  ops = [
    op_worker_post_message,
    op_worker_recv_message,
    // Notify host that guest worker closes.
    op_worker_close,
    op_worker_get_type,
    op_worker_sync_fetch,
  ],
);

#[op2]
fn op_worker_post_message(
  state: &mut OpState,
  #[serde] data: JsMessageData,
) -> Result<(), AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)?;
  Ok(())
}

#[op2(async(lazy), fast)]
#[serde]
async fn op_worker_recv_message(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<JsMessageData>, AnyError> {
  let handle = {
    let state = state.borrow();
    state.borrow::<WebWorkerInternalHandle>().clone()
  };
  handle
    .port
    .recv(state.clone())
    .or_cancel(handle.cancel)
    .await?
}

#[op2(fast)]
fn op_worker_close(state: &mut OpState) {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
}

#[op2]
#[serde]
fn op_worker_get_type(state: &mut OpState) -> WebWorkerType {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.worker_type
}
