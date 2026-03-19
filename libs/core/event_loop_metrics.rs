// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Thread-safe snapshot of event loop metrics, readable from any thread.
/// Updated by the owning event loop, readable by the parent (e.g. for
/// `worker.performance.eventLoopUtilization()`).
pub struct SharedEventLoopMetrics {
  /// The runtime's start_time Instant, stored as nanos since an arbitrary
  /// but process-wide consistent reference point. This allows cross-thread
  /// time computations using `Instant::now()`.
  start_time: Instant,
  /// Nanoseconds from start_time to loop start, or -1 if not started.
  loop_start_ns: AtomicI64,
  /// Total accumulated idle nanoseconds.
  accumulated_idle_ns: AtomicU64,
}

impl SharedEventLoopMetrics {
  pub fn new(start_time: Instant) -> Self {
    Self {
      start_time,
      loop_start_ns: AtomicI64::new(-1),
      accumulated_idle_ns: AtomicU64::new(0),
    }
  }

  /// Returns loop_start_ms, idle_time_ms, and active_ms.
  /// Computes active from `Instant::now()` so it works cross-thread.
  pub fn read_metrics(&self) -> (f64, f64, f64) {
    let loop_start_ns = self.loop_start_ns.load(Ordering::Relaxed);
    if loop_start_ns < 0 {
      return (-1.0, 0.0, 0.0);
    }
    let idle_ns = self.accumulated_idle_ns.load(Ordering::Relaxed);
    let now_ns =
      Instant::now().duration_since(self.start_time).as_nanos() as u64;
    let loop_start_ns = loop_start_ns as u64;
    let elapsed_ns = now_ns.saturating_sub(loop_start_ns);
    let active_ns = elapsed_ns.saturating_sub(idle_ns);

    (
      loop_start_ns as f64 / 1e6,
      idle_ns as f64 / 1e6,
      active_ns as f64 / 1e6,
    )
  }
}

/// Tracks event loop idle/active time.
///
/// Used by Node.js `eventLoopUtilization` and OpenTelemetry event loop metrics.
/// Uses `Cell` (not `RefCell`) for zero-overhead reads from ops on the same
/// thread. Also maintains an `Arc<SharedEventLoopMetrics>` for cross-thread
/// reads (e.g. parent reading worker metrics).
pub struct EventLoopMetrics {
  /// When the event loop first started polling (first `poll_event_loop` call).
  loop_start: Cell<Option<Instant>>,
  /// Set when `poll_event_loop` returns `Poll::Pending` (going idle).
  /// Cleared on next tick start.
  idle_start: Cell<Option<Instant>>,
  /// Total accumulated idle time in nanoseconds (local fast path).
  accumulated_idle_ns: Cell<u64>,
  /// Process/runtime start time, used as epoch for loop_start_ns in shared.
  start_time: Instant,
  /// Thread-safe metrics snapshot, updated alongside local Cell values.
  shared: Arc<SharedEventLoopMetrics>,
}

impl Default for EventLoopMetrics {
  fn default() -> Self {
    let start_time = Instant::now();
    Self {
      loop_start: Cell::new(None),
      idle_start: Cell::new(None),
      accumulated_idle_ns: Cell::new(0),
      start_time,
      shared: Arc::new(SharedEventLoopMetrics::new(start_time)),
    }
  }
}

impl EventLoopMetrics {
  /// Get a clone of the shared metrics Arc, for cross-thread access.
  pub fn shared(&self) -> Arc<SharedEventLoopMetrics> {
    self.shared.clone()
  }

  /// Called at the start of each `poll_event_loop` tick.
  #[inline]
  pub fn record_tick_start(&self) {
    let now = Instant::now();
    if self.loop_start.get().is_none() {
      self.loop_start.set(Some(now));
      let ns = now.duration_since(self.start_time).as_nanos() as i64;
      self.shared.loop_start_ns.store(ns, Ordering::Relaxed);
    }
    if let Some(idle_start) = self.idle_start.get() {
      let idle_ns = now.duration_since(idle_start).as_nanos() as u64;
      let total = self.accumulated_idle_ns.get() + idle_ns;
      self.accumulated_idle_ns.set(total);
      self
        .shared
        .accumulated_idle_ns
        .store(total, Ordering::Relaxed);
      self.idle_start.set(None);
    }
  }

  /// Called when `poll_event_loop` returns `Poll::Pending` (going idle).
  #[inline]
  pub fn record_tick_idle(&self) {
    self.idle_start.set(Some(Instant::now()));
  }

  /// Returns milliseconds from `epoch` to when the event loop started,
  /// or -1 if the loop hasn't started yet.
  ///
  /// `epoch` should be the same `Instant` used by `performance.now()`
  /// (i.e. `StartTime`).
  pub fn loop_start_ms(&self, epoch: Instant) -> f64 {
    match self.loop_start.get() {
      Some(loop_start) => {
        loop_start.duration_since(epoch).as_nanos() as f64 / 1e6
      }
      None => -1.0,
    }
  }

  /// Returns total accumulated idle time in milliseconds.
  /// Includes the current idle period if the loop is currently idle.
  pub fn idle_time_ms(&self) -> f64 {
    let mut ns = self.accumulated_idle_ns.get();
    if let Some(idle_start) = self.idle_start.get() {
      ns += Instant::now().duration_since(idle_start).as_nanos() as u64;
    }
    ns as f64 / 1e6
  }
}
