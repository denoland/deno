// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Error;
use deno_core::error::generic_error;
use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::InspectorMsg;
use deno_core::InspectorSessionKind;
use deno_core::JsRuntimeInspector;
use deno_core::LocalInspectorSessionOptions;
use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;

#[op2(fast)]
pub fn op_inspector_open() {
  // TODO: hook up to InspectorServer
}

#[op2(fast)]
pub fn op_inspector_close() {
  // TODO: hook up to InspectorServer
}

#[op2]
#[string]
pub fn op_inspector_url() -> Option<String> {
  // TODO: hook up to InspectorServer
  None
}

#[op2(fast)]
pub fn op_inspector_wait(state: &OpState) -> bool {
  match state.try_borrow::<Rc<RefCell<JsRuntimeInspector>>>() {
    Some(inspector) => {
      inspector
        .borrow_mut()
        .wait_for_session_and_break_on_next_statement();
      true
    }
    None => false,
  }
}

#[op2(fast)]
pub fn op_inspector_emit_protocol_event(
  #[string] _event_name: String,
  #[string] _params: String,
) {
  // TODO: inspector channel & protocol notifications
}

struct JSInspectorSession {
  tx: RefCell<Option<mpsc::UnboundedSender<String>>>,
  rx: RefCell<mpsc::UnboundedReceiver<InspectorMsg>>,
}

impl GarbageCollected for JSInspectorSession {}

#[op2]
#[cppgc]
pub fn op_inspector_connect(
  state: &mut OpState,
  connect_to_main_thread: bool,
) -> Result<JSInspectorSession, Error> {
  if connect_to_main_thread {
    return Err(generic_error("connectToMainThread not supported"));
  }

  let inspector = state
    .borrow::<Rc<RefCell<JsRuntimeInspector>>>()
    .borrow_mut();
  let session = inspector.create_local_session(LocalInspectorSessionOptions {
    kind: InspectorSessionKind::NonBlocking,
  });
  let (tx, rx) = session.split();

  Ok(JSInspectorSession {
    tx: RefCell::new(Some(tx)),
    rx: RefCell::new(rx),
  })
}

#[op2(fast)]
pub fn op_inspector_dispatch(
  #[cppgc] session: &JSInspectorSession,
  #[string] message: String,
) {
  if let Some(tx) = &*session.tx.borrow() {
    let _ = tx.unbounded_send(message);
  }
}

#[allow(clippy::await_holding_refcell_ref)]
#[op2(async)]
#[string]
pub async fn op_inspector_receive(
  #[cppgc] session: &JSInspectorSession,
) -> Option<String> {
  session.rx.borrow_mut().next().await.map(|m| m.content)
}

#[op2(fast)]
pub fn op_inspector_disconnect(#[cppgc] session: &JSInspectorSession) {
  drop(session.tx.borrow_mut().take());
}
