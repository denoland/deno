// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json::json;
use deno_core::serde_json::{self};
use serde::Serialize;

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

const VERSION_HEADER: &str = "TAP version 14";

/// A test reporter for the Test Anything Protocol as defined at
/// https://testanything.org/tap-version-14-specification.html
pub struct TapTestReporter {
  cwd: Url,
  is_concurrent: bool,
  header: bool,
  planned: usize,
  n: usize,
  step_n: usize,
  step_results: HashMap<usize, Vec<(TestStepDescription, TestStepResult)>>,
}

#[allow(clippy::print_stdout)]
impl TapTestReporter {
  pub fn new(cwd: Url, is_concurrent: bool) -> TapTestReporter {
    TapTestReporter {
      cwd,
      is_concurrent,
      header: false,
      planned: 0,
      n: 0,
      step_n: 0,
      step_results: HashMap::new(),
    }
  }

  fn escape_description(description: &str) -> String {
    description
      .replace('\\', "\\\\")
      .replace('\n', "\\n")
      .replace('\r', "\\r")
      .replace('#', "\\#")
  }

  fn print_diagnostic(
    indent: usize,
    failure: &TestFailure,
    location: DiagnosticLocation,
    options: Option<&TestFailureFormatOptions>,
  ) {
    // Unspecified behaviour:
    // The diagnostic schema is not specified by the TAP spec,
    // but there is an example, so we use it.

    // YAML is a superset of JSON, so we can avoid a YAML dependency here.
    // This makes the output less readable though.
    let diagnostic = serde_json::to_string(&json!({
      "message": failure.format(options),
      "severity": "fail".to_string(),
      "at": location,
    }))
    .expect("failed to serialize TAP diagnostic");
    println!("{:indent$}  ---", "", indent = indent);
    println!("{:indent$}  {}", "", diagnostic, indent = indent);
    println!("{:indent$}  ...", "", indent = indent);
  }

  fn print_line(
    indent: usize,
    status: &str,
    step: usize,
    description: &str,
    directive: &str,
  ) {
    println!(
      "{:indent$}{} {} - {}{}",
      "",
      status,
      step,
      Self::escape_description(description),
      directive,
      indent = indent
    );
  }

  fn print_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    options: Option<&TestFailureFormatOptions>,
  ) {
    if self.step_n == 0 {
      println!("# Subtest: {}", desc.root_name)
    }

    let (status, directive) = match result {
      TestStepResult::Ok => ("ok", ""),
      TestStepResult::Ignored => ("ok", " # SKIP"),
      TestStepResult::Failed(_failure) => ("not ok", ""),
    };
    self.step_n += 1;
    Self::print_line(4, status, self.step_n, &desc.name, directive);

    if let TestStepResult::Failed(failure) = result {
      Self::print_diagnostic(
        4,
        failure,
        DiagnosticLocation {
          file: to_relative_path_or_remote_url(&self.cwd, &desc.origin),
          line: desc.location.line_number,
        },
        options,
      );
    }
  }
}

#[allow(clippy::print_stdout)]
impl TestReporter for TapTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}

  fn report_plan(&mut self, plan: &TestPlan) {
    if !self.header {
      println!("{}", VERSION_HEADER);
      self.header = true;
    }
    self.planned += plan.total;

    if !self.is_concurrent {
      // Unspecified behavior: Consumers tend to interpret a comment as a test suite name.
      // During concurrent execution these would not correspond to the actual test file, so skip them.
      println!(
        "# {}",
        to_relative_path_or_remote_url(&self.cwd, &plan.origin)
      )
    }
  }

  fn report_wait(&mut self, _description: &TestDescription) {
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
  }

  fn report_slow(&mut self, _description: &TestDescription, _elapsed: u64) {}
  fn report_output(&mut self, _output: &[u8]) {}

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    _elapsed: u64,
    options: Option<&TestFailureFormatOptions>,
  ) {
    if self.is_concurrent {
      let results = self.step_results.remove(&description.id);
      for (desc, result) in results.iter().flat_map(|v| v.iter()) {
        self.print_step_result(desc, result, options);
      }
    }

    if self.step_n != 0 {
      println!("    1..{}", self.step_n);
      self.step_n = 0;
    }

    let (status, directive) = match result {
      TestResult::Ok => ("ok", ""),
      TestResult::Ignored => ("ok", " # SKIP"),
      TestResult::Failed(_failure) => ("not ok", ""),
      TestResult::Cancelled => ("not ok", ""),
    };
    self.n += 1;
    Self::print_line(0, status, self.n, &description.name, directive);

    if let TestResult::Failed(failure) = result {
      Self::print_diagnostic(
        0,
        failure,
        DiagnosticLocation {
          file: to_relative_path_or_remote_url(&self.cwd, &description.origin),
          line: description.location.line_number,
        },
        options,
      );
    }
  }

  fn report_uncaught_error(&mut self, _origin: &str, _errorr: Box<JsError>) {}

  fn report_step_register(&mut self, _description: &TestStepDescription) {}

  fn report_step_wait(&mut self, _description: &TestStepDescription) {
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
  }

  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    _elapsed: u64,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
    options: Option<&TestFailureFormatOptions>,
  ) {
    if self.is_concurrent {
      // All subtests must be reported immediately before the parent test.
      // So during concurrent execution we need to defer printing the results.
      // TODO(SyrupThinker) This only outputs one level of subtests, it could support multiple.
      self
        .step_results
        .entry(desc.root_id)
        .or_default()
        .push((desc.clone(), result.clone()));
      return;
    }

    self.print_step_result(desc, result, options);
  }

  fn report_summary(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
    _options: Option<&TestFailureFormatOptions>,
  ) {
    println!("1..{}", self.planned);
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    _options: Option<&TestFailureFormatOptions>,
  ) {
    println!("Bail out! SIGINT received.");
    common::report_sigint(
      &mut std::io::stdout(),
      &self.cwd,
      tests_pending,
      tests,
      test_steps,
    );
  }

  fn report_completed(&mut self) {}

  fn flush_report(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    Ok(())
  }
}

#[derive(Serialize)]
struct DiagnosticLocation {
  file: String,
  line: u32,
}
