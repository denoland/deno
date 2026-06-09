// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_error::JsErrorBox;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::unbounded_channel;

/// The line editor needs synchronous completions, but completion data comes
/// from async runtime methods. To get around this, we communicate with async
/// code by using a channel and blocking on the result.
pub fn editor_channel() -> (EditorSyncMessageSender, EditorSyncMessageHandler) {
  let (message_tx, message_rx) = channel(1);
  let (response_tx, response_rx) = unbounded_channel();

  (
    EditorSyncMessageSender {
      message_tx,
      response_rx: RefCell::new(response_rx),
    },
    EditorSyncMessageHandler {
      response_tx,
      message_rx,
    },
  )
}

pub enum EditorSyncMessage {
  PostMessage {
    method: String,
    params: Option<Value>,
  },
}

pub enum EditorSyncResponse {
  PostMessage(Value),
}

pub struct EditorSyncMessageSender {
  message_tx: Sender<EditorSyncMessage>,
  response_rx: RefCell<UnboundedReceiver<EditorSyncResponse>>,
}

impl EditorSyncMessageSender {
  pub fn post_message<T: serde::Serialize>(
    &self,
    method: &str,
    params: Option<T>,
  ) -> Result<Value, JsErrorBox> {
    match self
      .message_tx
      .blocking_send(EditorSyncMessage::PostMessage {
        method: method.to_string(),
        params: params
          .map(|params| serde_json::to_value(params))
          .transpose()
          .map_err(JsErrorBox::from_err)?,
      }) {
      Err(err) => Err(JsErrorBox::from_err(err)),
      _ => match self.response_rx.borrow_mut().blocking_recv().unwrap() {
        EditorSyncResponse::PostMessage(result) => Ok(result),
      },
    }
  }
}

pub struct EditorSyncMessageHandler {
  message_rx: Receiver<EditorSyncMessage>,
  response_tx: UnboundedSender<EditorSyncResponse>,
}

impl EditorSyncMessageHandler {
  pub async fn recv(&mut self) -> Option<EditorSyncMessage> {
    self.message_rx.recv().await
  }

  pub fn send(&self, response: EditorSyncResponse) -> Result<(), AnyError> {
    self
      .response_tx
      .send(response)
      .map_err(|err| anyhow!("{}", err))
  }
}
