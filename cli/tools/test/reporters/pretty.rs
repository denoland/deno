// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::common;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct PrettyTestReporter {
  parallel: bool,
  echo_output: bool,
  in_new_line: bool,
  filter: bool,
  scope_test_id: Option<usize>,
  cwd: Url,
  did_have_user_output: bool,
  started_tests: bool,
  child_results_buffer:
    HashMap<usize, IndexMap<usize, (TestStepDescription, TestStepResult, u64)>>,
  summary: TestSummary,
}

impl PrettyTestReporter {
  pub fn new(
    parallel: bool,
    echo_output: bool,
    filter: bool,
  ) -> PrettyTestReporter {
    PrettyTestReporter {
      parallel,
      echo_output,
      in_new_line: true,
      filter,
      scope_test_id: None,
      cwd: Url::from_directory_path(std::env::current_dir().unwrap()).unwrap(),
      did_have_user_output: false,
      started_tests: false,
      child_results_buffer: Default::default(),
      summary: TestSummary::new(),
    }
  }

  fn force_report_wait(&mut self, description: &TestDescription) {
    if !self.in_new_line {
      println!();
    }
    if self.parallel {
      print!(
        "{}",
        colors::gray(format!(
          "{} => ",
          to_relative_path_or_remote_url(&self.cwd, &description.origin)
        ))
      );
    }
    print!("{} ...", description.name);
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_wait(&mut self, description: &TestStepDescription) {
    self.write_output_end();
    if !self.in_new_line {
      println!();
    }
    print!("{}{} ...", "  ".repeat(description.level), description.name);
    self.in_new_line = false;
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
    self.scope_test_id = Some(description.id);
  }

  fn force_report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
  ) {
    self.write_output_end();
    if self.in_new_line || self.scope_test_id != Some(description.id) {
      self.force_report_step_wait(description);
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
    print!(" {}", status);
    if let TestStepResult::Failed(failure) = result {
      if let Some(inline_summary) = failure.format_inline_summary() {
        print!(" ({})", inline_summary)
      }
    }
    if !matches!(result, TestStepResult::Failed(TestFailure::Incomplete)) {
      print!(
        " {}",
        colors::gray(format!("({})", display::human_elapsed(elapsed.into())))
      );
    }
    println!();
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
      .remove(&description.id);
  }

  fn write_output_end(&mut self) {
    if self.did_have_user_output {
      println!("{}", colors::gray("----- output end -----"));
      self.in_new_line = true;
      self.did_have_user_output = false;
    }
  }
}

impl TestReporter for PrettyTestReporter {
  fn report_register(&mut self, _description: &TestDescription) {}
  fn report_plan(&mut self, plan: &TestPlan) {
    self.summary.total += plan.total;
    self.summary.filtered_out += plan.filtered_out;
    if self.parallel || (self.filter && plan.total == 0) {
      return;
    }
    let inflection = if plan.total == 1 { "test" } else { "tests" };
    println!(
      "{}",
      colors::gray(format!(
        "running {} {} from {}",
        plan.total,
        inflection,
        to_relative_path_or_remote_url(&self.cwd, &plan.origin)
      ))
    );
    self.in_new_line = true;
  }

  fn report_wait(&mut self, description: &TestDescription) {
    if !self.parallel {
      self.force_report_wait(description);
    }
    self.started_tests = true;
  }

  fn report_output(&mut self, output: &[u8]) {
    if !self.echo_output {
      return;
    }

    if !self.did_have_user_output && self.started_tests {
      self.did_have_user_output = true;
      if !self.in_new_line {
        println!();
      }
      println!("{}", colors::gray("------- output -------"));
      self.in_new_line = true;
    }

    // output everything to stdout in order to prevent
    // stdout and stderr racing
    std::io::stdout().write_all(output).unwrap();
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
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
          .push((description.clone(), failure.clone()));
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
    }

    let status = match result {
      TestResult::Ok => colors::green("ok").to_string(),
      TestResult::Ignored => colors::yellow("ignored").to_string(),
      TestResult::Failed(failure) => failure.format_label(),
      TestResult::Cancelled => colors::gray("cancelled").to_string(),
    };
    print!(" {}", status);
    if let TestResult::Failed(failure) = result {
      if let Some(inline_summary) = failure.format_inline_summary() {
        print!(" ({})", inline_summary)
      }
    }
    println!(
      " {}",
      colors::gray(format!("({})", display::human_elapsed(elapsed.into())))
    );
    self.in_new_line = true;
    self.scope_test_id = None;
  }

  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
    self.summary.failed += 1;
    self
      .summary
      .uncaught_errors
      .push((origin.to_string(), error));

    if !self.in_new_line {
      println!();
    }
    println!(
      "Uncaught error from {} {}",
      to_relative_path_or_remote_url(&self.cwd, origin),
      colors::red("FAILED")
    );
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
    elapsed: u64,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
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
          TestDescription {
            id: desc.id,
            name: common::format_test_step_ancestry(desc, tests, test_steps),
            ignore: false,
            only: false,
            origin: desc.origin.clone(),
            location: desc.location.clone(),
          },
          failure.clone(),
        ))
      }
    }

    if self.parallel {
      self.write_output_end();
      print!(
        "{} {} ...",
        colors::gray(format!(
          "{} =>",
          to_relative_path_or_remote_url(&self.cwd, &desc.origin)
        )),
        common::format_test_step_ancestry(desc, tests, test_steps)
      );
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

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_summary(&self.cwd, &self.summary, elapsed);
    self.in_new_line = true;
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    common::report_sigint(&self.cwd, tests_pending, tests, test_steps);
    self.in_new_line = true;
  }

  fn flush_report(
    &mut self,
    _elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    Ok(())
  }
}
