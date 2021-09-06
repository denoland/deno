// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op_async_unref;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use std::cell::RefCell;
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
      ("op_signal_bind", op_sync(op_signal_bind)),
      ("op_signal_unbind", op_sync(op_signal_unbind)),
      ("op_signal_poll", op_async_unref(op_signal_poll)),
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

#[cfg(target_os = "linux")]
fn signal_str_to_int(s: &str) -> Option<libc::c_int> {
  match s {
    "SIGHUP" => Some(1),
    "SIGINT" => Some(2),
    "SIGQUIT" => Some(3),
    "SIGILL" => Some(4),
    "SIGTRAP" => Some(5),
    "SIGABRT" => Some(6),
    "SIGBUS" => Some(7),
    "SIGFPE" => Some(8),
    "SIGKILL" => Some(9),
    "SIGUSR1" => Some(10),
    "SIGSEGV" => Some(11),
    "SIGUSR2" => Some(12),
    "SIGPIPE" => Some(13),
    "SIGALRM" => Some(14),
    "SIGTERM" => Some(15),
    "SIGSTKFLT" => Some(16),
    "SIGCHLD" => Some(17),
    "SIGCONT" => Some(18),
    "SIGSTOP" => Some(19),
    "SIGTSTP" => Some(20),
    "SIGTTIN" => Some(21),
    "SIGTTOU" => Some(22),
    "SIGURG" => Some(23),
    "SIGXCPU" => Some(24),
    "SIGXFSZ" => Some(25),
    "SIGVTALRM" => Some(26),
    "SIGPROF" => Some(27),
    "SIGWINCH" => Some(28),
    "SIGIO" => Some(29),
    "SIGPWR" => Some(30),
    "SIGSYS" => Some(31),
    _ => None,
  }
}

#[cfg(target_os = "macos")]
fn signal_str_to_int(s: &str) -> Option<libc::c_int> {
  match s {
    "SIGHUP" => Some(1),
    "SIGINT" => Some(2),
    "SIGQUIT" => Some(3),
    "SIGILL" => Some(4),
    "SIGTRAP" => Some(5),
    "SIGABRT" => Some(6),
    "SIGEMT" => Some(7),
    "SIGFPE" => Some(8),
    "SIGKILL" => Some(9),
    "SIGBUS" => Some(10),
    "SIGSEGV" => Some(11),
    "SIGSYS" => Some(12),
    "SIGPIPE" => Some(13),
    "SIGALRM" => Some(14),
    "SIGTERM" => Some(15),
    "SIGURG" => Some(16),
    "SIGSTOP" => Some(17),
    "SIGTSTP" => Some(18),
    "SIGCONT" => Some(19),
    "SIGCHLD" => Some(20),
    "SIGTTIN" => Some(21),
    "SIGTTOU" => Some(22),
    "SIGIO" => Some(23),
    "SIGXCPU" => Some(24),
    "SIGXFSZ" => Some(25),
    "SIGVTALRM" => Some(26),
    "SIGPROF" => Some(27),
    "SIGWINCH" => Some(28),
    "SIGINFO" => Some(29),
    "SIGUSR1" => Some(30),
    "SIGUSR2" => Some(31),
    _ => None,
  }
}

#[cfg(target_os = "windows")]
fn signal_str_to_int(_s: &str) -> Option<libc::c_int> {
  unimplemented!()
}

pub fn signal_str_to_int_unwrap(s: &str) -> Result<libc::c_int, AnyError> {
  signal_str_to_int(s)
    .ok_or_else(|| type_error(format!("Invalid signal : {}", s)))
}

#[cfg(unix)]
fn op_signal_bind(
  state: &mut OpState,
  sig: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  super::check_unstable(state, "Deno.signal");
  let signo = signal_str_to_int_unwrap(&sig)?;
  let resource = SignalStreamResource {
    signal: AsyncRefCell::new(signal(SignalKind::from_raw(signo)).unwrap()),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[cfg(unix)]
async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<bool, AnyError> {
  super::check_unstable2(&state, "Deno.signal");

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
pub fn op_signal_unbind(
  state: &mut OpState,
  rid: ResourceId,
  _: (),
) -> Result<(), AnyError> {
  super::check_unstable(state, "Deno.signal");
  state.resource_table.close(rid)?;
  Ok(())
}

#[cfg(not(unix))]
pub fn op_signal_bind(
  _state: &mut OpState,
  _args: (),
  _: (),
) -> Result<(), AnyError> {
  unimplemented!();
}

#[cfg(not(unix))]
fn op_signal_unbind(
  _state: &mut OpState,
  _args: (),
  _: (),
) -> Result<(), AnyError> {
  unimplemented!();
}

#[cfg(not(unix))]
async fn op_signal_poll(
  _state: Rc<RefCell<OpState>>,
  _args: (),
  _: (),
) -> Result<(), AnyError> {
  unimplemented!();
}
