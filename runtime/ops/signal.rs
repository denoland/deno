// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;
use deno_core::ZeroCopyBuf;
use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(any(unix, windows))]
use deno_core::error::bad_resource_id;
#[cfg(any(unix, windows))]
use deno_core::AsyncRefCell;
#[cfg(any(unix, windows))]
use deno_core::CancelFuture;
#[cfg(any(unix, windows))]
use deno_core::CancelHandle;
#[cfg(any(unix, windows))]
use deno_core::RcRef;
#[cfg(any(unix, windows))]
use deno_core::Resource;
#[cfg(any(unix, windows))]
use deno_core::ResourceId;
#[cfg(any(unix, windows))]
use std::borrow::Cow;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows::{CtrlC, ctrl_c};

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_sync(rt, "op_signal_bind", op_signal_bind);
  super::reg_sync(rt, "op_signal_unbind", op_signal_unbind);
  super::reg_async(rt, "op_signal_poll", op_signal_poll);
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
struct SignalStreamResource {
  signal: AsyncRefCell<Signal>,
  cancel: CancelHandle,
}

#[cfg(unix)]
impl Resource for SignalStreamResource {
  fn name(&self) -> Cow<str> {
    "signal".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn op_signal_bind(
  state: &mut OpState,
  signo: i32,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  super::check_unstable(state, "Deno.signal");
  let resource = SignalStreamResource {
    signal: AsyncRefCell::new(signal(SignalKind::from_raw(signo)).expect("")),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[cfg(unix)]
async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<bool, AnyError> {
  super::check_unstable2(&state, "Deno.signal");

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SignalStreamResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let mut signal = RcRef::map(&resource, |r| &r.signal).borrow_mut().await;

  match signal.recv().or_cancel(cancel).await {
    Ok(result) => Ok(result.is_none()),
    Err(_) => Ok(true),
  }
}

#[cfg(unix)]
pub fn op_signal_unbind(
  state: &mut OpState,
  rid: ResourceId,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.signal");
  state
    .resource_table
    .close(rid)
    .ok_or_else(bad_resource_id)?;
  Ok(())
}

#[cfg(windows)]
struct SignalStreamResource {
  signal: AsyncRefCell<CtrlC>,
  cancel: CancelHandle,
}

#[cfg(windows)]
impl Resource for SignalStreamResource {
  fn name(&self) -> Cow<str> {
    "ctrlc".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[cfg(windows)]
pub fn op_signal_bind(
    state: &mut OpState,
    signo: i32,
    _zero_copy: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
    // SIGINT
    if signo != 2 {
        unimplemented!()
    }
    super::check_unstable(state, "Deno.signal");
    let resource = SignalStreamResource {
        signal: AsyncRefCell::new(ctrl_c().expect("")),
        cancel: Default::default(),
    };
    let rid = state.resource_table.add(resource);
    Ok(rid)
}

#[cfg(windows)]
async fn op_signal_poll(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
    _zero_copy: Option<ZeroCopyBuf>,
) -> Result<bool, AnyError> {
    super::check_unstable2(&state, "Deno.signal");

    let resource = state
        .borrow_mut()
        .resource_table
        .get::<SignalStreamResource>(rid)
        .ok_or_else(bad_resource_id)?;
    let cancel = RcRef::map(&resource, |r| &r.cancel);
    let mut signal = RcRef::map(&resource, |r| &r.signal).borrow_mut().await;

    match signal.recv().or_cancel(cancel).await {
        Ok(result) => Ok(result.is_none()),
        Err(_) => Ok(true),
    }
}

#[cfg(windows)]
pub fn op_signal_unbind(
    state: &mut OpState,
    rid: ResourceId,
    _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
    super::check_unstable(state, "Deno.signal");
    state
        .resource_table
        .close(rid)
        .ok_or_else(bad_resource_id)?;
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
pub fn op_signal_bind(
  _state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  unimplemented!();
}

#[cfg(all(not(unix), not(windows)))]
fn op_signal_unbind(
  _state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  unimplemented!();
}

#[cfg(all(not(unix), not(windows)))]
async fn op_signal_poll(
  _state: Rc<RefCell<OpState>>,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  unimplemented!();
}
