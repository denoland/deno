// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use file_test_runner::NO_CAPTURE;
use file_test_runner::TestResult;

use crate::IS_CI;
use crate::colors;

pub fn flaky_test_ci(
  test_name: &str,
  parallelism: &Parallelism,
  run_test: impl Fn() -> TestResult,
) -> TestResult {
  if *IS_CI {
    run_flaky_test(test_name, Some(parallelism), run_test)
  } else {
    run_test()
  }
}

pub struct Parallelism {
  parallelism: Arc<file_test_runner::parallelism::Parallelism>,
  has_raised: AtomicBool,
}

impl Default for Parallelism {
  fn default() -> Self {
    let parallelism = if *NO_CAPTURE {
      file_test_runner::parallelism::Parallelism::none()
    } else {
      file_test_runner::parallelism::Parallelism::from_env()
    };
    Self {
      parallelism: Arc::new(parallelism),
      has_raised: Default::default(),
    }
  }
}

impl Parallelism {
  pub fn for_run_options(
    &self,
  ) -> Arc<file_test_runner::parallelism::Parallelism> {
    self.parallelism.clone()
  }

  fn limit_to_one(&self) -> bool {
    if !self
      .has_raised
      .swap(true, std::sync::atomic::Ordering::Relaxed)
    {
      self
        .parallelism
        .set_parallelism(NonZeroUsize::new(1).unwrap());
      true
    } else {
      false
    }
  }
}

pub fn run_flaky_test(
  test_name: &str,
  parallelism: Option<&Parallelism>,
  action: impl Fn() -> TestResult,
) -> TestResult {
  for _ in 0..2 {
    let result = action();
    if !result.is_failed() {
      return result;
    }
    std::thread::sleep(Duration::from_millis(100));
  }

  // if we got here, it means the CI is very flaky at the moment
  // so reduce concurrency down to 1
  #[allow(clippy::print_stderr)]
  if let Some(parallelism) = parallelism
    && parallelism.limit_to_one()
  {
    eprintln!(
      "{} {} was flaky. Reducing test concurrency to 1.",
      colors::bold_red("***WARNING***"),
      colors::gray(test_name)
    );
    // try running the tests again
    for _ in 0..2 {
      let result = action();
      if !result.is_failed() {
        return result;
      }
      std::thread::sleep(Duration::from_millis(100));
    }
  }

  // surface on third try
  action()
}
