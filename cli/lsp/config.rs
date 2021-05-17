// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use lspower::jsonrpc::Error as LSPError;
use lspower::jsonrpc::Result as LSPResult;
use lspower::lsp;
use std::collections::HashMap;

pub const SETTINGS_SECTION: &str = "deno";

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
  #[serde(default)]
  pub imports: ImportCompletionSettings,
}

impl Default for CompletionSettings {
  fn default() -> Self {
    Self {
      complete_function_calls: false,
      names: true,
      paths: true,
      auto_imports: true,
      imports: ImportCompletionSettings::default(),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompletionSettings {
  #[serde(default)]
  pub hosts: HashMap<String, bool>,
}

impl Default for ImportCompletionSettings {
  fn default() -> Self {
    Self {
      hosts: HashMap::default(),
    }
  }
}

/// Deno language server specific settings that can be applied uniquely to a
/// specifier.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SpecifierSettings {
  /// A flag that indicates if Deno is enabled for this specifier or not.
  pub enable: bool,
}

/// Deno language server specific settings that are applied to a workspace.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  /// A flag that indicates if Deno is enabled for the workspace.
  pub enable: bool,

  /// An option that points to a path string of the config file to apply to
  /// code within the workspace.
  pub config: Option<String>,

  /// An option that points to a path string of the import map to apply to the
  /// code within the workspace.
  pub import_map: Option<String>,

  /// Code lens specific settings for the workspace.
  #[serde(default)]
  pub code_lens: CodeLensSettings,

  /// A flag that indicates if internal debug logging should be made available.
  #[serde(default)]
  pub internal_debug: bool,

  /// A flag that indicates if linting is enabled for the workspace.
  #[serde(default)]
  pub lint: bool,

  /// A flag that indicates if Dene should validate code against the unstable
  /// APIs for the workspace.
  #[serde(default)]
  pub suggest: CompletionSettings,

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
  pub specifier_settings: HashMap<ModuleSpecifier, SpecifierSettings>,
  pub workspace_settings: WorkspaceSettings,
}

impl Config {
  #[allow(unused)]
  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
    self.specifier_settings.contains_key(specifier)
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    if let Some(settings) = self.specifier_settings.get(specifier) {
      settings.enable
    } else {
      self.workspace_settings.enable
    }
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

  pub fn update_specifier(
    &mut self,
    specifier: ModuleSpecifier,
    value: Value,
  ) -> LSPResult<()> {
    let settings: SpecifierSettings = serde_json::from_value(value)
      .map_err(|err| LSPError::invalid_params(err.to_string()))?;
    self.specifier_settings.insert(specifier, settings);
    Ok(())
  }

  pub fn update_workspace(&mut self, value: Value) -> LSPResult<()> {
    let settings: WorkspaceSettings = serde_json::from_value(value)
      .map_err(|err| LSPError::invalid_params(err.to_string()))?;
    self.workspace_settings = settings;
    self.specifier_settings = HashMap::new();
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;
  use deno_core::serde_json::json;

  #[test]
  fn test_config_contains() {
    let mut config = Config::default();
    let specifier = resolve_url("https://deno.land/x/a.ts").unwrap();
    assert!(!config.contains(&specifier));
    config
      .update_specifier(
        specifier.clone(),
        json!({
          "enable": true
        }),
      )
      .expect("could not update specifier");
    assert!(config.contains(&specifier));
  }

  #[test]
  fn test_config_specifier_enabled() {
    let mut config = Config::default();
    let specifier = resolve_url("file:///a.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier));
    config
      .update_workspace(json!({
        "enable": true
      }))
      .expect("could not update");
    assert!(config.specifier_enabled(&specifier));
    config
      .update_specifier(
        specifier.clone(),
        json!({
          "enable": false
        }),
      )
      .expect("could not update");
    assert!(!config.specifier_enabled(&specifier));
  }
}
