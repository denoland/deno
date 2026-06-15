// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::v8;
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
  #[class(inherit)]
  #[error(transparent)]
  Serialize(#[inherit] deno_error::JsErrorBox),
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

// A broadcast message is delivered as (name, serialized data, shared array
// buffer stash id). The stash id refers to an entry in `BroadcastSabStash`
// holding the out-of-band `SharedArrayBuffer` backing stores that go with the
// message; it is `0` when the message carries none.
pub type BroadcastChannelMessage = (String, Vec<u8>, u32);

type SharedArrayBuffers = Vec<v8::SharedRef<v8::BackingStore>>;

// A message received from the broadcast channel: (name, serialized data,
// out-of-band SharedArrayBuffer backing stores).
type ReceivedMessage = (String, Vec<u8>, Arc<Mutex<SharedArrayBuffers>>);

/// Per-isolate holding area for the out-of-band `SharedArrayBuffer` backing
/// stores of a broadcast message, between serialization and the (deferred)
/// deserialization by every local channel. A `BroadcastChannel` message can be
/// deserialized an arbitrary number of times (once per receiving channel, in
/// this and other isolates), so the backing stores are carried alongside the
/// message and cloned per receiver rather than taken from a shared store.
#[derive(Default)]
pub(crate) struct BroadcastSabStash {
  // `Mutex` so the stored backing stores are `Send`-able inside the broadcast
  // message; `SharedRef<BackingStore>` is `Send` but not `Sync`.
  map: HashMap<u32, Arc<Mutex<SharedArrayBuffers>>>,
  next_id: u32,
}

impl BroadcastSabStash {
  fn insert(&mut self, sabs: Arc<Mutex<SharedArrayBuffers>>) -> u32 {
    // Ids start at 1; 0 is reserved to mean "no SharedArrayBuffers".
    self.next_id = self.next_id.wrapping_add(1).max(1);
    let id = self.next_id;
    self.map.insert(id, sabs);
    id
  }

  fn get(&self, id: u32) -> Option<Arc<Mutex<SharedArrayBuffers>>> {
    self.map.get(&id).cloned()
  }

  fn remove(&mut self, id: u32) {
    self.map.remove(&id);
  }
}

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

/// Serializes a `BroadcastChannel` message, collecting any `SharedArrayBuffer`
/// backing stores out-of-band so the message can be deserialized by many
/// receivers. Returns `[data, sabId]`, where `sabId` is `0` when the message
/// carries no `SharedArrayBuffer`s and otherwise refers to a `BroadcastSabStash`
/// entry that must be freed with `op_broadcast_free` once dispatched locally.
#[op2(reentrant)]
pub fn op_broadcast_serialize<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: Rc<RefCell<OpState>>,
  value: v8::Local<'a, v8::Value>,
  error_callback: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Value>, BroadcastChannelError> {
  let error_callback = v8::Local::<v8::Function>::try_from(error_callback).ok();
  // Must not hold an `OpState` borrow across this call: serializing a host
  // object (e.g. a Blob) re-enters JS and borrows `OpState` again.
  let (data, sabs) =
    deno_core::serialize_broadcast(scope, value, error_callback)
      .map_err(BroadcastChannelError::Serialize)?;

  let sab_id = if sabs.is_empty() {
    0
  } else {
    state
      .borrow_mut()
      .borrow_mut::<BroadcastSabStash>()
      .insert(Arc::new(Mutex::new(sabs)))
  };

  let data = {
    let store = v8::ArrayBuffer::new_backing_store_from_vec(data).make_shared();
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
    let len = buffer.byte_length();
    v8::Uint8Array::new(scope, buffer, 0, len).unwrap()
  };
  let sab_id = v8::Integer::new_from_unsigned(scope, sab_id);
  Ok(v8::Array::new_with_elements(scope, &[data.into(), sab_id.into()]).into())
}

/// Deserializes a `BroadcastChannel` message produced by
/// `op_broadcast_serialize` / received via `op_broadcast_recv`. `sab_id` refers
/// to a `BroadcastSabStash` entry holding the out-of-band `SharedArrayBuffer`
/// backing stores (or `0` for none); the entry is left in place so the same
/// message can be deserialized again by other channels.
#[op2(reentrant)]
pub fn op_broadcast_deserialize<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: Rc<RefCell<OpState>>,
  #[buffer] data: JsBuffer,
  #[smi] sab_id: u32,
  deserializers: Option<v8::Local<'a, v8::Object>>,
) -> Result<v8::Local<'a, v8::Value>, BroadcastChannelError> {
  // The `OpState` borrow is dropped before deserializing, which may re-enter JS
  // and borrow `OpState` again (e.g. for host objects like Blob).
  let sabs = if sab_id == 0 {
    Vec::new()
  } else {
    state
      .borrow()
      .borrow::<BroadcastSabStash>()
      .get(sab_id)
      .map(|sabs| sabs.lock().clone())
      .unwrap_or_default()
  };
  deno_core::deserialize_broadcast(scope, &data, sabs, deserializers)
    .map_err(BroadcastChannelError::Serialize)
}

/// Frees a `BroadcastSabStash` entry once the message has been dispatched to
/// every channel in this isolate.
#[op2(fast)]
pub fn op_broadcast_free(state: &mut OpState, #[smi] sab_id: u32) {
  if sab_id != 0 {
    state.borrow_mut::<BroadcastSabStash>().remove(sab_id);
  }
}

#[op2]
pub fn op_broadcast_send(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] name: String,
  #[buffer] buf: JsBuffer,
  #[smi] sab_id: u32,
) -> Result<(), BroadcastChannelError> {
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<InMemoryBroadcastChannelResource>(rid)?;
  // Move a clone of the out-of-band SharedArrayBuffers into the message so
  // receivers in other isolates can rebuild them.
  let sabs = if sab_id == 0 {
    Arc::new(Mutex::new(Vec::new()))
  } else {
    let stash = state.borrow::<BroadcastSabStash>();
    stash
      .get(sab_id)
      .map(|sabs| Arc::new(Mutex::new(sabs.lock().clone())))
      .unwrap_or_default()
  };
  let bc = state.borrow::<InMemoryBroadcastChannel>().clone();
  bc.send(&resource, name, buf.to_vec(), sabs)
}

#[op2]
pub async fn op_broadcast_recv(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<BroadcastChannelMessage>, BroadcastChannelError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<InMemoryBroadcastChannelResource>(rid)?;
  let bc = state.borrow().borrow::<InMemoryBroadcastChannel>().clone();
  let Some((name, data, sabs)) = bc.recv(&resource).await? else {
    return Ok(None);
  };
  // Stash the message's SharedArrayBuffers in this isolate so the deferred
  // deserialization by each local channel can rebuild them. `0` => none.
  let sab_id = if sabs.lock().is_empty() {
    0
  } else {
    state
      .borrow_mut()
      .borrow_mut::<BroadcastSabStash>()
      .insert(sabs)
  };
  Ok(Some((name, data, sab_id)))
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

#[derive(Clone)]
struct InMemoryChannelMessage {
  name: Arc<String>,
  data: Arc<Vec<u8>>,
  uuid: Uuid,
  // Out-of-band `SharedArrayBuffer` backing stores carried with the message, so
  // every receiver can rebuild its own `SharedArrayBuffer`s. Empty when the
  // message carries none.
  sabs: Arc<Mutex<SharedArrayBuffers>>,
}

impl std::fmt::Debug for InMemoryChannelMessage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // `SharedRef<BackingStore>` is not `Debug`; report the count instead.
    f.debug_struct("InMemoryChannelMessage")
      .field("name", &self.name)
      .field("data", &self.data)
      .field("uuid", &self.uuid)
      .field("sabs", &self.sabs.lock().len())
      .finish()
  }
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
    sabs: Arc<Mutex<SharedArrayBuffers>>,
  ) -> Result<(), BroadcastChannelError> {
    let name = Arc::new(name);
    let data = Arc::new(data);
    let uuid = resource.uuid;
    self.0.lock().send(InMemoryChannelMessage {
      name,
      data,
      uuid,
      sabs,
    })?;
    Ok(())
  }

  async fn recv(
    &self,
    resource: &InMemoryBroadcastChannelResource,
  ) -> Result<Option<ReceivedMessage>, BroadcastChannelError> {
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
          // Each receiver gets its own clones of the backing stores.
          let sabs = Arc::new(Mutex::new(message.sabs.lock().clone()));
          return Ok(Some((name, data, sabs)));
        }
      }
    }
  }
}
