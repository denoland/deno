// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-handle wakers + ready queue for `UvLoopInner`.
//!
//! Each TCP / pipe / TTY handle owns its own `Waker`. When tokio's
//! reactor signals readiness, the handle's waker pushes the handle's
//! raw pointer (as `usize`) onto a per-kind ready queue and wakes the
//! event loop. `run_io` then polls ONLY the handles in the ready
//! queues, rather than scanning every live handle on every pass.
//!
//! The `in_queue` flag coalesces duplicate wakeups between drains.
//! On close, the handle's waker pointer is nulled so late wakeups
//! become no-ops; `run_io` additionally validates each popped pointer
//! against the live-handle list before polling.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Wake;

use futures::task::AtomicWaker;

/// State shared between `UvLoopInner` and every per-handle waker.
/// Held behind an `Arc`; waker callbacks may fire from tokio's reactor
/// thread, so all fields are Send + Sync.
#[derive(Default)]
pub(crate) struct LoopShared {
  pub loop_waker: AtomicWaker,
  pub ready_tcp: Mutex<VecDeque<usize>>,
  pub ready_pipe: Mutex<VecDeque<usize>>,
  pub ready_tty: Mutex<VecDeque<usize>>,
}

impl LoopShared {
  pub fn new() -> Arc<Self> {
    Arc::new(Self::default())
  }

  /// Manually mark a TCP handle ready (used on state changes like
  /// `uv_read_start` where no tokio wake has fired yet).
  #[allow(dead_code)]
  pub fn mark_tcp_ready(&self, ptr: usize) {
    if ptr == 0 {
      return;
    }
    self.ready_tcp.lock().unwrap().push_back(ptr);
    self.loop_waker.wake();
  }

  #[allow(dead_code)]
  pub fn mark_pipe_ready(&self, ptr: usize) {
    if ptr == 0 {
      return;
    }
    self.ready_pipe.lock().unwrap().push_back(ptr);
    self.loop_waker.wake();
  }

  #[allow(dead_code)]
  pub fn mark_tty_ready(&self, ptr: usize) {
    if ptr == 0 {
      return;
    }
    self.ready_tty.lock().unwrap().push_back(ptr);
    self.loop_waker.wake();
  }
}

/// Per-kind waker. Generic so the same type backs TCP / pipe / TTY
/// wakers -- the kind determines which ready queue the wake pushes to.
pub(crate) struct HandleWaker<const KIND: u8> {
  /// Raw handle pointer (as usize). Nulled on close to make late
  /// wakeups from tokio's reactor harmless.
  ptr: AtomicUsize,
  /// Coalesces duplicate wakeups. `true` means this handle is already
  /// sitting in its ready queue waiting to be drained.
  in_queue: AtomicBool,
  shared: Arc<LoopShared>,
}

pub(crate) const KIND_TCP: u8 = 0;
pub(crate) const KIND_PIPE: u8 = 1;
pub(crate) const KIND_TTY: u8 = 2;

pub(crate) type TcpHandleWaker = HandleWaker<KIND_TCP>;
pub(crate) type PipeHandleWaker = HandleWaker<KIND_PIPE>;
pub(crate) type TtyHandleWaker = HandleWaker<KIND_TTY>;

impl<const KIND: u8> HandleWaker<KIND> {
  pub fn new(ptr: usize, shared: Arc<LoopShared>) -> Arc<Self> {
    Arc::new(Self {
      ptr: AtomicUsize::new(ptr),
      in_queue: AtomicBool::new(false),
      shared,
    })
  }

  /// Clear the handle pointer so future wakeups become no-ops. Called
  /// when the handle is being torn down.
  pub fn detach(&self) {
    self.ptr.store(0, Ordering::Release);
  }

  /// Mark this handle as no-longer-queued. Called by `run_io` right
  /// before polling the handle: a subsequent wake during the poll
  /// will re-queue it for the next pass.
  pub fn reset_queued(&self) {
    self.in_queue.store(false, Ordering::Release);
  }

  /// Return the current handle pointer (0 if detached).
  #[allow(dead_code)]
  pub fn ptr(&self) -> usize {
    self.ptr.load(Ordering::Acquire)
  }

  fn push_ready(&self) {
    let ptr = self.ptr.load(Ordering::Acquire);
    if ptr == 0 {
      return;
    }
    if !self.in_queue.swap(true, Ordering::AcqRel) {
      match KIND {
        KIND_TCP => self.shared.ready_tcp.lock().unwrap().push_back(ptr),
        KIND_PIPE => self.shared.ready_pipe.lock().unwrap().push_back(ptr),
        KIND_TTY => self.shared.ready_tty.lock().unwrap().push_back(ptr),
        _ => unreachable!(),
      }
      self.shared.loop_waker.wake();
    }
  }

  /// Explicit (non-waker) request to poll this handle next tick.
  /// Used after state changes like `uv_read_start` / `uv_write` /
  /// `uv_listen` where tokio hasn't fired a readiness wake yet but
  /// we need at least one poll to register interest.
  pub fn mark_ready(&self) {
    self.push_ready();
  }
}

impl<const KIND: u8> Wake for HandleWaker<KIND> {
  fn wake(self: Arc<Self>) {
    self.push_ready();
  }
  fn wake_by_ref(self: &Arc<Self>) {
    self.push_ready();
  }
}
