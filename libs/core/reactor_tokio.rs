// Copyright 2018-2026 the Deno authors. MIT license.

use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use tokio::time::Instant;

use crate::reactor::Reactor;
use crate::reactor::ReactorInstant;

/// Default reactor implementation backed by tokio.
#[derive(Default)]
pub struct TokioReactor;

impl Reactor for TokioReactor {
  type Timer = TokioTimer;
  type Instant = Instant;

  fn timer(&self, deadline: Self::Instant) -> Self::Timer {
    TokioTimer::new(deadline)
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

// =============================================================================
// `TokioTimer` -- the timer future returned by `TokioReactor`.
// =============================================================================
//
// Background: tokio's time driver rounds every deadline up to the next integer
// millisecond ("tick") and processes wheel slots at slot boundaries. A
// `sleep(1ms)` therefore typically wakes 1-2 ms later, which makes
// `setTimeout(cb, 1)` fire at ~2.3 ms instead of ~1.1 ms (Node/Bun on Linux).
//
// libuv (Node/Bun) passes `next_deadline - now` directly to `epoll_wait` /
// `kevent` in nanoseconds; the kernel scheduler honors sub-ms timeouts, so the
// floor is the scheduler quantum (~100 us), not 1 ms.
//
// Our implementation attaches a per-timer kernel timer fd to tokio's *own*
// mio reactor. When the kernel timer fires the fd becomes readable and tokio
// wakes naturally as part of its existing I/O park -- no userspace thread, no
// cross-thread wake.
//
// Per platform:
//   - Linux: `timerfd_create` + `timerfd_settime`. Standard, the wake path
//     funnels through tokio's mio epoll alongside every other fd.
//   - macOS: own `kqueue()` + `EVFILT_TIMER` (`NOTE_NSECONDS`). The kqueue fd
//     itself becomes readable when an event is pending, so we register *that*
//     fd with tokio's mio kqueue via `AsyncFd`. There is therefore a kqueue
//     inside a kqueue on this platform; that adds some kernel transitions vs.
//     libuv (which puts the timer on the *same* kqueue as I/O), but it does
//     not require a userspace thread.
//   - Windows: tokio's wheel. We don't have an IOCP-friendly kernel timer
//     implementation yet, so this platform keeps the previous behaviour.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub use fallback::TokioTimer;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use kernel::TokioTimer;

// -- Linux / macOS: kernel timer fd registered with tokio's mio reactor -----
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod kernel {
  use std::future::Future;
  use std::os::fd::AsRawFd;
  use std::os::fd::OwnedFd;
  use std::pin::Pin;
  use std::task::Context;
  use std::task::Poll;

  use tokio::io::unix::AsyncFd;
  use tokio::time::Instant;

  use crate::reactor::ReactorTimer;

  /// Lazy: the kernel timer fd is created and armed on the first `poll`. This
  /// matches how `MutableSleep::change` drives us (it always polls the timer
  /// once right after creation) and avoids syscalls for `TokioTimer` values
  /// that are constructed and immediately dropped (rare, but cheap to handle).
  pub struct TokioTimer {
    deadline: Instant,
    state: State,
  }

  enum State {
    Unarmed,
    Armed { fd: AsyncFd<OwnedFd> },
    Fired,
  }

  impl TokioTimer {
    pub(super) fn new(deadline: Instant) -> Self {
      Self {
        deadline,
        state: State::Unarmed,
      }
    }

    fn arm(&mut self) -> std::io::Result<AsyncFd<OwnedFd>> {
      let dur = self.deadline.saturating_duration_since(Instant::now());
      let raw = create_armed_timer(dur)?;
      AsyncFd::with_interest(raw, tokio::io::Interest::READABLE)
    }
  }

  impl Future for TokioTimer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
      if matches!(self.state, State::Fired) {
        return Poll::Ready(());
      }
      if Instant::now() >= self.deadline {
        self.state = State::Fired;
        return Poll::Ready(());
      }

      if matches!(self.state, State::Unarmed) {
        match self.arm() {
          Ok(fd) => self.state = State::Armed { fd },
          Err(_) => {
            // If we couldn't arm a kernel timer (e.g. fd limit reached),
            // resolve immediately rather than getting stuck. The JS layer
            // will re-check the precise deadline and round-trip again.
            self.state = State::Fired;
            cx.waker().wake_by_ref();
            return Poll::Ready(());
          }
        }
      }

      let State::Armed { fd } = &mut self.state else {
        unreachable!();
      };
      match fd.poll_read_ready(cx) {
        Poll::Pending => Poll::Pending,
        Poll::Ready(Err(_)) => {
          self.state = State::Fired;
          Poll::Ready(())
        }
        Poll::Ready(Ok(mut guard)) => {
          // Drain the event so the fd isn't immediately re-readied. Errors
          // here are benign -- we're marking ourselves fired either way.
          let _ = drain(guard.get_ref().as_raw_fd());
          guard.clear_ready();
          self.state = State::Fired;
          Poll::Ready(())
        }
      }
    }
  }

  impl ReactorTimer for TokioTimer {
    type Instant = Instant;

    fn reset(&mut self, deadline: impl Into<Instant>) {
      self.deadline = deadline.into();
      // Re-arm on next poll. We could `timerfd_settime` the existing fd or
      // re-`EV_ADD` the existing kqueue entry, but dropping the old fd and
      // letting `poll` allocate a fresh one keeps the state machine trivial,
      // at the cost of one extra syscall per reset.
      self.state = State::Unarmed;
    }

    fn deadline(&self) -> Instant {
      self.deadline
    }
  }

  // -- Linux: timerfd ------------------------------------------------------

  #[cfg(target_os = "linux")]
  fn create_armed_timer(dur: std::time::Duration) -> std::io::Result<OwnedFd> {
    use std::os::fd::FromRawFd;
    // SAFETY: timerfd_create returns a fresh owned fd on success.
    let fd = unsafe {
      libc::timerfd_create(
        libc::CLOCK_MONOTONIC,
        libc::TFD_NONBLOCK | libc::TFD_CLOEXEC,
      )
    };
    if fd < 0 {
      return Err(std::io::Error::last_os_error());
    }
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // A zero `it_value` disarms the timer; bump to 1 ns so the kernel will
    // actually fire (and the JS layer will re-check the precise deadline).
    let nanos = dur.as_nanos().max(1);
    let secs = (nanos / 1_000_000_000) as libc::time_t;
    let subsec = (nanos % 1_000_000_000) as libc::c_long;
    let it = libc::itimerspec {
      it_value: libc::timespec {
        tv_sec: secs,
        tv_nsec: subsec,
      },
      it_interval: libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
      },
    };
    // SAFETY: owned fd is valid and `it` lives for the call.
    let rc = unsafe {
      libc::timerfd_settime(owned.as_raw_fd(), 0, &it, std::ptr::null_mut())
    };
    if rc < 0 {
      return Err(std::io::Error::last_os_error());
    }
    Ok(owned)
  }

  #[cfg(target_os = "linux")]
  fn drain(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    let mut buf = [0u8; 8];
    // SAFETY: `fd` is a valid timerfd, buf has the right size.
    let rc = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    if rc < 0 {
      let err = std::io::Error::last_os_error();
      if err.kind() == std::io::ErrorKind::WouldBlock {
        return Ok(());
      }
      return Err(err);
    }
    Ok(())
  }

  // -- macOS: kqueue EVFILT_TIMER -----------------------------------------

  #[cfg(target_os = "macos")]
  fn create_armed_timer(dur: std::time::Duration) -> std::io::Result<OwnedFd> {
    use std::os::fd::FromRawFd;
    // SAFETY: kqueue returns a fresh owned fd on success.
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
      return Err(std::io::Error::last_os_error());
    }
    // CLOEXEC on the kqueue fd so it doesn't leak into child processes.
    // SAFETY: kq is valid.
    let flags = unsafe { libc::fcntl(kq, libc::F_GETFD) };
    if flags >= 0 {
      unsafe {
        libc::fcntl(kq, libc::F_SETFD, flags | libc::FD_CLOEXEC);
      }
    }
    let owned = unsafe { OwnedFd::from_raw_fd(kq) };

    // NOTE_NSECONDS for nanosecond precision. `ident=1` is arbitrary; each
    // kqueue is local to one timer so there's no collision. `data` is signed
    // (intptr_t); saturate to isize::MAX so we don't wrap.
    let nanos = dur.as_nanos().max(1);
    let data: isize = nanos.min(isize::MAX as u128) as isize;
    let ev = libc::kevent {
      ident: 1,
      filter: libc::EVFILT_TIMER,
      flags: libc::EV_ADD | libc::EV_ONESHOT,
      fflags: libc::NOTE_NSECONDS,
      data,
      udata: std::ptr::null_mut(),
    };
    // SAFETY: `ev` is valid for the call duration; `kq` is owned.
    let rc = unsafe {
      libc::kevent(
        owned.as_raw_fd(),
        &ev,
        1,
        std::ptr::null_mut(),
        0,
        std::ptr::null(),
      )
    };
    if rc < 0 {
      return Err(std::io::Error::last_os_error());
    }
    Ok(owned)
  }

  #[cfg(target_os = "macos")]
  fn drain(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    let mut ev: libc::kevent = unsafe { std::mem::zeroed() };
    let ts = libc::timespec {
      tv_sec: 0,
      tv_nsec: 0,
    };
    // SAFETY: `ev`/`ts` are valid for the call; `fd` is a valid kqueue.
    let rc = unsafe { libc::kevent(fd, std::ptr::null(), 0, &mut ev, 1, &ts) };
    if rc < 0 {
      return Err(std::io::Error::last_os_error());
    }
    Ok(())
  }
}

// -- Other platforms: tokio's wheel ----------------------------------------
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod fallback {
  use std::future::Future;
  use std::pin::Pin;
  use std::task::Context;
  use std::task::Poll;

  use tokio::time::Instant;
  use tokio::time::Sleep;

  use crate::reactor::ReactorTimer;

  pub struct TokioTimer {
    sleep: Pin<Box<Sleep>>,
  }

  impl TokioTimer {
    pub(super) fn new(deadline: Instant) -> Self {
      Self {
        sleep: Box::pin(tokio::time::sleep_until(deadline)),
      }
    }
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
}
