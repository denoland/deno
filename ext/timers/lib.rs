// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! This module helps deno implement timers and performance APIs.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

pub trait TimersPermission {
  fn allow_hrtime(&mut self) -> bool;
  fn check_unstable(&self, state: &OpState, api_name: &'static str);
}

pub fn init<P: TimersPermission + 'static>() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/timers",
      "01_timers.js",
      "02_performance.js",
    ))
    .ops(vec![
      ("op_now", op_sync(op_now::<P>)),
      ("op_timer_handle", op_sync(op_timer_handle)),
      ("op_sleep", op_async(op_sleep)),
      ("op_sleep_sync", op_sync(op_sleep_sync::<P>)),
    ])
    .state(|state| {
      state.put(StartTime::now());
      Ok(())
    })
    .build()
}

pub type StartTime = Instant;

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
pub fn op_now<TP>(
  state: &mut OpState,
  _argument: (),
  _: (),
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

pub struct TimerHandle(Rc<CancelHandle>);

impl Resource for TimerHandle {
  fn name(&self) -> Cow<str> {
    "timer".into()
  }

  fn close(self: Rc<Self>) {
    self.0.cancel();
  }
}

/// Creates a [`TimerHandle`] resource that can be used to cancel invocations of
/// [`op_sleep`].
pub fn op_timer_handle(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<ResourceId, AnyError> {
  let rid = state
    .resource_table
    .add(TimerHandle(CancelHandle::new_rc()));
  Ok(rid)
}

/// Waits asynchronously until either `millis` milliseconds have passed or the
/// [`TimerHandle`] resource given by `rid` has been canceled.
pub async fn op_sleep(
  state: Rc<RefCell<OpState>>,
  millis: u64,
  rid: ResourceId,
) -> Result<(), AnyError> {
  let handle = state.borrow().resource_table.get::<TimerHandle>(rid)?;
  tokio::time::sleep(Duration::from_millis(millis))
    .or_cancel(handle.0.clone())
    .await?;
  Ok(())
}

pub fn op_sleep_sync<TP>(
  state: &mut OpState,
  millis: u64,
  _: (),
) -> Result<(), AnyError>
where
  TP: TimersPermission + 'static,
{
  state.borrow::<TP>().check_unstable(state, "Deno.sleepSync");
  std::thread::sleep(Duration::from_millis(millis));
  Ok(())
}
