// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WorkerEvent;
use deno_core::error::null_opbuf;
use deno_core::futures::channel::mpsc;
use deno_core::op_sync;
use deno_core::Extension;

pub fn init(
  sender: mpsc::Sender<WorkerEvent>,
  handle: WebWorkerHandle,
) -> Extension {
  // Post message to host as guest worker.
  let sender_ = sender.clone();

  Extension::builder()
    .ops(vec![
      (
        "op_worker_post_message",
        op_sync(move |_state, _args: (), buf| {
          let buf = buf.ok_or_else(null_opbuf)?;
          let msg_buf: Box<[u8]> = (*buf).into();
          sender_
            .clone()
            .try_send(WorkerEvent::Message(msg_buf))
            .expect("Failed to post message to host");
          Ok(())
        }),
      ),
      // Notify host that guest worker closes.
      (
        "op_worker_close",
        op_sync(move |_state, _args: (), _bufs| {
          // Notify parent that we're finished
          sender.clone().close_channel();
          // Terminate execution of current worker
          handle.terminate();
          Ok(())
        }),
      ),
    ])
    .build()
}
