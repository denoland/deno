use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::serde_json;
use deno_core::serde_json::json;
use lspower::lsp;

use crate::lsp::config::SETTINGS_SECTION;
use crate::lsp::repl::get_repl_workspace_settings;

use super::lsp_custom;

#[derive(Clone)]
pub struct Client(Arc<dyn ClientTrait>);

impl std::fmt::Debug for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Client").finish()
  }
}

impl Client {
  pub fn from_lspower(client: lspower::Client) -> Self {
    Self(Arc::new(LspowerClient(client)))
  }

  pub fn new_for_repl() -> Self {
    Self(Arc::new(ReplClient))
  }

  pub async fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diags: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) {
    self.0.publish_diagnostics(uri, diags, version).await;
  }

  pub async fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) {
    self.0.send_registry_state_notification(params).await;
  }

  pub async fn configuration(
    &self,
    items: Vec<lsp::ConfigurationItem>,
  ) -> Result<Vec<serde_json::Value>, AnyError> {
    self.0.configuration(items).await
  }

  pub async fn show_message(
    &self,
    message_type: lsp::MessageType,
    message: impl std::fmt::Display,
  ) {
    self
      .0
      .show_message(message_type, format!("{}", message))
      .await
  }

  pub async fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> Result<(), AnyError> {
    self.0.register_capability(registrations).await
  }
}

type AsyncReturn<T> = Pin<Box<dyn Future<Output = T> + 'static + Send>>;

trait ClientTrait: Send + Sync {
  fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) -> AsyncReturn<()>;
  fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) -> AsyncReturn<()>;
  fn configuration(
    &self,
    items: Vec<lsp::ConfigurationItem>,
  ) -> AsyncReturn<Result<Vec<serde_json::Value>, AnyError>>;
  fn show_message(
    &self,
    message_type: lsp::MessageType,
    text: String,
  ) -> AsyncReturn<()>;
  fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> AsyncReturn<Result<(), AnyError>>;
}

#[derive(Clone)]
struct LspowerClient(lspower::Client);

impl ClientTrait for LspowerClient {
  fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) -> AsyncReturn<()> {
    let client = self.0.clone();
    Box::pin(async move {
      client.publish_diagnostics(uri, diagnostics, version).await
    })
  }

  fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) -> AsyncReturn<()> {
    let client = self.0.clone();
    Box::pin(async move {
      client
        .send_custom_notification::<lsp_custom::RegistryStateNotification>(
          params,
        )
        .await
    })
  }

  fn configuration(
    &self,
    items: Vec<lsp::ConfigurationItem>,
  ) -> AsyncReturn<Result<Vec<serde_json::Value>, AnyError>> {
    let client = self.0.clone();
    Box::pin(async move {
      client
        .configuration(items)
        .await
        .map_err(|err| anyhow!("{}", err))
    })
  }

  fn show_message(
    &self,
    message_type: lsp::MessageType,
    message: String,
  ) -> AsyncReturn<()> {
    let client = self.0.clone();
    Box::pin(async move { client.show_message(message_type, message).await })
  }

  fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> AsyncReturn<Result<(), AnyError>> {
    let client = self.0.clone();
    Box::pin(async move {
      client
        .register_capability(registrations)
        .await
        .map_err(|err| anyhow!("{}", err))
    })
  }
}

#[derive(Clone)]
struct ReplClient;

impl ClientTrait for ReplClient {
  fn publish_diagnostics(
    &self,
    _uri: lsp::Url,
    _diagnostics: Vec<lsp::Diagnostic>,
    _version: Option<i32>,
  ) -> AsyncReturn<()> {
    Box::pin(future::ready(()))
  }

  fn send_registry_state_notification(
    &self,
    _params: lsp_custom::RegistryStateNotificationParams,
  ) -> AsyncReturn<()> {
    Box::pin(future::ready(()))
  }

  fn configuration(
    &self,
    items: Vec<lsp::ConfigurationItem>,
  ) -> AsyncReturn<Result<Vec<serde_json::Value>, AnyError>> {
    let is_global_config_request = items.len() == 1
      && items[0].scope_uri.is_none()
      && items[0].section.as_deref() == Some(SETTINGS_SECTION);
    let response = if is_global_config_request {
      vec![serde_json::to_value(get_repl_workspace_settings()).unwrap()]
    } else {
      // all specifiers are enabled for the REPL
      items
        .into_iter()
        .map(|_| {
          json!({
            "enable": true,
          })
        })
        .collect()
    };
    Box::pin(future::ready(Ok(response)))
  }

  fn show_message(
    &self,
    _message_type: lsp::MessageType,
    _message: String,
  ) -> AsyncReturn<()> {
    Box::pin(future::ready(()))
  }

  fn register_capability(
    &self,
    _registrations: Vec<lsp::Registration>,
  ) -> AsyncReturn<Result<(), AnyError>> {
    Box::pin(future::ready(Ok(())))
  }
}
