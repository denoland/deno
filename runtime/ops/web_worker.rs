// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod sync_fetch;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::AnyError;
use deno_core::op;

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
      op_worker_post_message::decl(),
      op_worker_recv_message::decl(),
      // Notify host that guest worker closes.
      op_worker_close::decl(),
      op_worker_get_type::decl(),
      op_worker_sync_fetch::decl(),
    ])
    .build()
}

#[op]
fn op_worker_post_message(
  state: &mut OpState,
  data: JsMessageData,
) -> Result<(), AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)?;
  Ok(())
}

#[op]
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

#[op]
fn op_worker_close(state: &mut OpState) -> Result<(), AnyError> {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
  Ok(())
}

#[op]
fn op_worker_get_type(state: &mut OpState) -> Result<WebWorkerType, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  Ok(handle.worker_type)
}
