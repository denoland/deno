// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use tower_lsp::lsp_types as lsp;

pub const PERFORMANCE_REQUEST: &str = "deno/performance";
pub const TASK_REQUEST: &str = "deno/taskDefinitions";
pub const VIRTUAL_TEXT_DOCUMENT: &str = "deno/virtualTextDocument";
pub const LATEST_DIAGNOSTIC_BATCH_INDEX: &str =
  "deno/internalLatestDiagnosticBatchIndex";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
  pub name: String,
  pub command: Option<String>,
  pub source_uri: lsp::Uri,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DenoConfigurationData {
  pub scope_uri: lsp::Uri,
  pub workspace_root_scope_uri: Option<lsp::Uri>,
  pub deno_json: Option<lsp::TextDocumentIdentifier>,
  pub package_json: Option<lsp::TextDocumentIdentifier>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidRefreshDenoConfigurationTreeNotificationParams {
  pub data: Vec<DenoConfigurationData>,
  pub deno_dir_npm_folder_uri: Option<lsp::Uri>,
}

pub enum DidRefreshDenoConfigurationTreeNotification {}

impl lsp::notification::Notification
  for DidRefreshDenoConfigurationTreeNotification
{
  type Params = DidRefreshDenoConfigurationTreeNotificationParams;
  const METHOD: &'static str = "deno/didRefreshDenoConfigurationTree";
}

#[derive(Debug, Eq, Hash, PartialEq, Copy, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DenoConfigurationChangeType {
  Added,
  Changed,
  Removed,
}

impl DenoConfigurationChangeType {
  pub fn from_file_change_type(file_event: lsp::FileChangeType) -> Self {
    match file_event {
      lsp::FileChangeType::CREATED => Self::Added,
      lsp::FileChangeType::CHANGED => Self::Changed,
      lsp::FileChangeType::DELETED => Self::Removed,
      _ => Self::Changed, // non-exhaustable enum
    }
  }
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
  pub scope_uri: lsp::Uri,
  pub file_uri: lsp::Uri,
  #[serde(rename = "type")]
  pub typ: DenoConfigurationChangeType,
  pub configuration_type: DenoConfigurationType,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeDenoConfigurationNotificationParams {
  pub changes: Vec<DenoConfigurationChangeEvent>,
}

// TODO(nayeemrmn): This is being replaced by
// `DidRefreshDenoConfigurationTreeNotification` for Deno > v2.0.0. Remove it
// soon.
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
