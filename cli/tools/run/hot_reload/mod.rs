// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::emit::Emitter;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::file_watcher::WatcherRestartMode;
use deno_ast::MediaType;
use deno_core::error::generic_error;
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
  watcher_communicator: WatcherCommunicator,
  script_ids: HashMap<String, String>,
  emitter: Arc<Emitter>,
}

impl HotReloadManager {
  pub fn new(
    emitter: Arc<Emitter>,
    session: LocalInspectorSession,
    watcher_communicator: WatcherCommunicator,
  ) -> Self {
    Self {
      session,
      emitter,
      watcher_communicator,
      script_ids: HashMap::new(),
    }
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  pub async fn start(&mut self) -> Result<(), AnyError> {
    self.enable_debugger().await
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  pub async fn stop(&mut self) -> Result<(), AnyError> {
    self
      .watcher_communicator
      .change_restart_mode(WatcherRestartMode::Automatic);
    self.disable_debugger().await
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  async fn enable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.enable", None)
      .await?;
    self
      .session
      .post_message::<()>("Runtime.enable", None)
      .await?;
    Ok(())
  }

  // TODO(bartlomieju): this code is duplicated in `cli/tools/coverage/mod.rs`
  async fn disable_debugger(&mut self) -> Result<(), AnyError> {
    self
      .session
      .post_message::<()>("Debugger.disable", None)
      .await?;
    self
      .session
      .post_message::<()>("Runtime.disable", None)
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

  async fn dispatch_hmr_event(
    &mut self,
    script_id: &str,
  ) -> Result<(), AnyError> {
    let expr = format!(
      "dispatchEvent(new CustomEvent(\"hmr\", {{ detail: {{ path: \"{}\" }} }}));",
      script_id
    );

    let _result = self
      .session
      .post_message(
        "Runtime.evaluate",
        Some(json!({
          "expression": expr,
          "contextId": Some(1),
        })),
      )
      .await?;

    Ok(())
  }
}

// TODO(bartlomieju): Shouldn't use `tokio::select!` here, as futures are not cancel safe
pub async fn run_hot_reload(
  hmr_manager: &mut HotReloadManager,
) -> Result<(), AnyError> {
  hmr_manager
    .watcher_communicator
    .change_restart_mode(WatcherRestartMode::Manual);
  let mut session_rx = hmr_manager.session.take_notification_rx();
  loop {
    select! {
      biased;
      // TODO(SyrupThinker): Deferred retry with timeout
      Some(notification) = session_rx.next() => {
        let notification = serde_json::from_value::<RpcNotification>(notification)?;
        // TODO(bartlomieju): this is not great... and the code is duplicated with the REPL.
        if notification.method == "Runtime.exceptionThrown" {
          let params = notification.params;
          let exception_details = params.get("exceptionDetails").unwrap().as_object().unwrap();
          let text = exception_details.get("text").unwrap().as_str().unwrap();
          let exception = exception_details.get("exception").unwrap().as_object().unwrap();
          let description = exception.get("description").and_then(|d| d.as_str()).unwrap_or("undefined");
          break Err(generic_error(format!("{text} {description}")));
        } else if notification.method == "Debugger.scriptParsed" {
          let params = serde_json::from_value::<ScriptParsed>(notification.params)?;
          if params.url.starts_with("file://") {
            // if let Ok(path_url) = Url::parse(&params.url) {
            //   eprintln!("path url {:#?}", path_url.as_str());
            //   let _ = hmr_manager.watcher_communicator.watch_paths(vec![path_url.to_file_path().unwrap()]);
            //   eprintln!("started watching path");
            //   tokio::task::yield_now().await;
            // }
            hmr_manager.script_ids.insert(params.url, params.script_id);
          }
        }
      }
      changed_paths = hmr_manager.watcher_communicator.watch_for_changed_paths() => {
        eprintln!("changed patchs in hot {:#?}", changed_paths);
        let changed_paths = changed_paths?;

        let Some(changed_paths) = changed_paths else {
          let _ = hmr_manager.watcher_communicator.force_restart();
          continue;
        };

        let filtered_paths: Vec<PathBuf> = changed_paths.into_iter().filter(|p| p.extension().map_or(false, |ext| {
          let ext_str = ext.to_str().unwrap();
          matches!(ext_str, "js" | "ts" | "jsx" | "tsx")
        })).collect();

        for path in filtered_paths {
          let Some(path_str) = path.to_str() else {
            let _ = hmr_manager.watcher_communicator.force_restart();
            continue;
          };
          let Ok(module_url) = Url::from_file_path(path_str) else {
            let _ = hmr_manager.watcher_communicator.force_restart();
            continue;
          };

          log::info!("{} Reloading changed module {}", colors::intense_blue("HMR"), module_url.as_str());

          let Some(id) = hmr_manager.script_ids.get(module_url.as_str()).cloned() else {
            let _ = hmr_manager.watcher_communicator.force_restart();
            continue;
          };

          // TODO(bartlomieju): I really don't like `hmr_manager.emitter` etc here.
          // Maybe use `deno_ast` directly?
          let media_type = MediaType::from_path(&path);
          let source_code = tokio::fs::read_to_string(path).await?;
          let source_arc: Arc<str> = Arc::from(source_code.as_str());
          let source_code = {
            let parsed_source = hmr_manager.emitter.parsed_source_cache.get_or_parse_module(
              &module_url,
              source_arc.clone(),
              media_type,
            )?;
            let mut options = hmr_manager.emitter.emit_options.clone();
            options.inline_source_map = false;
            let transpiled_source = parsed_source.transpile(&options)?;
            transpiled_source.text.to_string()
            // hmr_manager.emitter.emit_parsed_source(&module_url, media_type, &source_arc)?
          };

          // eprintln!("transpiled source code {:#?}", source_code);
          // TODO(bartlomieju): this loop should do 2 retries at most
          loop {
            let result = hmr_manager.set_script_source(&id, source_code.as_str()).await?;

            if matches!(result.status, Status::Ok) {
              hmr_manager.dispatch_hmr_event(module_url.as_str()).await?;
              break;
            }

            log::info!("{} Failed to reload module {}: {}.", colors::intense_blue("HMR"), module_url, colors::gray(result.status.explain()));
            if !result.status.should_retry() {
              log::info!("{} Restarting the process...", colors::intense_blue("HMR"));
              // TODO(bartlomieju): Print into that sending failed?
              let _ = hmr_manager.watcher_communicator.force_restart();
              break;
            }
          }
        }
      }
      _ = hmr_manager.session.receive_from_v8_session() => {}
    }
  }
}
