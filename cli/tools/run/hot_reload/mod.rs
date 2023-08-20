// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::{collections::HashMap, path::PathBuf};

use deno_core::{
  error::AnyError,
  futures::StreamExt,
  serde_json::{self, json},
  LocalInspectorSession,
};
use deno_runtime::colors;
use tokio::select;

use crate::tools::run::hot_reload::json_types::{
  RpcNotification, ScriptParsed, SetScriptSourceReturnObject, Status,
};

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

  // FIXME(SyrupThinker): Inspector code duplication

  pub async fn start(&mut self) -> Result<(), AnyError> {
    self.enable_debugger().await?;

    Ok(())
  }

  pub async fn stop(&mut self) -> Result<(), AnyError> {
    self.disable_debugger().await?;

    Ok(())
  }

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
          for path in changed_paths?.iter().filter(|p| p.extension().map_or(false, |ext| ext == "js")) {
            if let Some(path_str) = path.to_str() {
              let module_url = "file://".to_owned() + path_str;
              log::info!("{} Reloading changed module {}", colors::intense_blue("Hot-reload"), module_url);

              if let Some(id) = self.script_ids.get(&module_url).map(String::clone) {
                let src = tokio::fs::read_to_string(path).await?;

                loop {
                  let result = self.set_script_source(&id, &src).await?;
                  if !matches!(result.status, Status::Ok) {
                    log::warn!("{} Failed to reload module {}: {}", colors::intense_blue("Hot-reload"), module_url, colors::red(result.status.explain()));
                  }
                  if !result.status.should_retry() {
                    break;
                  }
                }
              }
            }
          }
        }
        _ = self.session.receive_from_v8_session() => {}
      }
    }
  }

  async fn enable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.enable", None)
      .await?;
    Ok(())
  }

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
