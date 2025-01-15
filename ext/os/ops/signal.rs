// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::cell::RefCell;
#[cfg(unix)]
use std::collections::BTreeMap;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::Arc;

use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SignalError {
  #[class(type)]
  #[error(transparent)]
  InvalidSignalStr(#[from] crate::signal::InvalidSignalStrError),
  #[class(type)]
  #[error(transparent)]
  InvalidSignalInt(#[from] crate::signal::InvalidSignalIntError),
  #[class(type)]
  #[error("Binding to signal '{0}' is not allowed")]
  SignalNotAllowed(String),
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
}

#[cfg(unix)]
#[derive(Default)]
pub struct SignalState {
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

#[cfg(unix)]
#[op2(fast)]
#[smi]
pub fn op_signal_bind(
  state: &mut OpState,
  #[string] sig: &str,
) -> Result<ResourceId, SignalError> {
  let signo = crate::signal::signal_str_to_int(sig)?;
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
pub fn op_signal_bind(
  state: &mut OpState,
  #[string] sig: &str,
) -> Result<ResourceId, SignalError> {
  let signo = crate::signal::signal_str_to_int(sig)?;
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
pub async fn op_signal_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<bool, ResourceError> {
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
) -> Result<(), ResourceError> {
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
