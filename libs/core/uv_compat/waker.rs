// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-handle wakers + ready queue for `UvLoopInner`.
//!
//! Each TCP / pipe / TTY handle owns its own `Waker`. When tokio's
//! reactor signals readiness, the handle's waker pushes a clone of
//! itself onto a per-kind ready queue and wakes the event loop.
//! `run_io` then polls ONLY the handles in the ready queues, rather
//! than scanning every live handle on every pass.
//!
//! The `in_queue` flag coalesces duplicate wakeups between drains.
//! On close, the handle's waker pointer is zeroed via `detach()`:
//! - future wakes become no-ops (push_ready checks ptr before queuing)
//! - already-queued entries self-invalidate: `run_io` pops the Arc,
//!   loads `ptr` (0 after detach), and skips
//!
//! Because the Arc keeps the waker allocation alive across the
//! handle's destruction, inspecting `live_ptr()` after a pop is
//! always safe, replacing what would otherwise be an O(n) scan of
//! the live-handle list with a plain load.
//!
//! ## Threading
//!
//! Handles are created, polled, and closed on the event loop thread,
//! but wakes can arrive from OTHER threads. On Windows, TTY readiness
//! is delivered by a thread pool wait callback registered with
//! `RegisterWaitForSingleObject`, and line mode console reads complete
//! on a dedicated reader thread; both call `Waker::wake` on the
//! per-handle waker from their own thread while the loop thread may be
//! draining the queues. The queues therefore use `Mutex` and the
//! per-handle fields use atomics. The mutexes are uncontended in
//! practice, so this costs nothing on the hot single-threaded path.
//!
//! `detach()` and `live_ptr()` are only ever called on the loop
//! thread. A cross-thread wake racing `detach()` may still enqueue the
//! handle, but the entry self-invalidates: `run_io` rechecks
//! `live_ptr()` on the loop thread before touching the handle.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Wake;

use futures::task::AtomicWaker;

/// State shared between `UvLoopInner` and every per-handle waker.
#[derive(Default)]
pub(crate) struct LoopShared {
  pub loop_waker: AtomicWaker,
  pub ready_tcp: Mutex<VecDeque<Arc<TcpHandleWaker>>>,
  pub ready_pipe: Mutex<VecDeque<Arc<PipeHandleWaker>>>,
  pub ready_tty: Mutex<VecDeque<Arc<TtyHandleWaker>>>,
}

impl LoopShared {
  pub fn new() -> Arc<Self> {
    Arc::new(Self::default())
  }
}

macro_rules! impl_handle_waker {
  ($name:ident, $queue:ident) => {
    pub(crate) struct $name {
      /// Raw handle pointer (as usize). Zeroed by `detach()` when the
      /// handle is torn down; queued entries self-invalidate and late
      /// wakes become no-ops. Relaxed ordering suffices: the pointer
      /// is only dereferenced on the loop thread, which rechecks
      /// `live_ptr()` after popping (program order with `detach()`).
      ptr: AtomicUsize,
      /// Coalesces duplicate wakeups. `true` means this handle is
      /// already sitting in its ready queue waiting to be drained.
      in_queue: AtomicBool,
      shared: Arc<LoopShared>,
    }

    impl $name {
      pub fn new(ptr: usize, shared: Arc<LoopShared>) -> Arc<Self> {
        Arc::new(Self {
          ptr: AtomicUsize::new(ptr),
          in_queue: AtomicBool::new(false),
          shared,
        })
      }

      /// Zero the handle pointer so future wakes become no-ops and
      /// already-queued entries self-invalidate.
      pub fn detach(&self) {
        self.ptr.store(0, Ordering::Relaxed);
      }

      /// Mark this handle as no-longer-queued. Called by `run_io`
      /// right before polling so a wake during the poll re-queues
      /// the handle for the next pass.
      pub fn reset_queued(&self) {
        self.in_queue.store(false, Ordering::Release);
      }

      /// Returns the handle pointer, or 0 if detached. Safe to call
      /// after handle memory has been freed; the Arc keeps the
      /// waker allocation alive.
      #[inline]
      pub fn live_ptr(&self) -> usize {
        self.ptr.load(Ordering::Relaxed)
      }

      fn push_ready(self: &Arc<Self>) {
        if self.ptr.load(Ordering::Relaxed) == 0 {
          return;
        }
        if !self.in_queue.swap(true, Ordering::AcqRel) {
          self.shared.$queue.lock().unwrap().push_back(self.clone());
          self.shared.loop_waker.wake();
        }
      }

      /// Explicit (non-waker) request to poll this handle next tick.
      /// Used after state changes like `uv_read_start` / `uv_write` /
      /// `uv_listen` where tokio hasn't fired a readiness wake yet
      /// but we need at least one poll to register interest.
      pub fn mark_ready(self: &Arc<Self>) {
        self.push_ready();
      }
    }

    impl Wake for $name {
      fn wake(self: Arc<Self>) {
        (&self).push_ready();
      }
      fn wake_by_ref(self: &Arc<Self>) {
        self.push_ready();
      }
    }
  };
}

impl_handle_waker!(TcpHandleWaker, ready_tcp);
impl_handle_waker!(PipeHandleWaker, ready_pipe);
impl_handle_waker!(TtyHandleWaker, ready_tty);
