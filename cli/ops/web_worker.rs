// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::state::State;
use crate::web_worker::WebWorkerHandle;
use crate::worker::WorkerEvent;
use deno_core::OpRegistry;
use futures::channel::mpsc;
use std::rc::Rc;

pub fn init(
  s: &Rc<State>,
  sender: &mpsc::Sender<WorkerEvent>,
  handle: WebWorkerHandle,
) {
  // Post message to host as guest worker.
  let sender_ = sender.clone();
  s.register_op_json_sync(
    "op_worker_post_message",
    move |_state, _args, bufs| {
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
  let sender_ = sender.clone();
  s.register_op_json_sync("op_worker_close", move |_state, _args, _bufs| {
    // Notify parent that we're finished
    sender_.clone().close_channel();
    // Terminate execution of current worker
    handle.terminate();
    Ok(json!({}))
  });
}
