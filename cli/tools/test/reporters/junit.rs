// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use super::*;

pub struct JunitTestReporter {
  path: String,
  // Stores TestCases (i.e. Tests) by the Test ID
  cases: IndexMap<usize, quick_junit::TestCase>,
}

impl JunitTestReporter {
  pub fn new(path: String) -> Self {
    Self {
      path,
      cases: IndexMap::new(),
    }
  }

  fn convert_status(status: &TestResult) -> quick_junit::TestCaseStatus {
    match status {
      TestResult::Ok => quick_junit::TestCaseStatus::success(),
      TestResult::Ignored => quick_junit::TestCaseStatus::skipped(),
      TestResult::Failed(failure) => quick_junit::TestCaseStatus::NonSuccess {
        kind: quick_junit::NonSuccessKind::Failure,
        message: Some(failure.to_string()),
        ty: None,
        description: None,
        reruns: vec![],
      },
      TestResult::Cancelled => quick_junit::TestCaseStatus::NonSuccess {
        kind: quick_junit::NonSuccessKind::Error,
        message: Some("Cancelled".to_string()),
        ty: None,
        description: None,
        reruns: vec![],
      },
    }
  }
}

impl TestReporter for JunitTestReporter {
  fn report_register(&mut self, description: &TestDescription) {
    let mut case = quick_junit::TestCase::new(
      description.name.clone(),
      quick_junit::TestCaseStatus::skipped(),
    );
    let file_name = description.location.file_name.clone();
    let file_name = file_name.strip_prefix("file://").unwrap_or(&file_name);
    case
      .extra
      .insert(String::from("filename"), String::from(file_name));
    case.extra.insert(
      String::from("line"),
      description.location.line_number.to_string(),
    );
    case.extra.insert(
      String::from("col"),
      description.location.column_number.to_string(),
    );
    self.cases.insert(description.id, case);
  }

  fn report_plan(&mut self, _plan: &TestPlan) {}

  fn report_wait(&mut self, _description: &TestDescription) {}

  fn report_output(&mut self, _output: &[u8]) {
    /*
     TODO(skycoop): Right now I can't include stdout/stderr in the report because
     we have a global pair of output streams that don't differentiate between the
     output of different tests. This is a nice to have feature, so we can come
     back to it later
    */
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    if let Some(case) = self.cases.get_mut(&description.id) {
      case.status = Self::convert_status(result);
      case.set_time(Duration::from_millis(elapsed));
    }
  }

  fn report_uncaught_error(&mut self, _origin: &str, _error: Box<JsError>) {}

  fn report_step_register(&mut self, _description: &TestStepDescription) {}

  fn report_step_wait(&mut self, _description: &TestStepDescription) {}

  fn report_step_result(
    &mut self,
    description: &TestStepDescription,
    result: &TestStepResult,
    _elapsed: u64,
    _tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) {
    let status = match result {
      TestStepResult::Ok => "passed",
      TestStepResult::Ignored => "skipped",
      TestStepResult::Failed(_) => "failure",
    };

    let root_id: usize;
    let mut name = String::new();
    {
      let mut ancestors = vec![];
      let mut current_desc = description;
      loop {
        if let Some(d) = test_steps.get(&current_desc.parent_id) {
          ancestors.push(&d.name);
          current_desc = d;
        } else {
          root_id = current_desc.parent_id;
          break;
        }
      }
      ancestors.reverse();
      for n in ancestors {
        name.push_str(n);
        name.push_str(" ... ");
      }
      name.push_str(&description.name);
    }

    if let Some(case) = self.cases.get_mut(&root_id) {
      case.add_property(quick_junit::Property::new(
        format!("step[{}]", status),
        name,
      ));
    }
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
        self.report_result(description, &TestResult::Cancelled, 0)
      }
    }
  }

  fn report_completed(&mut self) {
    // TODO(mmastrac): This reporter does not handle stdout/stderr yet, and when we do, we may need to redirect
    // pre-and-post-test output somewhere.
  }

  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    _test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    let mut suites: IndexMap<String, quick_junit::TestSuite> = IndexMap::new();
    for (id, case) in &self.cases {
      if let Some(test) = tests.get(id) {
        suites
          .entry(test.location.file_name.clone())
          .and_modify(|s| {
            s.add_test_case(case.clone());
          })
          .or_insert_with(|| {
            quick_junit::TestSuite::new(test.location.file_name.clone())
              .add_test_case(case.clone())
              .to_owned()
          });
      }
    }

    let mut report = quick_junit::Report::new("deno test");
    report.set_time(*elapsed).add_test_suites(
      suites
        .values()
        .cloned()
        .collect::<Vec<quick_junit::TestSuite>>(),
    );

    if self.path == "-" {
      report
        .serialize(std::io::stdout())
        .with_context(|| "Failed to write JUnit report to stdout")?;
    } else {
      let file = crate::util::fs::create_file(&PathBuf::from(&self.path))
        .context("Failed to open JUnit report file.")?;
      report.serialize(file).with_context(|| {
        format!("Failed to write JUnit report to {}", self.path)
      })?;
    }

    Ok(())
  }
}
