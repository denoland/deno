// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WorkerEvent;
use deno_core::error::generic_error;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_worker_post_message", op_sync(op_worker_post_message)),
      ("op_worker_get_message", op_async(op_worker_get_message)),
      // Notify host that guest worker closes.
      ("op_worker_close", op_sync(op_worker_close)),
      // Notify host that guest worker has unhandled error.
      (
        "op_worker_unhandled_error",
        op_sync(op_worker_unhandled_error),
      ),
    ])
    .build()
}

fn op_worker_post_message(
  state: &mut OpState,
  _: (),
  buf: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let buf = buf.ok_or_else(null_opbuf)?;
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle
    .post_event(WorkerEvent::Message(buf))
    .expect("Failed to post message to host");
  Ok(())
}

async fn op_worker_get_message(
  state: Rc<RefCell<OpState>>,
  _: (),
  _: (),
) -> Result<ZeroCopyBuf, AnyError> {
  let temp = {
    let a = state.borrow();
    a.borrow::<WebWorkerInternalHandle>().clone()
  };

  let maybe_data = temp.get_message().await;

  Ok(maybe_data.unwrap_or_else(ZeroCopyBuf::empty))
}

#[allow(clippy::unnecessary_wraps)]
fn op_worker_close(state: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
  Ok(())
}

/// A worker that encounters an uncaught error will pass this error
/// to its parent worker using this op. The parent worker will use
/// this same op to pass the error to its own parent (in case
/// `e.preventDefault()` was not called in `worker.onerror`). This
/// is done until the error reaches the root/ main worker.
#[allow(clippy::unnecessary_wraps)]
fn op_worker_unhandled_error(
  state: &mut OpState,
  message: String,
  _: (),
) -> Result<(), AnyError> {
  let sender = state.borrow::<WebWorkerInternalHandle>().clone();
  sender
    .post_event(WorkerEvent::Error(generic_error(message)))
    .expect("Failed to propagate error event to parent worker");
  Ok(())
}
