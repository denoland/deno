// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use std::cell::RefCell;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

use crate::lsp::ReplCompletionItem;

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

pub enum RustylineSyncMessage {
  PostMessage {
    method: String,
    params: Option<Value>,
  },
  LspCompletions {
    line_text: String,
    position: usize,
    explicit: bool,
  },
  Preview {
    line_text: String,
  },
}

pub enum RustylineSyncResponse {
  PostMessage(Result<Value, AnyError>),
  LspCompletions(Vec<ReplCompletionItem>),
  Preview(Option<(String, usize)>),
}

pub struct RustylineSyncMessageSender {
  message_tx: Sender<RustylineSyncMessage>,
  response_rx: RefCell<UnboundedReceiver<RustylineSyncResponse>>,
}

impl RustylineSyncMessageSender {
  pub fn post_message<T: serde::Serialize>(
    &self,
    method: &str,
    params: Option<T>,
  ) -> Result<Value, AnyError> {
    if let Err(err) =
      self
        .message_tx
        .blocking_send(RustylineSyncMessage::PostMessage {
          method: method.to_string(),
          params: params
            .map(|params| serde_json::to_value(params))
            .transpose()?,
        })
    {
      Err(anyhow!("{}", err))
    } else {
      match self.response_rx.borrow_mut().blocking_recv().unwrap() {
        RustylineSyncResponse::PostMessage(result) => result,
        _ => unreachable!(),
      }
    }
  }

  pub fn lsp_completions(
    &self,
    line_text: &str,
    position: usize,
    explicit: bool,
  ) -> Vec<ReplCompletionItem> {
    if self
      .message_tx
      .blocking_send(RustylineSyncMessage::LspCompletions {
        line_text: line_text.to_string(),
        position,
        explicit,
      })
      .is_err()
    {
      Vec::new()
    } else {
      match self.response_rx.borrow_mut().blocking_recv().unwrap() {
        RustylineSyncResponse::LspCompletions(result) => result,
        _ => unreachable!(),
      }
    }
  }

  pub fn preview(&self, line_text: &str) -> Option<(String, usize)> {
    if self
      .message_tx
      .blocking_send(RustylineSyncMessage::Preview {
        line_text: line_text.to_string(),
      })
      .is_err()
    {
      None
    } else {
      match self.response_rx.borrow_mut().blocking_recv().unwrap() {
        RustylineSyncResponse::Preview(result) => result,
        _ => unreachable!(),
      }
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
