// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError as BroadcastSendError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError as MpscSendError;
use uuid::Uuid;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BroadcastChannelError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(
    #[from]
    #[inherit]
    deno_core::error::ResourceError,
  ),
  #[class(generic)]
  #[error(transparent)]
  MPSCSendError(MpscSendError<Box<dyn std::fmt::Debug + Send + Sync>>),
  #[class(generic)]
  #[error(transparent)]
  BroadcastSendError(
    BroadcastSendError<Box<dyn std::fmt::Debug + Send + Sync>>,
  ),
}

impl<T: std::fmt::Debug + Send + Sync + 'static> From<MpscSendError<T>>
  for BroadcastChannelError
{
  fn from(value: MpscSendError<T>) -> Self {
    BroadcastChannelError::MPSCSendError(MpscSendError(Box::new(value.0)))
  }
}
impl<T: std::fmt::Debug + Send + Sync + 'static> From<BroadcastSendError<T>>
  for BroadcastChannelError
{
  fn from(value: BroadcastSendError<T>) -> Self {
    BroadcastChannelError::BroadcastSendError(BroadcastSendError(Box::new(
      value.0,
    )))
  }
}

pub type BroadcastChannelMessage = (String, Vec<u8>);

#[op2(fast)]
#[smi]
pub fn op_broadcast_subscribe(
  state: &mut OpState,
) -> Result<ResourceId, BroadcastChannelError> {
  let bc = state.borrow::<InMemoryBroadcastChannel>();
  let resource = bc.subscribe()?;
  Ok(state.resource_table.add(resource))
}

#[op2(fast)]
pub fn op_broadcast_unsubscribe(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), BroadcastChannelError> {
  let resource = state
    .resource_table
    .get::<InMemoryBroadcastChannelResource>(rid)?;
  let bc = state.borrow::<InMemoryBroadcastChannel>();
  bc.unsubscribe(&resource)
}

#[op2]
pub fn op_broadcast_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] name: String,
  #[buffer] buf: JsBuffer,
) -> Result<(), BroadcastChannelError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<InMemoryBroadcastChannelResource>(rid)?;
  let bc = state.borrow().borrow::<InMemoryBroadcastChannel>().clone();
  bc.send(&resource, name, buf.to_vec())
}

#[op2(async)]
#[serde]
pub async fn op_broadcast_recv(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<BroadcastChannelMessage>, BroadcastChannelError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<InMemoryBroadcastChannelResource>(rid)?;
  let bc = state.borrow().borrow::<InMemoryBroadcastChannel>().clone();
  bc.recv(&resource).await
}

#[derive(Clone)]
pub struct InMemoryBroadcastChannel(
  Arc<Mutex<broadcast::Sender<InMemoryChannelMessage>>>,
);

pub struct InMemoryBroadcastChannelResource {
  rx: tokio::sync::Mutex<(
    broadcast::Receiver<InMemoryChannelMessage>,
    mpsc::UnboundedReceiver<()>,
  )>,
  cancel_tx: mpsc::UnboundedSender<()>,
  uuid: Uuid,
}

impl deno_core::Resource for InMemoryBroadcastChannelResource {}

#[derive(Clone, Debug)]
struct InMemoryChannelMessage {
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

impl InMemoryBroadcastChannel {
  fn subscribe(
    &self,
  ) -> Result<InMemoryBroadcastChannelResource, BroadcastChannelError> {
    let (cancel_tx, cancel_rx) = mpsc::unbounded_channel();
    let broadcast_rx = self.0.lock().subscribe();
    let rx = tokio::sync::Mutex::new((broadcast_rx, cancel_rx));
    let uuid = Uuid::new_v4();
    Ok(InMemoryBroadcastChannelResource {
      rx,
      cancel_tx,
      uuid,
    })
  }

  fn unsubscribe(
    &self,
    resource: &InMemoryBroadcastChannelResource,
  ) -> Result<(), BroadcastChannelError> {
    Ok(resource.cancel_tx.send(())?)
  }

  fn send(
    &self,
    resource: &InMemoryBroadcastChannelResource,
    name: String,
    data: Vec<u8>,
  ) -> Result<(), BroadcastChannelError> {
    let name = Arc::new(name);
    let data = Arc::new(data);
    let uuid = resource.uuid;
    self
      .0
      .lock()
      .send(InMemoryChannelMessage { name, data, uuid })?;
    Ok(())
  }

  async fn recv(
    &self,
    resource: &InMemoryBroadcastChannelResource,
  ) -> Result<Option<BroadcastChannelMessage>, BroadcastChannelError> {
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
