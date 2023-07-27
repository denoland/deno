// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use serde::Deserialize;
use super::fmt::format_test_error;

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestLocation {
  pub file_name: String,
  pub line_number: u32,
  pub column_number: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
  pub id: usize,
  pub name: String,
  pub ignore: bool,
  pub only: bool,
  pub origin: String,
  pub location: TestLocation,
}

impl TestDescription {
  pub fn static_id(&self) -> String {
    checksum::gen(&[self.location.file_name.as_bytes(), self.name.as_bytes()])
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestOutput {
  String(String),
  Bytes(Vec<u8>),
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestFailure {
  JsError(Box<JsError>),
  FailedSteps(usize),
  IncompleteSteps,
  LeakedOps(Vec<String>, bool), // Details, isOpCallTracingEnabled
  LeakedResources(Vec<String>), // Details
  // The rest are for steps only.
  Incomplete,
  OverlapsWithSanitizers(IndexSet<String>), // Long names of overlapped tests
  HasSanitizersAndOverlaps(IndexSet<String>), // Long names of overlapped tests
}

impl ToString for TestFailure {
  fn to_string(&self) -> String {
    match self {
      TestFailure::JsError(js_error) => format_test_error(js_error),
      TestFailure::FailedSteps(1) => "1 test step failed.".to_string(),
      TestFailure::FailedSteps(n) => format!("{} test steps failed.", n),
      TestFailure::IncompleteSteps => "Completed while steps were still running. Ensure all steps are awaited with `await t.step(...)`.".to_string(),
      TestFailure::Incomplete => "Didn't complete before parent. Await step with `await t.step(...)`.".to_string(),
      TestFailure::LeakedOps(details, is_op_call_tracing_enabled) => {
        let mut string = "Leaking async ops:".to_string();
        for detail in details {
          string.push_str(&format!("\n  - {}", detail));
        }
        if !is_op_call_tracing_enabled {
          string.push_str("\nTo get more details where ops were leaked, run again with --trace-ops flag.");
        }
        string
      }
      TestFailure::LeakedResources(details) => {
        let mut string = "Leaking resources:".to_string();
        for detail in details {
          string.push_str(&format!("\n  - {}", detail));
        }
        string
      }
      TestFailure::OverlapsWithSanitizers(long_names) => {
        let mut string = "Started test step while another test step with sanitizers was running:".to_string();
        for long_name in long_names {
          string.push_str(&format!("\n  * {}", long_name));
        }
        string
      }
      TestFailure::HasSanitizersAndOverlaps(long_names) => {
        let mut string = "Started test step with sanitizers while another test step was running:".to_string();
        for long_name in long_names {
          string.push_str(&format!("\n  * {}", long_name));
        }
        string
      }
    }
  }
}

impl TestFailure {
  fn format_label(&self) -> String {
    match self {
      TestFailure::Incomplete => colors::gray("INCOMPLETE").to_string(),
      _ => colors::red("FAILED").to_string(),
    }
  }

  fn format_inline_summary(&self) -> Option<String> {
    match self {
      TestFailure::FailedSteps(1) => Some("due to 1 failed step".to_string()),
      TestFailure::FailedSteps(n) => Some(format!("due to {} failed steps", n)),
      TestFailure::IncompleteSteps => {
        Some("due to incomplete steps".to_string())
      }
      _ => None,
    }
  }

  fn hide_in_summary(&self) -> bool {
    // These failure variants are hidden in summaries because they are caused
    // by child errors that will be summarized separately.
    matches!(
      self,
      TestFailure::FailedSteps(_) | TestFailure::IncompleteSteps
    )
  }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResult {
  Ok,
  Ignored,
  Failed(TestFailure),
  Cancelled,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStepDescription {
  pub id: usize,
  pub name: String,
  pub origin: String,
  pub location: TestLocation,
  pub level: usize,
  pub parent_id: usize,
  pub root_id: usize,
  pub root_name: String,
}

impl TestStepDescription {
  pub fn static_id(&self) -> String {
    checksum::gen(&[
      self.location.file_name.as_bytes(),
      &self.level.to_be_bytes(),
      self.name.as_bytes(),
    ])
  }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStepResult {
  Ok,
  Ignored,
  Failed(TestFailure),
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPlan {
  pub origin: String,
  pub total: usize,
  pub filtered_out: usize,
  pub used_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestEvent {
  Register(TestDescription),
  Plan(TestPlan),
  Wait(usize),
  Output(Vec<u8>),
  Result(usize, TestResult, u64),
  UncaughtError(String, Box<JsError>),
  StepRegister(TestStepDescription),
  StepWait(usize),
  StepResult(usize, TestStepResult, u64),
  Sigint,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSummary {
  pub total: usize,
  pub passed: usize,
  pub failed: usize,
  pub ignored: usize,
  pub passed_steps: usize,
  pub failed_steps: usize,
  pub ignored_steps: usize,
  pub filtered_out: usize,
  pub measured: usize,
  pub failures: Vec<(TestDescription, TestFailure)>,
  pub uncaught_errors: Vec<(String, Box<JsError>)>,
}
