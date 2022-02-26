// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use lsp::WorkspaceFolder;
use lspower::lsp;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

pub const SETTINGS_SECTION: &str = "deno";

#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
  pub code_action_disabled_support: bool,
  pub line_folding_only: bool,
  pub status_notification: bool,
  pub workspace_configuration: bool,
  pub workspace_did_change_watched_files: bool,
}

fn is_true() -> bool {
  true
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
  /// Flag for providing test code lens on `Deno.test` statements.  There is
  /// also the `test_args` setting, but this is not used by the server.
  #[serde(default = "is_true")]
  pub test: bool,
}

impl Default for CodeLensSettings {
  fn default() -> Self {
    Self {
      implementations: false,
      references: false,
      references_all_functions: false,
      test: true,
    }
  }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensSpecifierSettings {
  /// Flag for providing test code lens on `Deno.test` statements.  There is
  /// also the `test_args` setting, but this is not used by the server.
  #[serde(default = "is_true")]
  pub test: bool,
}

impl Default for CodeLensSpecifierSettings {
  fn default() -> Self {
    Self { test: true }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompletionSettings {
  #[serde(default)]
  pub complete_function_calls: bool,
  #[serde(default = "is_true")]
  pub names: bool,
  #[serde(default = "is_true")]
  pub paths: bool,
  #[serde(default = "is_true")]
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompletionSettings {
  /// A flag that indicates if non-explicitly set origins should be checked for
  /// supporting import suggestions.
  #[serde(default = "is_true")]
  pub auto_discover: bool,
  /// A map of origins which have had explicitly set if import suggestions are
  /// enabled.
  #[serde(default)]
  pub hosts: HashMap<String, bool>,
}

impl Default for ImportCompletionSettings {
  fn default() -> Self {
    Self {
      auto_discover: true,
      hosts: HashMap::default(),
    }
  }
}

/// Deno language server specific settings that can be applied uniquely to a
/// specifier.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecifierSettings {
  /// A flag that indicates if Deno is enabled for this specifier or not.
  pub enable: bool,
  /// Code lens specific settings for the resource.
  #[serde(default)]
  pub code_lens: CodeLensSpecifierSettings,
}

/// Deno language server specific settings that are applied to a workspace.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  /// A flag that indicates if Deno is enabled for the workspace.
  #[serde(default)]
  pub enable: bool,

  /// An option that points to a path string of the path to utilise as the
  /// cache/DENO_DIR for the language server.
  pub cache: Option<String>,

  /// Override the default stores used to validate certificates. This overrides
  /// the environment variable `DENO_TLS_CA_STORE` if present.
  pub certificate_stores: Option<Vec<String>>,

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

  /// An option which sets the cert file to use when attempting to fetch remote
  /// resources. This overrides `DENO_CERT` if present.
  pub tls_certificate: Option<String>,

  /// An option, if set, will unsafely ignore certificate errors when fetching
  /// remote resources.
  #[serde(default)]
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,

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

#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
  pub client_capabilities: ClientCapabilities,
  pub settings: Settings,
  pub workspace_folders: Option<Vec<lsp::WorkspaceFolder>>,
}

impl ConfigSnapshot {
  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    if let Some(settings) = self.settings.specifiers.get(specifier) {
      settings.1.enable
    } else {
      self.settings.workspace.enable
    }
  }
}

#[derive(Debug, Clone)]
pub struct SpecifierWithClientUri {
  pub specifier: ModuleSpecifier,
  pub client_uri: ModuleSpecifier,
}

#[derive(Debug, Default, Clone)]
pub struct Settings {
  pub specifiers:
    BTreeMap<ModuleSpecifier, (ModuleSpecifier, SpecifierSettings)>,
  pub workspace: WorkspaceSettings,
}

#[derive(Debug)]
pub struct Config {
  pub client_capabilities: ClientCapabilities,
  settings: Settings,
  pub workspace_folders: Option<Vec<WorkspaceFolder>>,
}

impl Config {
  pub fn new() -> Self {
    Self {
      client_capabilities: ClientCapabilities::default(),
      settings: Default::default(),
      workspace_folders: None,
    }
  }

  pub fn get_workspace_settings(&self) -> WorkspaceSettings {
    self.settings.workspace.clone()
  }

  /// Set the workspace settings directly, which occurs during initialization
  /// and when the client does not support workspace configuration requests
  pub fn set_workspace_settings(
    &mut self,
    value: Value,
  ) -> Result<(), AnyError> {
    let workspace_settings = serde_json::from_value(value)?;
    self.settings.workspace = workspace_settings;
    Ok(())
  }

  pub fn snapshot(&self) -> Arc<ConfigSnapshot> {
    Arc::new(ConfigSnapshot {
      client_capabilities: self.client_capabilities.clone(),
      settings: self.settings.clone(),
      workspace_folders: self.workspace_folders.clone(),
    })
  }

  pub fn has_specifier_settings(&self, specifier: &ModuleSpecifier) -> bool {
    self.settings.specifiers.contains_key(specifier)
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    self
      .settings
      .specifiers
      .get(specifier)
      .map(|(_, s)| s.enable)
      .unwrap_or_else(|| self.settings.workspace.enable)
  }

  pub fn specifier_code_lens_test(&self, specifier: &ModuleSpecifier) -> bool {
    let value = self
      .settings
      .specifiers
      .get(specifier)
      .map(|(_, s)| s.code_lens.test)
      .unwrap_or_else(|| self.settings.workspace.code_lens.test);
    value
  }

  pub fn update_capabilities(
    &mut self,
    capabilities: &lsp::ClientCapabilities,
  ) {
    if let Some(experimental) = &capabilities.experimental {
      self.client_capabilities.status_notification = experimental
        .get("statusNotification")
        .and_then(|it| it.as_bool())
        == Some(true)
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
      self.client_capabilities.code_action_disabled_support = text_document
        .code_action
        .as_ref()
        .and_then(|it| it.disabled_support)
        .unwrap_or(false);
    }
  }

  pub fn get_specifiers_with_client_uris(&self) -> Vec<SpecifierWithClientUri> {
    self
      .settings
      .specifiers
      .iter()
      .map(|(s, (u, _))| SpecifierWithClientUri {
        specifier: s.clone(),
        client_uri: u.clone(),
      })
      .collect::<Vec<_>>()
  }

  pub fn set_specifier_settings(
    &mut self,
    specifier: ModuleSpecifier,
    client_uri: ModuleSpecifier,
    settings: SpecifierSettings,
  ) {
    self
      .settings
      .specifiers
      .insert(specifier, (client_uri, settings));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;
  use deno_core::serde_json::json;

  #[derive(Debug, Default)]
  struct MockLanguageServer;

  #[lspower::async_trait]
  impl lspower::LanguageServer for MockLanguageServer {
    async fn initialize(
      &self,
      _params: lspower::lsp::InitializeParams,
    ) -> lspower::jsonrpc::Result<lsp::InitializeResult> {
      Ok(lspower::lsp::InitializeResult {
        capabilities: lspower::lsp::ServerCapabilities::default(),
        server_info: None,
      })
    }

    async fn shutdown(&self) -> lspower::jsonrpc::Result<()> {
      Ok(())
    }
  }

  fn setup() -> Config {
    Config::new()
  }

  #[test]
  fn test_config_specifier_enabled() {
    let mut config = setup();
    let specifier = resolve_url("file:///a.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier));
    config
      .set_workspace_settings(json!({
        "enable": true
      }))
      .expect("could not update");
    assert!(config.specifier_enabled(&specifier));
  }

  #[test]
  fn test_set_workspace_settings_defaults() {
    let mut config = setup();
    config
      .set_workspace_settings(json!({}))
      .expect("could not update");
    assert_eq!(
      config.get_workspace_settings(),
      WorkspaceSettings {
        enable: false,
        cache: None,
        certificate_stores: None,
        config: None,
        import_map: None,
        code_lens: CodeLensSettings {
          implementations: false,
          references: false,
          references_all_functions: false,
          test: true,
        },
        internal_debug: false,
        lint: false,
        suggest: CompletionSettings {
          complete_function_calls: false,
          names: true,
          paths: true,
          auto_imports: true,
          imports: ImportCompletionSettings {
            auto_discover: true,
            hosts: HashMap::new(),
          }
        },
        tls_certificate: None,
        unsafely_ignore_certificate_errors: None,
        unstable: false,
      }
    );
  }
}
