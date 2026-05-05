// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::Waker;

use deno_core::OpState;
use deno_core::op2;
use deno_core::resolve_import;
use deno_error::JsErrorBox;

/// A pending resolve request from the Rust module loader to JS hooks.
struct PendingResolve {
  id: u32,
  specifier: String,
  referrer: String,
}

/// A pending load request from the Rust module loader to JS hooks.
struct PendingLoad {
  id: u32,
  url: String,
}

type ResolveSender = futures::channel::oneshot::Sender<Result<String, String>>;
type LoadSender =
  futures::channel::oneshot::Sender<Result<Option<String>, String>>;

/// Shared hook registry between ops and the module loader.
///
/// When hooks are active, the Rust module loader pushes requests into
/// the pending queues and returns async futures. The JS side polls
/// for requests via async ops, calls the user's hook functions, and
/// sends responses back via sync ops.
#[derive(Clone, Default)]
pub struct LoaderHookRegistry {
  pub resolve_active: Rc<Cell<bool>>,
  pub load_active: Rc<Cell<bool>>,
  next_id: Rc<Cell<u32>>,

  pending_resolves: Rc<RefCell<VecDeque<PendingResolve>>>,
  resolve_waker: Rc<RefCell<Option<Waker>>>,
  resolve_senders: Rc<RefCell<HashMap<u32, ResolveSender>>>,

  pending_loads: Rc<RefCell<VecDeque<PendingLoad>>>,
  load_waker: Rc<RefCell<Option<Waker>>>,
  load_senders: Rc<RefCell<HashMap<u32, LoadSender>>>,
}

impl LoaderHookRegistry {
  fn next_id(&self) -> u32 {
    let id = self.next_id.get();
    self.next_id.set(id + 1);
    id
  }

  /// Push a resolve request and return a receiver for the response.
  pub fn push_resolve(
    &self,
    specifier: String,
    referrer: String,
  ) -> futures::channel::oneshot::Receiver<Result<String, String>> {
    let id = self.next_id();
    let (sender, receiver) = futures::channel::oneshot::channel();
    self.resolve_senders.borrow_mut().insert(id, sender);
    self
      .pending_resolves
      .borrow_mut()
      .push_back(PendingResolve {
        id,
        specifier,
        referrer,
      });
    if let Some(waker) = self.resolve_waker.borrow_mut().take() {
      waker.wake();
    }
    receiver
  }

  /// Push a load request and return a receiver for the response.
  pub fn push_load(
    &self,
    url: String,
  ) -> futures::channel::oneshot::Receiver<Result<Option<String>, String>> {
    let id = self.next_id();
    let (sender, receiver) = futures::channel::oneshot::channel();
    self.load_senders.borrow_mut().insert(id, sender);
    self
      .pending_loads
      .borrow_mut()
      .push_back(PendingLoad { id, url });
    if let Some(waker) = self.load_waker.borrow_mut().take() {
      waker.wake();
    }
    receiver
  }
}

/// Mark hooks as active. Called from JS when `register()` is invoked.
#[op2(fast)]
pub fn op_loader_register(
  state: &mut OpState,
  has_resolve: bool,
  has_load: bool,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  if has_resolve {
    registry.resolve_active.set(true);
  }
  if has_load {
    registry.load_active.set(true);
  }
}

/// Poll for a pending resolve request. Returns `[id, specifier, referrer]`
/// or null if the registry is shut down.
#[op2]
#[serde]
pub async fn op_loader_poll_resolve(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<(u32, String, String)>, JsErrorBox> {
  let registry = state.borrow().borrow::<LoaderHookRegistry>().clone();

  std::future::poll_fn(|cx| {
    if let Some(req) = registry.pending_resolves.borrow_mut().pop_front() {
      return std::task::Poll::Ready(Ok(Some((
        req.id,
        req.specifier,
        req.referrer,
      ))));
    }
    *registry.resolve_waker.borrow_mut() = Some(cx.waker().clone());
    std::task::Poll::Pending
  })
  .await
}

/// Respond to a resolve request with the resolved URL.
#[op2]
pub fn op_loader_respond_resolve(
  state: &mut OpState,
  id: u32,
  #[string] url: Option<String>,
  #[string] error: Option<String>,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  if let Some(sender) = registry.resolve_senders.borrow_mut().remove(&id) {
    let result = if let Some(err) = error {
      Err(err)
    } else {
      Ok(url.expect("url must be provided if no error"))
    };
    let _ = sender.send(result);
  }
}

/// Poll for a pending load request. Returns `[id, url]` or null.
#[op2]
#[serde]
pub async fn op_loader_poll_load(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<(u32, String)>, JsErrorBox> {
  let registry = state.borrow().borrow::<LoaderHookRegistry>().clone();

  std::future::poll_fn(|cx| {
    if let Some(req) = registry.pending_loads.borrow_mut().pop_front() {
      return std::task::Poll::Ready(Ok(Some((req.id, req.url))));
    }
    *registry.load_waker.borrow_mut() = Some(cx.waker().clone());
    std::task::Poll::Pending
  })
  .await
}

/// Respond to a load request. `source` is null to delegate to default loading.
#[op2]
pub fn op_loader_respond_load(
  state: &mut OpState,
  id: u32,
  #[string] source: Option<String>,
  #[string] error: Option<String>,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  if let Some(sender) = registry.load_senders.borrow_mut().remove(&id) {
    let result = if let Some(err) = error {
      Err(err)
    } else {
      Ok(source)
    };
    let _ = sender.send(result);
  }
}

/// Default resolve: wraps `deno_core::resolve_import` for use as `nextResolve`.
#[op2]
#[string]
pub fn op_loader_default_resolve(
  #[string] specifier: &str,
  #[string] referrer: &str,
) -> Result<String, JsErrorBox> {
  resolve_import(specifier, referrer)
    .map(|url| url.to_string())
    .map_err(JsErrorBox::from_err)
}
