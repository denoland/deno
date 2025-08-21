// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::AsyncRefCell;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::error::ResourceError;
use deno_core::op2;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SignalError {
  #[class(type)]
  #[error(transparent)]
  InvalidSignalStr(#[from] deno_signals::InvalidSignalStrError),
  #[class(type)]
  #[error(transparent)]
  InvalidSignalInt(#[from] deno_signals::InvalidSignalIntError),
  #[class(type)]
  #[error("Binding to signal '{0}' is not allowed")]
  SignalNotAllowed(String),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

struct SignalStreamResource {
  signo: i32,
  id: u32,
  rx: AsyncRefCell<tokio::sync::watch::Receiver<()>>,
}

impl Resource for SignalStreamResource {
  fn name(&self) -> Cow<'_, str> {
    "signal".into()
  }

  fn close(self: Rc<Self>) {
    deno_signals::unregister(self.signo, self.id);
  }
}

#[op2(fast)]
#[smi]
pub fn op_signal_bind(
  state: &mut OpState,
  #[string] sig: &str,
) -> Result<ResourceId, SignalError> {
  let signo = deno_signals::signal_str_to_int(sig)?;
  if deno_signals::is_forbidden(signo) {
    return Err(SignalError::SignalNotAllowed(sig.to_string()));
  }

  let (tx, rx) = tokio::sync::watch::channel(());
  let id = deno_signals::register(
    signo,
    true,
    Box::new(move || {
      let _ = tx.send(());
    }),
  )?;

  let rid = state.resource_table.add(SignalStreamResource {
    signo,
    id,
    rx: AsyncRefCell::new(rx),
  });

  Ok(rid)
}

#[op2(async)]
pub async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<bool, ResourceError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SignalStreamResource>(rid)?;

  let mut rx = RcRef::map(&resource, |r| &r.rx).borrow_mut().await;

  Ok(rx.changed().await.is_err())
}

#[op2(fast)]
pub fn op_signal_unbind(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), ResourceError> {
  let resource = state.resource_table.take::<SignalStreamResource>(rid)?;
  resource.close();
  Ok(())
}
