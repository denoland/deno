// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WorkerEvent;
use deno_core::error::generic_error;
use deno_core::error::null_opbuf;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::ZeroCopyBuf;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      (
        "op_worker_post_message",
        op_sync(move |state, _args: (), buf: Option<ZeroCopyBuf>| {
          let buf = buf.ok_or_else(null_opbuf)?;
          let msg_buf: Box<[u8]> = (*buf).into();
          let handle = state.borrow::<WebWorkerInternalHandle>().clone();
          handle
            .post_event(WorkerEvent::Message(msg_buf))
            .expect("Failed to post message to host");
          Ok(())
        }),
      ),
      (
        "op_worker_get_message",
        op_async(move |state, _: (), _: ()| async move {
          let temp = {
            let a = state.borrow();
            a.borrow::<WebWorkerInternalHandle>().clone()
          };

          let maybe_data = temp.get_message().await;

          Ok(maybe_data.unwrap_or_default())
        }),
      ),
      // Notify host that guest worker closes.
      (
        "op_worker_close",
        op_sync(|state, _: (), _: ()| {
          // Notify parent that we're finished
          let mut handle =
            state.borrow_mut::<WebWorkerInternalHandle>().clone();

          handle.terminate();
          Ok(())
        }),
      ),
      // Notify host that guest worker has unhandled error.
      (
        "op_worker_unhandled_error",
        op_sync(|state, message: String, _: ()| {
          let sender = state.borrow::<WebWorkerInternalHandle>().clone();
          sender
            .post_event(WorkerEvent::Error(generic_error(message)))
            .expect("Failed to propagate error event to parent worker");
          Ok(true)
        }),
      ),
    ])
    .build()
}
