// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::IsTerminal;
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

use console_static_text::ConsoleStaticText;
use file_test_runner::RunOptions;
use file_test_runner::TestResult;
use file_test_runner::reporter::LogReporter;
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

pub fn get_test_reporter<TData>()
-> Arc<dyn file_test_runner::reporter::Reporter<TData>> {
  if *file_test_runner::NO_CAPTURE
    || *IS_CI
    || !std::io::stderr().is_terminal()
    || std::env::var("DENO_TEST_UTIL_REPORTER").ok().as_deref() == Some("log")
  {
    Arc::new(file_test_runner::reporter::LogReporter::default())
  } else {
    Arc::new(PtyReporter::new())
  }
}

struct PtyReporterPendingTest {
  name: String,
  start_time: Instant,
}

struct PtyReporterFailedTest {
  name: String,
  path: String,
}

struct PtyReporterData {
  static_text: ConsoleStaticText,
  pending_tests: Vec<PtyReporterPendingTest>,
  failed_tests: Vec<PtyReporterFailedTest>,
  passed_tests: usize,
  ignored_tests: usize,
}

impl PtyReporterData {
  pub fn render_clear(&mut self) -> String {
    self.static_text.render_clear().unwrap_or_default()
  }

  pub fn render(&mut self) -> Option<String> {
    let mut items = Vec::new();
    const MAX_ITEM_DISPLAY: usize = 10;
    if !self.pending_tests.is_empty() {
      let text = if self.pending_tests.len() > MAX_ITEM_DISPLAY {
        "oldest pending:"
      } else {
        "pending:"
      };
      items.push(console_static_text::TextItem::Text(
        colors::yellow(text).into(),
      ));
      items.extend(self.pending_tests.iter().take(MAX_ITEM_DISPLAY).map(
        |item| {
          console_static_text::TextItem::Text(
            format!(
              "- {} ({}s)",
              item.name,
              item.start_time.elapsed().as_secs()
            )
            .into(),
          )
        },
      ));
    }
    if !self.failed_tests.is_empty() {
      items.push(console_static_text::TextItem::Text(
        colors::red("failed:").to_string().into(),
      ));
      for item in self.failed_tests.iter().rev().take(MAX_ITEM_DISPLAY) {
        items.push(console_static_text::TextItem::Text(
          format!("- {} ({})", item.name, colors::gray(&item.path)).into(),
        ));
      }
    }

    items.push(console_static_text::TextItem::Text(
      format!(
        "    {} Pending - {} Passed - {} Failed - {} Ignored",
        self.pending_tests.len(),
        self.passed_tests,
        self.failed_tests.len(),
        self.ignored_tests
      )
      .into(),
    ));

    self.static_text.render_items(items.iter())
  }
}

struct PtyReporter {
  data: Arc<Mutex<PtyReporterData>>,
  _tx: std::sync::mpsc::Sender<()>,
}

impl PtyReporter {
  pub fn new() -> Self {
    let (tx, rx) = channel();
    let data = Arc::new(Mutex::new(PtyReporterData {
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
      ignored_tests: Default::default(),
    }));
    #[allow(clippy::disallowed_methods)]
    std::thread::spawn({
      let data = data.clone();
      move || {
        loop {
          match rx.recv_timeout(Duration::from_millis(1_000)) {
            Err(RecvTimeoutError::Timeout) => {
              let mut data = data.lock();
              if let Some(text) = data.render() {
                let mut stderr = std::io::stderr().lock();
                _ = stderr.write_all(text.as_bytes());
                _ = stderr.flush();
              }
            }
            _ => {
              return;
            }
          }
        }
      }
    });
    Self { data, _tx: tx }
  }
}

impl<TData> file_test_runner::reporter::Reporter<TData> for PtyReporter {
  fn report_category_start(
    &self,
    category: &file_test_runner::collection::CollectedTestCategory<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    let mut data = self.data.lock();
    let mut final_text = data.render_clear().into_bytes();
    _ = LogReporter::write_report_category_start(&mut final_text, category);
    if let Some(text) = data.render() {
      final_text.extend_from_slice(text.as_bytes());
    }
    let mut stderr = std::io::stderr().lock();
    _ = stderr.write_all(&final_text);
    _ = stderr.flush();
  }

  fn report_category_end(
    &self,
    _category: &file_test_runner::collection::CollectedTestCategory<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
  }

  fn report_test_start(
    &self,
    test: &file_test_runner::collection::CollectedTest<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    let mut data = self.data.lock();
    data.pending_tests.push(PtyReporterPendingTest {
      name: test.name.clone(),
      start_time: std::time::Instant::now(),
    });
    if let Some(final_text) = data.render() {
      let mut stderr = std::io::stderr().lock();
      _ = stderr.write_all(final_text.as_bytes());
      _ = stderr.flush();
    }
  }

  fn report_test_end(
    &self,
    test: &file_test_runner::collection::CollectedTest<TData>,
    duration: Duration,
    result: &TestResult,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    let mut data = self.data.lock();
    let clear_text = data.static_text.render_clear().unwrap_or_default();
    if let Some(index) =
      data.pending_tests.iter().position(|t| t.name == test.name)
    {
      data.pending_tests.remove(index);
    }
    match result {
      TestResult::Passed { .. } => {
        data.passed_tests += 1;
      }
      TestResult::Ignored => {
        data.ignored_tests += 1;
      }
      TestResult::Failed { .. } => {
        data.failed_tests.push(PtyReporterFailedTest {
          name: test.name.to_string(),
          path: match test.line_and_column {
            Some((line, col)) => {
              format!("{}:{}:{}", test.path.display(), line + 1, col + 1)
            }
            None => test.path.display().to_string(),
          },
        });
      }
      TestResult::SubTests { .. } => {
        // ignore
      }
    }
    let mut final_text = clear_text.into_bytes();
    _ = LogReporter::write_report_test_end(
      &mut final_text,
      test,
      duration,
      result,
      &file_test_runner::reporter::ReporterContext { is_parallel: true },
    );
    if let Some(text) = data.render() {
      final_text.extend_from_slice(text.as_bytes());
    }
    let mut stderr = std::io::stderr().lock();
    _ = stderr.write_all(&final_text);
    _ = stderr.flush();
  }

  fn report_failures(
    &self,
    failures: &[file_test_runner::reporter::ReporterFailure<TData>],
    total_tests: usize,
  ) {
    let clear_text = self
      .data
      .lock()
      .static_text
      .render_clear()
      .unwrap_or_default();
    let mut final_text = clear_text.into_bytes();
    _ = LogReporter::write_report_failures(
      &mut final_text,
      failures,
      total_tests,
    );
    let mut stderr = std::io::stderr().lock();
    _ = stderr.write_all(&final_text);
    _ = stderr.flush();
  }
}
