// Copyright 2018-2025 the Deno authors. MIT license.

use std::num::NonZeroUsize;
use std::time::Duration;

use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use parking_lot::Mutex;

use crate::IS_CI;
use crate::colors;
use crate::semaphore::Semaphore;

pub fn flaky_test_ci(
  test_name: &str,
  parallelism: Option<&Parallelism>,
  run_test: impl Fn() -> TestResult,
) -> TestResult {
  run_maybe_flaky_test(test_name, *IS_CI, parallelism, run_test)
}

struct SingleConcurrencyFlagGuard<'a>(&'a Parallelism);

impl<'a> Drop for SingleConcurrencyFlagGuard<'a> {
  fn drop(&mut self) {
    let mut value = self.0.has_raised_count.lock();
    *value -= 1;
    if *value == 0 {
      self.0.semaphore.set_max(self.0.max_parallelism.get());
    }
  }
}

pub struct Parallelism {
  semaphore: Semaphore,
  max_parallelism: NonZeroUsize,
  has_raised_count: Mutex<usize>,
}

impl Default for Parallelism {
  fn default() -> Self {
    let max_parallelism = RunOptions::default_parallelism();
    Self {
      max_parallelism,
      semaphore: Semaphore::new(max_parallelism.get()),
      has_raised_count: Default::default(),
    }
  }
}

impl Parallelism {
  pub fn max_parallelism(&self) -> NonZeroUsize {
    self.max_parallelism
  }

  fn acquire(&self) -> crate::semaphore::Permit<'_> {
    self.semaphore.acquire()
  }

  fn raise_single_concurrency_flag(&self) -> SingleConcurrencyFlagGuard<'_> {
    {
      let mut value = self.has_raised_count.lock();
      if *value == 0 {
        self.semaphore.set_max(1);
      }
      *value += 1;
    }
    SingleConcurrencyFlagGuard(self)
  }
}

pub fn run_maybe_flaky_test(
  test_name: &str,
  is_flaky: bool,
  parallelism: Option<&Parallelism>,
  main_action: impl Fn() -> TestResult,
) -> TestResult {
  let ci_parallelism = parallelism.filter(|_| *IS_CI);
  let action = || run_with_parallelism(ci_parallelism, &main_action);
  if !is_flaky {
    return action();
  }
  for i in 0..2 {
    let result = action();
    if !result.is_failed() {
      return result;
    }
    #[allow(clippy::print_stderr)]
    if *IS_CI {
      ::std::eprintln!(
        "{} {} was flaky on run {}",
        colors::bold_red("Warning"),
        colors::gray(test_name),
        i,
      );
    }
    std::thread::sleep(Duration::from_millis(100));
  }

  // on the CI, try running the test in isolation with no other tests running
  #[allow(clippy::print_stderr)]
  let _maybe_guard = if let Some(parallelism) = ci_parallelism {
    let guard = parallelism.raise_single_concurrency_flag();
    ::std::eprintln!(
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

fn run_with_parallelism(
  parallelism: Option<&Parallelism>,
  action: impl Fn() -> TestResult,
) -> TestResult {
  let _maybe_permit = parallelism.map(|p| p.acquire());
  let duration = std::time::Instant::now();
  let result = action();
  result.with_duration(duration.elapsed())
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
