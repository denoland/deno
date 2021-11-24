use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use lspower::lsp;

use super::lsp_custom;

#[derive(Clone)]
pub struct Client(Arc<dyn ClientTrait>);

impl std::fmt::Debug for Client {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Client").finish()
  }
}

impl Client {
  pub fn from_lspower(client: lspower::Client) -> Client {
    Client(Arc::new(LspowerClient(client)))
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

type FutureReturn<T> = Pin<Box<dyn Future<Output = T> + 'static + Send>>;

trait ClientTrait: Send + Sync {
  fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) -> FutureReturn<()>;
  fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) -> FutureReturn<()>;
  // todo(dsherret): how to return `AnyError` or something similar instead of `String` here?
  fn configuration(
    &self,
    items: Vec<lsp::ConfigurationItem>,
  ) -> FutureReturn<Result<Vec<serde_json::Value>, AnyError>>;
  fn show_message(
    &self,
    message_type: lsp::MessageType,
    text: String,
  ) -> FutureReturn<()>;
  fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> FutureReturn<Result<(), AnyError>>;
}

#[derive(Clone)]
struct LspowerClient(lspower::Client);

impl ClientTrait for LspowerClient {
  fn publish_diagnostics(
    &self,
    uri: lsp::Url,
    diagnostics: Vec<lsp::Diagnostic>,
    version: Option<i32>,
  ) -> FutureReturn<()> {
    let client = self.0.clone();
    Box::pin(async move {
      client.publish_diagnostics(uri, diagnostics, version).await
    })
  }

  fn send_registry_state_notification(
    &self,
    params: lsp_custom::RegistryStateNotificationParams,
  ) -> FutureReturn<()> {
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
  ) -> FutureReturn<Result<Vec<serde_json::Value>, AnyError>> {
    let client = self.0.clone();
    Box::pin(async move {
      match client.configuration(items).await {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow!("{}", err.to_string())),
      }
    })
  }

  fn show_message(
    &self,
    message_type: lsp::MessageType,
    message: String,
  ) -> FutureReturn<()> {
    let client = self.0.clone();
    Box::pin(async move { client.show_message(message_type, message).await })
  }

  fn register_capability(
    &self,
    registrations: Vec<lsp::Registration>,
  ) -> FutureReturn<Result<(), AnyError>> {
    let client = self.0.clone();
    Box::pin(async move {
      match client.register_capability(registrations).await {
        Ok(()) => Ok(()),
        Err(err) => Err(anyhow!("{}", err.to_string())),
      }
    })
  }
}
