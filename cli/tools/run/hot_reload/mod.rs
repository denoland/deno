// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use deno_core::LocalInspectorSession;
use deno_runtime::colors;
use tokio::select;

use crate::tools::run::hot_reload::json_types::RpcNotification;
use crate::tools::run::hot_reload::json_types::ScriptParsed;
use crate::tools::run::hot_reload::json_types::SetScriptSourceReturnObject;
use crate::tools::run::hot_reload::json_types::Status;

mod json_types;

pub struct HotReloadManager {
  session: LocalInspectorSession,
  path_change_receiver: tokio::sync::broadcast::Receiver<Vec<PathBuf>>,
  script_ids: HashMap<String, String>,
}

impl HotReloadManager {
  pub fn new(
    session: LocalInspectorSession,
    path_change_receiver: tokio::sync::broadcast::Receiver<Vec<PathBuf>>,
  ) -> Self {
    Self {
      session,
      path_change_receiver,
      script_ids: HashMap::new(),
    }
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  pub async fn start(&mut self) -> Result<(), AnyError> {
    self.enable_debugger().await
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  pub async fn stop(&mut self) -> Result<(), AnyError> {
    self.disable_debugger().await
  }

  // TODO(bartlomieju): Shouldn't use `tokio::select!` here, as futures are not cancel safe
  pub async fn run(&mut self) -> Result<(), AnyError> {
    let mut session_rx = self.session.take_notification_rx();
    loop {
      select! {
        biased;
        // TODO(SyrupThinker): Deferred retry with timeout
        Some(notification) = session_rx.next() => {
          let notification = serde_json::from_value::<RpcNotification>(notification)?;
          if notification.method == "Debugger.scriptParsed" {
            let params = serde_json::from_value::<ScriptParsed>(notification.params)?;
            if params.url.starts_with("file://") {
              self.script_ids.insert(params.url, params.script_id);
            }
          }
        }
        changed_paths = self.path_change_receiver.recv() => {
          // TODO(bartlomieju): check for other extensions
          for path in changed_paths?.iter().filter(|p| p.extension().map_or(false, |ext| ext == "js")) {
            if let Some(path_str) = path.to_str() {
              let module_url = "file://".to_owned() + path_str;
              log::info!("{} Reloading changed module {}", colors::intense_blue("Hot-reload"), module_url);

              let Some(id) = self.script_ids.get(&module_url).cloned() else {
                continue;
              };

              // TODO(bartlomieju): this should use `FileFetcher` interface instead
              // TODO(bartlomieju): we need to run the file through our transpile infrastructure as well
              let src = tokio::fs::read_to_string(path).await?;

              // TODO(bartlomieju): this loop seems fishy
              loop {
                let result = self.set_script_source(&id, &src).await?;
                if !matches!(result.status, Status::Ok) {
                  log::warn!("{} Failed to reload module {}: {}", colors::intense_blue("Hot-reload"), module_url, colors::red(result.status.explain()));
                }
                if !result.status.should_retry() {
                  // TODO(bartlomieju): Force a reload by the file watcher.
                  break;
                }
              }
            }
          }
        }
        _ = self.session.receive_from_v8_session() => {}
      }
    }
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  async fn enable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.enable", None)
      .await?;
    Ok(())
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  async fn disable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.disable", None)
      .await?;
    Ok(())
  }

  async fn set_script_source(
    &mut self,
    script_id: &str,
    source: &str,
  ) -> Result<SetScriptSourceReturnObject, AnyError> {
    let result = self
      .session
      .post_message(
        "Debugger.setScriptSource",
        Some(json!({
          "scriptId": script_id,
          "scriptSource": source,
          "allowTopFrameEditing": true,
        })),
      )
      .await?;

    Ok(serde_json::from_value::<SetScriptSourceReturnObject>(
      result,
    )?)
  }
}
