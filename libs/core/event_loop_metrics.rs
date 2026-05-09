// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Tracks event loop idle/active time.
///
/// Used by Node.js `eventLoopUtilization` and OpenTelemetry event loop
/// metrics. All fields are atomics so the same instance can be shared between
/// the owning event loop and other threads (e.g. a parent reading worker
/// ELU). Wrap in `Arc` and clone freely.
pub struct EventLoopMetrics {
  /// Reference Instant for all stored nanoseconds. Set at construction.
  start_time: Instant,
  /// Nanoseconds from `start_time` to when the loop is considered started.
  /// Initialized to 0 so ELU is meaningful from the first user code (e.g.
  /// top-level module evaluation) without a separate priming call.
  loop_start_ns: AtomicI64,
  /// Total accumulated idle nanoseconds, excluding any in-progress idle
  /// period (which is in `idle_start_ns`).
  accumulated_idle_ns: AtomicU64,
  /// Nanoseconds from `start_time` to the last `Poll::Pending` return,
  /// or -1 if the loop is not currently idle.
  idle_start_ns: AtomicI64,
}

impl Default for EventLoopMetrics {
  fn default() -> Self {
    Self {
      start_time: Instant::now(),
      loop_start_ns: AtomicI64::new(0),
      accumulated_idle_ns: AtomicU64::new(0),
      idle_start_ns: AtomicI64::new(-1),
    }
  }
}

impl EventLoopMetrics {
  /// Called at the start of each event-loop poll. Folds any in-progress idle
  /// period into the accumulator.
  #[inline]
  pub fn record_tick_start(&self) {
    let idle_start = self.idle_start_ns.swap(-1, Ordering::Relaxed);
    if idle_start >= 0 {
      let now_ns =
        Instant::now().duration_since(self.start_time).as_nanos() as u64;
      let idle = now_ns.saturating_sub(idle_start as u64);
      self.accumulated_idle_ns.fetch_add(idle, Ordering::Relaxed);
    }
  }

  /// Called when the event loop returns `Poll::Pending` (going idle).
  #[inline]
  pub fn record_tick_idle(&self) {
    let now_ns =
      Instant::now().duration_since(self.start_time).as_nanos() as i64;
    self.idle_start_ns.store(now_ns, Ordering::Relaxed);
  }

  /// Returns `(loop_start_ms, idle_ms, active_ms)`, all relative to
  /// `start_time`. `idle_ms` includes any in-progress idle period;
  /// `active_ms` is the remaining elapsed time.
  pub fn read(&self) -> (f64, f64, f64) {
    let loop_start_ns = self.loop_start_ns.load(Ordering::Relaxed) as u64;
    let now_ns =
      Instant::now().duration_since(self.start_time).as_nanos() as u64;
    let mut idle_ns = self.accumulated_idle_ns.load(Ordering::Relaxed);
    let idle_start = self.idle_start_ns.load(Ordering::Relaxed);
    if idle_start >= 0 {
      idle_ns =
        idle_ns.saturating_add(now_ns.saturating_sub(idle_start as u64));
    }
    let elapsed_ns = now_ns.saturating_sub(loop_start_ns);
    let active_ns = elapsed_ns.saturating_sub(idle_ns);
    (
      loop_start_ns as f64 / 1e6,
      idle_ns as f64 / 1e6,
      active_ns as f64 / 1e6,
    )
  }
}
