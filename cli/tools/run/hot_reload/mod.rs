// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::emit::Emitter;
use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use deno_core::url::Url;
use deno_core::LocalInspectorSession;
use deno_runtime::colors;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::select;

mod json_types;

use json_types::RpcNotification;
use json_types::ScriptParsed;
use json_types::SetScriptSourceReturnObject;
use json_types::Status;

pub struct HotReloadManager {
  session: LocalInspectorSession,
  path_change_receiver: tokio::sync::broadcast::Receiver<Vec<PathBuf>>,
  file_watcher_restart_sender: tokio::sync::mpsc::UnboundedSender<()>,
  script_ids: HashMap<String, String>,
  emitter: Arc<Emitter>,
}

impl HotReloadManager {
  pub fn new(
    emitter: Arc<Emitter>,
    session: LocalInspectorSession,
    path_change_receiver: tokio::sync::broadcast::Receiver<Vec<PathBuf>>,
    file_watcher_restart_sender: tokio::sync::mpsc::UnboundedSender<()>,
  ) -> Self {
    Self {
      session,
      emitter,
      path_change_receiver,
      file_watcher_restart_sender,
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
          let changed_paths = changed_paths?;
          let filtered_paths: Vec<PathBuf> = changed_paths.into_iter().filter(|p| p.extension().map_or(false, |ext| {
            let ext_str = ext.to_str().unwrap();
            matches!(ext_str, "js" | "ts" | "jsx" | "tsx")
          })).collect();

          for path in filtered_paths {
            let Some(path_str) = path.to_str() else {
              continue;
            };
            let Ok(module_url) = Url::from_file_path(path_str) else {
              continue;
            };

            log::info!("{} Reloading changed module {}", colors::intense_blue("HMR"), module_url.as_str());

            let Some(id) = self.script_ids.get(module_url.as_str()).cloned() else {
              continue;
            };

            let media_type = MediaType::from_path(&path);
            let source = tokio::fs::read_to_string(path).await?;
            let source_arc: Arc<str> = Arc::from(source.as_str());
            let source_code = self.emitter.emit_parsed_source(&module_url, media_type, &source_arc)?;

            // TODO(bartlomieju): this loop seems fishy
            loop {
              let result = self.set_script_source(&id, source_code.as_str()).await?;
              if !matches!(result.status, Status::Ok) {
                log::info!("{} Failed to reload module {}: {}.", colors::intense_blue("HMR"), module_url, colors::gray(result.status.explain()));
                if !result.status.should_retry() {
                  log::info!("{} Restarting the process...", colors::intense_blue("HMR"));
                  // TODO(bartlomieju): Print into that sending failed?
                  let _ = self.file_watcher_restart_sender.send(());
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
