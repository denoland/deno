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
//! - already-queued entries self-invalidate — `run_io` pops the Arc,
//!   loads `ptr` (0 after detach), and skips
//!
//! Because the Arc keeps the waker allocation alive across the
//! handle's destruction, inspecting `live_ptr()` after a pop is
//! always safe — replacing what would otherwise be an O(n) scan of
//! the live-handle list with a plain load.
//!
//! ## Threading
//!
//! Deno's event loop is single-threaded: the tokio current-thread
//! runtime drives both the reactor and the executor on the same
//! thread, so waker invocations never cross threads. That lets us
//! use `Cell` / `RefCell` instead of atomics / `Mutex` for the
//! queue and waker fields.
//!
//! `Arc` is still required (not `Rc`) because the `Wake` trait's
//! method signatures are `fn wake(self: Arc<Self>)`. The `unsafe
//! impl Send + Sync` on the waker types asserts the single-thread
//! invariant; `std::task::Waker` requires `Send + Sync` of its
//! backing data, so we lie at the type level and keep the
//! invariant by construction.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::task::Wake;

use futures::task::AtomicWaker;

/// State shared between `UvLoopInner` and every per-handle waker.
/// Held behind an `Arc`; we assert `Sync` manually — see the module
/// doc "Threading" section.
#[derive(Default)]
pub(crate) struct LoopShared {
  pub loop_waker: AtomicWaker,
  pub ready_tcp: RefCell<VecDeque<Arc<TcpHandleWaker>>>,
  pub ready_pipe: RefCell<VecDeque<Arc<PipeHandleWaker>>>,
  pub ready_tty: RefCell<VecDeque<Arc<TtyHandleWaker>>>,
}

// SAFETY: deno's event loop is single-threaded. All accesses to the
// ready queues happen from that thread (either directly in run_io or
// via tokio's current-thread reactor firing a waker). See module doc.
unsafe impl Sync for LoopShared {}

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
      /// wakes become no-ops.
      ptr: Cell<usize>,
      /// Coalesces duplicate wakeups. `true` means this handle is
      /// already sitting in its ready queue waiting to be drained.
      in_queue: Cell<bool>,
      shared: Arc<LoopShared>,
    }

    // SAFETY: single-threaded event loop — see module doc.
    unsafe impl Send for $name {}
    unsafe impl Sync for $name {}

    impl $name {
      pub fn new(ptr: usize, shared: Arc<LoopShared>) -> Arc<Self> {
        Arc::new(Self {
          ptr: Cell::new(ptr),
          in_queue: Cell::new(false),
          shared,
        })
      }

      /// Zero the handle pointer so future wakes become no-ops and
      /// already-queued entries self-invalidate.
      pub fn detach(&self) {
        self.ptr.set(0);
      }

      /// Mark this handle as no-longer-queued. Called by `run_io`
      /// right before polling so a wake during the poll re-queues
      /// the handle for the next pass.
      pub fn reset_queued(&self) {
        self.in_queue.set(false);
      }

      /// Returns the handle pointer, or 0 if detached. Safe to call
      /// after handle memory has been freed — the Arc keeps the
      /// waker allocation alive.
      #[inline]
      pub fn live_ptr(&self) -> usize {
        self.ptr.get()
      }

      fn push_ready(self: &Arc<Self>) {
        if self.ptr.get() == 0 {
          return;
        }
        if !self.in_queue.replace(true) {
          self.shared.$queue.borrow_mut().push_back(self.clone());
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
