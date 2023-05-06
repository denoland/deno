// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use async_trait::async_trait;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::ConfigurationItem;

use crate::lsp::repl::get_repl_workspace_settings;

use super::config::SpecifierSettings;
use super::config::SETTINGS_SECTION;
use super::lsp_custom;
use super::testing::lsp_custom as testing_lsp_custom;
use super::urls::LspClientUrl;

#[derive(Debug)]
pub enum TestingNotification {
  Module(testing_lsp_custom::TestModuleNotificationParams),
  DeleteModule(testing_lsp_custom::TestModuleDeleteNotificationParams),
  Progress(testing_lsp_custom::TestRunProgressParams),
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum LspClientKind {
  #[default]
  CodeEditor,
  Repl,
}

#[derive(Clone)]
pub struct Client(Arc<dyn ClientTrait>);

impl std::fmt::Debug for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Client").finish()
  }
}

impl Client {
  pub fn from_tower(client: tower_lsp::Client) -> Self {
    Self(Arc::new(TowerClient(client)))
  }

  pub fn new_for_repl() -> Self {
    Self(Arc::new(ReplClient))
  }

  pub fn kind(&self) -> LspClientKind {
    self.0.kind()
  }

  /// Gets additional methods that should only be called outside
  /// the LSP's lock to prevent deadlocking scenarios.
  pub fn when_outside_lsp_lock(&self) -> OutsideLockClient {
    OutsideLockClient(self.0.clone())
  }

  pub fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    tokio::task::spawn(async move {
      client.send_registry_state_notification(params).await;
    });
  }

  pub fn send_test_notification(&self, params: TestingNotification) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    tokio::task::spawn(async move {
      client.send_test_notification(params).await;
    });
  }

  pub fn show_message(
    &self,
    message_type: lsp::MessageType,
    message: impl std::fmt::Display,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    let message = message.to_string();
    tokio::task::spawn(async move {
      client.show_message(message_type, message).await;
    });
  }
}

/// DANGER: The methods on this client should only be called outside
/// the LSP's lock. The reason is you never want to call into the client
/// while holding the lock because the client might call back into the
/// server and cause a deadlock.
pub struct OutsideLockClient(Arc<dyn ClientTrait>);

impl OutsideLockClient {
  pub async fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> Result<(), AnyError> {
    self.0.register_capability(registrations).await
  }

  pub async fn specifier_configurations(
    &self,
    specifiers: Vec<LspClientUrl>,
  ) -> Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError> {
    self
      .0
      .specifier_configurations(
        specifiers.into_iter().map(|s| s.into_url()).collect(),
      )
      .await
  }

  pub async fn specifier_configuration(
    &self,
    specifier: &LspClientUrl,
  ) -> Result<SpecifierSettings, AnyError> {
    let values = self
      .0
      .specifier_configurations(vec![specifier.as_url().clone()])
      .await?;
    if let Some(value) = values.into_iter().next() {
      value.map_err(|err| {
        anyhow!(
          "Error converting specifier settings ({}): {}",
          specifier,
          err
        )
      })
    } else {
      bail!(
        "Expected the client to return a configuration item for specifier: {}",
        specifier
      );
    }
  }

  pub async fn workspace_configuration(&self) -> Result<Value, AnyError> {
    self.0.workspace_configuration().await
  }

  pub async fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diags: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) {
    self.0.publish_diagnostics(uri, diags, version).await;
  }
}

#[async_trait]
trait ClientTrait: Send + Sync {
  fn kind(&self) -> LspClientKind;
  async fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  );
  async fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  );
  async fn send_test_notification(&self, params: TestingNotification);
  async fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError>;
  async fn workspace_configuration(&self) -> Result<Value, AnyError>;
  async fn show_message(&self, message_type: lsp::MessageType, text: String);
  async fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> Result<(), AnyError>;
}

#[derive(Clone)]
struct TowerClient(tower_lsp::Client);

#[async_trait]
impl ClientTrait for TowerClient {
  fn kind(&self) -> LspClientKind {
    LspClientKind::CodeEditor
  }

  async fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) {
    self.0.publish_diagnostics(uri, diagnostics, version).await
  }

  async fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) {
    self
      .0
      .send_notification::<lsp_custom::RegistryStateNotification>(params)
      .await
  }

  async fn send_test_notification(&self, notification: TestingNotification) {
    match notification {
      TestingNotification::Module(params) => {
        self
          .0
          .send_notification::<testing_lsp_custom::TestModuleNotification>(
            params,
          )
          .await
      }
      TestingNotification::DeleteModule(params) => self
        .0
        .send_notification::<testing_lsp_custom::TestModuleDeleteNotification>(
          params,
        )
        .await,
      TestingNotification::Progress(params) => {
        self
          .0
          .send_notification::<testing_lsp_custom::TestRunProgressNotification>(
            params,
          )
          .await
      }
    }
  }

  async fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError> {
    let config_response = self
      .0
      .configuration(
        uris
          .into_iter()
          .map(|uri| ConfigurationItem {
            scope_uri: Some(uri),
            section: Some(SETTINGS_SECTION.to_string()),
          })
          .collect(),
      )
      .await?;

    Ok(
      config_response
        .into_iter()
        .map(|value| {
          serde_json::from_value::<SpecifierSettings>(value).map_err(|err| {
            anyhow!("Error converting specifier settings: {}", err)
          })
        })
        .collect(),
    )
  }

  async fn workspace_configuration(&self) -> Result<Value, AnyError> {
    let config_response = self
      .0
      .configuration(vec![ConfigurationItem {
        scope_uri: None,
        section: Some(SETTINGS_SECTION.to_string()),
      }])
      .await;
    match config_response {
      Ok(value_vec) => match value_vec.get(0).cloned() {
        Some(value) => Ok(value),
        None => bail!("Missing response workspace configuration."),
      },
      Err(err) => {
        bail!("Error getting workspace configuration: {}", err)
      }
    }
  }

  async fn show_message(
    &self,
    message_type: lsp::MessageType,
    message: String,
  ) {
    self.0.show_message(message_type, message).await
  }

  async fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> Result<(), AnyError> {
    self
      .0
      .register_capability(registrations)
      .await
      .map_err(|err| anyhow!("{}", err))
  }
}

#[derive(Clone)]
struct ReplClient;

#[async_trait]
impl ClientTrait for ReplClient {
  fn kind(&self) -> LspClientKind {
    LspClientKind::Repl
  }

  async fn publish_diagnostics(
    &self,
    _uri: lsp::Url,
    _diagnostics: Vec<lsp::Diagnostic>,
    _version: Option<i32>,
  ) {
  }

  async fn send_registry_state_notification(
    &self,
    _params: lsp_custom::RegistryStateNotificationParams,
  ) {
  }

  async fn send_test_notification(&self, _params: TestingNotification) {}

  async fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError> {
    // all specifiers are enabled for the REPL
    let settings = uris
      .into_iter()
      .map(|_| {
        Ok(SpecifierSettings {
          enable: true,
          ..Default::default()
        })
      })
      .collect();
    Ok(settings)
  }

  async fn workspace_configuration(&self) -> Result<Value, AnyError> {
    Ok(serde_json::to_value(get_repl_workspace_settings()).unwrap())
  }

  async fn show_message(
    &self,
    _message_type: lsp::MessageType,
    _message: String,
  ) {
  }

  async fn register_capability(
    &self,
    _registrations: Vec<lsp::Registration>,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}
