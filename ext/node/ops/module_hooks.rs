// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::Waker;

use deno_core::OpState;
use deno_core::op2;
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

type ResolveSender =
  deno_core::futures::channel::oneshot::Sender<Result<Option<String>, String>>;
type LoadSender =
  deno_core::futures::channel::oneshot::Sender<Result<Option<String>, String>>;

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

  /// Specifiers that were intercepted by a resolve hook (not fallthrough).
  /// These are virtual modules that should skip prepare_load since they
  /// don't exist on disk.
  pub hook_intercepted_specifiers: Rc<RefCell<HashSet<String>>>,

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
  /// `Ok(Some(url))` = hook resolved, `Ok(None)` = fallthrough to default.
  pub fn push_resolve(
    &self,
    specifier: String,
    referrer: String,
  ) -> deno_core::futures::channel::oneshot::Receiver<
    Result<Option<String>, String>,
  > {
    let id = self.next_id();
    let (sender, receiver) = deno_core::futures::channel::oneshot::channel();
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
  /// `Ok(Some(source))` = hook provided source, `Ok(None)` = fallthrough.
  pub fn push_load(
    &self,
    url: String,
  ) -> deno_core::futures::channel::oneshot::Receiver<
    Result<Option<String>, String>,
  > {
    let id = self.next_id();
    let (sender, receiver) = deno_core::futures::channel::oneshot::channel();
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

/// Mark hooks as active. Called from JS when `registerHooks()` is invoked.
#[op2(fast)]
pub fn op_module_hooks_register(
  state: &mut OpState,
  has_resolve: bool,
  has_load: bool,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  registry.resolve_active.set(has_resolve);
  registry.load_active.set(has_load);
}

/// Poll for a pending resolve request. Returns `[id, specifier, referrer]`
/// or null if the registry is shut down.
#[op2]
#[serde]
pub async fn op_module_hooks_poll_resolve(
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

/// Respond to a resolve request. `url` is null for fallthrough to default.
#[op2]
pub fn op_module_hooks_respond_resolve(
  state: &mut OpState,
  id: u32,
  #[string] url: Option<String>,
  #[string] error: Option<String>,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  if let Some(sender) = registry.resolve_senders.borrow_mut().remove(&id) {
    let result: Result<Option<String>, String> = if let Some(err) = error {
      Err(err)
    } else {
      Ok(url) // None = fallthrough
    };
    let _ = sender.send(result);
  }
}

/// Poll for a pending load request. Returns `[id, url]` or null.
#[op2]
#[serde]
pub async fn op_module_hooks_poll_load(
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
pub fn op_module_hooks_respond_load(
  state: &mut OpState,
  id: u32,
  #[string] source: Option<String>,
  #[string] error: Option<String>,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  if let Some(sender) = registry.load_senders.borrow_mut().remove(&id) {
    let result: Result<Option<String>, String> = if let Some(err) = error {
      Err(err)
    } else {
      Ok(source) // None = fallthrough
    };
    let _ = sender.send(result);
  }
}
