// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::time::Instant;

use console_static_text::ConsoleStaticText;
use file_test_runner::TestResult;
use file_test_runner::collection::CollectedCategoryOrTest;
use file_test_runner::collection::CollectedTestCategory;
use file_test_runner::reporter::LogReporter;
use parking_lot::Mutex;
use serde::Serialize;

use crate::IS_CI;
use crate::colors;
use crate::semaphore::Semaphore;

pub struct ShardConfig {
  pub index: usize,
  pub total: usize,
}

impl ShardConfig {
  /// Reads CI_SHARD_INDEX and CI_SHARD_TOTAL from env.
  /// Returns None if not set or total <= 1.
  pub fn from_env() -> Option<Self> {
    let total: usize = std::env::var("CI_SHARD_TOTAL").ok()?.parse().ok()?;
    let index: usize = std::env::var("CI_SHARD_INDEX").ok()?.parse().ok()?;
    if total <= 1 {
      return None;
    }
    Some(Self { index, total })
  }
}

/// Filter a collected test category to only include tests assigned to this shard.
/// Uses round-robin distribution by sorted test name.
pub fn filter_to_shard<T>(
  category: CollectedTestCategory<T>,
  shard: &ShardConfig,
) -> CollectedTestCategory<T> {
  let all_names = collect_test_names(&category);

  let my_tests = assign_shard_tests(&all_names, shard);

  let total_count = all_names.len();
  let shard_count = my_tests.len();
  crate::eprintln!(
    "shard {}/{}: running {shard_count}/{total_count} tests",
    shard.index + 1,
    shard.total,
  );

  let (mine, _) = category.partition(|test| my_tests.contains(&test.name));
  mine
}

fn collect_test_names<T>(category: &CollectedTestCategory<T>) -> Vec<String> {
  let mut names = Vec::new();
  fn walk<T>(children: &[CollectedCategoryOrTest<T>], names: &mut Vec<String>) {
    for child in children {
      match child {
        CollectedCategoryOrTest::Test(t) => names.push(t.name.clone()),
        CollectedCategoryOrTest::Category(c) => walk(&c.children, names),
      }
    }
  }
  walk(&category.children, &mut names);
  names
}

fn assign_shard_tests(
  all_names: &[String],
  shard: &ShardConfig,
) -> HashSet<String> {
  // round-robin: distribute by sorted name index
  let mut sorted: Vec<_> = all_names.to_vec();
  sorted.sort();
  sorted
    .into_iter()
    .enumerate()
    .filter(|(i, _)| i % shard.total == shard.index)
    .map(|(_, name)| name)
    .collect()
}

/// Tracks the number of times each test has been flaky
pub struct FlakyTestTracker {
  flaky_counts: Mutex<HashMap<String, usize>>,
}

impl FlakyTestTracker {
  pub fn record_flaky(&self, test_name: &str) {
    let mut counts = self.flaky_counts.lock();
    *counts.entry(test_name.to_string()).or_insert(0) += 1;
  }

  pub fn get_count(&self, test_name: &str) -> usize {
    let counts = self.flaky_counts.lock();
    counts.get(test_name).copied().unwrap_or(0)
  }
}

impl Default for FlakyTestTracker {
  fn default() -> Self {
    Self {
      flaky_counts: Mutex::new(HashMap::new()),
    }
  }
}

pub fn flaky_test_ci(
  test_name: &str,
  flaky_test_tracker: &FlakyTestTracker,
  parallelism: Option<&Parallelism>,
  run_test: impl Fn() -> TestResult,
) -> TestResult {
  run_maybe_flaky_test(
    test_name,
    *IS_CI,
    flaky_test_tracker,
    parallelism,
    run_test,
  )
}

/// Coordinates semaphore max adjustments between the memory monitor
/// and the flaky test single-concurrency mode, preventing race conditions.
struct ParallelismController {
  semaphore: Semaphore,
  max_parallelism: usize,
  state: Mutex<ControllerState>,
}

struct ControllerState {
  single_concurrency_count: usize,
}

impl ParallelismController {
  fn new(max: usize) -> Self {
    Self {
      semaphore: Semaphore::new(max),
      max_parallelism: max,
      state: Mutex::new(ControllerState {
        single_concurrency_count: 0,
      }),
    }
  }

  fn acquire(&self) -> crate::semaphore::Permit<'_> {
    self.semaphore.acquire()
  }

  fn enter_single_concurrency(&self) {
    let mut state = self.state.lock();
    if state.single_concurrency_count == 0 {
      self.semaphore.set_max(1);
    }
    state.single_concurrency_count += 1;
  }

  fn exit_single_concurrency(&self) {
    let mut state = self.state.lock();
    state.single_concurrency_count -= 1;
    if state.single_concurrency_count == 0 {
      // restore to max_parallelism; if memory is still low the
      // monitor will re-reduce on its next check
      self.semaphore.set_max(self.max_parallelism);
    }
  }

  /// Try to reduce parallelism by 1 for memory pressure.
  /// Returns false if single-concurrency mode is active or already at 1.
  fn try_reduce_for_memory(&self) -> bool {
    let state = self.state.lock();
    if state.single_concurrency_count > 0 {
      return false;
    }
    let current = self.semaphore.get_max();
    if current > 1 {
      self.semaphore.set_max(current - 1);
      true
    } else {
      false
    }
  }

  /// Try to increase parallelism by 1 after memory recovery.
  /// Returns false if single-concurrency mode is active or already at max.
  fn try_increase_for_memory(&self) -> bool {
    let state = self.state.lock();
    if state.single_concurrency_count > 0 {
      return false;
    }
    let current = self.semaphore.get_max();
    if current < self.max_parallelism {
      self.semaphore.set_max(current + 1);
      true
    } else {
      false
    }
  }
}

struct SingleConcurrencyFlagGuard<'a>(&'a Parallelism);

impl<'a> Drop for SingleConcurrencyFlagGuard<'a> {
  fn drop(&mut self) {
    self.0.controller.exit_single_concurrency();
  }
}

pub struct Parallelism {
  controller: Arc<ParallelismController>,
  max_parallelism: file_test_runner::Parallelism,
  // dropping this shuts down the memory monitor thread
  _monitor_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl Default for Parallelism {
  fn default() -> Self {
    let parallelism = file_test_runner::Parallelism::default();
    let controller = Arc::new(ParallelismController::new(parallelism.get()));
    let monitor_tx = spawn_memory_monitor(controller.clone());
    Self {
      max_parallelism: parallelism,
      controller,
      _monitor_tx: monitor_tx,
    }
  }
}

fn spawn_memory_monitor(
  controller: Arc<ParallelismController>,
) -> Option<std::sync::mpsc::Sender<()>> {
  let info = crate::memory::mem_info()?;
  let threshold = info.total / 10; // 10% of total memory

  let (tx, rx) = std::sync::mpsc::channel::<()>();
  #[allow(clippy::disallowed_methods)]
  std::thread::spawn(move || {
    loop {
      match rx.recv_timeout(Duration::from_secs(2)) {
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        // channel closed, exit
        _ => return,
      }
      let Some(current) = crate::memory::mem_info() else {
        continue;
      };
      if current.available < threshold {
        controller.try_reduce_for_memory();
      } else {
        controller.try_increase_for_memory();
      }
    }
  });

  Some(tx)
}

impl Parallelism {
  pub fn max_parallelism(&self) -> file_test_runner::Parallelism {
    self.max_parallelism
  }

  fn acquire(&self) -> crate::semaphore::Permit<'_> {
    self.controller.acquire()
  }

  fn raise_single_concurrency_flag(&self) -> SingleConcurrencyFlagGuard<'_> {
    self.controller.enter_single_concurrency();
    SingleConcurrencyFlagGuard(self)
  }
}

pub fn run_maybe_flaky_test(
  test_name: &str,
  is_flaky: bool,
  flaky_test_tracker: &FlakyTestTracker,
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
    flaky_test_tracker.record_flaky(test_name);
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
      flaky_test_tracker.record_flaky(test_name);
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedTestResult {
  name: String,
  path: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  duration: Option<u128>,
  #[serde(skip_serializing_if = "is_false")]
  failed: bool,
  #[serde(skip_serializing_if = "is_false")]
  ignored: bool,
  #[serde(skip_serializing_if = "is_zero")]
  flaky_count: usize,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  sub_tests: Vec<RecordedTestResult>,
}

fn is_false(value: &bool) -> bool {
  !value
}

fn is_zero(value: &usize) -> bool {
  *value == 0
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordedReport {
  tests: Vec<RecordedTestResult>,
}

struct JsonReporter {
  data: Arc<Mutex<RecordedReport>>,
  flaky_tracker: Arc<FlakyTestTracker>,
  test_module_name: String,
}

impl JsonReporter {
  pub fn new(
    flaky_tracker: Arc<FlakyTestTracker>,
    test_module_name: String,
  ) -> Self {
    Self {
      data: Default::default(),
      flaky_tracker,
      test_module_name,
    }
  }

  fn write_report_to_file(&self) {
    let json = {
      let data = self.data.lock();
      serde_json::to_string(&*data).unwrap()
    };
    let shard_suffix = match ShardConfig::from_env() {
      Some(shard) => format!("_shard-{}", shard.index),
      None => String::new(),
    };
    let file_path = crate::root_path().join("target").join(format!(
      "test_results_{}{shard_suffix}.json",
      self.test_module_name
    ));

    file_path.write(json);
  }

  fn flatten_and_record_test(
    &self,
    tests: &mut Vec<RecordedTestResult>,
    test_name: String,
    path: String,
    result: &TestResult,
    main_duration: Option<Duration>,
  ) {
    match result {
      TestResult::SubTests {
        sub_tests,
        duration,
      } => {
        let mut sub_test_results = Vec::with_capacity(sub_tests.len());
        for sub_test in sub_tests {
          let full_name = format!("{}::{}", test_name, sub_test.name);
          self.flatten_and_record_test(
            &mut sub_test_results,
            full_name,
            path.clone(),
            &sub_test.result,
            None,
          );
        }
        let flaky_count = self.flaky_tracker.get_count(&test_name);
        tests.push(RecordedTestResult {
          name: test_name,
          path,
          duration: duration.or(main_duration).map(|d| d.as_millis()),
          failed: sub_tests.iter().any(|s| s.result.is_failed()),
          ignored: false,
          flaky_count,
          sub_tests: sub_test_results,
        })
      }
      TestResult::Passed { duration } => {
        let flaky_count = self.flaky_tracker.get_count(&test_name);
        let test_result = RecordedTestResult {
          name: test_name,
          path,
          duration: duration.or(main_duration).map(|d| d.as_millis()),
          failed: false,
          ignored: false,
          flaky_count,
          sub_tests: Vec::new(),
        };
        tests.push(test_result);
      }
      TestResult::Failed { duration, .. } => {
        let flaky_count = self.flaky_tracker.get_count(&test_name);
        let test_result = RecordedTestResult {
          name: test_name,
          path,
          duration: duration.or(main_duration).map(|d| d.as_millis()),
          failed: true,
          ignored: false,
          flaky_count,
          sub_tests: Vec::new(),
        };
        tests.push(test_result.clone());
      }
      TestResult::Ignored => {
        let flaky_count = self.flaky_tracker.get_count(&test_name);
        let test_result = RecordedTestResult {
          name: test_name,
          path,
          duration: None,
          failed: false,
          ignored: true,
          flaky_count,
          sub_tests: Vec::new(),
        };
        tests.push(test_result);
      }
    }
  }
}

impl<TData> file_test_runner::reporter::Reporter<TData> for JsonReporter {
  fn report_category_start(
    &self,
    _category: &file_test_runner::collection::CollectedTestCategory<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
  }

  fn report_category_end(
    &self,
    _category: &file_test_runner::collection::CollectedTestCategory<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
  }

  fn report_test_start(
    &self,
    _test: &file_test_runner::collection::CollectedTest<TData>,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
  }

  fn report_test_end(
    &self,
    test: &file_test_runner::collection::CollectedTest<TData>,
    duration: Duration,
    result: &TestResult,
    _context: &file_test_runner::reporter::ReporterContext,
  ) {
    let mut data = self.data.lock();

    let relative_path = test
      .path
      .strip_prefix(crate::root_path())
      .unwrap_or(&test.path);
    let path = match test.line_and_column {
      Some((line, col)) => {
        format!("{}:{}:{}", relative_path.display(), line + 1, col + 1)
      }
      None => relative_path.display().to_string(),
    }
    .replace("\\", "/");

    // Use the helper function to recursively flatten subtests
    self.flatten_and_record_test(
      &mut data.tests,
      test.name.to_string(),
      path,
      result,
      Some(duration),
    );
  }

  fn report_failures(
    &self,
    _failures: &[file_test_runner::reporter::ReporterFailure<TData>],
    _total_tests: usize,
  ) {
    // Write the report to file when failures are reported (at the end of test run)
    self.write_report_to_file();
  }
}

pub trait ReporterData {
  fn times_flaky() -> usize;
}

pub fn get_test_reporter<TData: 'static>(
  test_module_name: &str,
  flaky_test_tracker: Arc<FlakyTestTracker>,
) -> Arc<dyn file_test_runner::reporter::Reporter<TData>> {
  let mut reporters: Vec<Box<dyn file_test_runner::reporter::Reporter<TData>>> =
    Vec::with_capacity(2);
  reporters.push(get_display_reporter());
  if *IS_CI {
    reporters.push(Box::new(JsonReporter::new(
      flaky_test_tracker,
      test_module_name.to_string(),
    )));
  }
  Arc::new(file_test_runner::reporter::AggregateReporter::new(
    reporters,
  ))
}

fn get_display_reporter<TData>()
-> Box<dyn file_test_runner::reporter::Reporter<TData>> {
  if *file_test_runner::NO_CAPTURE
    || *IS_CI
    || !std::io::stderr().is_terminal()
    || std::env::var("DENO_TEST_UTIL_REPORTER").ok().as_deref() == Some("log")
  {
    Box::new(file_test_runner::reporter::LogReporter::default())
  } else {
    Box::new(PtyReporter::new())
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
  hide: bool,
}

impl PtyReporterData {
  pub fn render_clear(&mut self) -> String {
    self.static_text.render_clear().unwrap_or_default()
  }

  pub fn render(&mut self) -> Option<String> {
    if self.hide {
      return Some(self.render_clear());
    }
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
      hide: false,
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
    data.hide = false;
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
    let clear_text = {
      let mut data = self.data.lock();
      data.hide = true;
      data.render_clear()
    };
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
