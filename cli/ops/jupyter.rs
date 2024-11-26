// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// NOTE(bartlomieju): unfortunately it appears that clippy is broken
// and can't allow a single line ignore for `await_holding_lock`.
#![allow(clippy::await_holding_lock)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use jupyter_runtime::InputRequest;
use jupyter_runtime::JupyterMessage;
use jupyter_runtime::JupyterMessageContent;
use jupyter_runtime::KernelIoPubConnection;
use jupyter_runtime::StreamContent;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::OpState;
use tokio::sync::mpsc;

use crate::tools::jupyter::server::StdinConnectionProxy;

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

#[op2]
#[string]
pub fn op_jupyter_input(
  state: &mut OpState,
  #[string] prompt: String,
  is_password: bool,
) -> Option<String> {
  let (last_execution_request, stdin_connection_proxy) = {
    (
      state.borrow::<Arc<Mutex<Option<JupyterMessage>>>>().clone(),
      state.borrow::<Arc<Mutex<StdinConnectionProxy>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.lock().clone();
  if let Some(last_request) = maybe_last_request {
    let JupyterMessageContent::ExecuteRequest(msg) = &last_request.content
    else {
      return None;
    };

    if !msg.allow_stdin {
      return None;
    }

    let content = InputRequest {
      prompt,
      password: is_password,
    };

    let msg = JupyterMessage::new(content, Some(&last_request));

    let Ok(()) = stdin_connection_proxy.lock().tx.send(msg) else {
      return None;
    };

    // Need to spawn a separate thread here, because `blocking_recv()` can't
    // be used from the Tokio runtime context.
    let join_handle = std::thread::spawn(move || {
      stdin_connection_proxy.lock().rx.blocking_recv()
    });
    let Ok(Some(response)) = join_handle.join() else {
      return None;
    };

    let JupyterMessageContent::InputReply(msg) = response.content else {
      return None;
    };

    return Some(msg.value);
  }

  None
}

#[derive(Debug, thiserror::Error)]
pub enum JupyterBroadcastError {
  #[error(transparent)]
  SerdeJson(serde_json::Error),
  #[error(transparent)]
  ZeroMq(AnyError),
}

#[op2(async)]
pub async fn op_jupyter_broadcast(
  state: Rc<RefCell<OpState>>,
  #[string] message_type: String,
  #[serde] content: serde_json::Value,
  #[serde] metadata: serde_json::Value,
  #[serde] buffers: Vec<deno_core::JsBuffer>,
) -> Result<(), JupyterBroadcastError> {
  let (iopub_connection, last_execution_request) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<KernelIoPubConnection>>>().clone(),
      s.borrow::<Arc<Mutex<Option<JupyterMessage>>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.lock().clone();
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
      JupyterBroadcastError::SerdeJson(err)
    })?;

    let jupyter_message = JupyterMessage::new(content, Some(&last_request))
      .with_metadata(metadata)
      .with_buffers(buffers.into_iter().map(|b| b.to_vec().into()).collect());

    iopub_connection
      .lock()
      .send(jupyter_message)
      .await
      .map_err(JupyterBroadcastError::ZeroMq)?;
  }

  Ok(())
}

#[op2(fast)]
pub fn op_print(state: &mut OpState, #[string] msg: &str, is_err: bool) {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<StreamContent>>();

  if is_err {
    if let Err(err) = sender.send(StreamContent::stderr(msg)) {
      log::error!("Failed to send stderr message: {}", err);
    }
    return;
  }

  if let Err(err) = sender.send(StreamContent::stdout(msg)) {
    log::error!("Failed to send stdout message: {}", err);
  }
}
