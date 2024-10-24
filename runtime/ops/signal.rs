// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;

use std::borrow::Cow;
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::BTreeMap;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use tokio::signal::unix::signal;
#[cfg(unix)]
use tokio::signal::unix::Signal;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
#[cfg(windows)]
use tokio::signal::windows::ctrl_break;
#[cfg(windows)]
use tokio::signal::windows::ctrl_c;
#[cfg(windows)]
use tokio::signal::windows::CtrlBreak;
#[cfg(windows)]
use tokio::signal::windows::CtrlC;

deno_core::extension!(
  deno_signal,
  ops = [op_signal_bind, op_signal_unbind, op_signal_poll],
  state = |state| {
    #[cfg(unix)]
    {
      state.put(SignalState::default());
    }
  }
);

#[derive(Debug, thiserror::Error)]
pub enum SignalError {
  #[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "openbsd",
    target_os = "openbsd",
    target_os = "macos",
    target_os = "solaris",
    target_os = "illumos"
  ))]
  #[error("Invalid signal: {0}")]
  InvalidSignalStr(String),
  #[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "openbsd",
    target_os = "openbsd",
    target_os = "macos",
    target_os = "solaris",
    target_os = "illumos"
  ))]
  #[error("Invalid signal: {0}")]
  InvalidSignalInt(libc::c_int),
  #[cfg(target_os = "windows")]
  #[error("Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK), but got {0}")]
  InvalidSignalStr(String),
  #[cfg(target_os = "windows")]
  #[error("Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK), but got {0}")]
  InvalidSignalInt(libc::c_int),
  #[error("Binding to signal '{0}' is not allowed")]
  SignalNotAllowed(String),
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

#[cfg(unix)]
#[derive(Default)]
struct SignalState {
  enable_default_handlers: BTreeMap<libc::c_int, Arc<AtomicBool>>,
}

#[cfg(unix)]
impl SignalState {
  /// Disable the default signal handler for the given signal.
  ///
  /// Returns the shared flag to enable the default handler later, and whether a default handler already existed.
  fn disable_default_handler(
    &mut self,
    signo: libc::c_int,
  ) -> (Arc<AtomicBool>, bool) {
    use std::collections::btree_map::Entry;

    match self.enable_default_handlers.entry(signo) {
      Entry::Occupied(entry) => {
        let enable = entry.get();
        enable.store(false, std::sync::atomic::Ordering::Release);
        (enable.clone(), true)
      }
      Entry::Vacant(entry) => {
        let enable = Arc::new(AtomicBool::new(false));
        entry.insert(enable.clone());
        (enable, false)
      }
    }
  }
}

#[cfg(unix)]
/// The resource for signal stream.
/// The second element is the waker of polling future.
struct SignalStreamResource {
  signal: AsyncRefCell<Signal>,
  enable_default_handler: Arc<AtomicBool>,
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

macro_rules! first_literal {
  ($head:literal $(, $tail:literal)*) => {
    $head
  };
}
macro_rules! signal_dict {
  ($(($number:literal, $($name:literal)|+)),*) => {
    pub fn signal_str_to_int(s: &str) -> Result<libc::c_int, SignalError> {
      match s {
        $($($name)|* => Ok($number),)*
        _ => Err(SignalError::InvalidSignalStr(s.to_string())),
      }
    }

    pub fn signal_int_to_str(s: libc::c_int) -> Result<&'static str, SignalError> {
      match s {
        $($number => Ok(first_literal!($($name),+)),)*
        _ => Err(SignalError::InvalidSignalInt(s)),
      }
    }
  }
}

#[cfg(target_os = "freebsd")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2"),
  (32, "SIGTHR"),
  (33, "SIGLIBRT")
);

#[cfg(target_os = "openbsd")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2"),
  (32, "SIGTHR")
);

#[cfg(any(target_os = "android", target_os = "linux"))]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGBUS"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGUSR1"),
  (11, "SIGSEGV"),
  (12, "SIGUSR2"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGSTKFLT"),
  (17, "SIGCHLD"),
  (18, "SIGCONT"),
  (19, "SIGSTOP"),
  (20, "SIGTSTP"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGURG"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGIO" | "SIGPOLL"),
  (30, "SIGPWR"),
  (31, "SIGSYS" | "SIGUNUSED")
);

#[cfg(target_os = "macos")]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGURG"),
  (17, "SIGSTOP"),
  (18, "SIGTSTP"),
  (19, "SIGCONT"),
  (20, "SIGCHLD"),
  (21, "SIGTTIN"),
  (22, "SIGTTOU"),
  (23, "SIGIO"),
  (24, "SIGXCPU"),
  (25, "SIGXFSZ"),
  (26, "SIGVTALRM"),
  (27, "SIGPROF"),
  (28, "SIGWINCH"),
  (29, "SIGINFO"),
  (30, "SIGUSR1"),
  (31, "SIGUSR2")
);

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
signal_dict!(
  (1, "SIGHUP"),
  (2, "SIGINT"),
  (3, "SIGQUIT"),
  (4, "SIGILL"),
  (5, "SIGTRAP"),
  (6, "SIGABRT" | "SIGIOT"),
  (7, "SIGEMT"),
  (8, "SIGFPE"),
  (9, "SIGKILL"),
  (10, "SIGBUS"),
  (11, "SIGSEGV"),
  (12, "SIGSYS"),
  (13, "SIGPIPE"),
  (14, "SIGALRM"),
  (15, "SIGTERM"),
  (16, "SIGUSR1"),
  (17, "SIGUSR2"),
  (18, "SIGCHLD"),
  (19, "SIGPWR"),
  (20, "SIGWINCH"),
  (21, "SIGURG"),
  (22, "SIGPOLL"),
  (23, "SIGSTOP"),
  (24, "SIGTSTP"),
  (25, "SIGCONT"),
  (26, "SIGTTIN"),
  (27, "SIGTTOU"),
  (28, "SIGVTALRM"),
  (29, "SIGPROF"),
  (30, "SIGXCPU"),
  (31, "SIGXFSZ"),
  (32, "SIGWAITING"),
  (33, "SIGLWP"),
  (34, "SIGFREEZE"),
  (35, "SIGTHAW"),
  (36, "SIGCANCEL"),
  (37, "SIGLOST"),
  (38, "SIGXRES"),
  (39, "SIGJVM1"),
  (40, "SIGJVM2")
);

#[cfg(target_os = "windows")]
signal_dict!((2, "SIGINT"), (21, "SIGBREAK"));

#[cfg(unix)]
#[op2(fast)]
#[smi]
fn op_signal_bind(
  state: &mut OpState,
  #[string] sig: &str,
) -> Result<ResourceId, SignalError> {
  let signo = signal_str_to_int(sig)?;
  if signal_hook_registry::FORBIDDEN.contains(&signo) {
    return Err(SignalError::SignalNotAllowed(sig.to_string()));
  }

  let signal = AsyncRefCell::new(signal(SignalKind::from_raw(signo))?);

  let (enable_default_handler, has_default_handler) = state
    .borrow_mut::<SignalState>()
    .disable_default_handler(signo);

  let resource = SignalStreamResource {
    signal,
    cancel: Default::default(),
    enable_default_handler: enable_default_handler.clone(),
  };
  let rid = state.resource_table.add(resource);

  if !has_default_handler {
    // restore default signal handler when the signal is unbound
    // this can error if the signal is not supported, if so let's just leave it as is
    let _ = signal_hook::flag::register_conditional_default(
      signo,
      enable_default_handler,
    );
  }

  Ok(rid)
}

#[cfg(windows)]
#[op2(fast)]
#[smi]
fn op_signal_bind(
  state: &mut OpState,
  #[string] sig: &str,
) -> Result<ResourceId, SignalError> {
  let signo = signal_str_to_int(sig)?;
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

#[op2(async)]
async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<bool, deno_core::error::AnyError> {
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

#[op2(fast)]
pub fn op_signal_unbind(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), deno_core::error::AnyError> {
  let resource = state.resource_table.take::<SignalStreamResource>(rid)?;

  #[cfg(unix)]
  {
    resource
      .enable_default_handler
      .store(true, std::sync::atomic::Ordering::Release);
  }

  resource.close();
  Ok(())
}
