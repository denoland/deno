// Copyright 2018-2026 the Deno authors. MIT license.

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct DotTestReporter {
  n: usize,
  width: usize,
  cwd: Url,
  retried_tests: HashSet<usize>,
  pending_step_tally: common::PendingStepTally,
  summary: TestSummary,
  failure_format_options: TestFailureFormatOptions,
}

#[allow(clippy::print_stdout, reason = "test reporter")]
impl DotTestReporter {
  pub fn new(
    cwd: Url,
    failure_format_options: TestFailureFormatOptions,
  ) -> DotTestReporter {
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
      retried_tests: Default::default(),
      pending_step_tally: Default::default(),
      summary: TestSummary::new(),
      failure_format_options,
    }
  }

  fn print_status(&mut self, status: String) {
    // Non-TTY console prints every result on a separate line.
    if self.width == 0 {
      println!("{}", status);
      return;
    }

    if self.n != 0 && self.n.is_multiple_of(self.width) {
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

#[allow(clippy::print_stdout, reason = "test reporter")]
impl TestReporter for DotTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}

  fn report_plan(&mut self, plan: &TestPlan) {
    self.summary.total += plan.total;
    self.summary.filtered_out += plan.filtered_out;
  }

  fn report_wait(&mut self, _description: &TestDescription) {
    // flush for faster feedback when line buffered
    std::io::stdout().flush().ok();
  }

  fn report_slow(&mut self, _description: &TestDescription, _elapsed: u64) {}
  fn report_output(&mut self, _output: &[u8]) {}

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    _elapsed: u64,
  ) {
    common::commit_step_results(
      &mut self.pending_step_tally,
      &mut self.summary,
      description.id,
    );

    match &result {
      TestResult::Ok => {
        self.summary.passed += 1;
        if self.retried_tests.contains(&description.id) {
          self.summary.flaky += 1;
        }
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

  fn report_retry(
    &mut self,
    description: &TestDescription,
    _attempt: u32,
    _failure: &TestFailure,
  ) {
    self.retried_tests.insert(description.id);
    common::discard_step_results(&mut self.pending_step_tally, description.id);
  }

  fn report_repeat(&mut self, description: &TestDescription, _repetition: u32) {
    // Drop the previous repetition's step results so each step is counted once,
    // not once per repetition.
    common::discard_step_results(&mut self.pending_step_tally, description.id);
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
    std::io::stdout().flush().ok();
  }

  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    _elapsed: u64,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::record_step_result(
      &mut self.pending_step_tally,
      desc,
      result,
      tests,
      test_steps,
    );

    self.print_test_step_result(result);
  }

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_summary(
      &mut std::io::stdout(),
      &self.cwd,
      &self.summary,
      elapsed,
      &self.failure_format_options,
    );
    println!();
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_sigint(
      &mut std::io::stdout(),
      &self.cwd,
      tests_pending,
      tests,
      test_steps,
    );
  }

  fn report_exit(
    &mut self,
    exit_code: i32,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_exit(
      &mut std::io::stdout(),
      &self.cwd,
      exit_code,
      tests_pending,
      tests,
      test_steps,
    );
  }

  fn report_isolate_exit(&mut self, origin: &str, exit_code: i32) {
    common::report_isolate_exit(
      &mut std::io::stdout(),
      &self.cwd,
      origin,
      exit_code,
    );
    if exit_code != 0 {
      self.summary.failed += 1;
    }
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
