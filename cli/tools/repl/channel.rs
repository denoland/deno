// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Mutex;

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

/// The editor uses synchronous completion hooks, but we need to call
/// async methods. To get around this, we communicate with async code by using
/// a channel and blocking on the result.
pub fn repl_sync_channel() -> (ReplSyncMessageSender, ReplSyncMessageHandler) {
  let (message_tx, message_rx) = channel(1);
  let (response_tx, response_rx) = unbounded_channel();

  (
    ReplSyncMessageSender {
      message_tx,
      response_rx: Mutex::new(response_rx),
    },
    ReplSyncMessageHandler {
      response_tx,
      message_rx,
    },
  )
}

pub enum ReplSyncMessage {
  PostMessage {
    method: String,
    params: Option<Value>,
  },
}

pub enum ReplSyncResponse {
  PostMessage(Value),
}

pub struct ReplSyncMessageSender {
  message_tx: Sender<ReplSyncMessage>,
  response_rx: Mutex<UnboundedReceiver<ReplSyncResponse>>,
}

impl ReplSyncMessageSender {
  pub fn post_message<T: serde::Serialize>(
    &self,
    method: &str,
    params: Option<T>,
  ) -> Result<Value, JsErrorBox> {
    match self.message_tx.blocking_send(ReplSyncMessage::PostMessage {
      method: method.to_string(),
      params: params
        .map(|params| serde_json::to_value(params))
        .transpose()
        .map_err(JsErrorBox::from_err)?,
    }) {
      Err(err) => Err(JsErrorBox::from_err(err)),
      _ => match self.response_rx.lock().unwrap().blocking_recv().unwrap() {
        ReplSyncResponse::PostMessage(result) => Ok(result),
      },
    }
  }
}

pub struct ReplSyncMessageHandler {
  message_rx: Receiver<ReplSyncMessage>,
  response_tx: UnboundedSender<ReplSyncResponse>,
}

impl ReplSyncMessageHandler {
  pub async fn recv(&mut self) -> Option<ReplSyncMessage> {
    self.message_rx.recv().await
  }

  pub fn send(&self, response: ReplSyncResponse) -> Result<(), AnyError> {
    self
      .response_tx
      .send(response)
      .map_err(|err| anyhow!("{}", err))
  }
}
