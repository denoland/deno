// Copyright 2018-2026 the Deno authors. MIT license.

mod sync_fetch;

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::CancelFuture;
use deno_core::DetachedBuffer;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_web::JsMessageData;
use deno_web::MessagePortError;
pub use sync_fetch::SyncFetchError;

use self::sync_fetch::op_worker_sync_fetch;
use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WorkerControlEvent;
use crate::web_worker::WorkerThreadType;

deno_core::extension!(
  deno_web_worker,
  ops = [
    op_worker_post_message,
    op_worker_post_message_raw,
    op_worker_recv_message,
    op_worker_recv_message_sync,
    op_worker_register_message_dispatch,
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
) -> Result<(), MessagePortError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)
}

/// Fast-path post: takes a pre-serialized buffer directly, bypassing
/// the JsMessageData serde overhead. Only for messages with no transferables.
#[op2]
fn op_worker_post_message_raw(
  state: &mut OpState,
  #[buffer(detach)] data: JsBuffer,
) -> Result<(), MessagePortError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  let detached = DetachedBuffer::from_v8slice(data.into_parts());
  if let Some(tx) = &*handle.port.tx.borrow() {
    tx.send((detached, vec![])).ok();
  }
  Ok(())
}

// Message delivery is now driven from the Rust event loop (see
// `deno_runtime::message_dispatch`): the dispatch pump drains the port and
// invokes the registered dispatcher directly. This op no longer returns
// individual messages — it stays pending purely as the keep-alive / ref-unref
// anchor and resolves `null` once the channel closes, so the JS receive loop's
// lifecycle handling (idle exit, `unrefOpPromise`) is unchanged.
#[op2(async(lazy), fast)]
#[serde]
async fn op_worker_recv_message(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let handle = {
    let state = state.borrow();
    state.borrow::<WebWorkerInternalHandle>().clone()
  };
  handle.port.closed().or_cancel(handle.cancel).await?;
  Ok(None)
}

/// Registers the worker's parent port + a dispatcher for Rust-driven message
/// delivery. Returns the registration id (passed to `op_message_dispatch_unregister`).
#[op2(fast)]
pub fn op_worker_register_message_dispatch(
  state: &mut OpState,
  scope: &mut v8::PinScope,
  cb: v8::Local<v8::Function>,
) -> u32 {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  let dispatcher = v8::Global::new(scope, cb);
  deno_web::register_message_dispatch(state, handle.port, dispatcher)
}

#[op2]
#[serde]
fn op_worker_recv_message_sync(
  state: &mut OpState,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.try_recv_sync(state)
}

#[op2(fast)]
fn op_worker_close(state: &mut OpState) {
  // Notify parent that we're finished
  let exit_code = state
    .try_borrow::<deno_os::ExitCode>()
    .map(|e| e.get())
    .unwrap_or(0);
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  // Send the exit code to the parent before terminating
  let _ = handle.post_event(WorkerControlEvent::Close(exit_code));
  handle.terminate();
}

#[op2]
fn op_worker_get_type(state: &mut OpState) -> WorkerThreadType {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.worker_type
}
