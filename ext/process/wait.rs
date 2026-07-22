// Copyright 2018-2026 the Deno authors. MIT license.

//! Waiting on the exit of a process that this runtime did not spawn.
//!
//! `Deno.Command`'s `status` promise can only observe a child this process
//! created, because it is backed by the child handle returned from spawning.
//! Code that only knows a process by its pid — for example a separate program
//! tasked with shutting down a daemon started elsewhere — has no such handle.
//! This module fills that gap with an event-driven wait that resolves the
//! moment the target process exits, without polling.
//!
//! Each platform exposes a native primitive for this:
//!
//! - Linux: `pidfd_open` turns a pid into a file descriptor that becomes
//!   readable when the process exits.
//! - macOS and the BSDs: a `kqueue` registration with the `EVFILT_PROC` filter
//!   and the `NOTE_EXIT` flag delivers an event when the process exits.
//! - Windows: a process handle opened with `SYNCHRONIZE` becomes signaled when
//!   the process terminates.
//!
//! On all three the descriptor/handle is registered with the async runtime (or
//! the OS thread pool on Windows) so the wait costs nothing until the process
//! actually exits.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_permissions::PermissionsContainer;

use super::ProcessError;

/// A live wait on some process's exit, held in the resource table so the
/// caller can cancel it (by closing the resource) and so the underlying OS
/// descriptor/handle is released on drop.
struct ProcessWaitResource {
  waiter: Waiter,
  cancel: CancelHandle,
}

impl Resource for ProcessWaitResource {
  fn name(&self) -> Cow<'_, str> {
    "processWait".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl ProcessWaitResource {
  async fn wait(self: Rc<Self>) -> Result<(), ProcessError> {
    self.waiter.wait().await
  }
}

/// Platform-specific backing for a single process-exit wait.
enum Waiter {
  /// The process was already gone when the wait was opened, so the wait
  /// resolves immediately.
  AlreadyExited,
  #[cfg(any(target_os = "linux", target_os = "android"))]
  Pidfd(tokio::io::unix::AsyncFd<std::os::fd::OwnedFd>),
  #[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
  ))]
  Kqueue(tokio::io::unix::AsyncFd<std::os::fd::OwnedFd>),
  #[cfg(windows)]
  Handle(windows_impl::ProcessHandle),
}

impl Waiter {
  async fn wait(&self) -> Result<(), ProcessError> {
    match self {
      Waiter::AlreadyExited => Ok(()),
      #[cfg(any(target_os = "linux", target_os = "android"))]
      Waiter::Pidfd(async_fd) => {
        // A pidfd becomes readable once, and stays readable, when the process
        // exits. Readiness alone is the signal; there is nothing to read, and
        // the fd is closed right after, so the ready guard is dropped without
        // clearing readiness.
        let _guard = async_fd.readable().await.map_err(ProcessError::Io)?;
        Ok(())
      }
      #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
      ))]
      Waiter::Kqueue(async_fd) => {
        use std::os::fd::AsRawFd;
        loop {
          let mut guard =
            async_fd.readable().await.map_err(ProcessError::Io)?;
          if kqueue_impl::drain_exit_event(async_fd.get_ref().as_raw_fd())? {
            return Ok(());
          }
          // No event was ready after all; re-arm and wait again.
          guard.clear_ready();
        }
      }
      #[cfg(windows)]
      Waiter::Handle(handle) => handle.wait().await,
    }
  }
}

/// Open a wait on the process with `pid`. If the process is already gone,
/// returns [`Waiter::AlreadyExited`] rather than an error, so that the common
/// race — the process exits between the caller signaling it and opening this
/// wait — resolves as a completed wait instead of failing.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_waiter(pid: i32) -> Result<Waiter, ProcessError> {
  use std::os::fd::FromRawFd;
  use std::os::fd::OwnedFd;
  use std::os::fd::RawFd;

  // SAFETY: `pidfd_open` takes a pid and a flags word; 0 is a valid flags
  // value and no pointers are involved.
  let ret =
    unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0) };
  if ret < 0 {
    let err = std::io::Error::last_os_error();
    return match err.raw_os_error() {
      Some(libc::ESRCH) => Ok(Waiter::AlreadyExited),
      _ => Err(ProcessError::Io(err)),
    };
  }
  // SAFETY: `pidfd_open` returned a non-negative descriptor that we now own.
  let owned = unsafe { OwnedFd::from_raw_fd(ret as RawFd) };
  set_cloexec(&owned)?;
  let async_fd = tokio::io::unix::AsyncFd::with_interest(
    owned,
    tokio::io::Interest::READABLE,
  )
  .map_err(ProcessError::Io)?;
  Ok(Waiter::Pidfd(async_fd))
}

#[cfg(any(
  target_os = "macos",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
  target_os = "dragonfly"
))]
fn open_waiter(pid: i32) -> Result<Waiter, ProcessError> {
  use std::os::fd::FromRawFd;
  use std::os::fd::OwnedFd;

  // SAFETY: `kqueue` takes no arguments and returns a new descriptor or -1.
  let kq = unsafe { libc::kqueue() };
  if kq < 0 {
    return Err(ProcessError::Io(std::io::Error::last_os_error()));
  }
  // SAFETY: `kq` is a fresh descriptor returned by `kqueue`.
  let owned = unsafe { OwnedFd::from_raw_fd(kq) };
  set_cloexec(&owned)?;

  // SAFETY: `kevent` is zero-initialized and then populated with a valid
  // EVFILT_PROC/NOTE_EXIT registration for `pid`.
  let mut change: libc::kevent = unsafe { std::mem::zeroed() };
  change.ident = pid as usize;
  change.filter = libc::EVFILT_PROC;
  change.flags = libc::EV_ADD | libc::EV_ONESHOT;
  change.fflags = libc::NOTE_EXIT;

  // SAFETY: register one change and retrieve no events; `kq` and `change` are
  // valid and the event/timeout pointers are null as permitted for a pure
  // registration call.
  let ret = unsafe {
    libc::kevent(kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null())
  };
  if ret < 0 {
    let err = std::io::Error::last_os_error();
    return match err.raw_os_error() {
      Some(libc::ESRCH) => Ok(Waiter::AlreadyExited),
      _ => Err(ProcessError::Io(err)),
    };
  }
  let async_fd = tokio::io::unix::AsyncFd::with_interest(
    owned,
    tokio::io::Interest::READABLE,
  )
  .map_err(ProcessError::Io)?;
  Ok(Waiter::Kqueue(async_fd))
}

#[cfg(windows)]
fn open_waiter(pid: i32) -> Result<Waiter, ProcessError> {
  windows_impl::open(pid)
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "android",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
  target_os = "dragonfly",
  windows
)))]
fn open_waiter(_pid: i32) -> Result<Waiter, ProcessError> {
  Err(ProcessError::Io(std::io::Error::new(
    std::io::ErrorKind::Unsupported,
    "waiting on a non-child process is not supported on this platform",
  )))
}

// The pidfd/kqueue descriptors are only ever polled for readiness (and drained
// with a zero-timeout `kevent`), never read or written, so they don't need
// O_NONBLOCK — and setting it on a kqueue descriptor fails with ENOTTY on macOS
// because that routes through the kqueue's absent ioctl handler. Only the
// close-on-exec flag matters, so a later `Deno.Command` spawn doesn't inherit
// the descriptor.
#[cfg(any(
  target_os = "linux",
  target_os = "android",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
  target_os = "dragonfly"
))]
fn set_cloexec(fd: &impl std::os::fd::AsRawFd) -> Result<(), ProcessError> {
  let raw = fd.as_raw_fd();
  // SAFETY: `raw` is valid; F_SETFD accepts the descriptor flags word.
  if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
    return Err(ProcessError::Io(std::io::Error::last_os_error()));
  }
  Ok(())
}

#[cfg(any(
  target_os = "macos",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
  target_os = "dragonfly"
))]
mod kqueue_impl {
  use super::ProcessError;

  /// Retrieve at most one pending kqueue event without blocking. Returns `true`
  /// when an exit event was delivered, `false` on a spurious wakeup with no
  /// event ready.
  pub(super) fn drain_exit_event(
    kq: std::os::fd::RawFd,
  ) -> Result<bool, ProcessError> {
    // SAFETY: zero-initialized output event; `kevent` fills it in.
    let mut ev: libc::kevent = unsafe { std::mem::zeroed() };
    let ts = libc::timespec {
      tv_sec: 0,
      tv_nsec: 0,
    };
    // SAFETY: retrieve up to one event with a zero timeout (non-blocking); the
    // changelist is empty and the output event/timeout pointers are valid.
    let n = unsafe { libc::kevent(kq, std::ptr::null(), 0, &mut ev, 1, &ts) };
    if n < 0 {
      let err = std::io::Error::last_os_error();
      // Interrupted before an event could be read; the readiness loop retries.
      if err.raw_os_error() == Some(libc::EINTR) {
        return Ok(false);
      }
      return Err(ProcessError::Io(err));
    }
    // The only filter registered is EVFILT_PROC/NOTE_EXIT, so any delivered
    // event means the process has exited.
    Ok(n > 0)
  }
}

#[cfg(windows)]
mod windows_impl {
  use std::os::windows::io::AsRawHandle;
  use std::os::windows::io::FromRawHandle;
  use std::os::windows::io::OwnedHandle;

  use windows_sys::Win32::Foundation::BOOLEAN;
  use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
  use windows_sys::Win32::Foundation::FALSE;
  use windows_sys::Win32::Foundation::GetLastError;
  use windows_sys::Win32::Foundation::HANDLE;
  use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
  use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
  use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
  use windows_sys::Win32::System::Threading::INFINITE;
  use windows_sys::Win32::System::Threading::OpenProcess;
  use windows_sys::Win32::System::Threading::RegisterWaitForSingleObject;
  use windows_sys::Win32::System::Threading::UnregisterWaitEx;
  use windows_sys::Win32::System::Threading::WT_EXECUTEINWAITTHREAD;
  use windows_sys::Win32::System::Threading::WT_EXECUTEONLYONCE;
  use windows_sys::Win32::System::Threading::WaitForSingleObject;

  use super::ProcessError;
  use super::Waiter;

  type Sender = tokio::sync::oneshot::Sender<()>;

  /// A process handle opened with `SYNCHRONIZE`, whose signaled state tracks
  /// the process's termination.
  pub(super) struct ProcessHandle {
    handle: OwnedHandle,
  }

  pub(super) fn open(pid: i32) -> Result<Waiter, ProcessError> {
    // SAFETY: Win32 call; `pid` is a positive process id and `FALSE` disables
    // handle inheritance.
    let raw = unsafe { OpenProcess(SYNCHRONIZE, FALSE, pid as u32) };
    if raw.is_null() {
      // SAFETY: reads the calling thread's last error.
      let err = unsafe { GetLastError() };
      // The process id doesn't refer to a live process — treat as exited.
      if err == ERROR_INVALID_PARAMETER {
        return Ok(Waiter::AlreadyExited);
      }
      return Err(ProcessError::Io(std::io::Error::from_raw_os_error(
        err as i32,
      )));
    }
    // SAFETY: `raw` is a valid handle that `OpenProcess` transferred to us.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw as _) };
    if is_signaled(&handle) {
      // Dropping `handle` closes it.
      return Ok(Waiter::AlreadyExited);
    }
    Ok(Waiter::Handle(ProcessHandle { handle }))
  }

  fn is_signaled(handle: &OwnedHandle) -> bool {
    // SAFETY: Win32 call with a valid handle and a zero timeout, which polls
    // the signaled state without blocking.
    let r = unsafe { WaitForSingleObject(handle.as_raw_handle() as _, 0) };
    r == WAIT_OBJECT_0
  }

  impl ProcessHandle {
    pub(super) async fn wait(&self) -> Result<(), ProcessError> {
      let (tx, rx) = tokio::sync::oneshot::channel::<()>();
      // The boxed sender is handed to the wait-thread callback. It is freed by
      // `Registration::drop` only after `UnregisterWaitEx` guarantees the
      // callback is no longer running.
      let boxed: *mut Option<Sender> = Box::into_raw(Box::new(Some(tx)));
      let mut wait_object: HANDLE = std::ptr::null_mut();
      // SAFETY: registers a one-shot wait on the process handle; `boxed` stays
      // valid until the registration is dropped.
      let rc = unsafe {
        RegisterWaitForSingleObject(
          &mut wait_object,
          self.handle.as_raw_handle() as _,
          Some(callback),
          boxed as *mut _,
          INFINITE,
          WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE,
        )
      };
      if rc == 0 {
        let err = std::io::Error::last_os_error();
        // SAFETY: registration failed, so the callback will never run; reclaim
        // the box we just leaked.
        drop(unsafe { Box::from_raw(boxed) });
        return Err(ProcessError::Io(err));
      }
      // Unregisters the wait and frees the box when this future completes or is
      // dropped (the caller cancelled the wait).
      let _registration = Registration { wait_object, boxed };
      // A receive error means the sender was dropped without sending, which
      // only happens once the callback has fired; either way the wait is over.
      let _ = rx.await;
      Ok(())
    }
  }

  struct Registration {
    wait_object: HANDLE,
    boxed: *mut Option<Sender>,
  }

  impl Drop for Registration {
    fn drop(&mut self) {
      // SAFETY: `UnregisterWaitEx` with `INVALID_HANDLE_VALUE` blocks until any
      // in-flight callback has returned, after which the boxed sender is no
      // longer referenced and can be freed. If the call fails (it shouldn't for
      // a valid wait object) a callback may still be pending, so the box is
      // leaked rather than freed, to avoid a use-after-free.
      unsafe {
        if UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE) != 0 {
          drop(Box::from_raw(self.boxed));
        }
      }
    }
  }

  unsafe extern "system" fn callback(
    ptr: *mut std::ffi::c_void,
    _fired: BOOLEAN,
  ) {
    // SAFETY: `ptr` is the boxed sender passed to `RegisterWaitForSingleObject`.
    // It outlives this callback because the owning `Registration` frees it only
    // after `UnregisterWaitEx(INVALID_HANDLE_VALUE)` waits for us to return.
    let sender = unsafe { &mut *(ptr as *mut Option<Sender>) };
    if let Some(tx) = sender.take() {
      let _ = tx.send(());
    }
  }
}

/// Opens an event-driven wait on the exit of the process identified by `pid`.
///
/// Requires full run permission (`--allow-run`), the same capability that
/// [`op_kill`](super::deprecated::op_kill) requires to signal a process it did
/// not spawn.
#[op2(fast, stack_trace)]
#[smi]
pub(crate) fn op_process_wait_open(
  state: &mut OpState,
  #[smi] pid: i32,
  #[string] api_name: String,
) -> Result<ResourceId, ProcessError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_run_all(&api_name)?;

  if pid <= 0 {
    return Err(ProcessError::InvalidPid);
  }

  let waiter = open_waiter(pid)?;
  let rid = state.resource_table.add(ProcessWaitResource {
    waiter,
    cancel: Default::default(),
  });
  Ok(rid)
}

/// Resolves when the process behind the wait opened by
/// [`op_process_wait_open`] exits. Closing the resource cancels the wait.
#[op2]
pub(crate) async fn op_process_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), ProcessError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<ProcessWaitResource>(rid)
    .map_err(ProcessError::Resource)?;
  let cancel = RcRef::map(resource.clone(), |r| &r.cancel);
  let result = resource.clone().wait().or_cancel(cancel).await;
  // Release the descriptor/handle once the wait settles, whether it completed
  // or was cancelled.
  if let Ok(resource) = state.borrow_mut().resource_table.take_any(rid) {
    resource.close();
  }
  match result {
    Ok(inner) => inner,
    Err(canceled) => Err(ProcessError::Canceled(canceled)),
  }
}
