// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use file_test_runner::TestResult;

use crate::IS_CI;
use crate::colors;
use crate::eprintln;

pub fn flaky_test_ci(
  test_name: &str,
  parallelism: Option<&Parallelism>,
  run_test: impl Fn() -> TestResult,
) -> TestResult {
  if *IS_CI {
    run_flaky_test(test_name, parallelism, run_test)
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
    let parallelism = file_test_runner::parallelism::Parallelism::from_env();
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
  for i in 0..2 {
    let result = action();
    if !result.is_failed() {
      return result;
    }
    if *IS_CI {
      eprintln!(
        "{} {} was flaky on run {}",
        colors::bold_red("Warning"),
        colors::gray(test_name),
        i,
      );
    }
    std::thread::sleep(Duration::from_millis(100));
  }

  // if we got here, it means the CI is very flaky at the moment
  // so reduce concurrency down to 1
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

pub struct TestTimeoutHolder {
  _tx: std::sync::mpsc::Sender<()>,
}

pub fn with_timeout(
  test_name: String,
  duration: Duration,
) -> TestTimeoutHolder {
  let (tx, rx) = ::std::sync::mpsc::channel::<()>();
  // ok to allow because we don't need to maintain logging context here
  #[allow(clippy::disallowed_methods)]
  std::thread::spawn(move || {
    if rx.recv_timeout(duration)
      == Err(::std::sync::mpsc::RecvTimeoutError::Timeout)
    {
      use std::io::Write;
      #[allow(clippy::print_stderr)]
      {
        ::std::eprintln!(
          "Test {test_name} timed out after {} seconds, aborting",
          duration.as_secs()
        );
      }
      _ = std::io::stderr().flush();
      #[allow(clippy::disallowed_methods)]
      ::std::process::exit(1);
    }
  });
  TestTimeoutHolder { _tx: tx }
}
