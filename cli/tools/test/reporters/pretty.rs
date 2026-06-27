// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::IsTerminal;

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;
use crate::util::console::filter_destructive_ansi;

pub struct PrettyTestReporter {
  parallel: bool,
  echo_output: bool,
  in_new_line: bool,
  phase: &'static str,
  filter: bool,
  repl: bool,
  /// Whether `--doc` is enabled. When set, a plan reporting zero tests is
  /// suppressed so that scanning a source file with no doc tests (and no
  /// executable tests of its own) doesn't print `running 0 tests from <file>`.
  doc: bool,
  scope_test_id: Option<usize>,
  cwd: Url,
  did_have_user_output: bool,
  started_tests: bool,
  ended_tests: bool,
  child_results_buffer: HashMap<
    usize,
    IndexMap<usize, (TestStepDescription, TestStepResult, Duration)>,
  >,
  /// Ids of tests that have been retried at least once, used to count flaky
  /// tests (those that ultimately passed after a retry).
  retried_tests: HashSet<usize>,
  /// Step results buffered until the owning test produces a terminal result,
  /// so a retried attempt's steps can be discarded rather than counted.
  pending_step_tally: common::PendingStepTally,
  summary: TestSummary,
  writer: Box<dyn std::io::Write>,
  failure_format_options: TestFailureFormatOptions,
}

impl PrettyTestReporter {
  pub fn new(
    parallel: bool,
    echo_output: bool,
    filter: bool,
    repl: bool,
    doc: bool,
    cwd: Url,
    failure_format_options: TestFailureFormatOptions,
  ) -> PrettyTestReporter {
    PrettyTestReporter {
      parallel,
      echo_output,
      in_new_line: true,
      phase: "",
      filter,
      repl,
      doc,
      scope_test_id: None,
      cwd,
      did_have_user_output: false,
      started_tests: false,
      ended_tests: false,
      child_results_buffer: Default::default(),
      retried_tests: Default::default(),
      pending_step_tally: Default::default(),
      summary: TestSummary::new(),
      writer: Box::new(std::io::stdout()),
      failure_format_options,
    }
  }

  /// Drops the step state buffered for a test that is about to re-run (a retry
  /// attempt or a new repetition): the pending tally so the previous run's
  /// steps aren't counted, and any unflushed step output so it doesn't leak
  /// into the next run.
  fn discard_buffered_steps(&mut self, root_id: usize) {
    common::discard_step_results(&mut self.pending_step_tally, root_id);
    self
      .child_results_buffer
      .retain(|_, steps| !steps.values().any(|(d, _, _)| d.root_id == root_id));
  }

  fn force_report_wait(&mut self, description: &TestDescription) {
    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    if self.parallel {
      write!(
        &mut self.writer,
        "{}",
        colors::gray(format!(
          "{} => ",
          to_relative_path_or_remote_url(&self.cwd, &description.origin)
        ))
      )
      .ok();
    }
    write!(&mut self.writer, "{} ...", description.name).ok();
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().ok();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_wait(&mut self, description: &TestStepDescription) {
    self.write_output_end();
    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    write!(
      &mut self.writer,
      "{}{} ...",
      "  ".repeat(description.level),
      description.name
    )
    .ok();
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().ok();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    elapsed: Duration,
  ) {
    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_step_wait(description);
    } else if std::io::stdout().is_terminal() {
      write!(
        &mut self.writer,
        "\r{}{} ...",
        "  ".repeat(description.level),
        description.name
      )
      .ok();
    }

    if !self.parallel {
      let child_results = self
        .child_results_buffer
        .remove(&description.id)
        .unwrap_or_default();
      for (desc, result, elapsed) in child_results.values() {
        self.force_report_step_result(desc, result, *elapsed);
      }
      if !child_results.is_empty() {
        self.force_report_step_wait(description);
      }
    }

    let status = match &result {
      TestStepResult::Ok => colors::green("ok").to_string(),
      TestStepResult::Ignored => colors::yellow("ignored").to_string(),
      TestStepResult::Failed(failure) => failure.format_label(),
    };
    write!(&mut self.writer, " {status}").ok();
    if let TestStepResult::Failed(failure) = result
      && let Some(inline_summary) = failure.format_inline_summary()
    {
      write!(&mut self.writer, " ({})", inline_summary).ok();
    }
    if !matches!(result, TestStepResult::Failed(TestFailure::Incomplete)) {
      write!(
        &mut self.writer,
        " {}",
        colors::gray(format!("({})", display::human_elapsed(elapsed)))
      )
      .ok();
    }
    writeln!(&mut self.writer).ok();
    self.in_new_line = true;
    if self.parallel {
      self.scope_test_id = None;
    } else {
      self.scope_test_id = Some(description.parent_id);
    }
    self
      .child_results_buffer
      .entry(description.parent_id)
      .or_default()
      .shift_remove(&description.id);
  }

  fn write_output_end(&mut self) {
    if self.did_have_user_output {
      writeln!(
        &mut self.writer,
        "{}",
        colors::gray(format!("----- {}output end -----", self.phase))
      )
      .ok();
      self.in_new_line = true;
      self.did_have_user_output = false;
    }
  }
}

impl TestReporter for PrettyTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}
  fn report_plan(&mut self, plan: &TestPlan) {
    self.write_output_end();
    self.summary.total += plan.total;
    self.summary.filtered_out += plan.filtered_out;
    if self.repl {
      return;
    }
    if self.parallel || (self.filter && plan.total == 0) {
      return;
    }
    // With `--doc`, every source file is scanned for doc tests and the file
    // itself is also run as a module. Files that have no doc tests and no
    // executable tests of their own would otherwise print a noisy
    // `running 0 tests from <file>` line, so suppress it.
    if self.doc && plan.total == 0 {
      return;
    }
    let inflection = if plan.total == 1 { "test" } else { "tests" };
    writeln!(
      &mut self.writer,
      "{}",
      colors::gray(format!(
        "running {} {} from {}",
        plan.total,
        inflection,
        to_relative_path_or_remote_url(&self.cwd, &plan.origin)
      ))
    )
    .ok();
    self.in_new_line = true;
  }

  fn report_wait(&mut self, description: &TestDescription) {
    self.write_output_end();
    if !self.parallel {
      self.force_report_wait(description);
    }
    self.started_tests = true;
  }

  fn report_slow(&mut self, description: &TestDescription, elapsed: Duration) {
    writeln!(
      &mut self.writer,
      "{}",
      colors::yellow_bold(format!(
        "'{}' has been running for over {}",
        description.name,
        colors::gray(format!("({})", display::human_elapsed(elapsed))),
      ))
    )
    .ok();
  }
  fn report_output(&mut self, output: &[u8]) {
    if !self.echo_output {
      return;
    }

    if !self.did_have_user_output {
      self.did_have_user_output = true;
      if !self.in_new_line {
        writeln!(&mut self.writer).ok();
      }
      self.phase = if !self.started_tests {
        "pre-test "
      } else if self.ended_tests {
        "post-test "
      } else {
        ""
      };
      writeln!(
        &mut self.writer,
        "{}",
        colors::gray(format!("------- {}output -------", self.phase))
      )
      .ok();
      self.in_new_line = true;
    }

    // output everything to stdout in order to prevent
    // stdout and stderr racing
    let filtered = filter_destructive_ansi(output);
    std::io::stdout().write_all(&filtered).ok();
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: Duration,
  ) {
    // Commit step results from the final attempt now that the test's fate is
    // known (results from any retried attempt were already discarded).
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

    if self.parallel {
      self.force_report_wait(description);
    }

    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_wait(description);
    } else if std::io::stdout().is_terminal() {
      // We believe the cursor is right after "test name ...", but external
      // output (e.g. from native addons writing directly to fd 1) may have
      // moved it. Use \r to return to column 0 and re-write the test name
      // so the result line is always intact. For normal tests this harmlessly
      // overwrites the same bytes. Only do this on a real terminal — on pipes
      // \r is a literal byte that would produce doubled output.
      write!(&mut self.writer, "\r").ok();
      if self.parallel {
        write!(
          &mut self.writer,
          "{}",
          colors::gray(format!(
            "{} => ",
            to_relative_path_or_remote_url(&self.cwd, &description.origin)
          ))
        )
        .ok();
      }
      write!(&mut self.writer, "{} ...", description.name).ok();
    }

    let status = match result {
      TestResult::Ok => colors::green("ok").to_string(),
      TestResult::Ignored => colors::yellow("ignored").to_string(),
      TestResult::Failed(failure) => failure.format_label(),
      TestResult::Cancelled => colors::gray("cancelled").to_string(),
    };
    write!(&mut self.writer, " {status}").ok();
    if let TestResult::Failed(failure) = result
      && let Some(inline_summary) = failure.format_inline_summary()
    {
      write!(&mut self.writer, " ({})", inline_summary).ok();
    }
    writeln!(
      &mut self.writer,
      " {}",
      colors::gray(format!("({})", display::human_elapsed(elapsed)))
    )
    .ok();
    self.in_new_line = true;
    self.scope_test_id = None;
  }

  fn report_retry(
    &mut self,
    description: &TestDescription,
    attempt: u32,
    failure: &TestFailure,
  ) {
    self.retried_tests.insert(description.id);
    // Drop the failed attempt's step results so they don't count.
    self.discard_buffered_steps(description.id);

    if self.repl {
      return;
    }

    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_wait(description);
    }

    writeln!(
      &mut self.writer,
      " {} {}",
      colors::yellow(format!("retrying (attempt {} failed)", attempt + 1)),
      colors::gray(format!("({})", failure.overview())),
    )
    .ok();
    self.in_new_line = true;
    self.scope_test_id = None;
  }

  fn report_repeat(&mut self, description: &TestDescription, _repetition: u32) {
    // Drop the previous repetition's step results so each step is counted once,
    // not once per repetition.
    self.discard_buffered_steps(description.id);
  }

  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
    self.summary.failed += 1;
    self
      .summary
      .uncaught_errors
      .push((origin.to_string(), error));

    if !self.in_new_line {
      writeln!(&mut self.writer).ok();
    }
    writeln!(
      &mut self.writer,
      "Uncaught error from {} {}",
      to_relative_path_or_remote_url(&self.cwd, origin),
      colors::red("FAILED")
    )
    .ok();
    self.in_new_line = true;
    self.did_have_user_output = false;
  }

  fn report_step_register(&mut self, _description: &TestStepDescription) {}

  fn report_step_wait(&mut self, description: &TestStepDescription) {
    if !self.parallel && self.scope_test_id == Some(description.parent_id) {
      self.force_report_step_wait(description);
    }
  }

  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    elapsed: Duration,
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

    if self.parallel {
      self.write_output_end();
      write!(
        &mut self.writer,
        "{} {} ...",
        colors::gray(format!(
          "{} =>",
          to_relative_path_or_remote_url(&self.cwd, &desc.origin)
        )),
        common::format_test_step_ancestry(desc, tests, test_steps)
      )
      .ok();
      self.in_new_line = false;
      self.scope_test_id = Some(desc.id);
      self.force_report_step_result(desc, result, elapsed);
    } else {
      let sibling_results =
        self.child_results_buffer.entry(desc.parent_id).or_default();
      if self.scope_test_id == Some(desc.id)
        || self.scope_test_id == Some(desc.parent_id)
      {
        let sibling_results = std::mem::take(sibling_results);
        self.force_report_step_result(desc, result, elapsed);
        // Flush buffered sibling results.
        for (desc, result, elapsed) in sibling_results.values() {
          self.force_report_step_result(desc, result, *elapsed);
        }
      } else {
        sibling_results
          .insert(desc.id, (desc.clone(), result.clone(), elapsed));
      }
    }
  }

  fn report_snapshot_summary(&mut self, summary: &TestSnapshotSummary) {
    self.summary.snapshots_updated += summary.updated;
    self
      .summary
      .snapshots_removed
      .extend(summary.removed.iter().cloned());
  }

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    self.write_output_end();
    common::report_summary(
      &mut self.writer,
      &self.cwd,
      &self.summary,
      elapsed,
      &self.failure_format_options,
    );
    if !self.repl {
      writeln!(&mut self.writer).ok();
    }
    self.in_new_line = true;
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_sigint(
      &mut self.writer,
      &self.cwd,
      tests_pending,
      tests,
      test_steps,
    );
    self.in_new_line = true;
  }

  fn report_exit(
    &mut self,
    exit_code: i32,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    self.write_output_end();
    common::report_exit(
      &mut self.writer,
      &self.cwd,
      exit_code,
      tests_pending,
      tests,
      test_steps,
    );
    self.in_new_line = true;
  }

  fn report_isolate_exit(&mut self, origin: &str, exit_code: i32) {
    self.write_output_end();
    common::report_isolate_exit(&mut self.writer, &self.cwd, origin, exit_code);
    if exit_code != 0 {
      self.summary.failed += 1;
    }
    self.in_new_line = true;
  }

  fn report_completed(&mut self) {
    self.write_output_end();
    self.ended_tests = true;
  }

  fn flush_report(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    self.writer.flush().ok();
    Ok(())
  }
}
