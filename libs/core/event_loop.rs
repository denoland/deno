// Copyright 2018-2025 the Deno authors. MIT license.

//! Event loop phase state.
//!
//! The actual phase-based event loop is driven by `poll_event_loop_inner` in
//! `jsruntime.rs`. This module provides auxiliary state for phases that need
//! Rust-side callback queues (currently only close callbacks).
//!
//! libuv-style phases (timers, idle, prepare, poll, check) are driven
//! directly through `UvLoopInner` when a uv_loop is registered.

use std::collections::VecDeque;

/// Close callback for resource cleanup.
pub(crate) struct CloseCallback {
  pub callback: Box<dyn FnOnce()>,
}

/// Phase-specific state for the event loop.
///
/// Currently only tracks close callbacks. Other phase hooks (idle, prepare,
/// check) are handled by `UvLoopInner` for the libuv compat path.
#[derive(Default)]
pub(crate) struct EventLoopPhases {
  /// Phase 6: Close callbacks.
  pub close_callbacks: VecDeque<CloseCallback>,
}

impl EventLoopPhases {
  /// Drain and run all close callbacks.
  pub fn run_close_callbacks(&mut self) {
    while let Some(cb) = self.close_callbacks.pop_front() {
      (cb.callback)();
    }
  }
}
