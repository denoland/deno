// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use lspower::jsonrpc::Error as LSPError;
use lspower::jsonrpc::Result as LSPResult;
use lspower::lsp_types;

#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
  pub status_notification: bool,
  pub workspace_configuration: bool,
  pub workspace_did_change_watched_files: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  pub enable: bool,
  pub config: Option<String>,
  pub import_map: Option<String>,

  #[serde(default)]
  pub lint: bool,
  #[serde(default)]
  pub unstable: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
  pub client_capabilities: ClientCapabilities,
  pub root_uri: Option<Url>,
  pub settings: WorkspaceSettings,
}

impl Config {
  pub fn update(&mut self, value: Value) -> LSPResult<()> {
    let settings: WorkspaceSettings = serde_json::from_value(value)
      .map_err(|err| LSPError::invalid_params(err.to_string()))?;
    self.settings = settings;
    Ok(())
  }

  #[allow(clippy::redundant_closure_call)]
  pub fn update_capabilities(
    &mut self,
    capabilities: &lsp_types::ClientCapabilities,
  ) {
    if let Some(experimental) = &capabilities.experimental {
      let get_bool =
        |k: &str| experimental.get(k).and_then(|it| it.as_bool()) == Some(true);

      self.client_capabilities.status_notification =
        get_bool("statusNotification");
    }

    if let Some(workspace) = &capabilities.workspace {
      self.client_capabilities.workspace_configuration =
        workspace.configuration.unwrap_or(false);
      self.client_capabilities.workspace_did_change_watched_files = workspace
        .did_change_watched_files
        .and_then(|it| it.dynamic_registration)
        .unwrap_or(false);
    }
  }
}
