// Copyright 2018-2026 the Deno authors. MIT license.

//! This module helps deno implement timers and performance APIs.

use std::cell::Cell;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::OpState;
use deno_core::op2;

thread_local! {
  /// Mirror of the isolate's `StartTime`, so that `op_now_ms` can read the
  /// time origin without an `OpState` type-map lookup. Each isolate runs on its
  /// own thread, so this stays in sync with the `StartTime` in its `OpState`.
  static START_TIME: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub struct StartTime(Instant);

impl Default for StartTime {
  fn default() -> Self {
    let now = Instant::now();
    START_TIME.set(Some(now));
    Self(now)
  }
}

impl std::ops::Deref for StartTime {
  type Target = Instant;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

fn expose_time(duration: Duration, out: &mut [u8]) {
  let seconds = duration.as_secs() as u32;
  let subsec_nanos = duration.subsec_nanos();

  if out.len() >= 8 {
    out[0..4].copy_from_slice(&seconds.to_ne_bytes());
    out[4..8].copy_from_slice(&subsec_nanos.to_ne_bytes());
  }
}

#[op2(fast)]
pub fn op_now(state: &mut OpState, #[buffer] buf: &mut [u8]) {
  let start_time = state.borrow::<StartTime>();
  let elapsed = start_time.elapsed();
  expose_time(elapsed, buf);
}

/// Like `op_now`, but returns the elapsed time directly as a
/// `DOMHighResTimeStamp` (milliseconds). Callers that only need the number —
/// `Event.timeStamp` — avoid the typed-array round trip this way.
#[op2(fast)]
pub fn op_now_ms() -> f64 {
  // The time origin is unset while building the startup snapshot, since
  // extension `state` setup does not run then. `Event` construction during
  // snapshot warmup reaches here, so fall back to a zero elapsed time.
  let elapsed = match START_TIME.get() {
    Some(start_time) => start_time.elapsed(),
    None => Duration::default(),
  };
  elapsed.as_secs_f64() * 1000.0
}

#[op2(fast)]
pub fn op_time_origin(state: &mut OpState, #[buffer] buf: &mut [u8]) {
  // https://w3c.github.io/hr-time/#dfn-estimated-monotonic-time-of-the-unix-epoch
  let wall_time = SystemTime::now();
  let monotonic_time = state.borrow::<StartTime>().elapsed();
  let epoch = wall_time.duration_since(UNIX_EPOCH).unwrap() - monotonic_time;
  expose_time(epoch, buf);
}

#[allow(clippy::unused_async, reason = "op specifically for this purpose")]
#[op2(async(lazy), fast)]
pub async fn op_defer() {}
