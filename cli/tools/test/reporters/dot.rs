// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::fmt::format_test_error;
use super::fmt::to_relative_path_or_remote_url;
use super::*;

pub struct DotTestReporter {
  n: usize,
  width: usize,
  cwd: Url,
  summary: TestSummary,
}

impl DotTestReporter {
  pub fn new() -> DotTestReporter {
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
      cwd: Url::from_directory_path(std::env::current_dir().unwrap()).unwrap(),
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

  // TODO(bartlomieju): deduplicate with PrettyTestReporter
  fn format_test_step_ancestry(
    &self,
    desc: &TestStepDescription,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> String {
    let root;
    let mut ancestor_names = vec![];
    let mut current_desc = desc;
    loop {
      if let Some(step_desc) = test_steps.get(&current_desc.parent_id) {
        ancestor_names.push(&step_desc.name);
        current_desc = step_desc;
      } else {
        root = tests.get(&current_desc.parent_id).unwrap();
        break;
      }
    }
    ancestor_names.reverse();
    let mut result = String::new();
    result.push_str(&root.name);
    result.push_str(" ... ");
    for name in ancestor_names {
      result.push_str(name);
      result.push_str(" ... ");
    }
    result.push_str(&desc.name);
    result
  }

  // TODO(bartlomieju): deduplicate with PrettyTestReporter
  fn format_test_for_summary(&self, desc: &TestDescription) -> String {
    format!(
      "{} {}",
      &desc.name,
      colors::gray(format!(
        "=> {}:{}:{}",
        to_relative_path_or_remote_url(&self.cwd, &desc.location.file_name),
        desc.location.line_number,
        desc.location.column_number
      ))
    )
  }

  // TODO(bartlomieju): deduplicate with PrettyTestReporter
  fn format_test_step_for_summary(
    &self,
    desc: &TestStepDescription,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> String {
    let long_name = self.format_test_step_ancestry(desc, tests, test_steps);
    format!(
      "{} {}",
      long_name,
      colors::gray(format!(
        "=> {}:{}:{}",
        to_relative_path_or_remote_url(&self.cwd, &desc.location.file_name),
        desc.location.line_number,
        desc.location.column_number
      ))
    )
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

  fn report_output(&mut self, _output: &[u8]) {}

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    _elapsed: u64,
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
            name: self.format_test_step_ancestry(desc, tests, test_steps),
            ignore: false,
            only: false,
            origin: desc.origin.clone(),
            location: desc.location.clone(),
          },
          failure.clone(),
        ))
      }
    }

    self.print_test_step_result(result);
  }

  // TODO(bartlomieju): deduplicate with PrettyTestReporter
  fn report_summary(
    &mut self,
    elapsed: &Duration,
    _tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    if !self.summary.failures.is_empty()
      || !self.summary.uncaught_errors.is_empty()
    {
      #[allow(clippy::type_complexity)] // Type alias doesn't look better here
      let mut failures_by_origin: BTreeMap<
        String,
        (Vec<(&TestDescription, &TestFailure)>, Option<&JsError>),
      > = BTreeMap::default();
      let mut failure_titles = vec![];
      for (description, failure) in &self.summary.failures {
        let (failures, _) = failures_by_origin
          .entry(description.origin.clone())
          .or_default();
        failures.push((description, failure));
      }

      for (origin, js_error) in &self.summary.uncaught_errors {
        let (_, uncaught_error) =
          failures_by_origin.entry(origin.clone()).or_default();
        let _ = uncaught_error.insert(js_error.as_ref());
      }

      println!();
      // note: the trailing whitespace is intentional to get a red background
      println!("\n{}\n", colors::white_bold_on_red(" ERRORS "));
      for (origin, (failures, uncaught_error)) in failures_by_origin {
        for (description, failure) in failures {
          if !failure.hide_in_summary() {
            let failure_title = self.format_test_for_summary(description);
            println!("{}", &failure_title);
            println!("{}: {}", colors::red_bold("error"), failure.to_string());
            println!();
            failure_titles.push(failure_title);
          }
        }
        if let Some(js_error) = uncaught_error {
          let failure_title = format!(
            "{} (uncaught error)",
            to_relative_path_or_remote_url(&self.cwd, &origin)
          );
          println!("{}", &failure_title);
          println!(
            "{}: {}",
            colors::red_bold("error"),
            format_test_error(js_error)
          );
          println!("This error was not caught from a test and caused the test runner to fail on the referenced module.");
          println!("It most likely originated from a dangling promise, event/timeout handler or top-level code.");
          println!();
          failure_titles.push(failure_title);
        }
      }
      // note: the trailing whitespace is intentional to get a red background
      println!("{}\n", colors::white_bold_on_red(" FAILURES "));
      for failure_title in failure_titles {
        println!("{failure_title}");
      }
    }

    let status = if self.summary.has_failed() {
      println!();
      colors::red("FAILED").to_string()
    } else {
      colors::green("ok").to_string()
    };

    let get_steps_text = |count: usize| -> String {
      if count == 0 {
        String::new()
      } else if count == 1 {
        " (1 step)".to_string()
      } else {
        format!(" ({count} steps)")
      }
    };

    let mut summary_result = String::new();

    write!(
      summary_result,
      "{} passed{} | {} failed{}",
      self.summary.passed,
      get_steps_text(self.summary.passed_steps),
      self.summary.failed,
      get_steps_text(self.summary.failed_steps),
    )
    .unwrap();

    let ignored_steps = get_steps_text(self.summary.ignored_steps);
    if self.summary.ignored > 0 || !ignored_steps.is_empty() {
      write!(
        summary_result,
        " | {} ignored{}",
        self.summary.ignored, ignored_steps
      )
      .unwrap()
    }

    if self.summary.measured > 0 {
      write!(summary_result, " | {} measured", self.summary.measured,).unwrap();
    }

    if self.summary.filtered_out > 0 {
      write!(
        summary_result,
        " | {} filtered out",
        self.summary.filtered_out
      )
      .unwrap()
    };

    println!(
      "\n{} | {} {}\n",
      status,
      summary_result,
      colors::gray(format!(
        "({})",
        display::human_elapsed(elapsed.as_millis())
      )),
    );
  }

  // TODO(bartlomieju): deduplicate with PrettyTestReporter
  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    if tests_pending.is_empty() {
      return;
    }
    let mut formatted_pending = BTreeSet::new();
    for id in tests_pending {
      if let Some(desc) = tests.get(id) {
        formatted_pending.insert(self.format_test_for_summary(desc));
      }
      if let Some(desc) = test_steps.get(id) {
        formatted_pending
          .insert(self.format_test_step_for_summary(desc, tests, test_steps));
      }
    }
    println!(
      "\n{} The following tests were pending:\n",
      colors::intense_blue("SIGINT")
    );
    for entry in formatted_pending {
      println!("{}", entry);
    }
    println!();
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
