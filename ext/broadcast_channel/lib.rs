// Copyright 2018-2025 the Deno authors. MIT license.

mod in_memory_broadcast_channel;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_error::JsErrorBox;
pub use in_memory_broadcast_channel::InMemoryBroadcastChannel;
pub use in_memory_broadcast_channel::InMemoryBroadcastChannelResource;
use tokio::sync::broadcast::error::SendError as BroadcastSendError;
use tokio::sync::mpsc::error::SendError as MpscSendError;

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
    .feature_checker
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

deno_core::extension!(deno_broadcast_channel,
  deps = [ deno_webidl, deno_web ],
  parameters = [BC: BroadcastChannel],
  ops = [
    op_broadcast_subscribe<BC>,
    op_broadcast_unsubscribe<BC>,
    op_broadcast_send<BC>,
    op_broadcast_recv<BC>,
  ],
  esm = [ "01_broadcast_channel.js" ],
  options = {
    bc: BC,
  },
  state = |state, options| {
    state.put(options.bc);
  },
);
