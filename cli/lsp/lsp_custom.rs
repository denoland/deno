// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use tower_lsp::lsp_types as lsp;

pub const CACHE_REQUEST: &str = "deno/cache";
pub const PERFORMANCE_REQUEST: &str = "deno/performance";
pub const TASK_REQUEST: &str = "deno/taskDefinitions";
pub const RELOAD_IMPORT_REGISTRIES_REQUEST: &str =
  "deno/reloadImportRegistries";
pub const VIRTUAL_TEXT_DOCUMENT: &str = "deno/virtualTextDocument";
pub const LATEST_DIAGNOSTIC_BATCH_INDEX: &str =
  "deno/internalLatestDiagnosticBatchIndex";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheParams {
  /// The document currently open in the editor.  If there are no `uris`
  /// supplied, the referrer will be cached.
  pub referrer: lsp::TextDocumentIdentifier,
  /// Any documents that have been specifically asked to be cached via the
  /// command.
  pub uris: Vec<lsp::TextDocumentIdentifier>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
  pub name: String,
  // TODO(nayeemrmn): Rename this to `command` in vscode_deno.
  #[serde(rename = "detail")]
  pub command: String,
  pub source_uri: lsp::Url,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegistryStateNotificationParams {
  pub origin: String,
  pub suggestions: bool,
}

pub enum RegistryStateNotification {}

impl lsp::notification::Notification for RegistryStateNotification {
  type Params = RegistryStateNotificationParams;

  const METHOD: &'static str = "deno/registryState";
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: lsp::TextDocumentIdentifier,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiagnosticBatchNotificationParams {
  pub batch_index: usize,
  pub messages_len: usize,
}

#[derive(Debug, Eq, Hash, PartialEq, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DenoConfigurationType {
  DenoJson,
  PackageJson,
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DenoConfigurationChangeEvent {
  #[serde(flatten)]
  pub file_event: lsp::FileEvent,
  pub configuration_type: DenoConfigurationType,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeDenoConfigurationNotificationParams {
  pub changes: Vec<DenoConfigurationChangeEvent>,
}

pub enum DidChangeDenoConfigurationNotification {}

impl lsp::notification::Notification
  for DidChangeDenoConfigurationNotification
{
  type Params = DidChangeDenoConfigurationNotificationParams;

  const METHOD: &'static str = "deno/didChangeDenoConfiguration";
}

pub enum DidUpgradeCheckNotification {}

impl lsp::notification::Notification for DidUpgradeCheckNotification {
  type Params = DidUpgradeCheckNotificationParams;

  const METHOD: &'static str = "deno/didUpgradeCheck";
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeAvailable {
  pub latest_version: String,
  pub is_canary: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidUpgradeCheckNotificationParams {
  pub upgrade_available: Option<UpgradeAvailable>,
}

/// This notification is only sent for testing purposes
/// in order to know what the latest diagnostics are.
pub enum DiagnosticBatchNotification {}

impl lsp::notification::Notification for DiagnosticBatchNotification {
  type Params = DiagnosticBatchNotificationParams;

  const METHOD: &'static str = "deno/internalTestDiagnosticBatch";
}
