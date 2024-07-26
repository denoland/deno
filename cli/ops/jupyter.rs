// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// NOTE(bartlomieju): unfortunately it appears that clippy is broken
// and can't allow a single line ignore for `await_holding_lock`.
#![allow(clippy::await_holding_lock)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bytes::Bytes;

use jupyter_runtime::CommId;
use jupyter_runtime::CommInfo;
use jupyter_runtime::CommMsg;
use jupyter_runtime::InputRequest;
use jupyter_runtime::JupyterMessage;
use jupyter_runtime::JupyterMessageContent;
use jupyter_runtime::KernelIoPubConnection;
use jupyter_runtime::StreamContent;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::parking_lot::Mutex as PlMutex;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::OpState;
use deno_core::ToJsBuffer;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::tools::jupyter::server::StdinConnectionProxy;

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_comm_recv,
    op_jupyter_comm_open,
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

pub struct CommChannel {
  pub target_name: String,
  pub sender: broadcast::Sender<(CommMsg, Vec<Bytes>)>,
  pub receiver: broadcast::Receiver<(CommMsg, Vec<Bytes>)>,
}

#[derive(Clone, Default)]
pub struct CommContainer(pub Arc<PlMutex<HashMap<String, CommChannel>>>);

impl CommContainer {
  pub fn create(
    &mut self,
    comm_id: &str,
    target_name: &str,
    // For pulling off the metadata and buffers
    _msg: Option<&JupyterMessage>,
  ) {
    let mut container = self.0.lock();

    // We will not replace existing comms
    if container.contains_key(comm_id) {
      return;
    }

    let (tx, rx) = broadcast::channel(16);
    let comm_channel = CommChannel {
      target_name: target_name.to_string(),
      sender: tx,
      receiver: rx,
    };

    container.insert(comm_id.to_string(), comm_channel);
  }

  pub fn comms(&self) -> HashMap<CommId, CommInfo> {
    let container = self.0.lock();

    container
      .iter()
      .map(|(comm_id, comm)| {
        (
          CommId(comm_id.to_string()),
          CommInfo {
            target_name: comm.target_name.clone(),
          },
        )
      })
      .collect()
  }
}

#[op2(fast)]
pub fn op_jupyter_comm_open(
  state: &mut OpState,
  #[string] comm_id: String,
  #[string] target_name: String,
) {
  let container = state.borrow_mut::<CommContainer>();
  container.create(&comm_id, &target_name, None);
  // eprintln!("created comm {} {}", comm_id, target_name);
}

#[op2(async)]
#[serde]
pub async fn op_jupyter_comm_recv(
  state: Rc<RefCell<OpState>>,
  #[string] comm_id: String,
) -> (serde_json::Value, Vec<ToJsBuffer>) {
  let mut receiver = {
    let state = state.borrow();
    let container = state.borrow::<CommContainer>();
    let container = container.0.lock();
    let maybe_comm = container.get(&comm_id);
    let Some(comm) = maybe_comm else {
      return (serde_json::Value::Null, vec![]);
    };
    comm.receiver.resubscribe()
  };

  // eprintln!("starting receive");
  let (msg, buffers) = receiver.recv().await.unwrap();
  // eprintln!("received");
  (
    serde_json::to_value(msg).unwrap(),
    buffers
      .into_iter()
      .map(|b| ToJsBuffer::from(b.to_vec()))
      .collect(),
  )
}

#[op2]
#[string]
pub fn op_jupyter_input(
  state: &mut OpState,
  #[string] prompt: String,
  is_password: bool,
) -> Result<Option<String>, AnyError> {
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
      return Ok(None);
    };

    if !msg.allow_stdin {
      return Ok(None);
    }

    let msg = JupyterMessage::new(
      JupyterMessageContent::InputRequest(InputRequest {
        prompt,
        password: is_password,
      }),
      Some(&last_request),
    );

    let Ok(()) = stdin_connection_proxy.lock().tx.send(msg) else {
      return Ok(None);
    };

    // Need to spawn a separate thread here, because `blocking_recv()` can't
    // be used from the Tokio runtime context.
    let join_handle = std::thread::spawn(move || {
      stdin_connection_proxy.lock().rx.blocking_recv()
    });
    let Ok(Some(response)) = join_handle.join() else {
      return Ok(None);
    };

    let JupyterMessageContent::InputReply(msg) = response.content else {
      return Ok(None);
    };

    return Ok(Some(msg.value));
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
      err
    })?;

    let jupyter_message = JupyterMessage::new(content, Some(&last_request))
      .with_metadata(metadata)
      .with_buffers(buffers.into_iter().map(|b| b.to_vec().into()).collect());

    iopub_connection.lock().send(jupyter_message).await?;
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
    if let Err(err) = sender.send(StreamContent::stderr(msg)) {
      log::error!("Failed to send stderr message: {}", err);
    }
    return Ok(());
  }

  if let Err(err) = sender.send(StreamContent::stdout(msg)) {
    log::error!("Failed to send stdout message: {}", err);
  }
  Ok(())
}
