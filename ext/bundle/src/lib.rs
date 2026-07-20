// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::FromV8;
use deno_core::OpState;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_error::JsErrorBox;

deno_core::extension!(
  deno_bundle_runtime,
  deps = [
    deno_web
  ],
  ops = [
    op_bundle,
    op_bundle_plugin_start,
    op_bundle_plugin_next,
    op_bundle_plugin_respond,
    op_bundle_plugin_finish,
  ],
  esm = [
    "bundle.ts"
  ],
  options = {
    bundle_provider: Option<Arc<dyn BundleProvider>>,
  },
  state = |state, options| {
    if let Some(bundle_provider) = options.bundle_provider {
      state.put(bundle_provider);
    } else {
      state.put::<Arc<dyn BundleProvider>>(Arc::new(()));
    }
  },
);

/// One JS-plugin hook invocation forwarded from the bundling thread to the
/// JS runtime. The JS side runs the user's plugin chain and answers over
/// `respond`; `None` means "no plugin handled this, use the default".
pub struct PluginHookRequest {
  pub kind: PluginHookRequestKind,
  pub respond: tokio::sync::oneshot::Sender<Option<PluginHookJsResult>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "hook", rename_all = "camelCase")]
pub enum PluginHookRequestKind {
  #[serde(rename_all = "camelCase")]
  Resolve {
    specifier: String,
    importer: Option<String>,
    kind: String,
  },
  #[serde(rename_all = "camelCase")]
  Load { id: String },
  #[serde(rename_all = "camelCase")]
  Transform { id: String, code: String },
}

/// The (already plugin-chain-reduced) answer from JS for a hook request.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHookJsResult {
  /// Resolved id (resolve hook).
  pub id: Option<String>,
  /// Whether the resolved id is external (resolve hook).
  pub external: Option<bool>,
  /// Module/transformed source (load and transform hooks).
  pub code: Option<String>,
  /// How to interpret `code` (load hook): js, jsx, ts, tsx, json, text,
  /// binary.
  pub loader: Option<String>,
  /// A JS plugin hook threw; fail the module with this message.
  pub error: Option<String>,
}

/// Sender handed to the `BundleProvider` when JS plugins participate in a
/// build; the bundler's resolve/load/transform stages forward hook requests
/// through it.
pub type PluginHookTx = tokio::sync::mpsc::Sender<PluginHookRequest>;

#[async_trait]
impl BundleProvider for () {
  async fn bundle(
    &self,
    _options: BundleOptions,
    _plugins: Option<PluginHookTx>,
  ) -> Result<BuildResponse, AnyError> {
    // Embedders that don't wire up a real provider (notably `denort`, used
    // by `deno compile` outputs) fall through to this no-op implementation.
    // Surface that limitation directly to the user instead of leaking the
    // historical "default BundleProvider does not do anything" string.
    // See denoland/deno#31597.
    Err(deno_core::anyhow::anyhow!(
      "Deno.bundle() is not available in compiled binaries (`deno compile`). \
       Run with `deno run` instead, or pre-bundle the entrypoints at build time."
    ))
  }
}

#[async_trait]
pub trait BundleProvider: Send + Sync {
  async fn bundle(
    &self,
    options: BundleOptions,
    plugins: Option<PluginHookTx>,
  ) -> Result<BuildResponse, AnyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Default, FromV8)]
pub struct BundleOptions {
  pub entrypoints: Vec<String>,
  pub output_path: Option<String>,
  pub output_dir: Option<String>,
  #[from_v8(default)]
  pub external: Vec<String>,
  #[from_v8(serde, default)]
  pub format: BundleFormat,
  #[from_v8(default)]
  pub minify: bool,
  #[from_v8(default)]
  pub keep_names: bool,
  #[from_v8(default)]
  pub code_splitting: bool,
  #[from_v8(default = true)]
  pub inline_imports: bool,
  #[from_v8(serde, default)]
  pub packages: PackageHandling,
  #[from_v8(serde)]
  pub sourcemap: Option<SourceMapType>,
  #[from_v8(serde, default)]
  pub platform: BundlePlatform,
  #[from_v8(default = true)]
  pub write: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundlePlatform {
  Browser,
  #[default]
  Deno,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BundleFormat {
  #[default]
  Esm,
  Cjs,
  Iife,
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceMapType {
  #[default]
  Linked,
  Inline,
  External,
}

impl std::fmt::Display for BundleFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      BundleFormat::Esm => write!(f, "esm"),
      BundleFormat::Cjs => write!(f, "cjs"),
      BundleFormat::Iife => write!(f, "iife"),
    }
  }
}

impl std::fmt::Display for SourceMapType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SourceMapType::Linked => write!(f, "linked"),
      SourceMapType::Inline => write!(f, "inline"),
      SourceMapType::External => write!(f, "external"),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageHandling {
  #[default]
  Bundle,
  External,
}

impl std::fmt::Display for PackageHandling {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PackageHandling::Bundle => write!(f, "bundle"),
      PackageHandling::External => write!(f, "external"),
    }
  }
}
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Message {
  pub text: String,
  pub location: Option<Location>,
  pub notes: Vec<Note>,
}

#[derive(Debug, Clone, FromV8, ToV8)]
pub struct PartialMessage {
  pub id: Option<String>,
  pub plugin_name: Option<String>,
  pub text: Option<String>,
  pub location: Option<Location>,
  pub notes: Option<Vec<Note>>,
  pub detail: Option<u32>,
}

#[derive(Debug, Clone, ToV8)]
pub struct BuildOutputFile {
  pub path: String,
  pub contents: Option<Uint8Array>,
  pub hash: String,
}
#[derive(Debug, Clone, ToV8)]
pub struct BuildResponse {
  pub errors: Vec<Message>,
  pub warnings: Vec<Message>,
  pub output_files: Option<Vec<BuildOutputFile>>,
}
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Note {
  pub text: String,
  pub location: Option<Location>,
}
#[derive(Debug, Clone, FromV8, ToV8)]
pub struct Location {
  pub file: String,
  pub namespace: Option<String>,
  pub line: u32,
  pub column: u32,
  pub length: Option<u32>,
  pub suggestion: Option<String>,
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<regex::Regex, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::Deserialize;
  let s = String::deserialize(deserializer)?;
  regex::Regex::new(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnResolveOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnLoadOptions {
  #[serde(deserialize_with = "deserialize_regex")]
  pub filter: regex::Regex,
  pub namespace: Option<String>,
}

#[op2]
pub async fn op_bundle(
  state: Rc<RefCell<OpState>>,
  #[scoped] options: BundleOptions,
) -> Result<BuildResponse, JsErrorBox> {
  // eprintln!("op_bundle: {:?}", options);
  let provider = {
    let state = state.borrow();
    state.borrow::<Arc<dyn BundleProvider>>().clone()
  };

  provider
    .bundle(options, None)
    .await
    .map_err(|e| JsErrorBox::generic(e.to_string()))
}

// --- JS plugin support ---
//
// `Deno.bundle()` with `plugins` runs the build with a request/response
// bridge: the provider bundles on its own thread and forwards each
// resolve/load/transform hook over an mpsc channel; the JS side pumps
// requests with `op_bundle_plugin_next`, runs the user's plugin chain, and
// answers with `op_bundle_plugin_respond`. When the pump returns `null` the
// build is done and `op_bundle_plugin_finish` yields the response.

type HookReceiver = tokio::sync::mpsc::Receiver<PluginHookRequest>;
type DoneReceiver =
  tokio::sync::oneshot::Receiver<Result<BuildResponse, AnyError>>;

struct PluginBundleSession {
  hook_rx: Rc<tokio::sync::Mutex<HookReceiver>>,
  done_rx: Rc<tokio::sync::Mutex<Option<DoneReceiver>>>,
  pending:
    HashMap<u32, tokio::sync::oneshot::Sender<Option<PluginHookJsResult>>>,
  next_request_id: u32,
  finished: Option<Result<BuildResponse, AnyError>>,
}

#[derive(Default)]
struct PluginBundleSessions {
  next_id: u32,
  sessions: HashMap<u32, PluginBundleSession>,
}

/// A `PluginHookRequestKind` tagged with the id JS must echo back in
/// `op_bundle_plugin_respond`. The routing id is named `requestId` so it
/// doesn't collide with a hook payload's own `id` field when flattened.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginHookRequestWire {
  request_id: u32,
  #[serde(flatten)]
  kind: PluginHookRequestKind,
}

// This op is `async` even though its body doesn't await: a sync op2 with a
// `#[scoped]` argument takes the fast-call path, where the v8 scope the
// `BundleOptions` conversion needs isn't available. Making it async avoids
// that path.
#[allow(
  clippy::unused_async,
  reason = "async is required for the #[scoped] argument conversion; see comment above"
)]
#[op2]
#[smi]
pub async fn op_bundle_plugin_start(
  state: Rc<RefCell<OpState>>,
  #[scoped] options: BundleOptions,
) -> u32 {
  let mut state = state.borrow_mut();
  let provider = state.borrow::<Arc<dyn BundleProvider>>().clone();
  let (hook_tx, hook_rx) = tokio::sync::mpsc::channel(16);
  let (done_tx, done_rx) = tokio::sync::oneshot::channel();
  deno_core::unsync::spawn(async move {
    let result = provider.bundle(options, Some(hook_tx)).await;
    let _ = done_tx.send(result);
  });

  if !state.has::<PluginBundleSessions>() {
    state.put(PluginBundleSessions::default());
  }
  let sessions = state.borrow_mut::<PluginBundleSessions>();
  sessions.next_id += 1;
  let id = sessions.next_id;
  sessions.sessions.insert(
    id,
    PluginBundleSession {
      hook_rx: Rc::new(tokio::sync::Mutex::new(hook_rx)),
      done_rx: Rc::new(tokio::sync::Mutex::new(Some(done_rx))),
      pending: HashMap::new(),
      next_request_id: 0,
      finished: None,
    },
  );
  id
}

/// Waits for the next plugin hook request, or `null` once the build has
/// finished. Must not be called concurrently for the same session (the
/// `Deno.bundle()` wrapper pumps serially).
#[op2]
#[serde]
pub async fn op_bundle_plugin_next(
  state: Rc<RefCell<OpState>>,
  #[smi] session_id: u32,
) -> Result<Option<PluginHookRequestWire>, JsErrorBox> {
  let (hook_rx, done_rx) = {
    let mut state = state.borrow_mut();
    let sessions = state.borrow_mut::<PluginBundleSessions>();
    let session = sessions
      .sessions
      .get(&session_id)
      .ok_or_else(|| JsErrorBox::generic("unknown bundle session"))?;
    if session.finished.is_some() {
      return Ok(None);
    }
    (session.hook_rx.clone(), session.done_rx.clone())
  };

  // These tokio mutexes are per-session and only this op locks them, which
  // it does serially (the JS pump awaits each call). Holding the guards
  // across the await below is intentional and safe.
  let mut done = done_rx.lock().await;
  if done.is_none() {
    return Ok(None);
  }
  let mut hooks = hook_rx.lock().await;

  let request = tokio::select! {
    biased;
    request = hooks.recv() => request,
    result = async { done.as_mut().unwrap().await } => {
      let result = result
        .map_err(|_| JsErrorBox::generic("bundle task disappeared"))?;
      *done = None;
      finish_session(&state, session_id, result);
      return Ok(None);
    }
  };

  match request {
    Some(request) => {
      let mut state = state.borrow_mut();
      let sessions = state.borrow_mut::<PluginBundleSessions>();
      let session = sessions
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| JsErrorBox::generic("unknown bundle session"))?;
      session.next_request_id += 1;
      let request_id = session.next_request_id;
      session.pending.insert(request_id, request.respond);
      Ok(Some(PluginHookRequestWire {
        request_id,
        kind: request.kind,
      }))
    }
    // The bundling side dropped the channel; wait for the result.
    None => {
      let result = done
        .as_mut()
        .unwrap()
        .await
        .map_err(|_| JsErrorBox::generic("bundle task disappeared"))?;
      *done = None;
      finish_session(&state, session_id, result);
      Ok(None)
    }
  }
}

fn finish_session(
  state: &Rc<RefCell<OpState>>,
  session_id: u32,
  result: Result<BuildResponse, AnyError>,
) {
  let mut state = state.borrow_mut();
  let sessions = state.borrow_mut::<PluginBundleSessions>();
  if let Some(session) = sessions.sessions.get_mut(&session_id) {
    session.finished = Some(result);
  }
}

#[op2]
pub fn op_bundle_plugin_respond(
  state: &mut OpState,
  #[smi] session_id: u32,
  #[smi] request_id: u32,
  #[serde] result: Option<PluginHookJsResult>,
) -> Result<(), JsErrorBox> {
  let sessions = state.borrow_mut::<PluginBundleSessions>();
  let session = sessions
    .sessions
    .get_mut(&session_id)
    .ok_or_else(|| JsErrorBox::generic("unknown bundle session"))?;
  let respond = session
    .pending
    .remove(&request_id)
    .ok_or_else(|| JsErrorBox::generic("unknown bundle hook request"))?;
  let _ = respond.send(result);
  Ok(())
}

#[op2]
pub fn op_bundle_plugin_finish(
  state: &mut OpState,
  #[smi] session_id: u32,
) -> Result<BuildResponse, JsErrorBox> {
  let sessions = state.borrow_mut::<PluginBundleSessions>();
  let session = sessions
    .sessions
    .remove(&session_id)
    .ok_or_else(|| JsErrorBox::generic("unknown bundle session"))?;
  match session.finished {
    Some(result) => result.map_err(|e| JsErrorBox::generic(e.to_string())),
    None => Err(JsErrorBox::generic("bundle has not finished yet")),
  }
}
