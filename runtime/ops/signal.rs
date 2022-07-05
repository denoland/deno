// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(unix)]
use tokio::signal::unix::{signal, Signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows::{ctrl_break, ctrl_c, CtrlBreak, CtrlC};

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

// TODO: CtrlClose could be mapped to SIGHUP but that needs a
// tokio::windows::signal::CtrlClose type, or something from a different crate
#[cfg(windows)]
enum WindowsSignal {
  Sigint(CtrlC),
  Sigbreak(CtrlBreak),
}

#[cfg(windows)]
impl From<CtrlC> for WindowsSignal {
  fn from(ctrl_c: CtrlC) -> Self {
    WindowsSignal::Sigint(ctrl_c)
  }
}

#[cfg(windows)]
impl From<CtrlBreak> for WindowsSignal {
  fn from(ctrl_break: CtrlBreak) -> Self {
    WindowsSignal::Sigbreak(ctrl_break)
  }
}

#[cfg(windows)]
impl WindowsSignal {
  pub async fn recv(&mut self) -> Option<()> {
    match self {
      WindowsSignal::Sigint(ctrl_c) => ctrl_c.recv().await,
      WindowsSignal::Sigbreak(ctrl_break) => ctrl_break.recv().await,
    }
  }
}

#[cfg(windows)]
struct SignalStreamResource {
  signal: AsyncRefCell<WindowsSignal>,
  cancel: CancelHandle,
}

#[cfg(windows)]
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

#[cfg(target_os = "freebsd")]
pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, AnyError> {
  match s {
    1 => Ok("SIGHUP"),
    2 => Ok("SIGINT"),
    3 => Ok("SIGQUIT"),
    4 => Ok("SIGILL"),
    5 => Ok("SIGTRAP"),
    6 => Ok("SIGABRT"),
    7 => Ok("SIGEMT"),
    8 => Ok("SIGFPE"),
    9 => Ok("SIGKILL"),
    10 => Ok("SIGBUS"),
    11 => Ok("SIGSEGV"),
    12 => Ok("SIGSYS"),
    13 => Ok("SIGPIPE"),
    14 => Ok("SIGALRM"),
    15 => Ok("SIGTERM"),
    16 => Ok("SIGURG"),
    17 => Ok("SIGSTOP"),
    18 => Ok("SIGTSTP"),
    19 => Ok("SIGCONT"),
    20 => Ok("SIGCHLD"),
    21 => Ok("SIGTTIN"),
    22 => Ok("SIGTTOU"),
    23 => Ok("SIGIO"),
    24 => Ok("SIGXCPU"),
    25 => Ok("SIGXFSZ"),
    26 => Ok("SIGVTALRM"),
    27 => Ok("SIGPROF"),
    28 => Ok("SIGWINCH"),
    29 => Ok("SIGINFO"),
    30 => Ok("SIGUSR1"),
    31 => Ok("SIGUSR2"),
    32 => Ok("SIGTHR"),
    33 => Ok("SIGLIBRT"),
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

#[cfg(any(target_os = "android", target_os = "linux"))]
pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, AnyError> {
  match s {
    1 => Ok("SIGHUP"),
    2 => Ok("SIGINT"),
    3 => Ok("SIGQUIT"),
    4 => Ok("SIGILL"),
    5 => Ok("SIGTRAP"),
    6 => Ok("SIGABRT"),
    7 => Ok("SIGBUS"),
    8 => Ok("SIGFPE"),
    9 => Ok("SIGKILL"),
    10 => Ok("SIGUSR1"),
    11 => Ok("SIGSEGV"),
    12 => Ok("SIGUSR2"),
    13 => Ok("SIGPIPE"),
    14 => Ok("SIGALRM"),
    15 => Ok("SIGTERM"),
    16 => Ok("SIGSTKFLT"),
    17 => Ok("SIGCHLD"),
    18 => Ok("SIGCONT"),
    19 => Ok("SIGSTOP"),
    20 => Ok("SIGTSTP"),
    21 => Ok("SIGTTIN"),
    22 => Ok("SIGTTOU"),
    23 => Ok("SIGURG"),
    24 => Ok("SIGXCPU"),
    25 => Ok("SIGXFSZ"),
    26 => Ok("SIGVTALRM"),
    27 => Ok("SIGPROF"),
    28 => Ok("SIGWINCH"),
    29 => Ok("SIGIO"),
    30 => Ok("SIGPWR"),
    31 => Ok("SIGSYS"),
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

#[cfg(target_os = "macos")]
pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, AnyError> {
  match s {
    1 => Ok("SIGHUP"),
    2 => Ok("SIGINT"),
    3 => Ok("SIGQUIT"),
    4 => Ok("SIGILL"),
    5 => Ok("SIGTRAP"),
    6 => Ok("SIGABRT"),
    7 => Ok("SIGEMT"),
    8 => Ok("SIGFPE"),
    9 => Ok("SIGKILL"),
    10 => Ok("SIGBUS"),
    11 => Ok("SIGSEGV"),
    12 => Ok("SIGSYS"),
    13 => Ok("SIGPIPE"),
    14 => Ok("SIGALRM"),
    15 => Ok("SIGTERM"),
    16 => Ok("SIGURG"),
    17 => Ok("SIGSTOP"),
    18 => Ok("SIGTSTP"),
    19 => Ok("SIGCONT"),
    20 => Ok("SIGCHLD"),
    21 => Ok("SIGTTIN"),
    22 => Ok("SIGTTOU"),
    23 => Ok("SIGIO"),
    24 => Ok("SIGXCPU"),
    25 => Ok("SIGXFSZ"),
    26 => Ok("SIGVTALRM"),
    27 => Ok("SIGPROF"),
    28 => Ok("SIGWINCH"),
    29 => Ok("SIGINFO"),
    30 => Ok("SIGUSR1"),
    31 => Ok("SIGUSR2"),
    _ => Err(type_error(format!("Invalid signal: {}", s))),
  }
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, AnyError> {
  match s {
    "SIGHUP" => Ok(1),
    "SIGINT" => Ok(2),
    "SIGQUIT" => Ok(3),
    "SIGILL" => Ok(4),
    "SIGTRAP" => Ok(5),
    "SIGIOT" => Ok(6),
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
    "SIGUSR1" => Ok(16),
    "SIGUSR2" => Ok(17),
    "SIGCLD" => Ok(18),
    "SIGCHLD" => Ok(18),
    "SIGPWR" => Ok(19),
    "SIGWINCH" => Ok(20),
    "SIGURG" => Ok(21),
    "SIGPOLL" => Ok(22),
    "SIGIO" => Ok(22),
    "SIGSTOP" => Ok(23),
    "SIGTSTP" => Ok(24),
    "SIGCONT" => Ok(25),
    "SIGTTIN" => Ok(26),
    "SIGTTOU" => Ok(27),
    "SIGVTALRM" => Ok(28),
    "SIGPROF" => Ok(29),
    "SIGXCPU" => Ok(30),
    "SIGXFSZ" => Ok(31),
    "SIGWAITING" => Ok(32),
    "SIGLWP" => Ok(33),
    "SIGFREEZE" => Ok(34),
    "SIGTHAW" => Ok(35),
    "SIGCANCEL" => Ok(36),
    "SIGLOST" => Ok(37),
    "SIGXRES" => Ok(38),
    "SIGJVM1" => Ok(39),
    "SIGJVM2" => Ok(40),
    _ => Err(type_error(format!("Invalid signal : {}", s))),
  }
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, AnyError> {
  match s {
    1 => Ok("SIGHUP"),
    2 => Ok("SIGINT"),
    3 => Ok("SIGQUIT"),
    4 => Ok("SIGILL"),
    5 => Ok("SIGTRAP"),
    6 => Ok("SIGABRT"),
    7 => Ok("SIGEMT"),
    8 => Ok("SIGFPE"),
    9 => Ok("SIGKILL"),
    10 => Ok("SIGBUS"),
    11 => Ok("SIGSEGV"),
    12 => Ok("SIGSYS"),
    13 => Ok("SIGPIPE"),
    14 => Ok("SIGALRM"),
    15 => Ok("SIGTERM"),
    16 => Ok("SIGUSR1"),
    17 => Ok("SIGUSR2"),
    18 => Ok("SIGCHLD"),
    19 => Ok("SIGPWR"),
    20 => Ok("SIGWINCH"),
    21 => Ok("SIGURG"),
    22 => Ok("SIGPOLL"),
    23 => Ok("SIGSTOP"),
    24 => Ok("SIGTSTP"),
    25 => Ok("SIGCONT"),
    26 => Ok("SIGTTIN"),
    27 => Ok("SIGTTOU"),
    28 => Ok("SIGVTALRM"),
    29 => Ok("SIGPROF"),
    30 => Ok("SIGXCPU"),
    31 => Ok("SIGXFSZ"),
    32 => Ok("SIGWAITING"),
    33 => Ok("SIGLWP"),
    34 => Ok("SIGFREEZE"),
    35 => Ok("SIGTHAW"),
    36 => Ok("SIGCANCEL"),
    37 => Ok("SIGLOST"),
    38 => Ok("SIGXRES"),
    39 => Ok("SIGJVM1"),
    40 => Ok("SIGJVM2"),
    _ => Err(type_error(format!("Invalid signal : {}", s))),
  }
}

#[cfg(target_os = "windows")]
pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, AnyError> {
  match s {
    "SIGINT" => Ok(2),
    "SIGBREAK" => Ok(21),
    _ => Err(type_error(
      "Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK).",
    )),
  }
}

#[cfg(target_os = "windows")]
pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, AnyError> {
  match s {
    2 => Ok("SIGINT"),
    21 => Ok("SIGBREAK"),
    _ => Err(type_error(
      "Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK).",
    )),
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

#[cfg(windows)]
#[op]
fn op_signal_bind(
  state: &mut OpState,
  sig: String,
) -> Result<ResourceId, AnyError> {
  let signo = signal_str_to_int(&sig)?;
  let resource = SignalStreamResource {
    signal: AsyncRefCell::new(match signo {
      // SIGINT
      2 => ctrl_c()
        .expect("There was an issue creating ctrl+c event stream.")
        .into(),
      // SIGBREAK
      21 => ctrl_break()
        .expect("There was an issue creating ctrl+break event stream.")
        .into(),
      _ => unimplemented!(),
    }),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

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

#[op]
pub fn op_signal_unbind(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  state.resource_table.close(rid)?;
  Ok(())
}
