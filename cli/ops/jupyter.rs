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
use deno_core::serde_json::json;
use deno_core::OpState;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_input,
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
#[string]
pub async fn op_jupyter_input(
  state: Rc<RefCell<OpState>>,
  #[string] prompt: String,
  #[serde] is_password: serde_json::Value,
) -> Result<Option<String>, AnyError> {
  let (_iopub_socket, last_execution_request, stdin_socket) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<Connection<zeromq::PubSocket>>>>()
        .clone(),
      s.borrow::<Rc<RefCell<Option<JupyterMessage>>>>().clone(),
      s.borrow::<Arc<Mutex<Connection<zeromq::RouterSocket>>>>()
        .clone(),
    )
  };

  let mut stdin = stdin_socket.lock().await;

  let maybe_last_request = last_execution_request.borrow().clone();
  if let Some(last_request) = maybe_last_request {
    if !last_request.allow_stdin() {
      return Ok(None);
    }

    /*
     * Using with_identities() because of jupyter client docs instruction
     * Requires cloning identities per :
     * https://jupyter-client.readthedocs.io/en/latest/messaging.html#messages-on-the-stdin-router-dealer-channel
     * The stdin socket of the client is required to have the
     *  same zmq IDENTITY as the client’s shell socket.
     *  Because of this, the input_request must be sent with the same IDENTITY
     *  routing prefix as the execute_reply in order for the frontend to receive the message.
     * """
     */
    last_request
      .new_message("input_request")
      .with_identities(&last_request)
      .with_content(json!({
        "prompt": prompt,
        "password": is_password,
      }))
      .send(&mut *stdin)
      .await?;

    let response = JupyterMessage::read(&mut *stdin).await?;

    return Ok(Some(response.value().to_string()));
  }

  Ok(None)
}

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
