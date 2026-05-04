// Copyright 2018-2026 the Deno authors. MIT license.

//! Simplified HMR (Hot Module Replacement) for the standalone/desktop runtime.
//!
//! Watches source files on disk, transpiles changed TypeScript/TSX/JSX files
//! using `deno_ast`, and hot-replaces them via V8's `Debugger.setScriptSource`.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::Duration;

use deno_core::LocalInspectorSession;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use deno_core::url::Url;
use deno_error::JsErrorBox;
use notify::RecursiveMode;
use notify::Watcher;
use notify::event::ModifyKind;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Coalesce events that arrive within this window into one transpile +
/// `setScriptSource` round-trip. Editors that save-then-format (or do
/// atomic-rename saves) emit several events per keystroke; without this we
/// would re-transpile each one.
const HMR_DEBOUNCE: Duration = Duration::from_millis(50);

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);
fn next_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// Minimal CDP types needed for HMR.
mod cdp {
  use serde::Deserialize;
  use serde_json::Value;

  #[derive(Debug, Deserialize)]
  pub struct Notification {
    pub method: String,
    pub params: Value,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct ScriptParsed {
    pub script_id: String,
    pub url: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct ExceptionThrown {
    pub exception_details: ExceptionDetails,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct ExceptionDetails {
    pub text: String,
    pub exception: Option<RemoteObject>,
  }

  #[derive(Debug, Clone, Deserialize)]
  pub struct RemoteObject {
    pub description: Option<String>,
  }

  impl ExceptionDetails {
    pub fn get_message_and_description(&self) -> (String, String) {
      let description = self
        .exception
        .clone()
        .and_then(|ex| ex.description)
        .unwrap_or_else(|| "undefined".to_string());
      (self.text.to_string(), description)
    }
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct SetScriptSourceResponse {
    pub status: Status,
    pub exception_details: Option<ExceptionDetails>,
  }

  #[derive(Debug, Deserialize)]
  pub enum Status {
    Ok,
    CompileError,
    BlockedByActiveGenerator,
    BlockedByActiveFunction,
    BlockedByTopLevelEsModuleChange,
  }
}

fn explain(response: &cdp::SetScriptSourceResponse) -> String {
  match response.status {
    cdp::Status::Ok => "OK".to_string(),
    cdp::Status::CompileError => {
      if let Some(details) = &response.exception_details {
        let (message, description) = details.get_message_and_description();
        format!(
          "compile error: {}{}",
          message,
          if description == "undefined" {
            "".to_string()
          } else {
            format!(" - {}", description)
          }
        )
      } else {
        "compile error: No exception details available".to_string()
      }
    }
    cdp::Status::BlockedByActiveGenerator => {
      "blocked by active generator".to_string()
    }
    cdp::Status::BlockedByActiveFunction => {
      "blocked by active function".to_string()
    }
    cdp::Status::BlockedByTopLevelEsModuleChange => {
      "blocked by top-level ES module change".to_string()
    }
  }
}

fn should_retry(status: &cdp::Status) -> bool {
  matches!(
    status,
    cdp::Status::BlockedByActiveGenerator
      | cdp::Status::BlockedByActiveFunction
  )
}

/// Transpile a TypeScript/TSX/JSX source file to JavaScript for HMR.
fn transpile_for_hmr(
  specifier: &Url,
  source_code: String,
) -> Result<String, JsErrorBox> {
  use deno_ast::*;
  let media_type = deno_media_type::MediaType::from_specifier(specifier);
  match media_type {
    deno_media_type::MediaType::TypeScript
    | deno_media_type::MediaType::Mts
    | deno_media_type::MediaType::Cts
    | deno_media_type::MediaType::Jsx
    | deno_media_type::MediaType::Tsx => {
      let parsed = parse_module(ParseParams {
        specifier: specifier.clone(),
        text: source_code.into(),
        media_type,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })
      .map_err(JsErrorBox::from_err)?;

      let transpiled = parsed
        .transpile(
          &TranspileOptions::default(),
          &TranspileModuleOptions::default(),
          &EmitOptions {
            source_map: SourceMapOption::None,
            ..Default::default()
          },
        )
        .map_err(JsErrorBox::from_err)?
        .into_source();
      Ok(transpiled.text)
    }
    // JS files don't need transpilation
    _ => Ok(source_code),
  }
}

#[derive(Debug)]
enum InspectorMessageState {
  Ready(Value),
  WaitingFor(oneshot::Sender<Value>),
}

#[derive(Debug)]
struct HmrStateInner {
  script_ids: HashMap<String, String>,
  messages: HashMap<i32, InspectorMessageState>,
  exception_tx: mpsc::UnboundedSender<JsErrorBox>,
}

#[derive(Clone, Debug)]
pub struct HmrState(Arc<Mutex<HmrStateInner>>);

impl HmrState {
  fn new(exception_tx: mpsc::UnboundedSender<JsErrorBox>) -> Self {
    Self(Arc::new(Mutex::new(HmrStateInner {
      script_ids: HashMap::new(),
      messages: HashMap::new(),
      exception_tx,
    })))
  }

  pub fn callback(&self, msg: deno_core::InspectorMsg) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      // Notifications: drop on parse failure rather than panic — the
      // inspector can ship payload shapes we don't model.
      match serde_json::from_str::<cdp::Notification>(&msg.content) {
        Ok(notification) => self.handle_notification(notification),
        Err(e) => log::debug!("HMR: failed to parse CDP notification: {}", e),
      }
      return;
    };

    let message: Value = match serde_json::from_str(&msg.content) {
      Ok(v) => v,
      Err(e) => {
        log::debug!("HMR: failed to parse CDP response {}: {}", msg_id, e);
        return;
      }
    };
    let mut state = self.0.lock();
    let Some(message_state) = state.messages.remove(&msg_id) else {
      state
        .messages
        .insert(msg_id, InspectorMessageState::Ready(message));
      return;
    };
    let InspectorMessageState::WaitingFor(sender) = message_state else {
      return;
    };
    let _ = sender.send(message);
  }

  fn handle_notification(&self, notification: cdp::Notification) {
    if notification.method == "Runtime.exceptionThrown" {
      let exception_thrown = match serde_json::from_value::<cdp::ExceptionThrown>(
        notification.params,
      ) {
        Ok(v) => v,
        Err(e) => {
          log::debug!("HMR: malformed Runtime.exceptionThrown: {}", e);
          return;
        }
      };
      let (message, description) = exception_thrown
        .exception_details
        .get_message_and_description();
      let _ = self
        .0
        .lock()
        .exception_tx
        .send(JsErrorBox::generic(format!("{} {}", message, description)));
    } else if notification.method == "Debugger.scriptParsed" {
      let params = match serde_json::from_value::<cdp::ScriptParsed>(
        notification.params,
      ) {
        Ok(v) => v,
        Err(e) => {
          log::debug!("HMR: malformed Debugger.scriptParsed: {}", e);
          return;
        }
      };
      if params.url.starts_with("file://") {
        // Store with the URL as-is (no canonicalization — VFS paths
        // don't exist on disk so canonicalize would fail).
        self
          .0
          .lock()
          .script_ids
          .insert(params.url.clone(), params.script_id);
      }
    }
  }
}

/// Callback invoked after a module is successfully hot-replaced.
pub type HmrReloadCallback = Box<dyn Fn() + Send + Sync>;

/// Result of attempting to apply a single change.
enum ChangeOutcome {
  /// `setScriptSource` succeeded — the URL was hot-replaced.
  Replaced(String),
  /// V8 can't apply the change in place; ask the host to reload.
  NeedsReload(String),
  /// Nothing to do (untracked file, transient I/O error, etc.).
  Skipped,
}

/// What happened to a watched source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileChange {
  /// File was modified or newly created — try a hot-replace.
  Updated,
  /// File was deleted or renamed — fall back to a page reload.
  Removed,
}

/// Desktop HMR runner. Watches source files and hot-replaces changed modules.
pub struct DesktopHmrRunner {
  session: LocalInspectorSession,
  state: HmrState,
  changed_rx: mpsc::UnboundedReceiver<(PathBuf, FileChange)>,
  exception_rx: mpsc::UnboundedReceiver<JsErrorBox>,
  /// The directory being watched on disk (original source location).
  watch_dir: PathBuf,
  /// The VFS root path inside the compiled binary. Script URLs use this base.
  vfs_root: PathBuf,
  _watcher: notify::RecommendedWatcher,
  /// Optional callback to trigger a reload (e.g. refresh the webview).
  on_reload: Option<HmrReloadCallback>,
  /// Optional sender to dispatch errors as DesktopEvents for the error reporter.
  desktop_event_tx: Option<crate::desktop::DesktopEventTx>,
}

impl DesktopHmrRunner {
  /// Create a new HMR runner. Watches the given root directory for changes.
  /// `vfs_root` is the embedded root path that V8 scripts are registered under.
  pub fn new(
    session: LocalInspectorSession,
    state: HmrState,
    watch_dir: PathBuf,
    vfs_root: PathBuf,
    exception_rx: mpsc::UnboundedReceiver<JsErrorBox>,
  ) -> Result<Self, JsErrorBox> {
    let (changed_tx, changed_rx) = mpsc::unbounded_channel();

    let mut watcher =
      notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        let Ok(event) = res else {
          return;
        };
        // Filter event kinds:
        //   Create / Modify(non-Metadata) → Updated (hot-replace candidate)
        //   Remove                        → Removed (fall back to reload)
        // Metadata-only modifications (chmod, touch) are pure noise.
        let change = match event.kind {
          notify::EventKind::Create(_) => FileChange::Updated,
          notify::EventKind::Modify(ModifyKind::Metadata(_)) => return,
          notify::EventKind::Modify(_) => FileChange::Updated,
          notify::EventKind::Remove(_) => FileChange::Removed,
          _ => return,
        };
        for path in event.paths {
          if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs") {
              let _ = changed_tx.send((path, change));
            }
          }
        }
      })
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    watcher
      .watch(&watch_dir, RecursiveMode::Recursive)
      .map_err(|e| JsErrorBox::generic(e.to_string()))?;

    let watch_dir_canonical = watch_dir
      .canonicalize()
      .unwrap_or_else(|_| watch_dir.clone());

    Ok(Self {
      session,
      state,
      changed_rx,
      exception_rx,
      watch_dir: watch_dir_canonical,
      vfs_root,
      _watcher: watcher,
      on_reload: None,
      desktop_event_tx: None,
    })
  }

  /// Set a callback to be invoked after each successful hot-replace.
  pub fn set_on_reload(&mut self, cb: HmrReloadCallback) {
    self.on_reload = Some(cb);
  }

  pub fn start(&mut self) {
    self
      .session
      .post_message::<()>(next_id(), "Debugger.enable", None);
    self
      .session
      .post_message::<()>(next_id(), "Runtime.enable", None);
  }

  pub async fn run(&mut self) -> Result<(), deno_core::error::CoreError> {
    loop {
      tokio::select! {
        biased;

        maybe_error = self.exception_rx.recv() => {
          if let Some(err) = maybe_error {
            log::error!("HMR exception: {}", err);
            if let Some(tx) = &self.desktop_event_tx {
              let _ = tx.try_send(crate::desktop::DesktopEvent::RuntimeError {
                message: err.to_string(),
                stack: None,
              });
            }
          }
        }

        maybe_path = self.changed_rx.recv() => {
          let Some(first) = maybe_path else {
            break Ok(());
          };
          // Coalesce a burst of events (e.g. save-then-format) into a single
          // pass, with the latest kind per path winning. Ends as soon as
          // the channel quiets down for HMR_DEBOUNCE.
          let mut pending: HashMap<PathBuf, FileChange> = HashMap::new();
          pending.insert(first.0, first.1);
          loop {
            match tokio::time::timeout(
              HMR_DEBOUNCE,
              self.changed_rx.recv(),
            )
            .await
            {
              Ok(Some((path, change))) => {
                pending.insert(path, change);
              }
              Ok(None) => break,
              Err(_) => break,
            }
          }

          let mut needs_reload = false;
          let mut handled: HashSet<String> = HashSet::new();
          for (path, change) in pending {
            match self.handle_change(&path, change).await {
              ChangeOutcome::Replaced(url) => {
                handled.insert(url);
              }
              ChangeOutcome::NeedsReload(reason) => {
                log::info!("HMR: {} — falling back to page reload", reason);
                needs_reload = true;
              }
              ChangeOutcome::Skipped => {}
            }
          }

          for url in &handled {
            self.dispatch_hmr_event(url);
            eprintln!("HMR: replaced {}", url);
          }

          if needs_reload || !handled.is_empty() {
            if let Some(on_reload) = &self.on_reload {
              on_reload();
            }
          }
        }
      }
    }
  }

  /// Process a single coalesced change. Returns whether the change was
  /// hot-replaced, requires a page reload, or was a no-op.
  async fn handle_change(
    &mut self,
    path: &PathBuf,
    change: FileChange,
  ) -> ChangeOutcome {
    let canonical = match (change, path.canonicalize()) {
      (FileChange::Updated, Ok(p)) => p,
      (FileChange::Updated, Err(_)) => return ChangeOutcome::Skipped,
      // Removed paths can't be canonicalized; reconstruct best-effort.
      (FileChange::Removed, _) => path.clone(),
    };

    let Ok(relative) = canonical.strip_prefix(&self.watch_dir) else {
      return ChangeOutcome::Skipped;
    };
    let vfs_path = self.vfs_root.join(relative);
    let Ok(module_url) = Url::from_file_path(&vfs_path) else {
      return ChangeOutcome::Skipped;
    };

    log::debug!(
      "HMR: {:?} {} -> VFS {}",
      change,
      canonical.display(),
      module_url
    );

    let script_id = {
      let state = self.state.0.lock();
      let id = state.script_ids.get(module_url.as_str()).cloned();
      if id.is_none() {
        let known: Vec<String> = state.script_ids.keys().cloned().collect();
        drop(state);
        log::debug!(
          "HMR: no script ID for {}, known scripts: {:?}",
          module_url,
          known,
        );
      }
      id
    };

    if change == FileChange::Removed {
      // Either the file was deleted/renamed or this was a tracked module
      // that V8 had loaded. Either way, setScriptSource isn't applicable —
      // ask the host to reload so it picks up the new state.
      return if script_id.is_some() {
        ChangeOutcome::NeedsReload(format!("{} removed", module_url))
      } else {
        ChangeOutcome::Skipped
      };
    }

    let Some(script_id) = script_id else {
      return ChangeOutcome::Skipped;
    };

    let source_code = match tokio::fs::read_to_string(&canonical).await {
      Ok(s) => s,
      Err(e) => {
        log::warn!("HMR: failed to read {}: {}", canonical.display(), e);
        return ChangeOutcome::Skipped;
      }
    };

    let source_code = match transpile_for_hmr(&module_url, source_code) {
      Ok(s) => s,
      Err(e) => {
        log::warn!("HMR: transpile error for {}: {}", module_url, e);
        return ChangeOutcome::Skipped;
      }
    };

    let mut tries = 1;
    loop {
      let msg_id = self.set_script_source(&script_id, &source_code);
      let value = match self.wait_for_response(msg_id).await {
        Some(v) => v,
        None => {
          log::warn!(
            "HMR: inspector dropped response for {}; aborting reload",
            module_url
          );
          return ChangeOutcome::Skipped;
        }
      };
      let result: cdp::SetScriptSourceResponse =
        match serde_json::from_value(value) {
          Ok(r) => r,
          Err(e) => {
            log::warn!("HMR: bad CDP response: {}", e);
            return ChangeOutcome::Skipped;
          }
        };

      if matches!(result.status, cdp::Status::Ok) {
        return ChangeOutcome::Replaced(module_url.into());
      }

      eprintln!("HMR: failed to reload {}: {}", module_url, explain(&result));

      // V8 can't replace modules whose top-level surface (imports, exported
      // bindings, top-level let/const) changed. The classic run-mode HMR
      // restarts the worker; here we tell the host to reload the page so the
      // user's edit isn't silently dropped.
      if matches!(result.status, cdp::Status::BlockedByTopLevelEsModuleChange) {
        return ChangeOutcome::NeedsReload(format!(
          "{} requires a top-level module reload",
          module_url
        ));
      }

      if should_retry(&result.status) && tries <= 2 {
        tries += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
        continue;
      }
      return ChangeOutcome::Skipped;
    }
  }

  async fn wait_for_response(&self, msg_id: i32) -> Option<Value> {
    if let Some(message_state) = self.state.0.lock().messages.remove(&msg_id) {
      let InspectorMessageState::Ready(mut value) = message_state else {
        unreachable!();
      };
      return Some(value["result"].take());
    }

    let (tx, rx) = oneshot::channel();
    self
      .state
      .0
      .lock()
      .messages
      .insert(msg_id, InspectorMessageState::WaitingFor(tx));
    // The inspector session may be torn down while we're waiting. Treat
    // that as a no-reload rather than a panic.
    match rx.await {
      Ok(mut value) => Some(value["result"].take()),
      Err(_) => {
        // Drop our pending entry so the state map doesn't grow.
        self.state.0.lock().messages.remove(&msg_id);
        None
      }
    }
  }

  fn set_script_source(&mut self, script_id: &str, source: &str) -> i32 {
    let msg_id = next_id();
    self.session.post_message(
      msg_id,
      "Debugger.setScriptSource",
      Some(json!({
        "scriptId": script_id,
        "scriptSource": source,
        "allowTopFrameEditing": true,
      })),
    );
    msg_id
  }

  fn dispatch_hmr_event(&mut self, module_url: &str) {
    // Encode via serde so embedded quotes/backslashes in `module_url` don't
    // break out of the string literal.
    let detail = json!({ "path": module_url }).to_string();
    let expr = format!(
      "dispatchEvent(new CustomEvent(\"hmr\", {{ detail: {} }}));",
      detail
    );
    // Intentionally no `contextId`: the inspector session is bound to a
    // single isolate, and pinning to context 1 misroutes the event in any
    // setup with multiple V8 contexts (workers, navigation, etc.).
    self.session.post_message(
      next_id(),
      "Runtime.evaluate",
      Some(json!({ "expression": expr })),
    );
  }
}

/// Set up HMR for the desktop runtime. Returns a runner that should be
/// polled concurrently with the event loop.
///
/// `watch_dir` is the original source directory on disk.
/// `vfs_root` is the VFS root path that V8 scripts are registered under.
pub fn setup_desktop_hmr(
  worker: &mut deno_lib::worker::LibMainWorker,
  watch_dir: PathBuf,
  vfs_root: PathBuf,
) -> Result<DesktopHmrRunner, JsErrorBox> {
  let (exception_tx, exception_rx) = mpsc::unbounded_channel();
  let state = HmrState::new(exception_tx);
  let state_clone = state.clone();
  let cb = Box::new(move |msg| state_clone.callback(msg));
  let session = worker.create_inspector_session(cb);

  let mut runner =
    DesktopHmrRunner::new(session, state, watch_dir, vfs_root, exception_rx)?;

  // Extract the desktop event sender from OpState if available, so HMR
  // errors can be dispatched as DesktopEvent::RuntimeError.
  let desktop_event_tx = worker
    .js_runtime()
    .op_state()
    .borrow()
    .try_borrow::<crate::desktop::DesktopEventSender>()
    .map(|s| s.0.clone());
  runner.desktop_event_tx = desktop_event_tx;

  runner.start();
  Ok(runner)
}
