// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! This module helps deno implement timers.
//!
//! As an optimization, we want to avoid an expensive calls into rust for every
//! setTimeout in JavaScript. Thus in //js/timers.ts a data structure is
//! implemented that calls into Rust for only the smallest timeout.  Thus we
//! only need to be able to start, cancel and await a single timer (or Delay, as Tokio
//! calls it) for an entire Isolate. This is what is implemented here.

use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::channel::oneshot;
use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

pub type StartTime = Instant;

type TimerFuture = Pin<Box<dyn Future<Output = Result<(), ()>>>>;

#[derive(Default)]
pub struct GlobalTimer {
  tx: Option<oneshot::Sender<()>>,
  pub future: Option<TimerFuture>,
}

impl GlobalTimer {
  pub fn cancel(&mut self) {
    if let Some(tx) = self.tx.take() {
      tx.send(()).ok();
    }
  }

  pub fn new_timeout(&mut self, deadline: Instant) {
    if self.tx.is_some() {
      self.cancel();
    }
    assert!(self.tx.is_none());
    self.future.take();

    let (tx, rx) = oneshot::channel();
    self.tx = Some(tx);

    let delay = tokio::time::delay_until(deadline.into());
    let rx = rx
      .map_err(|err| panic!("Unexpected error in receiving channel {:?}", err));

    let fut = futures::future::select(delay, rx)
      .then(|_| futures::future::ok(()))
      .boxed_local();
    self.future = Some(fut);
  }
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<GlobalTimer>(GlobalTimer::default());
    state.put::<StartTime>(StartTime::now());
  }
  super::reg_json_sync(rt, "op_global_timer_stop", op_global_timer_stop);
  super::reg_json_sync(rt, "op_global_timer_start", op_global_timer_start);
  super::reg_json_async(rt, "op_global_timer", op_global_timer);
  super::reg_json_sync(rt, "op_now", op_now);
  super::reg_json_sync(rt, "op_sleep_sync", op_sleep_sync);
}

fn op_global_timer_stop(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let global_timer = state.borrow_mut::<GlobalTimer>();
  global_timer.cancel();
  Ok(json!({}))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

// Set up a timer that will be later awaited by JS promise.
// It's a separate op, because canceling a timeout immediately
// after setting it caused a race condition (because Tokio timeout)
// might have been registered after next event loop tick.
//
// See https://github.com/denoland/deno/issues/7599 for more
// details.
fn op_global_timer_start(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let deadline = Instant::now() + Duration::from_millis(val);
  let global_timer = state.borrow_mut::<GlobalTimer>();
  global_timer.new_timeout(deadline);
  Ok(json!({}))
}

async fn op_global_timer(
  state: Rc<RefCell<OpState>>,
  _args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let maybe_timer_fut = {
    let mut s = state.borrow_mut();
    let global_timer = s.borrow_mut::<GlobalTimer>();
    global_timer.future.take()
  };
  if let Some(timer_fut) = maybe_timer_fut {
    let _ = timer_fut.await;
  }
  Ok(json!({}))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let start_time = state.borrow::<StartTime>();
  let seconds = start_time.elapsed().as_secs();
  let mut subsec_nanos = start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if state.borrow::<Permissions>().check_hrtime().is_err() {
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }

  Ok(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  }))
}

#[derive(Deserialize)]
struct SleepArgs {
  millis: u64,
}

fn op_sleep_sync(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  super::check_unstable(state, "Deno.sleepSync");
  let args: SleepArgs = serde_json::from_value(args)?;
  sleep(Duration::from_millis(args.millis));
  Ok(json!({}))
}
