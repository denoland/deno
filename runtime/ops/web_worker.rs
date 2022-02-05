// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod sync_fetch;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::CancelFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_web::JsMessageData;
use std::cell::RefCell;
use std::rc::Rc;

use self::sync_fetch::op_worker_sync_fetch;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_worker_post_message", op_sync(op_worker_post_message)),
      ("op_worker_recv_message", op_async(op_worker_recv_message)),
      // Notify host that guest worker closes.
      ("op_worker_close", op_sync(op_worker_close)),
      ("op_worker_get_type", op_sync(op_worker_get_type)),
      ("op_worker_sync_fetch", op_sync(op_worker_sync_fetch)),
    ])
    .build()
}

fn op_worker_post_message(
  state: &mut OpState,
  data: JsMessageData,
  _: (),
) -> Result<(), AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)?;
  Ok(())
}

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

fn op_worker_close(state: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
  Ok(())
}

fn op_worker_get_type(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<WebWorkerType, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  Ok(handle.worker_type)
}
