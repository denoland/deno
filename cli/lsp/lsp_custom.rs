// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use lspower::lsp;

pub const CACHE_REQUEST: &str = "deno/cache";
pub const PERFORMANCE_REQUEST: &str = "deno/performance";
pub const TASK_REQUEST: &str = "deno/task";
pub const RELOAD_IMPORT_REGISTRIES_REQUEST: &str =
  "deno/reloadImportRegistries";
pub const TEST_RUN_CANCEL_REQUEST: &str = "deno/testRunCancel";
pub const TEST_RUN_REQUEST: &str = "deno/testRun";
pub const VIRTUAL_TEXT_DOCUMENT: &str = "deno/virtualTextDocument";

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
pub struct EnqueuedTestModule {
  pub text_document: lsp::TextDocumentIdentifier,
  pub ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestData {
  /// The unique ID of the test
  pub id: String,
  /// The human readable test to display for the test.
  pub label: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub steps: Option<Vec<TestData>>,
  /// The range where the test is located.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub range: Option<lsp::Range>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModuleNotificationParams {
  /// The text document that the notification relates to.
  pub text_document: lsp::TextDocumentIdentifier,
  /// The human readable text to display for the test module.
  pub label: String,
  /// The tests identified in the module.
  pub tests: Vec<TestData>,
}

pub enum TestModuleNotification {}

impl lsp::notification::Notification for TestModuleNotification {
  type Params = TestModuleNotificationParams;

  const METHOD: &'static str = "deno/testModule";
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModuleDeleteNotificationParams {
  /// The text document that the notification relates to.
  pub text_document: lsp::TextDocumentIdentifier,
}

pub enum TestModuleDeleteNotification {}

impl lsp::notification::Notification for TestModuleDeleteNotification {
  type Params = TestModuleDeleteNotificationParams;

  const METHOD: &'static str = "deno/testModuleDelete";
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestRunKind {
  // The run profile is just to execute the tests
  Run,
  // The tests should be run and debugged, currently not implemented
  Debug,
  // The tests should be run, collecting and reporting coverage information,
  // currently not implemented
  Coverage,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunRequestParams {
  pub id: u32,
  pub kind: TestRunKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub exclude: Option<Vec<TestIdentifier>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include: Option<Vec<TestIdentifier>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunCancelParams {
  pub id: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunProgressParams {
  pub id: u32,
  pub message: TestRunProgressMessage,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestIdentifier {
  /// The module identifier which contains the test.
  pub text_document: lsp::TextDocumentIdentifier,
  /// An optional string identifying the individual test. If not present, then
  /// it identifies all the tests associated with the module.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  /// An optional structure identifying a step of the test. If not present, then
  /// no step is identified.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub step_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum TestRunProgressMessage {
  Enqueued {
    test: TestIdentifier,
  },
  Started {
    test: TestIdentifier,
  },
  Skipped {
    test: TestIdentifier,
  },
  Failed {
    test: TestIdentifier,
    messages: Vec<TestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
  },
  Errored {
    test: TestIdentifier,
    messages: Vec<TestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
  },
  Passed {
    test: TestIdentifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
  },
  Output {
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<TestIdentifier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<lsp::Location>,
  },
  End,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMessage {
  pub message: lsp::MarkupContent,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub expected_output: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub actual_output: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub location: Option<lsp::Location>,
}

pub enum TestRunProgressNotification {}

impl lsp::notification::Notification for TestRunProgressNotification {
  type Params = TestRunProgressParams;

  const METHOD: &'static str = "deno/testRunProgress";
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTextDocumentParams {
  pub text_document: lsp::TextDocumentIdentifier,
}
