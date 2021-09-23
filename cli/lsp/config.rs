// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::tokio_util::create_basic_runtime;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use log::error;
use lsp::WorkspaceFolder;
use lspower::lsp;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;

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

#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
  pub client_capabilities: ClientCapabilities,
  pub root_uri: Option<Url>,
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

enum ConfigRequest {
  All,
  Specifier(ModuleSpecifier, ModuleSpecifier),
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
  pub root_uri: Option<Url>,
  settings: Arc<RwLock<Settings>>,
  tx: mpsc::Sender<ConfigRequest>,
  pub workspace_folders: Option<Vec<WorkspaceFolder>>,
}

impl Config {
  pub fn new(client: lspower::Client) -> Self {
    let (tx, mut rx) = mpsc::channel::<ConfigRequest>(100);
    let settings = Arc::new(RwLock::new(Settings::default()));
    let settings_ref = settings.clone();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match rx.recv().await {
            None => break,
            Some(ConfigRequest::All) => {
              let (specifier_uri_map, items): (
                Vec<(ModuleSpecifier, ModuleSpecifier)>,
                Vec<lsp::ConfigurationItem>,
              ) = {
                let settings = settings_ref.read();
                (
                  settings
                    .specifiers
                    .iter()
                    .map(|(s, (u, _))| (s.clone(), u.clone()))
                    .collect(),
                  settings
                    .specifiers
                    .iter()
                    .map(|(_, (uri, _))| lsp::ConfigurationItem {
                      scope_uri: Some(uri.clone()),
                      section: Some(SETTINGS_SECTION.to_string()),
                    })
                    .collect(),
                )
              };
              if let Ok(configs) = client.configuration(items).await {
                let mut settings = settings_ref.write();
                for (i, value) in configs.into_iter().enumerate() {
                  match serde_json::from_value::<SpecifierSettings>(value) {
                    Ok(specifier_settings) => {
                      let (specifier, uri) = specifier_uri_map[i].clone();
                      settings
                        .specifiers
                        .insert(specifier, (uri, specifier_settings));
                    }
                    Err(err) => {
                      error!("Error converting specifier settings: {}", err);
                    }
                  }
                }
              }
            }
            Some(ConfigRequest::Specifier(specifier, uri)) => {
              if settings_ref.read().specifiers.contains_key(&specifier) {
                continue;
              }
              if let Ok(value) = client
                .configuration(vec![lsp::ConfigurationItem {
                  scope_uri: Some(uri.clone()),
                  section: Some(SETTINGS_SECTION.to_string()),
                }])
                .await
              {
                match serde_json::from_value::<SpecifierSettings>(
                  value[0].clone(),
                ) {
                  Ok(specifier_settings) => {
                    settings_ref
                      .write()
                      .specifiers
                      .insert(specifier, (uri, specifier_settings));
                  }
                  Err(err) => {
                    error!("Error converting specifier settings: {}", err);
                  }
                }
              } else {
                error!(
                  "Error retrieving settings for specifier: {}",
                  specifier
                );
              }
            }
          }
        }
      })
    });

    Self {
      client_capabilities: ClientCapabilities::default(),
      root_uri: None,
      settings,
      tx,
      workspace_folders: None,
    }
  }

  pub fn get_workspace_settings(&self) -> WorkspaceSettings {
    self.settings.read().workspace.clone()
  }

  /// Set the workspace settings directly, which occurs during initialization
  /// and when the client does not support workspace configuration requests
  pub fn set_workspace_settings(&self, value: Value) -> Result<(), AnyError> {
    let workspace_settings = serde_json::from_value(value)?;
    self.settings.write().workspace = workspace_settings;
    Ok(())
  }

  pub fn snapshot(&self) -> Result<ConfigSnapshot, AnyError> {
    Ok(ConfigSnapshot {
      client_capabilities: self.client_capabilities.clone(),
      root_uri: self.root_uri.clone(),
      settings: self
        .settings
        .try_read()
        .ok_or_else(|| anyhow!("Error reading settings."))?
        .clone(),
      workspace_folders: self.workspace_folders.clone(),
    })
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    let settings = self.settings.read();
    settings
      .specifiers
      .get(specifier)
      .map(|(_, s)| s.enable)
      .unwrap_or_else(|| settings.workspace.enable)
  }

  pub fn specifier_code_lens_test(&self, specifier: &ModuleSpecifier) -> bool {
    let settings = self.settings.read();
    let value = settings
      .specifiers
      .get(specifier)
      .map(|(_, s)| s.code_lens.test)
      .unwrap_or_else(|| settings.workspace.code_lens.test);
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

  /// Update all currently cached specifier settings
  pub async fn update_all_settings(&self) -> Result<(), AnyError> {
    self
      .tx
      .send(ConfigRequest::All)
      .await
      .map_err(|_| anyhow!("Error sending config update task."))
  }

  /// Update a specific specifiers settings from the client.
  pub async fn update_specifier_settings(
    &self,
    specifier: &ModuleSpecifier,
    uri: &ModuleSpecifier,
  ) -> Result<(), AnyError> {
    self
      .tx
      .send(ConfigRequest::Specifier(specifier.clone(), uri.clone()))
      .await
      .map_err(|_| anyhow!("Error sending config update task."))
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
    let mut maybe_client: Option<lspower::Client> = None;
    let (_service, _) = lspower::LspService::new(|client| {
      maybe_client = Some(client);
      MockLanguageServer::default()
    });
    Config::new(maybe_client.unwrap())
  }

  #[test]
  fn test_config_specifier_enabled() {
    let config = setup();
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
    let config = setup();
    config
      .set_workspace_settings(json!({}))
      .expect("could not update");
    assert_eq!(
      config.get_workspace_settings(),
      WorkspaceSettings {
        enable: false,
        cache: None,
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
        unstable: false,
      }
    );
  }
}
