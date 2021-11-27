// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use std::cell::RefCell;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

/// Rustyline uses synchronous methods in its interfaces, but we need to call
/// async methods. To get around this, we communicate with async code by using
/// a channel and blocking on the result.
pub fn rustyline_channel(
) -> (RustylineSyncMessageSender, RustylineSyncMessageHandler) {
  let (message_tx, message_rx) = channel(1);
  let (response_tx, response_rx) = unbounded_channel();

  (
    RustylineSyncMessageSender {
      message_tx,
      response_rx: RefCell::new(response_rx),
    },
    RustylineSyncMessageHandler {
      response_tx,
      message_rx,
    },
  )
}

pub type RustylineSyncMessage = (String, Option<Value>);
pub type RustylineSyncResponse = Result<Value, AnyError>;

pub struct RustylineSyncMessageSender {
  message_tx: Sender<RustylineSyncMessage>,
  response_rx: RefCell<UnboundedReceiver<RustylineSyncResponse>>,
}

impl RustylineSyncMessageSender {
  pub fn post_message(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> Result<Value, AnyError> {
    if let Err(err) =
      self.message_tx.blocking_send((method.to_string(), params))
    {
      Err(anyhow!("{}", err))
    } else {
      self.response_rx.borrow_mut().blocking_recv().unwrap()
    }
  }
}

pub struct RustylineSyncMessageHandler {
  message_rx: Receiver<RustylineSyncMessage>,
  response_tx: UnboundedSender<RustylineSyncResponse>,
}

impl RustylineSyncMessageHandler {
  pub async fn recv(&mut self) -> Option<RustylineSyncMessage> {
    self.message_rx.recv().await
  }

  pub fn send(&self, response: RustylineSyncResponse) -> Result<(), AnyError> {
    self
      .response_tx
      .send(response)
      .map_err(|err| anyhow!("{}", err))
  }
}
