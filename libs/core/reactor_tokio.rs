// Copyright 2018-2025 the Deno authors. MIT license.

use crate::reactor::Reactor;
use crate::reactor::ReactorInstant;
use crate::reactor::ReactorTimer;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::Sleep;

/// Default reactor implementation backed by tokio.
#[derive(Default)]
pub struct TokioReactor;

impl Reactor for TokioReactor {
  type Timer = TokioTimer;
  type Instant = Instant;

  fn timer(&self, deadline: Self::Instant) -> Self::Timer {
    TokioTimer {
      sleep: Box::pin(tokio::time::sleep_until(deadline)),
    }
  }

  fn now(&self) -> Self::Instant {
    Instant::now()
  }

  fn poll(&self, cx: &mut Context, _timeout: Option<Duration>) -> Poll<()> {
    // Tokio's reactor is driven implicitly by the runtime,
    // so we just yield back.
    cx.waker().wake_by_ref();
    Poll::Pending
  }

  fn spawn(
    &self,
    fut: Pin<Box<dyn Future<Output = ()> + 'static>>,
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    let handle = deno_unsync::spawn(fut);
    Box::pin(async move {
      let _ = handle.await;
    })
  }
}

/// A timer backed by tokio's [`Sleep`].
pub struct TokioTimer {
  sleep: Pin<Box<Sleep>>,
}

impl Future for TokioTimer {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    self.sleep.as_mut().poll(cx)
  }
}

impl ReactorTimer for TokioTimer {
  type Instant = Instant;

  fn reset(&mut self, deadline: impl Into<Instant>) {
    self.sleep.as_mut().reset(deadline.into());
  }

  fn deadline(&self) -> Instant {
    self.sleep.deadline()
  }
}

impl ReactorInstant for Instant {
  fn now() -> Self {
    Instant::now()
  }

  fn elapsed(&self) -> Duration {
    Instant::elapsed(self)
  }

  fn checked_add(&self, duration: Duration) -> Option<Self> {
    Instant::checked_add(self, duration)
  }
}
