// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

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
use deno_core::v8;
use deno_error::JsErrorBox;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Notify;
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
  // Set once the receiving side has been observed closed (the paired sender was
  // dropped and `rx` returned `None`). Used by the Rust-driven dispatch path:
  // the message-pump drains `rx` directly and, on close, flips this flag and
  // wakes any keep-alive recv op waiting in `closed()` so it can resolve.
  closed_flag: Cell<bool>,
  close_notify: Notify,
}

impl MessagePort {
  /// Non-blocking drain used by the Rust-driven dispatch pump. Receives up to
  /// `max` already-queued messages into `out`, registering `cx`'s waker so the
  /// event loop is re-polled when a new message arrives. Returns `true` if the
  /// channel has closed (paired sender dropped).
  pub fn poll_drain(
    &self,
    cx: &mut Context,
    max: usize,
    out: &mut Vec<MessagePortMessage>,
  ) -> bool {
    let mut rx = self.rx.borrow_mut();
    for _ in 0..max {
      match rx.poll_recv(cx) {
        Poll::Ready(Some(msg)) => out.push(msg),
        Poll::Ready(None) => return true,
        Poll::Pending => return false,
      }
    }
    // Hit the per-poll drain cap with possibly more queued: wake again so the
    // event loop re-polls and keeps draining, without starving other work.
    cx.waker().wake_by_ref();
    false
  }

  /// Resolves once the channel is observed closed via [`MessagePort::mark_closed`].
  /// Used as the keep-alive anchor for the Rust-driven dispatch recv ops.
  pub async fn closed(&self) {
    let notified = self.close_notify.notified();
    let mut notified = std::pin::pin!(notified);
    // Register interest before checking the flag so a `mark_closed` racing in
    // between cannot be missed.
    notified.as_mut().enable();
    if self.closed_flag.get() {
      return;
    }
    notified.await;
  }

  /// Marks the channel closed and wakes anything waiting in [`MessagePort::closed`].
  pub fn mark_closed(&self) {
    self.closed_flag.set(true);
    self.close_notify.notify_waiters();
  }

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
  ) -> Result<Option<JsMessageData>, MessagePortError> {
    let rx = &self.rx;

    let maybe_data = poll_fn(|cx| {
      let mut rx = rx.borrow_mut();
      rx.poll_recv(cx)
    })
    .await;

    if let Some((data, transferables)) = maybe_data {
      let js_transferables = if transferables.is_empty() {
        vec![]
      } else {
        serialize_transferables(&mut state.borrow_mut(), transferables)
      };
      return Ok(Some(JsMessageData {
        data,
        transferables: js_transferables,
      }));
    }
    Ok(None)
  }

  /// Try to receive a message synchronously without blocking.
  /// Returns `Ok(None)` if no message is available or the channel is closed.
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
    closed_flag: Cell::new(false),
    close_notify: Notify::new(),
  };

  let port2 = MessagePort {
    rx: RefCell::new(port2_rx),
    tx: RefCell::new(Some(port2_tx)),
    closed_flag: Cell::new(false),
    close_notify: Notify::new(),
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

#[op2]
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

// ---- Rust-driven message dispatch -------------------------------------------
//
// In a latency-bound worker pattern (request/response, ping-pong) the steady
// state is "one message arrives, dispatch it, wait for the next". The classic
// JS receive loop pays a fresh async op + JS Promise + microtask checkpoint per
// message for that. Instead, a receive loop can register its port and a
// dispatcher function here; the worker event loop (see
// `deno_runtime::message_dispatch`) then drains the port's queue directly and
// invokes the dispatcher from Rust, with no per-message Promise.
//
// A still-pending recv op (now resolving only on channel close) remains the
// keep-alive / ref-unref anchor, so worker lifecycle semantics are unchanged.

pub struct MessageDispatchSource {
  pub port: Rc<MessagePort>,
  pub dispatcher: v8::Global<v8::Function>,
}

/// Registry of ports whose message delivery is driven from the Rust event loop.
/// Stored in `OpState`; consulted each event-loop iteration by the worker
/// message pump.
#[derive(Default)]
pub struct MessageDispatchTable {
  // Slots are reused on unregister to keep ids stable and small.
  sources: Vec<Option<MessageDispatchSource>>,
}

impl MessageDispatchTable {
  pub fn register(
    &mut self,
    port: Rc<MessagePort>,
    dispatcher: v8::Global<v8::Function>,
  ) -> u32 {
    let source = MessageDispatchSource { port, dispatcher };
    for (i, slot) in self.sources.iter_mut().enumerate() {
      if slot.is_none() {
        *slot = Some(source);
        return i as u32;
      }
    }
    self.sources.push(Some(source));
    (self.sources.len() - 1) as u32
  }

  pub fn unregister(&mut self, id: u32) {
    if let Some(slot) = self.sources.get_mut(id as usize) {
      *slot = None;
    }
  }

  /// Snapshot of currently-registered sources, cloning the cheap handles so the
  /// pump can drop its borrow of `OpState` before re-entering JS.
  pub fn snapshot(
    &self,
  ) -> Vec<(u32, Rc<MessagePort>, v8::Global<v8::Function>)> {
    self
      .sources
      .iter()
      .enumerate()
      .filter_map(|(i, slot)| {
        slot
          .as_ref()
          .map(|s| (i as u32, s.port.clone(), s.dispatcher.clone()))
      })
      .collect()
  }
}

/// Register a port + dispatcher for Rust-driven message delivery.
pub fn register_message_dispatch(
  state: &mut OpState,
  port: Rc<MessagePort>,
  dispatcher: v8::Global<v8::Function>,
) -> u32 {
  if state.try_borrow::<MessageDispatchTable>().is_none() {
    state.put(MessageDispatchTable::default());
  }
  state
    .borrow_mut::<MessageDispatchTable>()
    .register(port, dispatcher)
}

#[op2(fast)]
pub fn op_message_dispatch_unregister(state: &mut OpState, #[smi] id: u32) {
  if let Some(table) = state.try_borrow_mut::<MessageDispatchTable>() {
    table.unregister(id);
  }
}
