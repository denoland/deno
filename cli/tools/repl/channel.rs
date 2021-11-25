// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
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
pub fn rustyline_channel<TMessage, TResponse>() -> (
  SyncMessageSender<TMessage, TResponse>,
  SyncMessageHandler<TMessage, TResponse>,
) {
  let (message_tx, message_rx) = channel(1);
  let (response_tx, response_rx) = unbounded_channel();

  (
    SyncMessageSender {
      message_tx,
      response_rx: RefCell::new(response_rx),
    },
    SyncMessageHandler {
      response_tx,
      message_rx,
    },
  )
}

pub struct SyncMessageSender<TSend, TReceive> {
  message_tx: Sender<TSend>,
  response_rx: RefCell<UnboundedReceiver<TReceive>>,
}

impl<TSend, TReceive> SyncMessageSender<TSend, TReceive> {
  pub fn send(&self, message: TSend) -> Result<TReceive, AnyError> {
    if let Err(err) = self.message_tx.blocking_send(message) {
      Err(anyhow!("{}", err))
    } else {
      Ok(self.response_rx.borrow_mut().blocking_recv().unwrap())
    }
  }
}

pub struct SyncMessageHandler<TReceive, TResponse> {
  message_rx: Receiver<TReceive>,
  response_tx: UnboundedSender<TResponse>,
}

impl<TReceive, TResponse> SyncMessageHandler<TReceive, TResponse> {
  pub async fn recv(&mut self) -> Option<TReceive> {
    self.message_rx.recv().await
  }

  pub fn send(&self, response: TResponse) -> Result<(), AnyError> {
    self
      .response_tx
      .send(response)
      .map_err(|err| anyhow!("{}", err))
  }
}
