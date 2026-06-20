// Copyright 2018-2026 the Deno authors. MIT license.

use super::*;

mod common;
mod compound;
mod dot;
mod junit;
mod pretty;
mod tap;

pub use compound::CompoundTestReporter;
pub use dot::DotTestReporter;
pub use junit::JunitTestReporter;
pub use pretty::PrettyTestReporter;
pub use tap::TapTestReporter;

pub trait TestReporter {
  fn report_register(&mut self, description: &TestDescription);
  fn report_plan(&mut self, plan: &TestPlan);
  fn report_wait(&mut self, description: &TestDescription);
  fn report_slow(&mut self, description: &TestDescription, elapsed: Duration);
  fn report_output(&mut self, output: &[u8]);
  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: Duration,
  );
  /// Called when a test attempt failed but will be retried (`retry` option).
  /// `attempt` is the zero-based index of the attempt that failed. This is
  /// informational; the test's terminal result is still delivered via
  /// [`Self::report_result`]. Defaults to a no-op.
  fn report_retry(
    &mut self,
    _description: &TestDescription,
    _attempt: u32,
    _failure: &TestFailure,
  ) {
  }
  /// Called when a test begins a fresh repetition (`repeats` option).
  /// `repetition` is the one-based index of the repetition that is starting.
  /// This lets a reporter discard the previous repetition's step results so
  /// they aren't counted more than once. Defaults to a no-op.
  fn report_repeat(
    &mut self,
    _description: &TestDescription,
    _repetition: u32,
  ) {
  }
  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>);
  fn report_step_register(&mut self, description: &TestStepDescription);
  fn report_step_wait(&mut self, description: &TestStepDescription);
  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    elapsed: Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  );
  /// Called after a test module ran with `--update-snapshots` and updated or
  /// removed snapshots. Reporters that print a final summary should
  /// accumulate these counts and include them there.
  fn report_snapshot_summary(&mut self, _summary: &TestSnapshotSummary) {}
  fn report_summary(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  );
  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  );
  /// Called when a test calls `Deno.exit()` while the exit sanitizer is
  /// disabled (`sanitizeExit: false`). The test run is being aborted and the
  /// process will exit with `exit_code`.
  fn report_exit(
    &mut self,
    exit_code: i32,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  );
  /// Called when a test isolate called `Deno.exit()` from outside any test
  /// function (top-level code or an `unload` listener). The isolate is
  /// terminated; the test runner continues with any remaining specifiers.
  fn report_isolate_exit(&mut self, origin: &str, exit_code: i32);
  fn report_completed(&mut self);
  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()>;
}
