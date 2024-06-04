// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::*;

pub struct CompoundTestReporter {
  test_reporters: Vec<Box<dyn TestReporter>>,
}

impl CompoundTestReporter {
  pub fn new(test_reporters: Vec<Box<dyn TestReporter>>) -> Self {
    Self { test_reporters }
  }
}

impl TestReporter for CompoundTestReporter {
  fn report_register(&mut self, description: &TestDescription) {
    for reporter in &mut self.test_reporters {
      reporter.report_register(description);
    }
  }

  fn report_plan(&mut self, plan: &TestPlan) {
    for reporter in &mut self.test_reporters {
      reporter.report_plan(plan);
    }
  }

  fn report_wait(&mut self, description: &TestDescription) {
    for reporter in &mut self.test_reporters {
      reporter.report_wait(description);
    }
  }

  fn report_slow(&mut self, description: &TestDescription, elapsed: u64) {
    for reporter in &mut self.test_reporters {
      reporter.report_slow(description, elapsed);
    }
  }

  fn report_output(&mut self, output: &[u8]) {
    for reporter in &mut self.test_reporters {
      reporter.report_output(output);
    }
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
    options: Option<&TestFailureFormatOptions>,
  ) {
    for reporter in &mut self.test_reporters {
      reporter.report_result(description, result, elapsed, options);
    }
  }

  fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
    for reporter in &mut self.test_reporters {
      reporter.report_uncaught_error(origin, error.clone());
    }
  }

  fn report_step_register(&mut self, description: &TestStepDescription) {
    for reporter in &mut self.test_reporters {
      reporter.report_step_register(description)
    }
  }

  fn report_step_wait(&mut self, description: &TestStepDescription) {
    for reporter in &mut self.test_reporters {
      reporter.report_step_wait(description)
    }
  }

  fn report_step_result(
    &mut self,
    desc: &TestStepDescription,
    result: &TestStepResult,
    elapsed: u64,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    options: Option<&TestFailureFormatOptions>,
  ) {
    for reporter in &mut self.test_reporters {
      reporter
        .report_step_result(desc, result, elapsed, tests, test_steps, options);
    }
  }

  fn report_summary(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    options: Option<&TestFailureFormatOptions>,
  ) {
    for reporter in &mut self.test_reporters {
      reporter.report_summary(elapsed, tests, test_steps, options);
    }
  }

  fn report_sigint(
    &mut self,
    tests_pending: &HashSet<usize>,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
    options: Option<&TestFailureFormatOptions>,
  ) {
    for reporter in &mut self.test_reporters {
      reporter.report_sigint(tests_pending, tests, test_steps, options);
    }
  }

  fn report_completed(&mut self) {
    for reporter in &mut self.test_reporters {
      reporter.report_completed();
    }
  }

  fn flush_report(
    &mut self,
    elapsed: &Duration,
    tests: &IndexMap<usize, TestDescription>,
    test_steps: &IndexMap<usize, TestStepDescription>,
  ) -> anyhow::Result<()> {
    let mut errors = vec![];
    for reporter in &mut self.test_reporters {
      if let Err(err) = reporter.flush_report(elapsed, tests, test_steps) {
        errors.push(err)
      }
    }

    if errors.is_empty() {
      Ok(())
    } else {
      bail!(
        "error in one or more wrapped reporters:\n{}",
        errors
          .iter()
          .enumerate()
          .fold(String::new(), |acc, (i, err)| {
            format!("{}Error #{}: {:?}\n", acc, i + 1, err)
          })
      )
    }
  }
}
