// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct DotTestReporter {
  n: usize,
  width: usize,
  cwd: Url,
  summary: TestSummary,
}

#[allow(clippy::print_stdout)]
impl DotTestReporter {
  pub fn new(cwd: Url) -> DotTestReporter {
    let console_width = if let Some(size) = crate::util::console::console_size()
    {
      size.cols as usize
    } else {
      0
    };
    let console_width = (console_width as f32 * 0.8) as usize;
    DotTestReporter {
      n: 0,
      width: console_width,
      cwd,
      summary: TestSummary::new(),
    }
  }

  fn print_status(&mut self, status: String) {
    // Non-TTY console prints every result on a separate line.
    if self.width == 0 {
      println!("{}", status);
      return;
    }

    if self.n != 0 && self.n % self.width == 0 {
      println!();
    }
    self.n += 1;

    print!("{}", status);
  }

  fn print_test_step_result(&mut self, result: &TestStepResult) {
    let status = match result {
      TestStepResult::Ok => fmt_ok(),
      TestStepResult::Ignored => fmt_ignored(),
      TestStepResult::Failed(_failure) => fmt_failed(),
    };
    self.print_status(status);
  }

  fn print_test_result(&mut self, result: &TestResult) {
    let status = match result {
      TestResult::Ok => fmt_ok(),
      TestResult::Ignored => fmt_ignored(),
      TestResult::Failed(_failure) => fmt_failed(),
      TestResult::Cancelled => fmt_cancelled(),
    };

    self.print_status(status);
  }
}

fn fmt_ok() -> String {
  colors::gray(".").to_string()
}

fn fmt_ignored() -> String {
  colors::cyan(",").to_string()
}

fn fmt_failed() -> String {
  colors::red_bold("!").to_string()
}

fn fmt_cancelled() -> String {
  colors::gray("!").to_string()
}

#[allow(clippy::print_stdout)]
impl TestReporter for DotTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}

  fn report_plan(&mut self, plan: &TestPlan) {
    self.summary.total += plan.total;
    self.summary.filtered_out += plan.filtered_out;
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
    _options: Option<&TestFailureFormatOptions>,
  ) {
    match &result {
      TestResult::Ok => {
        self.summary.passed += 1;
      }
      TestResult::Ignored => {
        self.summary.ignored += 1;
      }
      TestResult::Failed(failure) => {
        self.summary.failed += 1;
        self
          .summary
          .failures
          .push((description.into(), failure.clone()));
      }
      TestResult::Cancelled => {
        self.summary.failed += 1;
      }
    }

    self.print_test_result(result);
  }

  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
    self.summary.failed += 1;
    self
      .summary
      .uncaught_errors
      .push((origin.to_string(), error));

    println!(
      "Uncaught error from {} {}",
      to_relative_path_or_remote_url(&self.cwd, origin),
      colors::red("FAILED")
    );
  }

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
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    _options: Option<&TestFailureFormatOptions>,
  ) {
    match &result {
      TestStepResult::Ok => {
        self.summary.passed_steps += 1;
      }
      TestStepResult::Ignored => {
        self.summary.ignored_steps += 1;
      }
      TestStepResult::Failed(failure) => {
        self.summary.failed_steps += 1;
        self.summary.failures.push((
          TestFailureDescription {
            id: desc.id,
            name: common::format_test_step_ancestry(desc, tests, test_steps),
            origin: desc.origin.clone(),
            location: desc.location.clone(),
          },
          failure.clone(),
        ))
      }
    }

    self.print_test_step_result(result);
  }

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
    options: Option<&TestFailureFormatOptions>,
  ) {
    common::report_summary(
      &mut std::io::stdout(),
      &self.cwd,
      &self.summary,
      elapsed,
      options,
    );
    println!();
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    _options: Option<&TestFailureFormatOptions>,
  ) {
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
