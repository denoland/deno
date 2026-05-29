// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::Waker;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;

/// A pending load request from the Rust module loader to JS hooks.
struct PendingLoad {
  id: u32,
  url: String,
  import_type: Option<String>,
}

/// Load hook result: (source, format, effective_url).
///
/// - `source` is `Some` when a hook provided source directly.
/// - `format` is e.g. "commonjs", "module", "builtin".
/// - `effective_url` is the URL the JS hook chain ultimately delegated to
///   via `nextLoad(newUrl)`. Only meaningful when `source` is `None`
///   (fallthrough) and the user changed the URL inside their load hook,
///   so the Rust default loader knows which URL to actually fetch from.
type LoadResult = (Option<String>, Option<String>, Option<String>);
type LoadSender =
  deno_core::futures::channel::oneshot::Sender<Result<LoadResult, String>>;

/// Callback used to perform the default ESM resolution from JS hooks.
/// Installed by the embedder so that the JS terminal `nextResolve` fallback
/// can reach the real module loader (handling bare specifiers, package
/// exports, import maps, npm/jsr, etc.) the same way an un-hooked import
/// would.
pub type DefaultResolveCb =
  Rc<dyn Fn(&str, &str) -> Result<String, JsErrorBox>>;

/// Shared hook registry between ops and the module loader.
///
/// When load hooks are active, the Rust module loader pushes requests into
/// the pending queue. The JS side polls for requests via an async op, calls
/// the user's synchronous hook function, and sends the response back via a
/// sync op.
#[derive(Clone, Default)]
pub struct LoaderHookRegistry {
  resolve_callback: Rc<RefCell<Option<v8::Global<v8::Function>>>>,
  pub load_active: Rc<Cell<bool>>,
  /// True when either a resolve or load hook is registered. Used by the
  /// import-attribute validation callback to relax validation while hooks
  /// are active so user code can use custom module types / attributes.
  pub hooks_active: Rc<Cell<bool>>,
  next_id: Rc<Cell<u32>>,

  pending_loads: Rc<RefCell<VecDeque<PendingLoad>>>,
  load_waker: Rc<RefCell<Option<Waker>>>,
  load_senders: Rc<RefCell<HashMap<u32, LoadSender>>>,
  /// Maps load request ID to URL for dedup tracking.
  load_id_keys: Rc<RefCell<HashMap<u32, String>>>,
  /// Piggybacking senders for duplicate load requests.
  load_waiters: Rc<RefCell<HashMap<String, Vec<LoadSender>>>>,
  default_resolve: Rc<RefCell<Option<DefaultResolveCb>>>,
}

impl LoaderHookRegistry {
  fn next_id(&self) -> u32 {
    let id = self.next_id.get();
    self.next_id.set(id + 1);
    id
  }

  /// Install the default-resolution callback used by the JS hook chain when
  /// the terminal `nextResolve` is reached. The embedder is expected to
  /// provide a function that performs the same resolution as a normal
  /// (un-hooked) import.
  pub fn set_default_resolve(&self, cb: DefaultResolveCb) {
    *self.default_resolve.borrow_mut() = Some(cb);
  }

  /// Call the default-resolution callback. Used by
  /// `op_module_default_resolve`.
  pub fn default_resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<String, JsErrorBox> {
    let cb = self.default_resolve.borrow().clone();
    match cb {
      Some(cb) => cb(specifier, referrer),
      None => Err(JsErrorBox::generic(
        "default module resolver is not available",
      )),
    }
  }

  pub fn resolve(
    &self,
    scope: &mut v8::PinScope,
    specifier: &str,
    referrer: &str,
    import_attributes: &HashMap<String, String>,
  ) -> Result<Option<String>, JsErrorBox> {
    let callbacks = self.resolve_callback.borrow();
    let Some(callback) = callbacks.as_ref() else {
      return Ok(None);
    };
    let callback = v8::Local::new(scope, callback);
    let recv = v8::undefined(scope).into();
    let specifier = v8::String::new(scope, specifier)
      .ok_or_else(|| JsErrorBox::generic("failed to allocate specifier"))?;
    let referrer = v8::String::new(scope, referrer)
      .ok_or_else(|| JsErrorBox::generic("failed to allocate referrer"))?;
    let attributes_obj = v8::Object::new(scope);
    for (key, value) in import_attributes {
      let k = v8::String::new(scope, key).ok_or_else(|| {
        JsErrorBox::generic("failed to allocate attribute key")
      })?;
      let v = v8::String::new(scope, value).ok_or_else(|| {
        JsErrorBox::generic("failed to allocate attribute value")
      })?;
      attributes_obj.set(scope, k.into(), v.into());
    }
    let Some(result) = callback.call(
      scope,
      recv,
      &[specifier.into(), referrer.into(), attributes_obj.into()],
    ) else {
      return Err(JsErrorBox::generic("module resolve hook failed"));
    };
    if result.is_null_or_undefined() {
      return Ok(None);
    }
    if result.is_string() {
      let result = v8::Local::<v8::String>::try_from(result)
        .map_err(|_| JsErrorBox::generic("module resolve hook failed"))?;
      return Ok(Some(result.to_rust_string_lossy(scope)));
    }
    if let Ok(result) = v8::Local::<v8::Object>::try_from(result) {
      let error_key = v8::String::new(scope, "error")
        .ok_or_else(|| JsErrorBox::generic("failed to allocate error key"))?;
      if let Some(error) = result.get(scope, error_key.into())
        && !error.is_null_or_undefined()
      {
        let error = error
          .to_string(scope)
          .ok_or_else(|| JsErrorBox::generic("module resolve hook failed"))?;
        return Err(JsErrorBox::generic(error.to_rust_string_lossy(scope)));
      }
    }
    Err(JsErrorBox::generic(
      "module resolve hook must return a string or null",
    ))
  }

  /// Push a load request and return a receiver for the response.
  /// `Ok((Some(source), format, _))` = hook provided source,
  /// `Ok((None, _, effective_url))` = fallthrough, optionally redirected.
  /// `import_type` is the `type` value from `with { type: "..." }` import
  /// attributes, forwarded to JS so hooks see the correct
  /// `context.importAttributes`.
  pub fn push_load(
    &self,
    url: String,
    import_type: Option<String>,
  ) -> deno_core::futures::channel::oneshot::Receiver<Result<LoadResult, String>>
  {
    // Dedup: if there's already a pending load for this URL, piggyback.
    if self.load_waiters.borrow().contains_key(&url) {
      let (sender, receiver) = deno_core::futures::channel::oneshot::channel();
      self
        .load_waiters
        .borrow_mut()
        .get_mut(&url)
        .unwrap()
        .push(sender);
      return receiver;
    }
    self
      .load_waiters
      .borrow_mut()
      .insert(url.clone(), Vec::new());

    let id = self.next_id();
    let (sender, receiver) = deno_core::futures::channel::oneshot::channel();
    self.load_senders.borrow_mut().insert(id, sender);
    self.load_id_keys.borrow_mut().insert(id, url.clone());
    self
      .pending_loads
      .borrow_mut()
      .push_back(PendingLoad { id, url, import_type });
    if let Some(waker) = self.load_waker.borrow_mut().take() {
      waker.wake();
    }
    receiver
  }
}

/// Mark hooks as active. Called from JS when `registerHooks()` is invoked.
#[op2]
pub fn op_module_hooks_register(
  state: &mut OpState,
  #[scoped] resolve_callback: Option<v8::Global<v8::Function>>,
  has_load: bool,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  let has_resolve = resolve_callback.is_some();
  *registry.resolve_callback.borrow_mut() = resolve_callback;
  registry.load_active.set(has_load);
  registry.hooks_active.set(has_resolve || has_load);
}

/// Poll for a pending load request. Returns `[id, url, importType]` or null.
/// `importType` is the `type` value from `with { type: "..." }` import
/// attributes (e.g. "json", "text", "bytes"), or null when none was specified.
#[op2]
#[serde]
pub async fn op_module_hooks_poll_load(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<(u32, String, Option<String>)>, JsErrorBox> {
  let registry = state.borrow().borrow::<LoaderHookRegistry>().clone();

  std::future::poll_fn(|cx| {
    if let Some(req) = registry.pending_loads.borrow_mut().pop_front() {
      return std::task::Poll::Ready(Ok(Some((
        req.id,
        req.url,
        req.import_type,
      ))));
    }
    *registry.load_waker.borrow_mut() = Some(cx.waker().clone());
    std::task::Poll::Pending
  })
  .await
}

/// Run the default module resolver. Used by the JS hook chain's terminal
/// `nextResolve` so that hooks observing the default resolution see the real
/// URL that Deno would have resolved (bare specifiers, package exports,
/// import maps, npm/jsr, etc.) rather than a stub.
#[op2]
#[string]
pub fn op_module_default_resolve(
  state: &mut OpState,
  #[string] specifier: &str,
  #[string] referrer: &str,
) -> Result<String, JsErrorBox> {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  registry.default_resolve(specifier, referrer)
}

/// Respond to a load request. `source` is null to delegate to default loading.
/// `effective_url` is the URL the user's load hook chain delegated to via
/// `nextLoad(newUrl)`; the Rust default loader fetches source from there
/// while keeping the original URL as the module's identity.
#[op2]
pub fn op_module_hooks_respond_load(
  state: &mut OpState,
  id: u32,
  #[string] source: Option<String>,
  #[string] format: Option<String>,
  #[string] error: Option<String>,
  #[string] effective_url: Option<String>,
) {
  let registry = state.borrow::<LoaderHookRegistry>().clone();
  let result: Result<LoadResult, String> = if let Some(err) = error {
    Err(err)
  } else {
    Ok((source, format, effective_url))
  };
  // Fulfill piggybacking waiters.
  if let Some(key) = registry.load_id_keys.borrow_mut().remove(&id)
    && let Some(waiters) = registry.load_waiters.borrow_mut().remove(&key)
  {
    for waiter in waiters {
      let _ = waiter.send(result.clone());
    }
  }
  if let Some(sender) = registry.load_senders.borrow_mut().remove(&id) {
    let _ = sender.send(result);
  }
}
