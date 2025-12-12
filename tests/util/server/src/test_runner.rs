// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::IsTerminal;
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::time::Instant;

use console_static_text::ConsoleStaticText;
use file_test_runner::TestResult;
use file_test_runner::reporter::LogReporter;
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

pub fn get_test_reporter<TData>()
-> Arc<dyn file_test_runner::reporter::Reporter<TData>> {
  if *file_test_runner::NO_CAPTURE || *IS_CI || !std::io::stderr().is_terminal()
  {
    Arc::new(file_test_runner::reporter::LogReporter)
  } else {
    Arc::new(PtyReporter::new())
  }
}

struct PtyReporterPendingTest {
  name: String,
  start_time: Instant,
}

struct PtyReporterData {
  static_text: ConsoleStaticText,
  pending_tests: Vec<PtyReporterPendingTest>,
  failed_tests: Vec<String>,
  passed_tests: usize,
}

impl PtyReporterData {
  pub fn render(&mut self) -> Option<String> {
    let items: Vec<_> = self
      .pending_tests
      .iter()
      .map(|item| {
        console_static_text::TextItem::Text(
          format!(
            "test {} ... ({}s)",
            item.name,
            item.start_time.elapsed().as_secs()
          )
          .into(),
        )
      })
      .collect();
    self.static_text.render_items(items.iter())
  }
}

struct PtyReporter {
  data: Mutex<PtyReporterData>,
}

impl PtyReporter {
  pub fn new() -> Self {
    Self {
      data: Mutex::new(PtyReporterData {
        static_text: ConsoleStaticText::new(move || {
          let size = crossterm::terminal::size().ok();
          console_static_text::ConsoleSize {
            cols: size.map(|(cols, _)| cols),
            rows: size.map(|(_, rows)| rows),
          }
        }),
        pending_tests: Default::default(),
        failed_tests: Default::default(),
        passed_tests: Default::default(),
      }),
    }
  }
  pub fn render(&self) {
    let maybe_text = { self.data.lock().render() };
    if let Some(text) = maybe_text {
      _ = std::io::stderr().write_all(text.as_bytes());
    }
  }

  pub fn render_clear(&self) {
    let maybe_clear_text = { self.data.lock().static_text.render_clear() };
    if let Some(text) = maybe_clear_text {
      _ = std::io::stderr().write_all(text.as_bytes());
    }
  }
}

impl<TData> file_test_runner::reporter::Reporter<TData> for PtyReporter {
  fn report_category_start(
    &self,
    category: &file_test_runner::collection::CollectedTestCategory<TData>,
    context: &file_test_runner::reporter::ReporterContext,
  ) {
    self.render_clear();
    LogReporter.report_category_start(category, context);
    self.render();
  }

  fn report_category_end(
    &self,
    category: &file_test_runner::collection::CollectedTestCategory<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    self.render_clear();
    LogReporter.report_category_end(
      category,
      &file_test_runner::reporter::ReporterContext { is_parallel: true },
    );
    self.render();
  }

  fn report_test_start(
    &self,
    test: &file_test_runner::collection::CollectedTest<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    let maybe_text = {
      let mut data = self.data.lock();
      data.pending_tests.push(PtyReporterPendingTest {
        name: test.name.clone(),
        start_time: std::time::Instant::now(),
      });
      data.render()
    };
    if let Some(text) = maybe_text {
      _ = std::io::stderr().write_all(text.as_bytes());
    }
  }

  fn report_test_end(
    &self,
    test: &file_test_runner::collection::CollectedTest<TData>,
    duration: Duration,
    result: &TestResult,
    context: &file_test_runner::reporter::ReporterContext,
  ) {
    self.render_clear();
    LogReporter.report_test_end(test, duration, result, context);
    let maybe_text = {
      let mut data = self.data.lock();
      if let Some(index) =
        data.pending_tests.iter().position(|t| t.name == test.name)
      {
        data.pending_tests.remove(index);
      }
      if result.is_failed() {
        data.failed_tests.push(test.name.clone());
      } else if matches!(result, TestResult::Passed) {
        data.passed_tests += 1;
      }
      data.render()
    };
    if let Some(text) = maybe_text {
      _ = std::io::stderr().write_all(text.as_bytes());
    }
  }

  fn report_long_running_test(&self, _test_name: &str) {
    // don't bother reporting because the pty tests display this
  }

  fn report_failures(
    &self,
    failures: &[file_test_runner::reporter::ReporterFailure<TData>],
    total_tests: usize,
  ) {
    self.render_clear();
    LogReporter.report_failures(failures, total_tests);
  }
}
