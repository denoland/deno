// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use lspower::jsonrpc::Error as LSPError;
use lspower::jsonrpc::Result as LSPResult;
use lspower::lsp;

#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
  pub status_notification: bool,
  pub workspace_configuration: bool,
  pub workspace_did_change_watched_files: bool,
  pub line_folding_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensSettings {
  /// Flag for providing implementation code lenses.
  #[serde(default)]
  pub implementations: bool,
  /// Flag for providing reference code lenses.
  #[serde(default)]
  pub references: bool,
  /// Flag for providing reference code lens on all functions.  For this to have
  /// an impact, the `references` flag needs to be `true`.
  #[serde(default)]
  pub references_all_functions: bool,
}

impl Default for CodeLensSettings {
  fn default() -> Self {
    Self {
      implementations: false,
      references: false,
      references_all_functions: false,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionSettings {
  #[serde(default)]
  pub complete_function_calls: bool,
  #[serde(default)]
  pub names: bool,
  #[serde(default)]
  pub paths: bool,
  #[serde(default)]
  pub auto_imports: bool,
}

impl Default for CompletionSettings {
  fn default() -> Self {
    Self {
      complete_function_calls: false,
      names: true,
      paths: true,
      auto_imports: true,
    }
  }
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  pub enable: bool,
  pub config: Option<String>,
  pub import_map: Option<String>,
  #[serde(default)]
  pub code_lens: CodeLensSettings,
  #[serde(default)]
  pub suggest: CompletionSettings,

  #[serde(default)]
  pub lint: bool,
  #[serde(default)]
  pub unstable: bool,
}

impl WorkspaceSettings {
  /// Determine if any code lenses are enabled at all.  This allows short
  /// circuiting when there are no code lenses enabled.
  pub fn enabled_code_lens(&self) -> bool {
    self.code_lens.implementations || self.code_lens.references
  }
}

#[derive(Debug, Default, Clone)]
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
    capabilities: &lsp::ClientCapabilities,
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

    if let Some(text_document) = &capabilities.text_document {
      self.client_capabilities.line_folding_only = text_document
        .folding_range
        .as_ref()
        .and_then(|it| it.line_folding_only)
        .unwrap_or(false);
    }
  }
}
