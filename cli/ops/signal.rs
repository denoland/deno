// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(unix)]
use deno_core::error::bad_resource_id;
#[cfg(unix)]
use deno_core::serde_json;
#[cfg(unix)]
use deno_core::serde_json::json;
#[cfg(unix)]
use deno_core::AsyncMutFuture;
#[cfg(unix)]
use deno_core::AsyncRefCell;
#[cfg(unix)]
use deno_core::RcRef;
#[cfg(unix)]
use deno_core::Resource;
#[cfg(unix)]
use serde::Deserialize;
#[cfg(unix)]
use std::borrow::Cow;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_signal_bind", op_signal_bind);
  super::reg_json_sync(rt, "op_signal_unbind", op_signal_unbind);
  super::reg_json_async(rt, "op_signal_poll", op_signal_poll);
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
struct SignalStreamResource(AsyncRefCell<Signal>);

#[cfg(unix)]
impl Resource for SignalStreamResource {
  fn name(&self) -> Cow<str> {
    "signal".into()
  }
}

#[cfg(unix)]
impl SignalStreamResource {
  fn borrow_mut(self: Rc<Self>) -> AsyncMutFuture<Signal> {
    RcRef::map(self, |r| &r.0).borrow_mut()
  }
}

#[cfg(unix)]
#[derive(Deserialize)]
struct BindSignalArgs {
  signo: i32,
}

#[cfg(unix)]
#[derive(Deserialize)]
struct SignalArgs {
  rid: i32,
}

#[cfg(unix)]
fn op_signal_bind(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.signal");
  let args: BindSignalArgs = serde_json::from_value(args)?;
  let resource = SignalStreamResource(AsyncRefCell::new(
    signal(SignalKind::from_raw(args.signo)).expect(""),
  ));
  let rid = state.resource_table_2.add(resource);
  Ok(json!({
    "rid": rid,
  }))
}

#[cfg(unix)]
async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  super::check_unstable2(&state, "Deno.signal");
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  let resource = state
    .borrow_mut()
    .resource_table_2
    .get::<SignalStreamResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let result = resource.borrow_mut().await.recv().await;
  Ok(json!({ "done": result.is_none() }))
}

#[cfg(unix)]
pub fn op_signal_unbind(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.signal");
  let args: SignalArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  state
    .resource_table_2
    .close(rid)
    .ok_or_else(bad_resource_id)?;
  Ok(json!({}))
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  unimplemented!();
}

#[cfg(not(unix))]
async fn op_signal_poll(
  _state: Rc<RefCell<OpState>>,
  _args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  unimplemented!();
}
