// Copyright 2018-2025 the Deno authors. MIT license.

//! Reactor abstraction for timer and I/O primitives.
//!
//! Currently used by [`WebTimers`](crate::web_timeout::WebTimers) to abstract
//! over the timer backend. The default implementation (`reactor-tokio` feature)
//! delegates to tokio.
//!
//! Note: `uv_compat` does **not** use this trait -- it talks to tokio directly
//! because it needs lower-level control (poll_accept, try_read, try_write).

use std::future::Future;
use std::ops::Add;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

/// Abstraction over the async I/O reactor (tokio, mio, io_uring, custom).
/// This is the only seam between deno_core and the underlying async runtime.
pub trait Reactor: 'static {
  type Timer: ReactorTimer;
  type Instant: ReactorInstant;

  /// Create a new one-shot timer that fires at the given instant.
  fn timer(&self, deadline: Self::Instant) -> Self::Timer;

  /// Get the current instant.
  fn now(&self) -> Self::Instant;

  /// Poll the reactor for I/O readiness. This is called during the "poll" phase.
  /// Drives the underlying event source (epoll/kqueue/iocp).
  /// `timeout` = None means block indefinitely, Some(Duration::ZERO) means non-blocking.
  fn poll(&self, cx: &mut Context, timeout: Option<Duration>) -> Poll<()>;

  /// Spawn a future onto the reactor's executor (if it has one).
  /// Returns a handle that can be polled for the result.
  fn spawn(
    &self,
    fut: Pin<Box<dyn Future<Output = ()> + 'static>>,
  ) -> Pin<Box<dyn Future<Output = ()>>>;
}

/// A timer future that can be reset to fire at a different deadline.
pub trait ReactorTimer: Future<Output = ()> + Unpin {
  fn reset(&mut self, deadline: impl Into<Self::Instant>)
  where
    Self: Sized;

  type Instant: ReactorInstant;

  /// The deadline this timer is set to fire at.
  fn deadline(&self) -> Self::Instant;
}

/// An instant in time, used for timer deadlines.
pub trait ReactorInstant:
  Copy + Ord + Add<Duration, Output = Self> + Send + Sync + 'static
{
  fn now() -> Self;
  fn elapsed(&self) -> Duration;
  fn checked_add(&self, duration: Duration) -> Option<Self>;
}

/// The default reactor type, selected by feature flags.
/// When `reactor-tokio` is enabled, this is `TokioReactor`.
#[cfg(feature = "reactor-tokio")]
pub type DefaultReactor = crate::reactor_tokio::TokioReactor;
