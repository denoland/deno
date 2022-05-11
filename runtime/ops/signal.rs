// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
#[cfg(not(unix))]
use deno_core::error::generic_error;
#[cfg(not(target_os = "windows"))]
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use serde::Deserialize;
use serde::Serialize;

use deno_core::Extension;
#[cfg(unix)]
use deno_core::OpState;
#[cfg(unix)]
use std::cell::RefCell;
#[cfg(unix)]
use std::rc::Rc;

#[cfg(unix)]
use deno_core::AsyncRefCell;
#[cfg(unix)]
use deno_core::CancelFuture;
#[cfg(unix)]
use deno_core::CancelHandle;
#[cfg(unix)]
use deno_core::RcRef;
#[cfg(unix)]
use deno_core::Resource;
#[cfg(unix)]
use deno_core::ResourceId;
#[cfg(unix)]
use std::borrow::Cow;
#[cfg(unix)]
use tokio::signal::unix::{signal, Signal as TokioSignal, SignalKind};

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_signal_bind::decl(),
      op_signal_unbind::decl(),
      op_signal_poll::decl(),
    ])
    .build()
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
struct SignalStreamResource {
  signal: AsyncRefCell<TokioSignal>,
  cancel: CancelHandle,
}

#[cfg(unix)]
impl Resource for SignalStreamResource {
  fn name(&self) -> Cow<str> {
    "signal".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

#[cfg(target_os = "freebsd")]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Signal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGEMT = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGBUS = 10,
  SIGSEGV = 11,
  SIGSYS = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGURG = 16,
  SIGSTOP = 17,
  SIGTSTP = 18,
  SIGCONT = 19,
  SIGCHLD = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGIO = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGINFO = 29,
  SIGUSR1 = 30,
  SIGUSR2 = 31,
  SIGTHR = 32,
  SIGLIBRT = 33,
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Signal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGBUS = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGUSR1 = 10,
  SIGSEGV = 11,
  SIGUSR2 = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGSTKFLT = 16,
  SIGCHLD = 17,
  SIGCONT = 18,
  SIGSTOP = 19,
  SIGTSTP = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGURG = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGIO = 29,
  SIGPWR = 30,
  SIGSYS = 31,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Signal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGEMT = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGBUS = 10,
  SIGSEGV = 11,
  SIGSYS = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGURG = 16,
  SIGSTOP = 17,
  SIGTSTP = 18,
  SIGCONT = 19,
  SIGCHLD = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGIO = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGINFO = 29,
  SIGUSR1 = 30,
  SIGUSR2 = 31,
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Signal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGIOT = 6,
  SIGABRT = 6,
  SIGEMT = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGBUS = 10,
  SIGSEGV = 11,
  SIGSYS = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGUSR1 = 16,
  SIGUSR2 = 17,
  SIGCHLD = 18,
  SIGPWR = 19,
  SIGWINCH = 20,
  SIGURG = 21,
  SIGPOLL = 22,
  SIGSTOP = 23,
  SIGTSTP = 24,
  SIGCONT = 25,
  SIGTTIN = 26,
  SIGTTOU = 27,
  SIGVTALRM = 28,
  SIGPROF = 29,
  SIGXCPU = 30,
  SIGXFSZ = 31,
  SIGWAITING = 32,
  SIGLWP = 33,
  SIGFREEZE = 34,
  SIGTHAW = 35,
  SIGCANCEL = 36,
  SIGLOST = 37,
  SIGXRES = 38,
  SIGJVM1 = 39,
  SIGJVM2 = 40,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Signal {
  SIGKILL,
  SIGTERM,
}

#[cfg(unix)]
#[op]
fn op_signal_bind(
  state: &mut OpState,
  sig: Signal,
) -> Result<ResourceId, AnyError> {
  let signo = sig as libc::c_int;
  if signal_hook_registry::FORBIDDEN.contains(&signo) {
    return Err(type_error(format!(
      "Binding to signal '{:?}' is not allowed",
      sig
    )));
  }
  let resource = SignalStreamResource {
    signal: AsyncRefCell::new(signal(SignalKind::from_raw(signo))?),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[cfg(unix)]
#[op]
async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<bool, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SignalStreamResource>(rid)?;
  let cancel = RcRef::map(&resource, |r| &r.cancel);
  let mut signal = RcRef::map(&resource, |r| &r.signal).borrow_mut().await;

  match signal.recv().or_cancel(cancel).await {
    Ok(result) => Ok(result.is_none()),
    Err(_) => Ok(true),
  }
}

#[cfg(unix)]
#[op]
pub fn op_signal_unbind(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  state.resource_table.close(rid)?;
  Ok(())
}

#[cfg(not(unix))]
#[op]
pub fn op_signal_bind() -> Result<(), AnyError> {
  Err(generic_error("not implemented"))
}

#[cfg(not(unix))]
#[op]
fn op_signal_unbind() -> Result<(), AnyError> {
  Err(generic_error("not implemented"))
}

#[cfg(not(unix))]
#[op]
async fn op_signal_poll() -> Result<(), AnyError> {
  Err(generic_error("not implemented"))
}
