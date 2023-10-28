// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

//! This module helps deno implement timers and performance APIs.

use crate::hr_timer_lock::hr_timer_lock;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
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
}

pub type StartTime = Instant;

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
#[op2(fast)]
pub fn op_now<TP>(state: &mut OpState, #[buffer] buf: &mut [u8])
where
  TP: TimersPermission + 'static,
{
  let start_time = state.borrow::<StartTime>();
  let elapsed = start_time.elapsed();
  let seconds = elapsed.as_secs();
  let mut subsec_nanos = elapsed.subsec_nanos();

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.borrow_mut::<TP>().allow_hrtime() {
    let reduced_time_precision = 2_000_000; // 2ms in nanoseconds
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }
  if buf.len() < 8 {
    return;
  }
  let buf: &mut [u32] =
    // SAFETY: buffer is at least 8 bytes long.
    unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as _, 2) };
  buf[0] = seconds as u32;
  buf[1] = subsec_nanos;
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
#[op2(fast)]
#[smi]
pub fn op_timer_handle(state: &mut OpState) -> ResourceId {
  state
    .resource_table
    .add(TimerHandle(CancelHandle::new_rc()))
}

/// Waits asynchronously until either `millis` milliseconds have passed or the
/// [`TimerHandle`] resource given by `rid` has been canceled.
///
/// If the timer is canceled, this returns `false`. Otherwise, it returns `true`.
#[op2(async(lazy), fast)]
pub async fn op_sleep(
  state: Rc<RefCell<OpState>>,
  #[smi] millis: u64,
  #[smi] rid: ResourceId,
) -> Result<bool, AnyError> {
  // If the timer is not present in the resource table it was cancelled before
  // this op was polled.
  let Ok(handle) = state.borrow().resource_table.get::<TimerHandle>(rid) else {
    return Ok(false);
  };

  // If a timer is requested with <=100ms resolution, request the high-res timer. Since the default
  // Windows timer period is 15ms, this means a 100ms timer could fire at 115ms (15% late). We assume that
  // timers longer than 100ms are a reasonable cutoff here.

  // The high-res timers on Windows are still limited. Unfortunately this means that our shortest duration 4ms timers
  // can still be 25% late, but without a more complex timer system or spinning on the clock itself, we're somewhat
  // bounded by the OS' scheduler itself.
  let _hr_timer_lock = if millis <= 100 {
    Some(hr_timer_lock())
  } else {
    None
  };

  let res = tokio::time::sleep(Duration::from_millis(millis))
    .or_cancel(handle.0.clone())
    .await;

  // We release the high-res timer lock here, either by being cancelled or resolving.
  Ok(res.is_ok())
}
