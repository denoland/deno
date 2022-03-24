use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use lspower::lsp;
use lspower::lsp::ConfigurationItem;

use crate::lsp::repl::get_repl_workspace_settings;

use super::config::SpecifierSettings;
use super::config::SETTINGS_SECTION;
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

  pub async fn specifier_configurations(
    &self,
    specifiers: Vec<lsp::Url>,
  ) -> Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError> {
    self.0.specifier_configurations(specifiers).await
  }

  pub async fn specifier_configuration(
    &self,
    specifier: &lsp::Url,
  ) -> Result<SpecifierSettings, AnyError> {
    let values = self
      .0
      .specifier_configurations(vec![specifier.clone()])
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
  fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> AsyncReturn<Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError>>;
  fn workspace_configuration(&self) -> AsyncReturn<Result<Value, AnyError>>;
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

  fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> AsyncReturn<Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError>>
  {
    let client = self.0.clone();
    Box::pin(async move {
      let config_response = client
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
    })
  }

  fn workspace_configuration(&self) -> AsyncReturn<Result<Value, AnyError>> {
    let client = self.0.clone();
    Box::pin(async move {
      let config_response = client
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

  fn specifier_configurations(
    &self,
    uris: Vec<lsp::Url>,
  ) -> AsyncReturn<Result<Vec<Result<SpecifierSettings, AnyError>>, AnyError>>
  {
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
    Box::pin(future::ready(Ok(settings)))
  }

  fn workspace_configuration(&self) -> AsyncReturn<Result<Value, AnyError>> {
    Box::pin(future::ready(Ok(
      serde_json::to_value(get_repl_workspace_settings()).unwrap(),
    )))
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
