// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use deno_core::LocalInspectorSession;
use deno_core::error::CoreError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_terminal::colors;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::cdp;
use crate::module_loader::CliEmitter;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::file_watcher::WatcherRestartMode;

static NEXT_MSG_ID: AtomicI32 = AtomicI32::new(0);
fn next_id() -> i32 {
  NEXT_MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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
  match status {
    cdp::Status::Ok => false,
    cdp::Status::CompileError => false,
    cdp::Status::BlockedByActiveGenerator => true,
    cdp::Status::BlockedByActiveFunction => true,
    cdp::Status::BlockedByTopLevelEsModuleChange => false,
  }
}

#[derive(Debug)]
enum InspectorMessageState {
  Ready(serde_json::Value),
  WaitingFor(oneshot::Sender<serde_json::Value>),
}

#[derive(Debug)]
pub struct HmrRunnerInner {
  watcher_communicator: Arc<WatcherCommunicator>,
  script_ids: HashMap<String, String>,
  messages: HashMap<i32, InspectorMessageState>,
  emitter: Arc<CliEmitter>,
  exception_tx: UnboundedSender<JsErrorBox>,
  exception_rx: Option<UnboundedReceiver<JsErrorBox>>,
}

#[derive(Clone, Debug)]
pub struct HmrRunnerState(Arc<Mutex<HmrRunnerInner>>);

impl HmrRunnerState {
  pub fn new(
    emitter: Arc<CliEmitter>,
    watcher_communicator: Arc<WatcherCommunicator>,
  ) -> Self {
    let (exception_tx, exception_rx) = mpsc::unbounded_channel();

    Self(Arc::new(Mutex::new(HmrRunnerInner {
      emitter,
      watcher_communicator,
      script_ids: HashMap::new(),
      messages: HashMap::new(),
      exception_tx,
      exception_rx: Some(exception_rx),
    })))
  }

  pub fn callback(&self, msg: deno_core::InspectorMsg) {
    let deno_core::InspectorMsgKind::Message(msg_id) = msg.kind else {
      let notification = serde_json::from_str(&msg.content).unwrap();
      self.handle_notification(notification);
      return;
    };

    let message: serde_json::Value =
      serde_json::from_str(&msg.content).unwrap();
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
      // .map_err(JsErrorBox::from_err)?;
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
      // .map_err(JsErrorBox::from_err)?;
      if params.url.starts_with("file://") {
        let file_url = Url::parse(&params.url).unwrap();
        let file_path = file_url.to_file_path().unwrap();
        if let Ok(canonicalized_file_path) = file_path.canonicalize() {
          let canonicalized_file_url =
            Url::from_file_path(canonicalized_file_path).unwrap();
          self
            .0
            .lock()
            .script_ids
            .insert(canonicalized_file_url.into(), params.script_id);
        }
      }
    }
  }
}

/// This structure is responsible for providing Hot Module Replacement
/// functionality.
///
/// It communicates with V8 inspector over a local session and waits for
/// notifications about changed files from the `FileWatcher`.
///
/// Upon receiving such notification, the runner decides if the changed
/// path should be handled the `FileWatcher` itself (as if we were running
/// in `--watch` mode), or if the path is eligible to be hot replaced in the
/// current program.
///
/// Even if the runner decides that a path will be hot-replaced, the V8 isolate
/// can refuse to perform hot replacement, eg. a top-level variable/function
/// of an ES module cannot be hot-replaced. In such situation the runner will
/// force a full restart of a program by notifying the `FileWatcher`.
pub struct HmrRunner {
  session: LocalInspectorSession,
  state: HmrRunnerState,
}

impl HmrRunner {
  pub fn new(state: HmrRunnerState, session: LocalInspectorSession) -> Self {
    Self { session, state }
  }

  pub fn start(&mut self) {
    self
      .session
      .post_message::<()>(next_id(), "Debugger.enable", None);
    self
      .session
      .post_message::<()>(next_id(), "Runtime.enable", None);
  }

  fn watcher(&self) -> Arc<WatcherCommunicator> {
    self.state.0.lock().watcher_communicator.clone()
  }

  pub fn stop(&mut self) {
    self
      .watcher()
      .change_restart_mode(WatcherRestartMode::Automatic);
  }

  pub async fn run(&mut self) -> Result<(), CoreError> {
    self
      .watcher()
      .change_restart_mode(WatcherRestartMode::Manual);
    let watcher = self.watcher();
    let mut exception_rx = self.state.0.lock().exception_rx.take().unwrap();
    loop {
      select! {
        biased;

        maybe_error = exception_rx.recv() => {
          if let Some(err) = maybe_error {
            break Err(err.into());
          }
        },

        changed_paths = watcher.watch_for_changed_paths() => {
          let changed_paths = changed_paths.map_err(JsErrorBox::from_err)?;

          let Some(changed_paths) = changed_paths else {
            let _ = self.watcher().force_restart();
            continue;
          };

          let filtered_paths: Vec<PathBuf> = changed_paths.into_iter().filter(|p| p.extension().is_some_and(|ext| {
            let ext_str = ext.to_str().unwrap();
            matches!(ext_str, "js" | "ts" | "jsx" | "tsx")
          })).collect();

          // If after filtering there are no paths it means it's either a file
          // we can't HMR or an external file that was passed explicitly to
          // `--watch-hmr=<file>` path.
          if filtered_paths.is_empty() {
            let _ = self.watcher().force_restart();
            continue;
          }

          for path in filtered_paths {
            let Some(path_str) = path.to_str() else {
              let _ = self.watcher().force_restart();
              continue;
            };
            let Ok(module_url) = Url::from_file_path(path_str) else {
              let _ = self.watcher().force_restart();
              continue;
            };

            let Some(id) = self.state.0.lock().script_ids.get(module_url.as_str()).cloned() else {
              let _ = self.watcher().force_restart();
              continue;
            };

            let source_code = tokio::fs::read_to_string(deno_path_util::url_to_file_path(&module_url).unwrap()).await?;
            let source_code = self.state.0.lock().emitter.emit_for_hmr(
              &module_url,
              source_code,
            )?;

            let mut tries = 1;
            loop {
              let msg_id = self.set_script_source(&id, source_code.as_str());
              let value = self.wait_for_response(msg_id).await;
              let result: cdp::SetScriptSourceResponse = serde_json::from_value(value).map_err(|e| {
                JsErrorBox::from_err(e)
              })?;


              if matches!(result.status, cdp::Status::Ok) {
                self.dispatch_hmr_event(module_url.as_str());
                self.watcher().print(format!("Replaced changed module {}", module_url.as_str()));
                break;
              }

              self.watcher().print(format!("Failed to reload module {}: {}.", module_url, colors::gray(&explain(&result))));
              if should_retry(&result.status) && tries <= 2 {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
              }

              let _ = self.watcher().force_restart();
              break;
            }
          }
        }
      }
    }
  }

  async fn wait_for_response(&self, msg_id: i32) -> serde_json::Value {
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

  fn dispatch_hmr_event(&mut self, script_id: &str) {
    let expr = format!(
      "dispatchEvent(new CustomEvent(\"hmr\", {{ detail: {{ path: \"{}\" }} }}));",
      script_id
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
