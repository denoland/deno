// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerHandle;
use crate::web_worker::WorkerEvent;
use deno_core::futures::channel::mpsc;
use deno_core::serde_json::json;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  sender: mpsc::Sender<WorkerEvent>,
  handle: WebWorkerHandle,
) {
  // Post message to host as guest worker.
  let sender_ = sender.clone();
  super::reg_json_sync(
    rt,
    "op_worker_post_message",
    move |_state, _args: (), bufs| {
      assert_eq!(bufs.len(), 1, "Invalid number of arguments");
      let msg_buf: Box<[u8]> = (*bufs[0]).into();
      sender_
        .clone()
        .try_send(WorkerEvent::Message(msg_buf))
        .expect("Failed to post message to host");
      Ok(json!({}))
    },
  );

  // Notify host that guest worker closes.
  super::reg_json_sync(
    rt,
    "op_worker_close",
    move |_state, _args: (), _bufs| {
      // Notify parent that we're finished
      sender.clone().close_channel();
      // Terminate execution of current worker
      handle.terminate();
      Ok(json!({}))
    },
  );
}
