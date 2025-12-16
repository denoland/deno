// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use file_test_runner::TestResult;
use file_test_runner::parallelism::ParallelismProvider;
use parking_lot::Mutex;

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

struct SingleConcurrencyFlagGuard<'a>(&'a Parallelism);

impl<'a> Drop for SingleConcurrencyFlagGuard<'a> {
  fn drop(&mut self) {
    let mut value = self.0.has_raised_count.lock();
    *value -= 1;
    if *value == 0 {
      self.0.parallelism.set_max(self.0.max_parallelism);
    }
  }
}

pub struct Parallelism {
  parallelism: Arc<file_test_runner::parallelism::Parallelism>,
  max_parallelism: NonZeroUsize,
  has_raised_count: Mutex<usize>,
}

impl Default for Parallelism {
  fn default() -> Self {
    let parallelism = file_test_runner::parallelism::Parallelism::from_env();
    Self {
      max_parallelism: parallelism.max_parallelism(),
      parallelism: Arc::new(parallelism),
      has_raised_count: Default::default(),
    }
  }
}

impl Parallelism {
  pub fn for_run_options(
    &self,
  ) -> Arc<file_test_runner::parallelism::Parallelism> {
    self.parallelism.clone()
  }

  fn raise_single_concurrency_flag(&self) -> SingleConcurrencyFlagGuard<'_> {
    {
      let mut value = self.has_raised_count.lock();
      if *value == 0 {
        self.parallelism.set_max(NonZeroUsize::new(1).unwrap());
      }
      *value += 1;
    }
    SingleConcurrencyFlagGuard(self)
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

  // on the CI, try running the test in isolation with no other tests running
  let _maybe_guard = if let Some(parallelism) = parallelism.filter(|_| *IS_CI) {
    let guard = parallelism.raise_single_concurrency_flag();
    eprintln!(
      "{} {} was flaky. Temporarily reducing test concurrency to 1 and trying a few more times.",
      colors::bold_red("***WARNING***"),
      colors::gray(test_name)
    );

    for _ in 0..2 {
      let result = action();
      if !result.is_failed() {
        return result;
      }
      std::thread::sleep(Duration::from_millis(100));
    }

    Some(guard)
  } else {
    None
  };

  // surface result now
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
