// Copyright 2018-2026 the Deno authors. MIT license.

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
pub struct CloseCallback {
  pub callback: Box<dyn FnOnce()>,
}

/// V8 close callback that needs a scope to run JS.
/// Used by HandleWrap.close() to defer the JS close callback to the
/// close phase of the event loop, matching libuv's uv_close behavior.
pub struct V8CloseCallback {
  pub callback: Box<dyn FnOnce(&mut v8::PinScope<'_, '_>) + 'static>,
}

/// Phase-specific state for the event loop.
///
/// Currently only tracks close callbacks. Other phase hooks (idle, prepare,
/// check) are handled by `UvLoopInner` for the libuv compat path.
#[derive(Default)]
pub struct EventLoopPhases {
  /// Phase 6: Close callbacks.
  pub close_callbacks: VecDeque<CloseCallback>,
  /// Phase 6: V8 close callbacks (need a scope to call JS).
  pub v8_close_callbacks: VecDeque<V8CloseCallback>,
}

impl EventLoopPhases {
  /// Drain and run all close callbacks.
  pub fn run_close_callbacks(&mut self) {
    while let Some(cb) = self.close_callbacks.pop_front() {
      (cb.callback)();
    }
  }

  /// Drain all V8 close callbacks (called with a scope from jsruntime).
  pub(crate) fn drain_v8_close_callbacks(&mut self) -> Vec<V8CloseCallback> {
    self.v8_close_callbacks.drain(..).collect()
  }
}
