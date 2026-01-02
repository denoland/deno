// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::DetachedBuffer;
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
  rx: RefCell<UnboundedReceiver<MessagePortMessage>>,
  tx: RefCell<Option<UnboundedSender<MessagePortMessage>>>,
}

impl MessagePort {
  pub fn send(
    &self,
    state: &mut OpState,
    data: JsMessageData,
  ) -> Result<(), MessagePortError> {
    let transferables =
      deserialize_js_transferables(state, data.transferables)?;

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
  ) -> Result<Option<JsMessageData>, MessagePortError> {
    let rx = &self.rx;

    let maybe_data = poll_fn(|cx| {
      let mut rx = rx.borrow_mut();
      rx.poll_recv(cx)
    })
    .await;

    if let Some((data, transferables)) = maybe_data {
      let js_transferables =
        serialize_transferables(&mut state.borrow_mut(), transferables);
      return Ok(Some(JsMessageData {
        data,
        transferables: js_transferables,
      }));
    }
    Ok(None)
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
#[serde]
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

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum JsTransferable {
  ArrayBuffer(u32),
  Resource(String, ResourceId),
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

#[derive(Deserialize, Serialize)]
pub struct JsMessageData {
  pub data: DetachedBuffer,
  pub transferables: Vec<JsTransferable>,
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

#[op2(async)]
#[serde]
pub async fn op_message_port_recv_message(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<JsMessageData>, MessagePortError> {
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
#[serde]
pub fn op_message_port_recv_message_sync(
  state: &mut OpState, // Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<JsMessageData>, MessagePortError> {
  let resource = state
    .resource_table
    .get::<MessagePortResource>(rid)
    .map_err(MessagePortError::Resource)?;
  let mut rx = resource.port.rx.borrow_mut();

  match rx.try_recv() {
    Ok((d, t)) => Ok(Some(JsMessageData {
      data: d,
      transferables: serialize_transferables(state, t),
    })),
    Err(TryRecvError::Empty) => Ok(None),
    Err(TryRecvError::Disconnected) => Ok(None),
  }
}
