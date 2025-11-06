// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_error::JsErrorBox;
use deno_features::FeatureChecker;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError as BroadcastSendError;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError as MpscSendError;
use uuid::Uuid;

pub const UNSTABLE_FEATURE_NAME: &str = "broadcast-channel";

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
  #[class(inherit)]
  #[error(transparent)]
  Other(#[inherit] JsErrorBox),
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

#[async_trait]
pub trait BroadcastChannel: Clone {
  type Resource: Resource;

  fn subscribe(&self) -> Result<Self::Resource, BroadcastChannelError>;

  fn unsubscribe(
    &self,
    resource: &Self::Resource,
  ) -> Result<(), BroadcastChannelError>;

  async fn send(
    &self,
    resource: &Self::Resource,
    name: String,
    data: Vec<u8>,
  ) -> Result<(), BroadcastChannelError>;

  async fn recv(
    &self,
    resource: &Self::Resource,
  ) -> Result<Option<Message>, BroadcastChannelError>;
}

pub type Message = (String, Vec<u8>);

#[op2(fast)]
#[smi]
pub fn op_broadcast_subscribe<BC>(
  state: &mut OpState,
) -> Result<ResourceId, BroadcastChannelError>
where
  BC: BroadcastChannel + 'static,
{
  state
    .borrow::<Arc<FeatureChecker>>()
    .check_or_exit(UNSTABLE_FEATURE_NAME, "BroadcastChannel");
  let bc = state.borrow::<BC>();
  let resource = bc.subscribe()?;
  Ok(state.resource_table.add(resource))
}

#[op2(fast)]
pub fn op_broadcast_unsubscribe<BC>(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), BroadcastChannelError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow::<BC>();
  bc.unsubscribe(&resource)
}

#[op2(async)]
pub async fn op_broadcast_send<BC>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] name: String,
  #[buffer] buf: JsBuffer,
) -> Result<(), BroadcastChannelError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
  bc.send(&resource, name, buf.to_vec()).await
}

#[op2(async)]
#[serde]
pub async fn op_broadcast_recv<BC>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<Message>, BroadcastChannelError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
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

#[async_trait]
impl BroadcastChannel for InMemoryBroadcastChannel {
  type Resource = InMemoryBroadcastChannelResource;

  fn subscribe(&self) -> Result<Self::Resource, BroadcastChannelError> {
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

  fn unsubscribe(
    &self,
    resource: &Self::Resource,
  ) -> Result<(), BroadcastChannelError> {
    Ok(resource.cancel_tx.send(())?)
  }

  async fn send(
    &self,
    resource: &Self::Resource,
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
    resource: &Self::Resource,
  ) -> Result<Option<Message>, BroadcastChannelError> {
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
