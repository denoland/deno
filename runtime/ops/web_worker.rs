// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod sync_fetch;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::op_async;
use deno_core::CancelFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_web::JsMessageData;
use std::cell::RefCell;
use std::rc::Rc;

use self::sync_fetch::op_worker_sync_fetch;

pub fn init() -> Extension {
  Extension::builder()
    .ops(|ctx| {
      ctx.register("op_worker_post_message", op_worker_post_message);
      ctx.register("op_worker_recv_message", op_worker_recv_message);
      // Notify host that guest worker closes.
      ctx.register("op_worker_close", op_worker_close);
      ctx.register("op_worker_get_type", op_worker_get_type);
      ctx.register("op_worker_sync_fetch", op_worker_sync_fetch);
    })
    .build()
}

#[op]
fn op_worker_post_message(
  state: &mut OpState,
  data: JsMessageData,
  _: (),
) -> Result<(), AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)?;
  Ok(())
}

#[op_async]
async fn op_worker_recv_message(
  state: Rc<RefCell<OpState>>,
  _: (),
  _: (),
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

#[op]
fn op_worker_close(state: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
  Ok(())
}

#[op]
fn op_worker_get_type(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<WebWorkerType, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  Ok(handle.worker_type)
}
