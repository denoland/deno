// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! This module helps deno implement timers.
//!
//! As an optimization, we want to avoid an expensive calls into rust for every
//! setTimeout in JavaScript. Thus in //js/timers.ts a data structure is
//! implemented that calls into Rust for only the smallest timeout.  Thus we
//! only need to be able to start, cancel and await a single timer (or Delay, as Tokio
//! calls it) for an entire Isolate. This is what is implemented here.

use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::channel::oneshot;
use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

pub trait TimersPermission {
  fn allow_hrtime(&mut self) -> bool;
  fn check_unstable(&self, state: &OpState, api_name: &'static str);
}

pub fn init(rt: &mut deno_core::JsRuntime) {
  rt.execute(
    "deno:op_crates/timers/01_timers.js",
    include_str!("01_timers.js"),
  )
  .unwrap();
}

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

    let delay = tokio::time::sleep_until(deadline.into()).boxed_local();
    let rx = rx
      .map_err(|err| panic!("Unexpected error in receiving channel {:?}", err));

    let fut = futures::future::select(delay, rx)
      .then(|_| futures::future::ok(()))
      .boxed_local();
    self.future = Some(fut);
  }
}

#[allow(clippy::unnecessary_wraps)]
pub fn op_global_timer_stop(
  state: &mut OpState,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let global_timer = state.borrow_mut::<GlobalTimer>();
  global_timer.cancel();
  Ok(())
}

// Set up a timer that will be later awaited by JS promise.
// It's a separate op, because canceling a timeout immediately
// after setting it caused a race condition (because Tokio timeout)
// might have been registered after next event loop tick.
//
// See https://github.com/denoland/deno/issues/7599 for more
// details.
#[allow(clippy::unnecessary_wraps)]
pub fn op_global_timer_start(
  state: &mut OpState,
  timeout: u64,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let deadline = Instant::now() + Duration::from_millis(timeout);
  let global_timer = state.borrow_mut::<GlobalTimer>();
  global_timer.new_timeout(deadline);
  Ok(())
}

pub async fn op_global_timer(
  state: Rc<RefCell<OpState>>,
  _args: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let maybe_timer_fut = {
    let mut s = state.borrow_mut();
    let global_timer = s.borrow_mut::<GlobalTimer>();
    global_timer.future.take()
  };
  if let Some(timer_fut) = maybe_timer_fut {
    let _ = timer_fut.await;
  }
  Ok(())
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
#[allow(clippy::unnecessary_wraps)]
pub fn op_now<TP>(
  state: &mut OpState,
  _argument: (),
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<f64, AnyError>
where
  TP: TimersPermission + 'static,
{
  let start_time = state.borrow::<StartTime>();
  let seconds = start_time.elapsed().as_secs();
  let mut subsec_nanos = start_time.elapsed().subsec_nanos() as f64;
  let reduced_time_precision = 2_000_000.0; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.borrow_mut::<TP>().allow_hrtime() {
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }

  let result = (seconds * 1_000) as f64 + (subsec_nanos / 1_000_000.0);

  Ok(result)
}

#[allow(clippy::unnecessary_wraps)]
pub fn op_sleep_sync<TP>(
  state: &mut OpState,
  millis: u64,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError>
where
  TP: TimersPermission + 'static,
{
  state.borrow::<TP>().check_unstable(state, "Deno.sleepSync");
  sleep(Duration::from_millis(millis));
  Ok(())
}
