// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use jupyter_runtime::JupyterMessage;
use jupyter_runtime::JupyterMessageContent;
use jupyter_runtime::KernelIoPubConnection;
use jupyter_runtime::StreamContent;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::OpState;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_broadcast,
  ],
  options = {
    sender: mpsc::UnboundedSender<StreamContent>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print(),
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
  let (iopub_connection, last_execution_request) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<KernelIoPubConnection>>>().clone(),
      s.borrow::<Arc<Mutex<Option<JupyterMessage>>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.lock().await.clone();
  if let Some(last_request) = maybe_last_request {
    let content = JupyterMessageContent::from_type_and_content(
      &message_type,
      content.clone(),
    )
    .map_err(|err| {
      log::error!(
          "Error deserializing content from jupyter.broadcast, message_type: {}:\n\n{}\n\n{}",
          &message_type,
          content,
          err
      );
      err
    })?;

    let jupyter_message = JupyterMessage::new(content, Some(&last_request))
      .with_metadata(metadata)
      .with_buffers(buffers.into_iter().map(|b| b.to_vec().into()).collect());

    (iopub_connection.lock().await)
      .send(jupyter_message)
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
  let sender = state.borrow_mut::<mpsc::UnboundedSender<StreamContent>>();

  if is_err {
    if let Err(err) = sender.send(StreamContent::stderr(msg.into())) {
      log::error!("Failed to send stderr message: {}", err);
    }
    return Ok(());
  }

  if let Err(err) = sender.send(StreamContent::stdout(msg.into())) {
    log::error!("Failed to send stdout message: {}", err);
  }
  Ok(())
}
