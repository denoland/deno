use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;

use deno_core::DetachedBuffer;
use deno_core::{CancelFuture, Resource};
use deno_core::{CancelHandle, OpState};
use deno_core::{RcRef, ResourceId};
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

enum Transferable {
  MessagePort(MessagePort),
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
  ) -> Result<(), AnyError> {
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
  ) -> Result<Option<JsMessageData>, AnyError> {
    #![allow(clippy::await_holding_refcell_ref)] // TODO(ry) remove!
    let mut rx = self
      .rx
      .try_borrow_mut()
      .map_err(|_| type_error("Port receiver is already borrowed"))?;
    if let Some((data, transferables)) = rx.recv().await {
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
  fn name(&self) -> Cow<str> {
    "messagePort".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[op]
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
  #[serde(rename_all = "camelCase")]
  MessagePort(ResourceId),
  ArrayBuffer(u32),
}

fn deserialize_js_transferables(
  state: &mut OpState,
  js_transferables: Vec<JsTransferable>,
) -> Result<Vec<Transferable>, AnyError> {
  let mut transferables = Vec::with_capacity(js_transferables.len());
  for js_transferable in js_transferables {
    match js_transferable {
      JsTransferable::MessagePort(id) => {
        let resource = state
          .resource_table
          .take::<MessagePortResource>(id)
          .map_err(|_| type_error("Invalid message port transfer"))?;
        resource.cancel.cancel();
        let resource = Rc::try_unwrap(resource)
          .map_err(|_| type_error("Message port is not ready for transfer"))?;
        transferables.push(Transferable::MessagePort(resource.port));
      }
      JsTransferable::ArrayBuffer(id) => {
        transferables.push(Transferable::ArrayBuffer(id));
      }
    }
  }
  Ok(transferables)
}

fn serialize_transferables(
  state: &mut OpState,
  transferables: Vec<Transferable>,
) -> Vec<JsTransferable> {
  let mut js_transferables = Vec::with_capacity(transferables.len());
  for transferable in transferables {
    match transferable {
      Transferable::MessagePort(port) => {
        let rid = state.resource_table.add(MessagePortResource {
          port,
          cancel: CancelHandle::new(),
        });
        js_transferables.push(JsTransferable::MessagePort(rid));
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
  data: DetachedBuffer,
  transferables: Vec<JsTransferable>,
}

#[op]
pub fn op_message_port_post_message(
  state: &mut OpState,
  rid: ResourceId,
  data: JsMessageData,
) -> Result<(), AnyError> {
  for js_transferable in &data.transferables {
    if let JsTransferable::MessagePort(id) = js_transferable {
      if *id == rid {
        return Err(type_error("Can not transfer self message port"));
      }
    }
  }

  let resource = state.resource_table.get::<MessagePortResource>(rid)?;

  resource.port.send(state, data)
}

#[op]
pub async fn op_message_port_recv_message(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<Option<JsMessageData>, AnyError> {
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
