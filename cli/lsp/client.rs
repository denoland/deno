// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use async_trait::async_trait;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::unsync::spawn;
use lsp_types::Uri;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::ConfigurationItem;

use crate::lsp::repl::get_repl_workspace_settings;

use super::config::WorkspaceSettings;
use super::config::SETTINGS_SECTION;
use super::lsp_custom;
use super::testing::lsp_custom as testing_lsp_custom;

#[derive(Debug)]
pub enum TestingNotification {
  Module(testing_lsp_custom::TestModuleNotificationParams),
  DeleteModule(testing_lsp_custom::TestModuleDeleteNotificationParams),
  Progress(testing_lsp_custom::TestRunProgressParams),
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

  /// Gets additional methods that should only be called outside
  /// the LSP's lock to prevent deadlocking scenarios.
  pub fn when_outside_lsp_lock(&self) -> OutsideLockClient {
    OutsideLockClient(self.0.clone())
  }

  pub async fn publish_diagnostics(
    &self,
    uri: Uri,
    diags: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) {
    self.0.publish_diagnostics(uri, diags, version).await;
  }

  pub fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    spawn(async move {
      client.send_registry_state_notification(params).await;
    });
  }

  /// This notification is sent to the client during internal testing
  /// purposes only in order to let the test client know when the latest
  /// diagnostics have been published.
  pub fn send_diagnostic_batch_notification(
    &self,
    params: lsp_custom::DiagnosticBatchNotificationParams,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    spawn(async move {
      client.send_diagnostic_batch_notification(params).await;
    });
  }

  pub fn send_test_notification(&self, params: TestingNotification) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    spawn(async move {
      client.send_test_notification(params).await;
    });
  }

  pub fn send_did_change_deno_configuration_notification(
    &self,
    params: lsp_custom::DidChangeDenoConfigurationNotificationParams,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    spawn(async move {
      client
        .send_did_change_deno_configuration_notification(params)
        .await;
    });
  }

  pub fn send_did_upgrade_check_notification(
    &self,
    params: lsp_custom::DidUpgradeCheckNotificationParams,
  ) {
    // do on a task in case the caller currently is in the lsp lock
    let client = self.0.clone();
    spawn(async move {
      client.send_did_upgrade_check_notification(params).await;
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
    spawn(async move {
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

  pub async fn workspace_configuration(
    &self,
    scopes: Vec<Option<lsp::Uri>>,
  ) -> Result<Vec<WorkspaceSettings>, AnyError> {
    self.0.workspace_configuration(scopes).await
  }
}

#[async_trait]
trait ClientTrait: Send + Sync {
  async fn publish_diagnostics(
    &self,
    uri: lsp::Uri,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  );
  async fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  );
  async fn send_diagnostic_batch_notification(
    &self,
    params: lsp_custom::DiagnosticBatchNotificationParams,
  );
  async fn send_test_notification(&self, params: TestingNotification);
  async fn send_did_change_deno_configuration_notification(
    &self,
    params: lsp_custom::DidChangeDenoConfigurationNotificationParams,
  );
  async fn send_did_upgrade_check_notification(
    &self,
    params: lsp_custom::DidUpgradeCheckNotificationParams,
  );
  async fn workspace_configuration(
    &self,
    scopes: Vec<Option<lsp::Uri>>,
  ) -> Result<Vec<WorkspaceSettings>, AnyError>;
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
  async fn publish_diagnostics(
    &self,
    uri: lsp::Uri,
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

  async fn send_diagnostic_batch_notification(
    &self,
    params: lsp_custom::DiagnosticBatchNotificationParams,
  ) {
    self
      .0
      .send_notification::<lsp_custom::DiagnosticBatchNotification>(params)
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

  async fn send_did_change_deno_configuration_notification(
    &self,
    params: lsp_custom::DidChangeDenoConfigurationNotificationParams,
  ) {
    self
      .0
      .send_notification::<lsp_custom::DidChangeDenoConfigurationNotification>(
        params,
      )
      .await
  }

  async fn send_did_upgrade_check_notification(
    &self,
    params: lsp_custom::DidUpgradeCheckNotificationParams,
  ) {
    self
      .0
      .send_notification::<lsp_custom::DidUpgradeCheckNotification>(params)
      .await
  }

  async fn workspace_configuration(
    &self,
    scopes: Vec<Option<lsp::Uri>>,
  ) -> Result<Vec<WorkspaceSettings>, AnyError> {
    let config_response = self
      .0
      .configuration(
        scopes
          .iter()
          .flat_map(|scope_uri| {
            vec![
              ConfigurationItem {
                scope_uri: scope_uri.clone(),
                section: Some(SETTINGS_SECTION.to_string()),
              },
              ConfigurationItem {
                scope_uri: scope_uri.clone(),
                section: Some("javascript".to_string()),
              },
              ConfigurationItem {
                scope_uri: scope_uri.clone(),
                section: Some("typescript".to_string()),
              },
            ]
          })
          .collect(),
      )
      .await;
    match config_response {
      Ok(configs) => {
        let mut configs = configs.into_iter();
        let mut result = Vec::with_capacity(scopes.len());
        for _ in 0..scopes.len() {
          let deno = json!(configs.next());
          let javascript = json!(configs.next());
          let typescript = json!(configs.next());
          result.push(WorkspaceSettings::from_raw_settings(
            deno, javascript, typescript,
          ));
        }
        Ok(result)
      }
      Err(err) => {
        bail!("Error getting workspace configurations: {}", err)
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
  async fn publish_diagnostics(
    &self,
    _uri: lsp::Uri,
    _diagnostics: Vec<lsp::Diagnostic>,
    _version: Option<i32>,
  ) {
  }

  async fn send_registry_state_notification(
    &self,
    _params: lsp_custom::RegistryStateNotificationParams,
  ) {
  }

  async fn send_diagnostic_batch_notification(
    &self,
    _params: lsp_custom::DiagnosticBatchNotificationParams,
  ) {
  }

  async fn send_test_notification(&self, _params: TestingNotification) {}

  async fn send_did_change_deno_configuration_notification(
    &self,
    _params: lsp_custom::DidChangeDenoConfigurationNotificationParams,
  ) {
  }

  async fn send_did_upgrade_check_notification(
    &self,
    _params: lsp_custom::DidUpgradeCheckNotificationParams,
  ) {
  }

  async fn workspace_configuration(
    &self,
    scopes: Vec<Option<lsp::Uri>>,
  ) -> Result<Vec<WorkspaceSettings>, AnyError> {
    Ok(vec![get_repl_workspace_settings(); scopes.len()])
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
