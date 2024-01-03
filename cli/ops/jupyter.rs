// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::tools::jupyter::jupyter_msg::Connection;
use crate::tools::jupyter::jupyter_msg::JupyterMessage;
use crate::tools::jupyter::server::StdioMsg;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::Op;
use deno_core::OpState;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_broadcast,
  ],
  options = {
    sender: mpsc::UnboundedSender<StdioMsg>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print::DECL,
    _ => op,
  },
  state = |state, options| {
    state.put(options.sender);
  },
);

#[op2(async)]
pub async fn op_jupyter_broadcast(
  state: Rc<RefCell<OpState>>,
  #[string] message_type: String,
  #[serde] content: serde_json::Value,
  #[serde] metadata: serde_json::Value,
  #[serde] buffers: Vec<deno_core::JsBuffer>,
) -> Result<(), AnyError> {
  let (iopub_socket, last_execution_request) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<Connection<zeromq::PubSocket>>>>()
        .clone(),
      s.borrow::<Rc<RefCell<Option<JupyterMessage>>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.borrow().clone();
  if let Some(last_request) = maybe_last_request {
    last_request
      .new_message(&message_type)
      .with_content(content)
      .with_metadata(metadata)
      .with_buffers(buffers.into_iter().map(|b| b.to_vec().into()).collect())
      .send(&mut *iopub_socket.lock().await)
      .await?;
  }

  Ok(())
}

#[op2(fast)]
pub fn op_print(
  state: &mut OpState,
  #[string] msg: &str,
  is_err: bool,
) -> Result<(), AnyError> {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<StdioMsg>>();

  if is_err {
    if let Err(err) = sender.send(StdioMsg::Stderr(msg.into())) {
      eprintln!("Failed to send stderr message: {}", err);
    }
    return Ok(());
  }

  if let Err(err) = sender.send(StdioMsg::Stdout(msg.into())) {
    eprintln!("Failed to send stdout message: {}", err);
  }
  Ok(())
}
