// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::DetachedBuffer;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::TransferredResource;
use deno_core::op2;
use deno_error::JsErrorBox;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::unbounded_channel;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MessagePortError {
  #[class(type)]
  #[error("Invalid message port transfer")]
  InvalidTransfer,
  #[class(type)]
  #[error("Message port is not ready for transfer")]
  NotReady,
  #[class(type)]
  #[error("Can not transfer self message port")]
  TransferSelf,
  #[class(inherit)]
  #[error(transparent)]
  Canceled(#[from] deno_core::Canceled),
  #[class(inherit)]
  #[error(transparent)]
  Resource(deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Generic(JsErrorBox),
}

pub enum Transferable {
  Resource(String, Box<dyn TransferredResource>),
  MultiResource(String, Vec<Box<dyn TransferredResource>>),
  ArrayBuffer(u32),
}

type MessagePortMessage = (DetachedBuffer, Vec<Transferable>);

pub struct MessagePort {
  pub rx: RefCell<UnboundedReceiver<MessagePortMessage>>,
  pub tx: RefCell<Option<UnboundedSender<MessagePortMessage>>>,
}

impl MessagePort {
  pub fn send(
    &self,
    state: &mut OpState,
    data: JsMessageData,
  ) -> Result<(), MessagePortError> {
    let transferables = if data.transferables.is_empty() {
      vec![]
    } else {
      deserialize_js_transferables(state, data.transferables)?
    };

    // Swallow the failed to send error. It means the channel was disentangled,
    // but not cleaned up.
    if let Some(tx) = &*self.tx.borrow() {
      tx.send((data.data, transferables)).ok();
    }

    Ok(())
  }

  pub async fn recv(
    &self,
    state: Rc<RefCell<OpState>>,
  ) -> Result<Option<RecvMessageData>, MessagePortError> {
    let rx = &self.rx;

    let maybe_data = poll_fn(|cx| {
      let mut rx = rx.borrow_mut();
      rx.poll_recv(cx)
    })
    .await;

    if let Some((data, transferables)) = maybe_data {
      // Fast path: no transferables -> hand the buffer to JS directly.
      if transferables.is_empty() {
        return Ok(Some(RecvMessageData::Raw(data)));
      }
      let js_transferables =
        serialize_transferables(&mut state.borrow_mut(), transferables);
      return Ok(Some(RecvMessageData::Full(JsMessageData {
        data,
        transferables: js_transferables,
      })));
    }
    Ok(None)
  }

  /// Try to receive a message synchronously without blocking.
  /// Returns `Ok(None)` if no message is available or the channel is closed.
  ///
  /// Unlike the async `recv`, this keeps returning the full `JsMessageData`
  /// object: the sync path is the batch-drain used by fire-and-forget floods,
  /// which is not latency-bound and showed no benefit from the raw fast path.
  pub fn try_recv_sync(
    &self,
    state: &mut OpState,
  ) -> Result<Option<JsMessageData>, MessagePortError> {
    let mut rx = self.rx.borrow_mut();
    match rx.try_recv() {
      Ok((data, transferables)) => {
        let js_transferables = if transferables.is_empty() {
          vec![]
        } else {
          serialize_transferables(state, transferables)
        };
        Ok(Some(JsMessageData {
          data,
          transferables: js_transferables,
        }))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Ok(None),
    }
  }

  /// This forcefully disconnects the message port from its paired port. This
  /// will wake up the `.recv` on the paired port, which will return `Ok(None)`.
  pub fn disentangle(&self) {
    let mut tx = self.tx.borrow_mut();
    tx.take();
  }
}

pub fn create_entangled_message_port() -> (MessagePort, MessagePort) {
  let (port1_tx, port2_rx) = unbounded_channel::<MessagePortMessage>();
  let (port2_tx, port1_rx) = unbounded_channel::<MessagePortMessage>();

  let port1 = MessagePort {
    rx: RefCell::new(port1_rx),
    tx: RefCell::new(Some(port1_tx)),
  };

  let port2 = MessagePort {
    rx: RefCell::new(port2_rx),
    tx: RefCell::new(Some(port2_tx)),
  };

  (port1, port2)
}

pub struct MessagePortResource {
  port: MessagePort,
  cancel: CancelHandle,
}

impl Resource for MessagePortResource {
  fn name(&self) -> Cow<'_, str> {
    "messagePort".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }

  fn transfer(
    self: Rc<Self>,
  ) -> Result<Box<dyn TransferredResource>, JsErrorBox> {
    self.cancel.cancel();
    let resource = Rc::try_unwrap(self)
      .map_err(|_| JsErrorBox::from_err(MessagePortError::NotReady))?;
    Ok(Box::new(resource.port))
  }
}

impl TransferredResource for MessagePort {
  fn receive(self: Box<Self>) -> Rc<dyn Resource> {
    Rc::new(MessagePortResource {
      port: *self,
      cancel: CancelHandle::new(),
    })
  }
}

#[op2]
pub fn op_message_port_create_entangled(
  state: &mut OpState,
) -> (ResourceId, ResourceId) {
  let (port1, port2) = create_entangled_message_port();

  let port1_id = state.resource_table.add(MessagePortResource {
    port: port1,
    cancel: CancelHandle::new(),
  });

  let port2_id = state.resource_table.add(MessagePortResource {
    port: port2,
    cancel: CancelHandle::new(),
  });

  (port1_id, port2_id)
}

#[derive(Deserialize, Serialize, deno_core::ToV8)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
#[to_v8(tag = "kind", content = "data")]
pub enum JsTransferable {
  #[to_v8(rename = "arrayBuffer")]
  ArrayBuffer(u32),
  #[to_v8(rename = "resource")]
  Resource(String, ResourceId),
  #[to_v8(rename = "multiResource")]
  MultiResource(String, Vec<ResourceId>),
}

pub fn deserialize_js_transferables(
  state: &mut OpState,
  js_transferables: Vec<JsTransferable>,
) -> Result<Vec<Transferable>, MessagePortError> {
  let mut transferables = Vec::with_capacity(js_transferables.len());
  for js_transferable in js_transferables {
    match js_transferable {
      JsTransferable::Resource(name, rid) => {
        let resource = state
          .resource_table
          .take_any(rid)
          .map_err(|_| MessagePortError::InvalidTransfer)?;
        let tx = resource.transfer().map_err(MessagePortError::Generic)?;
        transferables.push(Transferable::Resource(name, tx));
      }
      JsTransferable::MultiResource(name, rids) => {
        let mut txs = Vec::with_capacity(rids.len());
        for rid in rids {
          let resource = state
            .resource_table
            .take_any(rid)
            .map_err(|_| MessagePortError::InvalidTransfer)?;
          let tx = resource.transfer().map_err(MessagePortError::Generic)?;
          txs.push(tx);
        }
        transferables.push(Transferable::MultiResource(name, txs));
      }
      JsTransferable::ArrayBuffer(id) => {
        transferables.push(Transferable::ArrayBuffer(id));
      }
    }
  }
  Ok(transferables)
}

pub fn serialize_transferables(
  state: &mut OpState,
  transferables: Vec<Transferable>,
) -> Vec<JsTransferable> {
  let mut js_transferables = Vec::with_capacity(transferables.len());
  for transferable in transferables {
    match transferable {
      Transferable::Resource(name, tx) => {
        let rx = tx.receive();
        let rid = state.resource_table.add_rc_dyn(rx);
        js_transferables.push(JsTransferable::Resource(name, rid));
      }
      Transferable::MultiResource(name, txs) => {
        let rids = txs
          .into_iter()
          .map(|tx| state.resource_table.add_rc_dyn(tx.receive()))
          .collect();
        js_transferables.push(JsTransferable::MultiResource(name, rids));
      }
      Transferable::ArrayBuffer(id) => {
        js_transferables.push(JsTransferable::ArrayBuffer(id));
      }
    }
  }
  js_transferables
}

// `JsMessageData` is returned from the message-receive ops once per delivered
// message, on the hottest worker path. Deriving `ToV8` builds the result object
// directly (interned `data`/`transferables` keys + a single
// `Object::with_prototype_and_properties` call) instead of routing the whole
// struct through serde_v8's `Serializer`, which rebuilt an object per message.
// `data` and `transferables` now have their own `deno_core::convert::ToV8`
// impls (a hand-written one for `DetachedBuffer`, a derived one for
// `JsTransferable`), so they no longer need the `#[to_v8(serde)]` escape
// hatch either. `Deserialize` is kept for the post-message ops (which take
// `JsMessageData` as an input argument) and `Serialize` for the one-shot
// worker-metadata bootstrap in `web_worker.rs`.
#[derive(Deserialize, Serialize, deno_core::ToV8)]
pub struct JsMessageData {
  pub data: DetachedBuffer,
  pub transferables: Vec<JsTransferable>,
}

// Returned from the message-receive ops. The overwhelmingly common case is a
// message with no transferables, which is handed to JS as the serialized buffer
// *directly* -- skipping the per-message `{ data, transferables }` object (and
// its empty transferables array) that `JsMessageData` would otherwise allocate
// on the hottest worker path. This mirrors the send side, where
// `op_*_post_message_raw` already bypasses the `JsMessageData` envelope for the
// no-transferables case. When a message does carry transferables it falls back
// to the full `JsMessageData` object; JS disambiguates by checking whether the
// received value is a typed array (`Raw`) or an object (`Full`).
pub enum RecvMessageData {
  Raw(DetachedBuffer),
  Full(JsMessageData),
}

impl<'a> deno_core::ToV8<'a> for RecvMessageData {
  type Error = JsErrorBox;

  fn to_v8<'i>(
    self,
    scope: &mut deno_core::v8::PinScope<'a, 'i>,
  ) -> Result<deno_core::v8::Local<'a, deno_core::v8::Value>, Self::Error> {
    match self {
      RecvMessageData::Raw(buffer) => {
        // Matches what `JsMessageData::data` yields: a `Uint8Array` view
        // over the same backing store, via `ToV8 for DetachedBuffer`.
        Ok(buffer.to_v8(scope).unwrap())
      }
      RecvMessageData::Full(data) => data
        .to_v8(scope)
        .map_err(|e| JsErrorBox::generic(e.to_string())),
    }
  }
}

impl From<JsMessageData> for RecvMessageData {
  fn from(data: JsMessageData) -> Self {
    if data.transferables.is_empty() {
      RecvMessageData::Raw(data.data)
    } else {
      RecvMessageData::Full(data)
    }
  }
}

#[op2]
pub fn op_message_port_post_message(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[serde] data: JsMessageData,
) -> Result<(), MessagePortError> {
  for js_transferable in &data.transferables {
    if let JsTransferable::Resource(_name, id) = js_transferable
      && *id == rid
    {
      return Err(MessagePortError::TransferSelf);
    }
  }

  let resource = state
    .resource_table
    .get::<MessagePortResource>(rid)
    .map_err(MessagePortError::Resource)?;
  resource.port.send(state, data)
}

#[op2]
pub async fn op_message_port_recv_message(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<RecvMessageData>, MessagePortError> {
  let resource = {
    let state = state.borrow();
    match state.resource_table.get::<MessagePortResource>(rid) {
      Ok(resource) => resource,
      Err(_) => return Ok(None),
    }
  };
  let cancel = RcRef::map(resource.clone(), |r| &r.cancel);
  resource.port.recv(state).or_cancel(cancel).await?
}

#[op2]
pub fn op_message_port_recv_message_sync(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let resource = state
    .resource_table
    .get::<MessagePortResource>(rid)
    .map_err(MessagePortError::Resource)?;
  resource.port.try_recv_sync(state)
}

/// Fast-path post: takes a pre-serialized buffer directly, bypassing
/// the JsMessageData serde overhead. Only for messages with no transferables.
#[op2]
pub fn op_message_port_post_message_raw(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[buffer(detach)] data: JsBuffer,
) -> Result<(), MessagePortError> {
  let resource = state
    .resource_table
    .get::<MessagePortResource>(rid)
    .map_err(MessagePortError::Resource)?;
  let detached = DetachedBuffer::from_v8slice(data.into_parts());
  if let Some(tx) = &*resource.port.tx.borrow() {
    tx.send((detached, vec![])).ok();
  }
  Ok(())
}
