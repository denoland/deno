// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::BroadcastChannel;
use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone)]
pub struct InMemoryBroadcastChannel(Arc<Mutex<broadcast::Sender<Message>>>);

pub struct InMemoryBroadcastChannelResource {
  rx: tokio::sync::Mutex<(
    broadcast::Receiver<Message>,
    mpsc::UnboundedReceiver<()>,
  )>,
  cancel_tx: mpsc::UnboundedSender<()>,
  uuid: Uuid,
}

#[derive(Clone, Debug)]
struct Message {
  name: Arc<String>,
  data: Arc<Vec<u8>>,
  uuid: Uuid,
}

impl Default for InMemoryBroadcastChannel {
  fn default() -> Self {
    let (tx, _) = broadcast::channel(256);
    Self(Arc::new(Mutex::new(tx)))
  }
}

#[async_trait]
impl BroadcastChannel for InMemoryBroadcastChannel {
  type Resource = InMemoryBroadcastChannelResource;

  fn subscribe(&self) -> Result<Self::Resource, AnyError> {
    let (cancel_tx, cancel_rx) = mpsc::unbounded_channel();
    let broadcast_rx = self.0.lock().subscribe();
    let rx = tokio::sync::Mutex::new((broadcast_rx, cancel_rx));
    let uuid = Uuid::new_v4();
    Ok(Self::Resource {
      rx,
      cancel_tx,
      uuid,
    })
  }

  fn unsubscribe(&self, resource: &Self::Resource) -> Result<(), AnyError> {
    Ok(resource.cancel_tx.send(())?)
  }

  async fn send(
    &self,
    resource: &Self::Resource,
    name: String,
    data: Vec<u8>,
  ) -> Result<(), AnyError> {
    let name = Arc::new(name);
    let data = Arc::new(data);
    let uuid = resource.uuid;
    self.0.lock().send(Message { name, data, uuid })?;
    Ok(())
  }

  async fn recv(
    &self,
    resource: &Self::Resource,
  ) -> Result<Option<crate::Message>, AnyError> {
    let mut g = resource.rx.lock().await;
    let (broadcast_rx, cancel_rx) = &mut *g;
    loop {
      let result = tokio::select! {
        r = broadcast_rx.recv() => r,
        _ = cancel_rx.recv() => return Ok(None),
      };
      use tokio::sync::broadcast::error::RecvError::*;
      match result {
        Err(Closed) => return Ok(None),
        Err(Lagged(_)) => (), // Backlogged, messages dropped.
        Ok(message) if message.uuid == resource.uuid => (), // Self-send.
        Ok(message) => {
          let name = String::clone(&message.name);
          let data = Vec::clone(&message.data);
          return Ok(Some((name, data)));
        }
      }
    }
  }
}

impl deno_core::Resource for InMemoryBroadcastChannelResource {}
