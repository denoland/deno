// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use tower_lsp::lsp_types as lsp;

pub const TEST_RUN_CANCEL_REQUEST: &str = "deno/testRunCancel";
pub const TEST_RUN_REQUEST: &str = "deno/testRun";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueuedTestModule {
  pub text_document: lsp::TextDocumentIdentifier,
  pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestData {
  /// The unique ID of the test
  pub id: String,
  /// The human readable test to display for the test.
  pub label: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[serde(default)]
  pub steps: Vec<TestData>,
  /// The range where the test is located.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub range: Option<lsp::Range>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestModuleNotificationKind {
  /// The test module notification represents an insertion of tests, not
  /// replacement of the test children.
  Insert,
  /// The test module notification represents a replacement of any tests within
  /// the test module.
  Replace,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModuleNotificationParams {
  /// The text document that the notification relates to.
  pub text_document: lsp::TextDocumentIdentifier,
  /// Indicates what kind of notification this represents.
  pub kind: TestModuleNotificationKind,
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
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[serde(default)]
  pub exclude: Vec<TestIdentifier>,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
