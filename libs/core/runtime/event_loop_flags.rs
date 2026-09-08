// Copyright 2018-2026 the Deno authors. MIT license.

//! Bitflags for event-loop scheduling and liveness.
//!
//! Two distinct words live here:
//!
//! * [`SchedFlags`] — *scheduling* bits, set at the point work is **enqueued**
//!   and cleared when the corresponding queue is observed empty. Its purpose is
//!   to let the event loop skip a queue entirely without borrowing the
//!   `RefCell` that holds it.
//! * [`EventLoopPendingState`] — *liveness* bits, recomputed once per tick to
//!   answer "does anything keep this event loop alive?".
//!
//! # Why the scheduling word is only *advisory*
//!
//! Every bit in [`SchedFlags`] is conservative in one direction only: a set bit
//! whose queue is actually empty costs one wasted (cheap) check, whereas a
//! clear bit whose queue is non-empty **hangs the event loop**. Bits are
//! therefore set unconditionally at every enqueue site, and cleared only after
//! the authoritative container has been observed empty — never blindly at the
//! top of a tick, which would race with work enqueued mid-tick (the same
//! wrinkle the lazy `UvLoop` creation hit: the loop can come into existence
//! *during* phase 2, so the pointer is re-read before phase 3).
//!
//! The lazy-clear pattern is:
//!
//! ```ignore
//! if !sched.has(SchedFlags::FOO) {
//!   // Definitely empty: no borrow, no work.
//! } else {
//!   drain_foo();
//!   // Only now, with the container observed empty, is it safe to clear.
//!   sched.clear_if(SchedFlags::FOO, foo.borrow().is_empty());
//! }
//! ```
//!
//! Because the clear is gated on an authoritative emptiness check taken *after*
//! the drain, a mid-drain enqueue either (a) happens before the check, which
//! then sees a non-empty container and leaves the bit set, or (b) happens after
//! the clear, in which case the enqueue site sets the bit again. There is no
//! window in which work is queued with the bit clear.

use std::cell::Cell;
use std::rc::Rc;

/// A shared handle to the event loop's scheduling word.
///
/// Cheap to clone (`Rc`), `!Send`/`!Sync` — every holder lives on the event
/// loop thread. Handed to `OpState`, `ContextState` and `ExceptionState` so
/// that all three can mark work as scheduled without a back-pointer to the
/// runtime.
#[derive(Clone, Default)]
pub(crate) struct SchedFlags(Rc<Cell<u16>>);

impl SchedFlags {
  /// `ContextState::unrefed_ops` may be non-empty.
  ///
  /// Set by `op_unref_op` / `op_void_async_deferred`-style unref paths.
  pub const UNREFED_OPS: u16 = 1 << 0;
  /// `ContextState::active_timers` may be non-empty.
  ///
  /// Set when JS arms a timer (`op_timer_queue*`).
  pub const ACTIVE_TIMERS: u16 = 1 << 1;
  /// `ExceptionState::pending_promise_rejections` may be non-empty.
  ///
  /// Set from the V8 promise reject callback and from
  /// `op_dispatch_promise_rejection`.
  pub const PROMISE_REJECTIONS: u16 = 1 << 2;
  /// `ExceptionState::pending_handled_promise_rejections` may be non-empty.
  ///
  /// Set from the V8 promise reject callback (`kHandlerAddedAfterReject`).
  pub const HANDLED_REJECTIONS: u16 = 1 << 3;

  /// Mark work as scheduled. Must be called at *every* enqueue site for the
  /// corresponding queue.
  #[inline(always)]
  pub fn set(&self, bits: u16) {
    self.0.set(self.0.get() | bits);
  }

  /// Clear `bits` unconditionally. Only valid immediately after observing the
  /// backing container empty.
  #[inline(always)]
  pub fn clear(&self, bits: u16) {
    self.0.set(self.0.get() & !bits);
  }

  /// Clear `bits` when `empty` is an authoritative "the queue is empty right
  /// now" observation.
  #[inline(always)]
  pub fn clear_if(&self, bits: u16, empty: bool) {
    if empty {
      self.clear(bits);
    }
  }

  /// `false` guarantees the corresponding queues are empty.
  #[inline(always)]
  pub fn has(&self, bits: u16) -> bool {
    self.0.get() & bits != 0
  }
}

/// What is keeping the event loop alive, as a bitfield.
///
/// Recomputed once per tick by [`EventLoopPendingState::new`]. Replaces the
/// twelve-`bool` struct this used to be: `is_pending()` is now a single mask
/// test instead of nine short-circuiting field loads, and the several
/// multi-field `||` chains in `poll_event_loop_inner` collapse to one `&`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct EventLoopPendingState(u16);

impl EventLoopPendingState {
  /// Any async op in flight (refed or not).
  pub const PENDING_OPS: u16 = 1 << 0;
  /// An async op in flight that has *not* been unref'd, a queued V8 task, or
  /// a refed user timer — i.e. an op that keeps the loop alive.
  pub const PENDING_REFED_OPS: u16 = 1 << 1;
  /// A dynamic import is loading.
  pub const DYN_IMPORTS: u16 = 1 << 2;
  /// A dynamically imported module is mid-evaluation.
  pub const DYN_MODULE_EVAL: u16 = 1 << 3;
  /// A statically imported module is mid-evaluation (top-level await).
  pub const MODULE_EVAL: u16 = 1 << 4;
  /// V8 has background work (compilation, GC) outstanding.
  pub const BACKGROUND_TASKS: u16 = 1 << 5;
  /// `process.nextTick` work is queued.
  pub const TICK_SCHEDULED: u16 = 1 << 6;
  /// Unhandled/handled promise rejections are queued.
  pub const PROMISE_EVENTS: u16 = 1 << 7;
  /// An embedder has taken an external op ref.
  pub const EXTERNAL_OPS: u16 = 1 << 8;
  /// `setImmediate` callbacks are outstanding.
  pub const OUTSTANDING_IMMEDIATES: u16 = 1 << 9;
  /// JS-managed timers exist (refed or not; tracked for the leak sanitizer).
  pub const TIMERS: u16 = 1 << 10;
  /// The libuv compat loop has alive handles.
  pub const UV_ALIVE_HANDLES: u16 = 1 << 11;

  /// The subset that keeps the event loop alive.
  ///
  /// Deliberately excludes `PENDING_OPS` (unref'd ops do not hold the loop
  /// open), `OUTSTANDING_IMMEDIATES` and `TIMERS` — matching the old
  /// `is_pending()` exactly.
  const KEEPS_ALIVE: u16 = Self::PENDING_REFED_OPS
    | Self::DYN_IMPORTS
    | Self::DYN_MODULE_EVAL
    | Self::MODULE_EVAL
    | Self::BACKGROUND_TASKS
    | Self::TICK_SCHEDULED
    | Self::PROMISE_EVENTS
    | Self::EXTERNAL_OPS
    | Self::UV_ALIVE_HANDLES;

  /// True when *any* of `bits` is set.
  #[inline(always)]
  pub fn has(&self, bits: u16) -> bool {
    self.0 & bits != 0
  }

  #[inline(always)]
  pub fn is_pending(&self) -> bool {
    self.0 & Self::KEEPS_ALIVE != 0
  }
}

/// Accumulates the pending-state bits as each source is checked.
pub(crate) struct EventLoopPendingBuilder(u16);

impl EventLoopPendingBuilder {
  #[inline(always)]
  pub fn new() -> Self {
    Self(0)
  }

  #[inline(always)]
  pub fn set_if(&mut self, bits: u16, cond: bool) -> bool {
    if cond {
      self.0 |= bits;
    }
    cond
  }

  #[inline(always)]
  pub fn build(self) -> EventLoopPendingState {
    EventLoopPendingState(self.0)
  }
}
