// Copyright 2018-2026 the Deno authors. MIT license.

//! Simplified HMR (Hot Module Replacement) for the standalone/desktop runtime.
//!
//! Watches source files on disk, transpiles changed TypeScript/TSX/JSX files
//! using `deno_ast`, and hot-replaces them via V8's `Debugger.setScriptSource`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use deno_core::LocalInspectorSession;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::Value;
use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use deno_core::url::Url;
use deno_error::JsErrorBox;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

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
      let notification: cdp::Notification =
        serde_json::from_str(&msg.content).unwrap();
      self.handle_notification(notification);
      return;
    };

    let message: Value = serde_json::from_str(&msg.content).unwrap();
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
      let exception_thrown =
        serde_json::from_value::<cdp::ExceptionThrown>(notification.params)
          .unwrap();
      let (message, description) = exception_thrown
        .exception_details
        .get_message_and_description();
      let _ = self
        .0
        .lock()
        .exception_tx
        .send(JsErrorBox::generic(format!("{} {}", message, description)));
    } else if notification.method == "Debugger.scriptParsed" {
      let params =
        serde_json::from_value::<cdp::ScriptParsed>(notification.params)
          .unwrap();
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

/// Desktop HMR runner. Watches source files and hot-replaces changed modules.
pub struct DesktopHmrRunner {
  session: LocalInspectorSession,
  state: HmrState,
  changed_rx: mpsc::UnboundedReceiver<PathBuf>,
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
        if let Ok(event) = res {
          if matches!(
            event.kind,
            notify::EventKind::Modify(_) | notify::EventKind::Create(_)
          ) {
            for path in event.paths {
              if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "js" | "ts" | "jsx" | "tsx") {
                  let _ = changed_tx.send(path);
                }
              }
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
          let Some(path) = maybe_path else {
            break Ok(());
          };

          let Ok(canonical) = path.canonicalize() else {
            continue;
          };

          // Map the on-disk path to the VFS path that V8 knows about.
          // e.g. /tmp/deno-desktop-test/app.tsx -> /tmp/.deno_compile_xxx/app.tsx
          let relative = match canonical.strip_prefix(&self.watch_dir) {
            Ok(rel) => rel,
            Err(_) => continue,
          };
          let vfs_path = self.vfs_root.join(relative);
          let Ok(module_url) = Url::from_file_path(&vfs_path) else {
            continue;
          };

          log::debug!(
            "HMR: file changed {} -> VFS {}",
            canonical.display(),
            module_url
          );

          let Some(script_id) = self.state.0.lock().script_ids.get(module_url.as_str()).cloned() else {
            // Not a tracked module, skip.
            log::debug!("HMR: no script ID for {}, known scripts: {:?}",
              module_url,
              self.state.0.lock().script_ids.keys().collect::<Vec<_>>()
            );
            continue;
          };

          // Read from the actual file on disk (not the VFS path).
          let source_code = match tokio::fs::read_to_string(&canonical).await {
            Ok(s) => s,
            Err(e) => {
              log::warn!("HMR: failed to read {}: {}", canonical.display(), e);
              continue;
            }
          };

          let source_code = match transpile_for_hmr(&module_url, source_code) {
            Ok(s) => s,
            Err(e) => {
              log::warn!("HMR: transpile error for {}: {}", module_url, e);
              continue;
            }
          };

          let mut tries = 1;
          loop {
            let msg_id = self.set_script_source(&script_id, &source_code);
            let value = self.wait_for_response(msg_id).await;
            let result: cdp::SetScriptSourceResponse =
              match serde_json::from_value(value) {
                Ok(r) => r,
                Err(e) => {
                  log::warn!("HMR: bad CDP response: {}", e);
                  break;
                }
              };

            if matches!(result.status, cdp::Status::Ok) {
              self.dispatch_hmr_event(module_url.as_str());
              if let Some(on_reload) = &self.on_reload {
                on_reload();
              }
              eprintln!("HMR: replaced {}", module_url);
              break;
            }

            eprintln!(
              "HMR: failed to reload {}: {}",
              module_url,
              explain(&result)
            );
            if should_retry(&result.status) && tries <= 2 {
              tries += 1;
              tokio::time::sleep(std::time::Duration::from_millis(100)).await;
              continue;
            }
            break;
          }
        }
      }
    }
  }

  async fn wait_for_response(&self, msg_id: i32) -> Value {
    if let Some(message_state) = self.state.0.lock().messages.remove(&msg_id) {
      let InspectorMessageState::Ready(mut value) = message_state else {
        unreachable!();
      };
      return value["result"].take();
    }

    let (tx, rx) = oneshot::channel();
    self
      .state
      .0
      .lock()
      .messages
      .insert(msg_id, InspectorMessageState::WaitingFor(tx));
    let mut value = rx.await.unwrap();
    value["result"].take()
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
    let expr = format!(
      "dispatchEvent(new CustomEvent(\"hmr\", {{ detail: {{ path: \"{}\" }} }}));",
      module_url
    );
    self.session.post_message(
      next_id(),
      "Runtime.evaluate",
      Some(json!({
        "expression": expr,
        "contextId": Some(1),
      })),
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
