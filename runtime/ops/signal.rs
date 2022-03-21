// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
#[cfg(not(unix))]
use deno_core::error::generic_error;
#[cfg(not(target_os = "windows"))]
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;

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
use tokio::signal::unix::{signal, Signal, SignalKind};

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
  signal: AsyncRefCell<Signal>,
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
pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, AnyError> {
  match s {
    "SIGHUP" => Ok(1),
    "SIGINT" => Ok(2),
    "SIGQUIT" => Ok(3),
    "SIGILL" => Ok(4),
    "SIGTRAP" => Ok(5),
    "SIGABRT" => Ok(6),
    "SIGEMT" => Ok(7),
    "SIGFPE" => Ok(8),
    "SIGKILL" => Ok(9),
    "SIGBUS" => Ok(10),
    "SIGSEGV" => Ok(11),
    "SIGSYS" => Ok(12),
    "SIGPIPE" => Ok(13),
    "SIGALRM" => Ok(14),
    "SIGTERM" => Ok(15),
    "SIGURG" => Ok(16),
    "SIGSTOP" => Ok(17),
    "SIGTSTP" => Ok(18),
    "SIGCONT" => Ok(19),
    "SIGCHLD" => Ok(20),
    "SIGTTIN" => Ok(21),
    "SIGTTOU" => Ok(22),
    "SIGIO" => Ok(23),
    "SIGXCPU" => Ok(24),
    "SIGXFSZ" => Ok(25),
    "SIGVTALRM" => Ok(26),
    "SIGPROF" => Ok(27),
    "SIGWINCH" => Ok(28),
    "SIGINFO" => Ok(29),
    "SIGUSR1" => Ok(30),
    "SIGUSR2" => Ok(31),
    "SIGTHR" => Ok(32),
    "SIGLIBRT" => Ok(33),
    _ => Err(type_error(format!("Invalid signal : {}", s))),
  }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, AnyError> {
  match s {
    "SIGHUP" => Ok(1),
    "SIGINT" => Ok(2),
    "SIGQUIT" => Ok(3),
    "SIGILL" => Ok(4),
    "SIGTRAP" => Ok(5),
    "SIGABRT" => Ok(6),
    "SIGBUS" => Ok(7),
    "SIGFPE" => Ok(8),
    "SIGKILL" => Ok(9),
    "SIGUSR1" => Ok(10),
    "SIGSEGV" => Ok(11),
    "SIGUSR2" => Ok(12),
    "SIGPIPE" => Ok(13),
    "SIGALRM" => Ok(14),
    "SIGTERM" => Ok(15),
    "SIGSTKFLT" => Ok(16),
    "SIGCHLD" => Ok(17),
    "SIGCONT" => Ok(18),
    "SIGSTOP" => Ok(19),
    "SIGTSTP" => Ok(20),
    "SIGTTIN" => Ok(21),
    "SIGTTOU" => Ok(22),
    "SIGURG" => Ok(23),
    "SIGXCPU" => Ok(24),
    "SIGXFSZ" => Ok(25),
    "SIGVTALRM" => Ok(26),
    "SIGPROF" => Ok(27),
    "SIGWINCH" => Ok(28),
    "SIGIO" => Ok(29),
    "SIGPWR" => Ok(30),
    "SIGSYS" => Ok(31),
    _ => Err(type_error(format!("Invalid signal : {}", s))),
  }
}

#[cfg(target_os = "macos")]
pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, AnyError> {
  match s {
    "SIGHUP" => Ok(1),
    "SIGINT" => Ok(2),
    "SIGQUIT" => Ok(3),
    "SIGILL" => Ok(4),
    "SIGTRAP" => Ok(5),
    "SIGABRT" => Ok(6),
    "SIGEMT" => Ok(7),
    "SIGFPE" => Ok(8),
    "SIGKILL" => Ok(9),
    "SIGBUS" => Ok(10),
    "SIGSEGV" => Ok(11),
    "SIGSYS" => Ok(12),
    "SIGPIPE" => Ok(13),
    "SIGALRM" => Ok(14),
    "SIGTERM" => Ok(15),
    "SIGURG" => Ok(16),
    "SIGSTOP" => Ok(17),
    "SIGTSTP" => Ok(18),
    "SIGCONT" => Ok(19),
    "SIGCHLD" => Ok(20),
    "SIGTTIN" => Ok(21),
    "SIGTTOU" => Ok(22),
    "SIGIO" => Ok(23),
    "SIGXCPU" => Ok(24),
    "SIGXFSZ" => Ok(25),
    "SIGVTALRM" => Ok(26),
    "SIGPROF" => Ok(27),
    "SIGWINCH" => Ok(28),
    "SIGINFO" => Ok(29),
    "SIGUSR1" => Ok(30),
    "SIGUSR2" => Ok(31),
    _ => Err(type_error(format!("Invalid signal: {}", s))),
  }
}

#[cfg(unix)]
#[op]
fn op_signal_bind(
  state: &mut OpState,
  sig: String,
) -> Result<ResourceId, AnyError> {
  let signo = signal_str_to_int(&sig)?;
  if signal_hook_registry::FORBIDDEN.contains(&signo) {
    return Err(type_error(format!(
      "Binding to signal '{}' is not allowed",
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
