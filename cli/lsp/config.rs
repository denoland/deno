// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
  pub status_notification: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  pub enable: bool,
  pub config: Option<String>,
  pub import_map: Option<String>,
  pub lint: bool,
  pub unstable: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
  pub client_capabilities: ClientCapabilities,
  pub settings: WorkspaceSettings,
}

impl Config {
  pub fn update(&mut self, value: Value) -> Result<(), AnyError> {
    let settings: WorkspaceSettings = serde_json::from_value(value)?;
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
  }
}
