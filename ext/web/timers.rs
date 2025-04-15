// Copyright 2018-2025 the Deno authors. MIT license.

//! This module helps deno implement timers and performance APIs.

use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::op2;
use deno_core::OpState;

pub trait TimersPermission {
  fn allow_hrtime(&mut self) -> bool;
}

impl TimersPermission for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn allow_hrtime(&mut self) -> bool {
    true
  }
}

pub struct StartTime(Instant);

impl Default for StartTime {
  fn default() -> Self {
    Self(Instant::now())
  }
}

impl std::ops::Deref for StartTime {
  type Target = Instant;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

fn expose_time<TP>(state: &mut OpState, duration: Duration, out: &mut [u8])
where
  TP: TimersPermission + 'static,
{
  let seconds = duration.as_secs() as u32;
  let mut subsec_nanos = duration.subsec_nanos();

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.borrow_mut::<TP>().allow_hrtime() {
    let reduced_time_precision = 2_000_000; // 2ms in nanoseconds
    subsec_nanos -= subsec_nanos % reduced_time_precision;
  }

  if out.len() >= 8 {
    out[0..4].copy_from_slice(&seconds.to_ne_bytes());
    out[4..8].copy_from_slice(&subsec_nanos.to_ne_bytes());
  }
}

#[op2(fast)]
pub fn op_now<TP>(state: &mut OpState, #[buffer] buf: &mut [u8])
where
  TP: TimersPermission + 'static,
{
  let start_time = state.borrow::<StartTime>();
  let elapsed = start_time.elapsed();
  expose_time::<TP>(state, elapsed, buf);
}

#[op2(fast)]
pub fn op_time_origin<TP>(state: &mut OpState, #[buffer] buf: &mut [u8])
where
  TP: TimersPermission + 'static,
{
  // https://w3c.github.io/hr-time/#dfn-estimated-monotonic-time-of-the-unix-epoch
  let wall_time = SystemTime::now();
  let monotonic_time = state.borrow::<StartTime>().elapsed();
  let epoch = wall_time.duration_since(UNIX_EPOCH).unwrap() - monotonic_time;
  expose_time::<TP>(state, epoch, buf);
}

#[allow(clippy::unused_async)]
#[op2(async(lazy), fast)]
pub async fn op_defer() {}
