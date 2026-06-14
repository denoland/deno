// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::VecDeque;
use std::io::Write;
use std::time::SystemTime;

use console_static_text::ansi::strip_ansi_codes;
use deno_core::serde_json;
use serde::Serialize;

use super::fmt::to_relative_path_or_remote_url;
use super::*;

/// A test reporter that emits a single JSON document modelled on the output of
/// Jest's (and Vitest's) `json` reporter, so existing tooling that consumes
/// that shape can be reused. The document is written to stdout once the run
/// completes.
///
/// Each Deno test and test step maps to an `assertionResult`; tests are grouped
/// into `testResults` (one entry per file, i.e. a Jest "test suite"). Retried
/// attempts of a test that ultimately passes are not surfaced; only the final
/// outcome is reported.
pub struct JsonTestReporter {
  cwd: Url,
  failure_format_options: TestFailureFormatOptions,
  start_time_ms: u64,
  // Collected results for tests and committed steps, keyed by id.
  results: IndexMap<usize, CollectedResult>,
  // Step results buffered per root test id until that test produces a terminal
  // result, so a retried attempt's steps can be discarded rather than emitted.
  pending_steps: IndexMap<usize, Vec<(usize, CollectedResult)>>,
}

struct CollectedResult {
  status: ResultStatus,
  duration: u64,
  failure_messages: Vec<String>,
}

#[derive(Clone, Copy)]
enum ResultStatus {
  Passed,
  Failed,
  Skipped,
}

impl ResultStatus {
  fn as_str(self) -> &'static str {
    match self {
      ResultStatus::Passed => "passed",
      ResultStatus::Failed => "failed",
      ResultStatus::Skipped => "skipped",
    }
  }
}

impl JsonTestReporter {
  pub fn new(
    cwd: Url,
    failure_format_options: TestFailureFormatOptions,
  ) -> Self {
    let start_time_ms = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .map(|d| d.as_millis() as u64)
      .unwrap_or(0);
    Self {
      cwd,
      failure_format_options,
      start_time_ms,
      results: IndexMap::new(),
      pending_steps: IndexMap::new(),
    }
  }

  fn collect_failure(&self, failure: &TestFailure) -> Vec<String> {
    let message = failure.format(&self.failure_format_options).into_owned();
    vec![strip_ansi_codes(&message).into_owned()]
  }
}

impl TestReporter for JsonTestReporter {
  fn report_register(&mut self, description: &TestDescription) {
    // Pre-register as skipped so tests that never run (e.g. cancelled) still
    // appear; overwritten once a result arrives.
    self.results.insert(
      description.id,
      CollectedResult {
        status: ResultStatus::Skipped,
        duration: 0,
        failure_messages: vec![],
      },
    );
  }

  fn report_plan(&mut self, _plan: &TestPlan) {}
  fn report_slow(&mut self, _description: &TestDescription, _elapsed: u64) {}
  fn report_wait(&mut self, _description: &TestDescription) {}
  fn report_output(&mut self, _output: &[u8]) {}

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    // Commit the final attempt's buffered steps now that the test is done.
    if let Some(steps) = self.pending_steps.shift_remove(&description.id) {
      for (id, collected) in steps {
        self.results.insert(id, collected);
      }
    }

    let collected = match result {
      TestResult::Ok => CollectedResult {
        status: ResultStatus::Passed,
        duration: elapsed,
        failure_messages: vec![],
      },
      TestResult::Ignored => CollectedResult {
        status: ResultStatus::Skipped,
        duration: elapsed,
        failure_messages: vec![],
      },
      TestResult::Failed(failure) => CollectedResult {
        status: ResultStatus::Failed,
        duration: elapsed,
        failure_messages: self.collect_failure(failure),
      },
      TestResult::Cancelled => CollectedResult {
        status: ResultStatus::Failed,
        duration: elapsed,
        failure_messages: vec!["Cancelled".to_string()],
      },
    };
    self.results.insert(description.id, collected);
  }

  fn report_retry(
    &mut self,
    description: &TestDescription,
    _attempt: u32,
    _failure: &TestFailure,
  ) {
    // Drop the failed attempt's buffered steps so they aren't reported.
    self.pending_steps.shift_remove(&description.id);
  }

  fn report_uncaught_error(&mut self, _origin: &str, _error: Box<JsError>) {}

  fn report_step_register(&mut self, _description: &TestStepDescription) {}
  fn report_step_wait(&mut self, _description: &TestStepDescription) {}

  fn report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    let collected = match result {
      TestStepResult::Ok => CollectedResult {
        status: ResultStatus::Passed,
        duration: elapsed,
        failure_messages: vec![],
      },
      TestStepResult::Ignored => CollectedResult {
        status: ResultStatus::Skipped,
        duration: elapsed,
        failure_messages: vec![],
      },
      TestStepResult::Failed(failure) => CollectedResult {
        status: ResultStatus::Failed,
        duration: elapsed,
        failure_messages: self.collect_failure(failure),
      },
    };
    self
      .pending_steps
      .entry(description.root_id)
      .or_default()
      .push((description.id, collected));
  }

  fn report_summary(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    for id in tests_pending {
      if let Some(description) = tests.get(id) {
        self.report_result(description, &TestResult::Cancelled, 0);
      }
    }
  }

  fn report_exit(
    &mut self,
    _exit_code: i32,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    for id in tests_pending {
      if let Some(description) = tests.get(id) {
        self.report_result(description, &TestResult::Cancelled, 0);
      }
    }
  }

  fn report_isolate_exit(&mut self, _origin: &str, _exit_code: i32) {}
  fn report_completed(&mut self) {}

  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    let mut suites: IndexMap<String, JsonSuiteResult> = IndexMap::new();

    for (id, collected) in &self.results {
      let (location, ancestor_titles, title) = resolve(*id, tests, test_steps);
      let Some(location) = location else {
        continue;
      };
      let file = to_relative_path_or_remote_url(&self.cwd, &location.file_name);
      let full_name = ancestor_titles
        .iter()
        .cloned()
        .chain(std::iter::once(title.clone()))
        .collect::<Vec<_>>()
        .join(" ");

      let assertion = JsonAssertionResult {
        ancestor_titles,
        full_name,
        title,
        status: collected.status.as_str(),
        duration: collected.duration,
        failure_messages: collected.failure_messages.clone(),
        location: JsonLocation {
          line: location.line_number,
          column: location.column_number,
        },
      };

      suites
        .entry(file.clone())
        .or_insert_with(|| JsonSuiteResult::new(file))
        .assertion_results
        .push(assertion);
    }

    let mut report = JsonReport::new(self.start_time_ms, *elapsed);
    for suite in suites.into_values() {
      report.add_suite(suite);
    }

    let mut writer = std::io::stdout();
    serde_json::to_writer_pretty(&mut writer, &report)?;
    writeln!(writer)?;
    Ok(())
  }
}

/// Resolves the location, ancestor titles, and title for a test or step id.
/// Returns `None` location for unknown ids (which are skipped).
fn resolve(
  id: usize,
  tests: &IndexMap<usize, TestDescription>,
  test_steps: &IndexMap<usize, TestStepDescription>,
) -> (Option<TestLocation>, Vec<String>, String) {
  if let Some(test) = tests.get(&id) {
    return (Some(test.location.clone()), vec![], test.name.clone());
  }
  let Some(step) = test_steps.get(&id) else {
    return (None, vec![], String::new());
  };

  let mut ancestors = VecDeque::new();
  let mut parent = Some(step.parent_id);
  while let Some(pid) = parent {
    if let Some(parent_step) = test_steps.get(&pid) {
      ancestors.push_front(parent_step.name.clone());
      parent = Some(parent_step.parent_id);
    } else if let Some(parent_test) = tests.get(&pid) {
      ancestors.push_front(parent_test.name.clone());
      parent = None;
    } else {
      parent = None;
    }
  }

  (
    Some(step.location.clone()),
    ancestors.into(),
    step.name.clone(),
  )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport {
  num_total_test_suites: usize,
  num_passed_test_suites: usize,
  num_failed_test_suites: usize,
  num_pending_test_suites: usize,
  num_total_tests: usize,
  num_passed_tests: usize,
  num_failed_tests: usize,
  num_pending_tests: usize,
  num_todo_tests: usize,
  start_time: u64,
  success: bool,
  was_interrupted: bool,
  test_results: Vec<JsonSuiteResult>,
  // Total run duration, folded into each suite's end time. Not serialized.
  #[serde(skip)]
  elapsed_ms: u64,
}

impl JsonReport {
  fn new(start_time: u64, elapsed: Duration) -> Self {
    Self {
      num_total_test_suites: 0,
      num_passed_test_suites: 0,
      num_failed_test_suites: 0,
      num_pending_test_suites: 0,
      num_total_tests: 0,
      num_passed_tests: 0,
      num_failed_tests: 0,
      num_pending_tests: 0,
      num_todo_tests: 0,
      start_time,
      success: true,
      was_interrupted: false,
      test_results: Vec::new(),
      elapsed_ms: elapsed.as_millis() as u64,
    }
  }

  fn add_suite(&mut self, mut suite: JsonSuiteResult) {
    self.num_total_test_suites += 1;
    suite.start_time = self.start_time;
    suite.end_time = self.start_time + self.elapsed_ms;

    let mut suite_failed = false;
    let mut failure_messages = Vec::new();
    for assertion in &suite.assertion_results {
      self.num_total_tests += 1;
      match assertion.status {
        "passed" => self.num_passed_tests += 1,
        "failed" => {
          self.num_failed_tests += 1;
          suite_failed = true;
          failure_messages.extend(assertion.failure_messages.iter().cloned());
        }
        _ => self.num_pending_tests += 1,
      }
    }

    if suite_failed {
      self.num_failed_test_suites += 1;
      self.success = false;
      suite.status = "failed";
      suite.message = failure_messages.join("\n");
    } else {
      self.num_passed_test_suites += 1;
    }

    self.test_results.push(suite);
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSuiteResult {
  name: String,
  status: &'static str,
  start_time: u64,
  end_time: u64,
  message: String,
  assertion_results: Vec<JsonAssertionResult>,
}

impl JsonSuiteResult {
  fn new(name: String) -> Self {
    Self {
      name,
      status: "passed",
      start_time: 0,
      end_time: 0,
      message: String::new(),
      assertion_results: Vec::new(),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonAssertionResult {
  ancestor_titles: Vec<String>,
  full_name: String,
  title: String,
  status: &'static str,
  duration: u64,
  failure_messages: Vec<String>,
  location: JsonLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonLocation {
  line: u32,
  column: u32,
}
