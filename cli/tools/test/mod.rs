// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt::Write as _;
use std::future::poll_fn;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::File;
use deno_config::glob::FilePatterns;
use deno_config::glob::WalkEntry;
use deno_core::anyhow;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::located_script_name;
use deno_core::serde_v8;
use deno_core::stats::RuntimeActivity;
use deno_core::stats::RuntimeActivityDiff;
use deno_core::stats::RuntimeActivityStats;
use deno_core::stats::RuntimeActivityStatsFactory;
use deno_core::stats::RuntimeActivityStatsFilter;
use deno_core::stats::RuntimeActivityType;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_error::JsErrorBox;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::tokio_util::create_and_run_current_thread;
use deno_runtime::worker::MainWorker;
use deno_runtime::WorkerExecutionMode;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::Level;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::Deserialize;
use tokio::signal;
use tokio::sync::mpsc::UnboundedSender;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TestFlags;
use crate::args::TestReporterConfig;
use crate::colors;
use crate::display;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_container::CheckSpecifiersOptions;
use crate::graph_util::has_graph_root_local_dependent_changed;
use crate::ops;
use crate::sys::CliSys;
use crate::util::extract::extract_doc_tests;
use crate::util::file_watcher;
use crate::util::fs::collect_specifiers;
use crate::util::path::get_extension;
use crate::util::path::is_script_ext;
use crate::util::path::matches_pattern_or_exact_path;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CoverageCollector;
use crate::worker::CreateCustomWorkerError;

mod channel;
pub mod fmt;
pub mod reporters;

pub use channel::create_single_test_event_channel;
pub use channel::create_test_event_channel;
pub use channel::TestEventReceiver;
pub use channel::TestEventSender;
pub use channel::TestEventWorkerSender;
use fmt::format_sanitizer_diff;
pub use fmt::format_test_error;
use reporters::CompoundTestReporter;
use reporters::DotTestReporter;
use reporters::JunitTestReporter;
use reporters::PrettyTestReporter;
use reporters::TapTestReporter;
use reporters::TestReporter;

use crate::tools::coverage::cover_files;
use crate::tools::coverage::reporter;
use crate::tools::test::channel::ChannelClosedError;

/// How many times we're allowed to spin the event loop before considering something a leak.
const MAX_SANITIZER_LOOP_SPINS: usize = 16;

#[derive(Default)]
struct TopLevelSanitizerStats {
  map: HashMap<(RuntimeActivityType, Cow<'static, str>), usize>,
}

fn get_sanitizer_item(
  activity: RuntimeActivity,
) -> (RuntimeActivityType, Cow<'static, str>) {
  let activity_type = activity.activity();
  match activity {
    RuntimeActivity::AsyncOp(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Resource(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Interval(_, _) => (activity_type, "".into()),
    RuntimeActivity::Timer(_, _) => (activity_type, "".into()),
  }
}

fn get_sanitizer_item_ref(
  activity: &RuntimeActivity,
) -> (RuntimeActivityType, Cow<str>) {
  let activity_type = activity.activity();
  match activity {
    RuntimeActivity::AsyncOp(_, _, name) => (activity_type, (*name).into()),
    RuntimeActivity::Resource(_, _, name) => (activity_type, name.into()),
    RuntimeActivity::Interval(_, _) => (activity_type, "".into()),
    RuntimeActivity::Timer(_, _) => (activity_type, "".into()),
  }
}

/// The test mode is used to determine how a specifier is to be tested.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TestMode {
  /// Test as documentation, type-checking fenced code blocks.
  Documentation,
  /// Test as an executable module, loading the module into the isolate and running each test it
  /// defines.
  Executable,
  /// Test as both documentation and an executable module.
  Both,
}

impl TestMode {
  /// Returns `true` if the test mode indicates that code snippet extraction is
  /// needed.
  fn needs_test_extraction(&self) -> bool {
    matches!(self, Self::Documentation | Self::Both)
  }

  /// Returns `true` if the test mode indicates that the test should be
  /// type-checked and run.
  fn needs_test_run(&self) -> bool {
    matches!(self, Self::Executable | Self::Both)
  }
}

#[derive(Clone, Debug, Default)]
pub struct TestFilter {
  pub substring: Option<String>,
  pub regex: Option<Regex>,
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
}

impl TestFilter {
  pub fn includes(&self, name: &String) -> bool {
    if let Some(substring) = &self.substring {
      if !name.contains(substring) {
        return false;
      }
    }
    if let Some(regex) = &self.regex {
      if !regex.is_match(name) {
        return false;
      }
    }
    if let Some(include) = &self.include {
      if !include.contains(name) {
        return false;
      }
    }
    if self.exclude.contains(name) {
      return false;
    }
    true
  }

  pub fn from_flag(flag: &Option<String>) -> Self {
    let mut substring = None;
    let mut regex = None;
    if let Some(flag) = flag {
      if flag.starts_with('/') && flag.ends_with('/') {
        let rs = flag.trim_start_matches('/').trim_end_matches('/');
        regex =
          Some(Regex::new(rs).unwrap_or_else(|_| Regex::new("$^").unwrap()));
      } else {
        substring = Some(flag.clone());
      }
    }
    Self {
      substring,
      regex,
      ..Default::default()
    }
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestLocation {
  pub file_name: String,
  pub line_number: u32,
  pub column_number: u32,
}

#[derive(Default)]
pub(crate) struct TestContainer(
  TestDescriptions,
  Vec<v8::Global<v8::Function>>,
);

impl TestContainer {
  pub fn register(
    &mut self,
    description: TestDescription,
    function: v8::Global<v8::Function>,
  ) {
    self.0.tests.insert(description.id, description);
    self.1.push(function)
  }

  pub fn is_empty(&self) -> bool {
    self.1.is_empty()
  }
}

#[derive(Default, Debug)]
pub struct TestDescriptions {
  tests: IndexMap<usize, TestDescription>,
}

impl TestDescriptions {
  pub fn len(&self) -> usize {
    self.tests.len()
  }

  pub fn is_empty(&self) -> bool {
    self.tests.is_empty()
  }
}

impl<'a> IntoIterator for &'a TestDescriptions {
  type Item = <&'a IndexMap<usize, TestDescription> as IntoIterator>::Item;
  type IntoIter =
    <&'a IndexMap<usize, TestDescription> as IntoIterator>::IntoIter;
  fn into_iter(self) -> Self::IntoIter {
    (&self.tests).into_iter()
  }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
  pub id: usize,
  pub name: String,
  pub ignore: bool,
  pub only: bool,
  pub origin: String,
  pub location: TestLocation,
  pub sanitize_ops: bool,
  pub sanitize_resources: bool,
}

/// May represent a failure of a test or test step.
#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestFailureDescription {
  pub id: usize,
  pub name: String,
  pub origin: String,
  pub location: TestLocation,
}

impl From<&TestDescription> for TestFailureDescription {
  fn from(value: &TestDescription) -> Self {
    Self {
      id: value.id,
      name: value.name.clone(),
      origin: value.origin.clone(),
      location: value.location.clone(),
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TestFailureFormatOptions {
  pub hide_stacktraces: bool,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestFailure {
  JsError(Box<JsError>),
  FailedSteps(usize),
  IncompleteSteps,
  Leaked(Vec<String>, Vec<String>), // Details, trailer notes
  // The rest are for steps only.
  Incomplete,
  OverlapsWithSanitizers(IndexSet<String>), // Long names of overlapped tests
  HasSanitizersAndOverlaps(IndexSet<String>), // Long names of overlapped tests
}

impl TestFailure {
  pub fn format(
    &self,
    options: &TestFailureFormatOptions,
  ) -> Cow<'static, str> {
    match self {
      TestFailure::JsError(js_error) => {
        Cow::Owned(format_test_error(js_error, options))
      }
      TestFailure::FailedSteps(1) => Cow::Borrowed("1 test step failed."),
      TestFailure::FailedSteps(n) => {
        Cow::Owned(format!("{} test steps failed.", n))
      }
      TestFailure::IncompleteSteps => {
        Cow::Borrowed("Completed while steps were still running. Ensure all steps are awaited with `await t.step(...)`.")
      }
      TestFailure::Incomplete => {
        Cow::Borrowed("Didn't complete before parent. Await step with `await t.step(...)`.")
      }
      TestFailure::Leaked(details, trailer_notes) => {
        let mut f = String::new();
        write!(f, "Leaks detected:").ok();
        for detail in details {
          write!(f, "\n  - {}", detail).ok();
        }
        for trailer in trailer_notes {
          write!(f, "\n{}", trailer).ok();
        }
        Cow::Owned(f)
      }
      TestFailure::OverlapsWithSanitizers(long_names) => {
        let mut f = String::new();
        write!(f, "Started test step while another test step with sanitizers was running:").ok();
        for long_name in long_names {
          write!(f, "\n  * {}", long_name).ok();
        }
        Cow::Owned(f)
      }
      TestFailure::HasSanitizersAndOverlaps(long_names) => {
        let mut f = String::new();
        write!(f, "Started test step with sanitizers while another test step was running:").ok();
        for long_name in long_names {
          write!(f, "\n  * {}", long_name).ok();
        }
        Cow::Owned(f)
      }
    }
  }

  pub fn overview(&self) -> String {
    match self {
      TestFailure::JsError(js_error) => js_error.exception_message.clone(),
      TestFailure::FailedSteps(1) => "1 test step failed".to_string(),
      TestFailure::FailedSteps(n) => format!("{n} test steps failed"),
      TestFailure::IncompleteSteps => {
        "Completed while steps were still running".to_string()
      }
      TestFailure::Incomplete => "Didn't complete before parent".to_string(),
      TestFailure::Leaked(_, _) => "Leaks detected".to_string(),
      TestFailure::OverlapsWithSanitizers(_) => {
        "Started test step while another test step with sanitizers was running"
          .to_string()
      }
      TestFailure::HasSanitizersAndOverlaps(_) => {
        "Started test step with sanitizers while another test step was running"
          .to_string()
      }
    }
  }

  fn format_label(&self) -> String {
    match self {
      TestFailure::Incomplete => colors::gray("INCOMPLETE").to_string(),
      _ => colors::red("FAILED").to_string(),
    }
  }

  fn format_inline_summary(&self) -> Option<String> {
    match self {
      TestFailure::FailedSteps(1) => Some("due to 1 failed step".to_string()),
      TestFailure::FailedSteps(n) => Some(format!("due to {} failed steps", n)),
      TestFailure::IncompleteSteps => {
        Some("due to incomplete steps".to_string())
      }
      _ => None,
    }
  }

  fn hide_in_summary(&self) -> bool {
    // These failure variants are hidden in summaries because they are caused
    // by child errors that will be summarized separately.
    matches!(
      self,
      TestFailure::FailedSteps(_) | TestFailure::IncompleteSteps
    )
  }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResult {
  Ok,
  Ignored,
  Failed(TestFailure),
  Cancelled,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStepDescription {
  pub id: usize,
  pub name: String,
  pub origin: String,
  pub location: TestLocation,
  pub level: usize,
  pub parent_id: usize,
  pub root_id: usize,
  pub root_name: String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStepResult {
  Ok,
  Ignored,
  Failed(TestFailure),
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPlan {
  pub origin: String,
  pub total: usize,
  pub filtered_out: usize,
  pub used_only: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize)]
pub enum TestStdioStream {
  Stdout,
  Stderr,
}

#[derive(Debug)]
pub enum TestEvent {
  Register(Arc<TestDescriptions>),
  Plan(TestPlan),
  Wait(usize),
  Output(Vec<u8>),
  Slow(usize, u64),
  Result(usize, TestResult, u64),
  UncaughtError(String, Box<JsError>),
  StepRegister(TestStepDescription),
  StepWait(usize),
  StepResult(usize, TestStepResult, u64),
  /// Indicates that this worker has completed running tests.
  Completed,
  /// Indicates that the user has cancelled the test run with Ctrl+C and
  /// the run should be aborted.
  Sigint,
  /// Used by the REPL to force a report to end without closing the worker
  /// or receiver.
  ForceEndReport,
}

impl TestEvent {
  // Certain messages require us to ensure that all output has been drained to ensure proper
  // interleaving of output messages.
  pub fn requires_stdio_sync(&self) -> bool {
    matches!(
      self,
      TestEvent::Plan(..)
        | TestEvent::Result(..)
        | TestEvent::StepWait(..)
        | TestEvent::StepResult(..)
        | TestEvent::UncaughtError(..)
        | TestEvent::ForceEndReport
        | TestEvent::Completed
    )
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSummary {
  pub total: usize,
  pub passed: usize,
  pub failed: usize,
  pub ignored: usize,
  pub passed_steps: usize,
  pub failed_steps: usize,
  pub ignored_steps: usize,
  pub filtered_out: usize,
  pub measured: usize,
  pub failures: Vec<(TestFailureDescription, TestFailure)>,
  pub uncaught_errors: Vec<(String, Box<JsError>)>,
}

#[derive(Debug, Clone)]
struct TestSpecifiersOptions {
  cwd: Url,
  concurrent_jobs: NonZeroUsize,
  fail_fast: Option<NonZeroUsize>,
  log_level: Option<log::Level>,
  filter: bool,
  specifier: TestSpecifierOptions,
  reporter: TestReporterConfig,
  junit_path: Option<String>,
  hide_stacktraces: bool,
}

#[derive(Debug, Default, Clone)]
pub struct TestSpecifierOptions {
  pub shuffle: Option<u64>,
  pub filter: TestFilter,
  pub trace_leaks: bool,
}

impl TestSummary {
  pub fn new() -> TestSummary {
    TestSummary {
      total: 0,
      passed: 0,
      failed: 0,
      ignored: 0,
      passed_steps: 0,
      failed_steps: 0,
      ignored_steps: 0,
      filtered_out: 0,
      measured: 0,
      failures: Vec::new(),
      uncaught_errors: Vec::new(),
    }
  }

  fn has_failed(&self) -> bool {
    self.failed > 0 || !self.failures.is_empty()
  }
}

fn get_test_reporter(options: &TestSpecifiersOptions) -> Box<dyn TestReporter> {
  let parallel = options.concurrent_jobs.get() > 1;
  let failure_format_options = TestFailureFormatOptions {
    hide_stacktraces: options.hide_stacktraces,
  };
  let reporter: Box<dyn TestReporter> = match &options.reporter {
    TestReporterConfig::Dot => Box::new(DotTestReporter::new(
      options.cwd.clone(),
      failure_format_options,
    )),
    TestReporterConfig::Pretty => Box::new(PrettyTestReporter::new(
      parallel,
      options.log_level != Some(Level::Error),
      options.filter,
      false,
      options.cwd.clone(),
      failure_format_options,
    )),
    TestReporterConfig::Junit => Box::new(JunitTestReporter::new(
      options.cwd.clone(),
      "-".to_string(),
      failure_format_options,
    )),
    TestReporterConfig::Tap => Box::new(TapTestReporter::new(
      options.cwd.clone(),
      options.concurrent_jobs > NonZeroUsize::new(1).unwrap(),
      failure_format_options,
    )),
  };

  if let Some(junit_path) = &options.junit_path {
    let junit = Box::new(JunitTestReporter::new(
      options.cwd.clone(),
      junit_path.to_string(),
      TestFailureFormatOptions {
        hide_stacktraces: options.hide_stacktraces,
      },
    ));
    return Box::new(CompoundTestReporter::new(vec![reporter, junit]));
  }

  reporter
}

async fn configure_main_worker(
  worker_factory: Arc<CliMainWorkerFactory>,
  specifier: &Url,
  permissions_container: PermissionsContainer,
  worker_sender: TestEventWorkerSender,
  options: &TestSpecifierOptions,
  sender: UnboundedSender<jupyter_runtime::messaging::content::StreamContent>,
) -> Result<
  (Option<Box<dyn CoverageCollector>>, MainWorker),
  CreateCustomWorkerError,
> {
  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Test,
      specifier.clone(),
      permissions_container,
      vec![
        ops::testing::deno_test::init_ops(worker_sender.sender),
        ops::lint::deno_lint_ext_for_test::init_ops(),
        ops::jupyter::deno_jupyter_for_test::init_ops(sender),
      ],
      Stdio {
        stdin: StdioPipe::inherit(),
        stdout: StdioPipe::file(worker_sender.stdout),
        stderr: StdioPipe::file(worker_sender.stderr),
      },
    )
    .await?;
  let coverage_collector = worker.maybe_setup_coverage_collector().await?;
  if options.trace_leaks {
    worker.execute_script_static(
      located_script_name!(),
      "Deno[Deno.internal].core.setLeakTracingEnabled(true);",
    )?;
  }
  let res = worker.execute_side_module().await;
  let worker = worker.into_main_worker();
  match res {
    Ok(()) => Ok(()),
    Err(CoreError::Js(err)) => {
      send_test_event(
        &worker.js_runtime.op_state(),
        TestEvent::UncaughtError(specifier.to_string(), Box::new(err)),
      )
      .map_err(|e| CoreError::JsBox(JsErrorBox::from_err(e)))?;
      Ok(())
    }
    Err(err) => Err(err),
  }?;
  Ok((coverage_collector, worker))
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
pub async fn test_specifier(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions_container: PermissionsContainer,
  specifier: ModuleSpecifier,
  worker_sender: TestEventWorkerSender,
  fail_fast_tracker: FailFastTracker,
  options: TestSpecifierOptions,
) -> Result<(), AnyError> {
  if fail_fast_tracker.should_stop() {
    return Ok(());
  }
  let jupyter_channel = tokio::sync::mpsc::unbounded_channel();
  let (coverage_collector, mut worker) = configure_main_worker(
    worker_factory,
    &specifier,
    permissions_container,
    worker_sender,
    &options,
    jupyter_channel.0,
  )
  .await?;

  match test_specifier_inner(
    &mut worker,
    coverage_collector,
    specifier.clone(),
    fail_fast_tracker,
    options,
  )
  .await
  {
    Ok(()) => Ok(()),
    Err(TestSpecifierError::Core(CoreError::Js(err))) => {
      send_test_event(
        &worker.js_runtime.op_state(),
        TestEvent::UncaughtError(specifier.to_string(), Box::new(err)),
      )?;
      Ok(())
    }
    Err(e) => Err(e.into()),
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TestSpecifierError {
  #[class(inherit)]
  #[error(transparent)]
  Core(#[from] CoreError),
  #[class(inherit)]
  #[error(transparent)]
  RunTestsForWorker(#[from] RunTestsForWorkerErr),
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
#[allow(clippy::too_many_arguments)]
async fn test_specifier_inner(
  worker: &mut MainWorker,
  mut coverage_collector: Option<Box<dyn CoverageCollector>>,
  specifier: ModuleSpecifier,
  fail_fast_tracker: FailFastTracker,
  options: TestSpecifierOptions,
) -> Result<(), TestSpecifierError> {
  // Ensure that there are no pending exceptions before we start running tests
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  worker.dispatch_load_event().map_err(CoreError::Js)?;

  run_tests_for_worker(worker, &specifier, &options, &fail_fast_tracker)
    .await?;

  // Ignore `defaultPrevented` of the `beforeunload` event. We don't allow the
  // event loop to continue beyond what's needed to await results.
  worker
    .dispatch_beforeunload_event()
    .map_err(CoreError::Js)?;
  worker.dispatch_unload_event().map_err(CoreError::Js)?;

  // Ensure all output has been flushed
  _ = worker
    .js_runtime
    .op_state()
    .borrow_mut()
    .borrow_mut::<TestEventSender>()
    .flush();

  // Ensure the worker has settled so we can catch any remaining unhandled rejections. We don't
  // want to wait forever here.
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  if let Some(coverage_collector) = &mut coverage_collector {
    worker
      .js_runtime
      .with_event_loop_future(
        coverage_collector.stop_collecting().boxed_local(),
        PollEventLoopOptions::default(),
      )
      .await?;
  }
  Ok(())
}

pub fn worker_has_tests(worker: &mut MainWorker) -> bool {
  let state_rc = worker.js_runtime.op_state();
  let state = state_rc.borrow();
  !state.borrow::<TestContainer>().is_empty()
}

/// Yields to tokio to allow async work to process, and then polls
/// the event loop once.
#[must_use = "The event loop result should be checked"]
pub async fn poll_event_loop(worker: &mut MainWorker) -> Result<(), CoreError> {
  // Allow any ops that to do work in the tokio event loop to do so
  tokio::task::yield_now().await;
  // Spin the event loop once
  poll_fn(|cx| {
    if let Poll::Ready(Err(err)) = worker
      .js_runtime
      .poll_event_loop(cx, PollEventLoopOptions::default())
    {
      return Poll::Ready(Err(err));
    }
    Poll::Ready(Ok(()))
  })
  .await
}

pub fn send_test_event(
  op_state: &RefCell<OpState>,
  event: TestEvent,
) -> Result<(), ChannelClosedError> {
  op_state
    .borrow_mut()
    .borrow_mut::<TestEventSender>()
    .send(event)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum RunTestsForWorkerErr {
  #[class(inherit)]
  #[error(transparent)]
  ChannelClosed(#[from] ChannelClosedError),
  #[class(inherit)]
  #[error(transparent)]
  Core(#[from] CoreError),
  #[class(inherit)]
  #[error(transparent)]
  SerdeV8(#[from] serde_v8::Error),
}

pub async fn run_tests_for_worker(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
  options: &TestSpecifierOptions,
  fail_fast_tracker: &FailFastTracker,
) -> Result<(), RunTestsForWorkerErr> {
  let state_rc = worker.js_runtime.op_state();
  // Take whatever tests have been registered
  let TestContainer(tests, test_functions) =
    std::mem::take(&mut *state_rc.borrow_mut().borrow_mut::<TestContainer>());

  let tests: Arc<TestDescriptions> = tests.into();
  send_test_event(&state_rc, TestEvent::Register(tests.clone()))?;
  let res = run_tests_for_worker_inner(
    worker,
    specifier,
    tests,
    test_functions,
    options,
    fail_fast_tracker,
  )
  .await;

  _ = send_test_event(&state_rc, TestEvent::Completed);
  res
}

async fn run_tests_for_worker_inner(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
  tests: Arc<TestDescriptions>,
  test_functions: Vec<v8::Global<v8::Function>>,
  options: &TestSpecifierOptions,
  fail_fast_tracker: &FailFastTracker,
) -> Result<(), RunTestsForWorkerErr> {
  let unfiltered = tests.len();
  let state_rc = worker.js_runtime.op_state();

  // Build the test plan in a single pass
  let mut tests_to_run = Vec::with_capacity(tests.len());
  let mut used_only = false;
  for ((_, d), f) in tests.tests.iter().zip(test_functions) {
    if !options.filter.includes(&d.name) {
      continue;
    }

    // If we've seen an "only: true" test, the remaining tests must be "only: true" to be added
    if used_only && !d.only {
      continue;
    }

    // If this is the first "only: true" test we've seen, clear the other tests since they were
    // only: false.
    if d.only && !used_only {
      used_only = true;
      tests_to_run.clear();
    }
    tests_to_run.push((d, f));
  }

  if let Some(seed) = options.shuffle {
    tests_to_run.shuffle(&mut SmallRng::seed_from_u64(seed));
  }

  send_test_event(
    &state_rc,
    TestEvent::Plan(TestPlan {
      origin: specifier.to_string(),
      total: tests_to_run.len(),
      filtered_out: unfiltered - tests_to_run.len(),
      used_only,
    }),
  )?;

  let mut had_uncaught_error = false;
  let stats = worker.js_runtime.runtime_activity_stats_factory();
  let ops = worker.js_runtime.op_names();

  // These particular ops may start and stop independently of tests, so we just filter them out
  // completely.
  let op_id_host_recv_message = ops
    .iter()
    .position(|op| *op == "op_host_recv_message")
    .unwrap();
  let op_id_host_recv_ctrl = ops
    .iter()
    .position(|op| *op == "op_host_recv_ctrl")
    .unwrap();

  // For consistency between tests with and without sanitizers, we _always_ include
  // the actual sanitizer capture before and after a test, but a test that ignores resource
  // or op sanitization simply doesn't throw if one of these constraints is violated.
  let mut filter = RuntimeActivityStatsFilter::default();
  filter = filter.with_resources();
  filter = filter.with_ops();
  filter = filter.with_timers();
  filter = filter.omit_op(op_id_host_recv_ctrl as _);
  filter = filter.omit_op(op_id_host_recv_message as _);

  // Count the top-level stats so we can filter them out if they complete and restart within
  // a test.
  let top_level_stats = stats.clone().capture(&filter);
  let mut top_level = TopLevelSanitizerStats::default();
  for activity in top_level_stats.dump().active {
    top_level
      .map
      .entry(get_sanitizer_item(activity))
      .and_modify(|n| *n += 1)
      .or_insert(1);
  }

  for (desc, function) in tests_to_run.into_iter() {
    if fail_fast_tracker.should_stop() {
      break;
    }

    // Each test needs a fresh reqwest connection pool to avoid inter-test weirdness with connections
    // failing. If we don't do this, a connection to a test server we just tore down might be re-used in
    // the next test.
    // TODO(mmastrac): this should be some sort of callback that we can implement for any subsystem
    worker
      .js_runtime
      .op_state()
      .borrow_mut()
      .try_take::<deno_runtime::deno_fetch::Client>();

    if desc.ignore {
      send_test_event(
        &state_rc,
        TestEvent::Result(desc.id, TestResult::Ignored, 0),
      )?;
      continue;
    }
    if had_uncaught_error {
      send_test_event(
        &state_rc,
        TestEvent::Result(desc.id, TestResult::Cancelled, 0),
      )?;
      continue;
    }
    send_test_event(&state_rc, TestEvent::Wait(desc.id))?;

    // Poll event loop once, to allow all ops that are already resolved, but haven't
    // responded to settle.
    // TODO(mmastrac): we should provide an API to poll the event loop until no further
    // progress is made.
    poll_event_loop(worker).await?;

    // We always capture stats, regardless of sanitization state
    let before = stats.clone().capture(&filter);

    let earlier = Instant::now();
    let call = worker.js_runtime.call(&function);

    let slow_state_rc = state_rc.clone();
    let slow_test_id = desc.id;
    let slow_test_warning = spawn(async move {
      // The slow test warning should pop up every DENO_SLOW_TEST_TIMEOUT*(2**n) seconds,
      // with a duration that is doubling each time. So for a warning time of 60s,
      // we should get a warning at 60s, 120s, 240s, etc.
      let base_timeout = env::var("DENO_SLOW_TEST_TIMEOUT").unwrap_or_default();
      let base_timeout = base_timeout.parse().unwrap_or(60).max(1);
      let mut multiplier = 1;
      let mut elapsed = 0;
      loop {
        tokio::time::sleep(Duration::from_secs(
          base_timeout * (multiplier - elapsed),
        ))
        .await;
        if send_test_event(
          &slow_state_rc,
          TestEvent::Slow(
            slow_test_id,
            Duration::from_secs(base_timeout * multiplier).as_millis() as _,
          ),
        )
        .is_err()
        {
          break;
        }
        multiplier *= 2;
        elapsed += 1;
      }
    });

    let result = worker
      .js_runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await;
    slow_test_warning.abort();
    let result = match result {
      Ok(r) => r,
      Err(error) => {
        if let CoreError::Js(js_error) = error {
          send_test_event(
            &state_rc,
            TestEvent::UncaughtError(specifier.to_string(), Box::new(js_error)),
          )?;
          fail_fast_tracker.add_failure();
          send_test_event(
            &state_rc,
            TestEvent::Result(desc.id, TestResult::Cancelled, 0),
          )?;
          had_uncaught_error = true;
          continue;
        } else {
          return Err(error.into());
        }
      }
    };

    // Check the result before we check for leaks
    let result = {
      let scope = &mut worker.js_runtime.handle_scope();
      let result = v8::Local::new(scope, result);
      serde_v8::from_v8::<TestResult>(scope, result)?
    };
    if matches!(result, TestResult::Failed(_)) {
      fail_fast_tracker.add_failure();
      let elapsed = earlier.elapsed().as_millis();
      send_test_event(
        &state_rc,
        TestEvent::Result(desc.id, result, elapsed as u64),
      )?;
      continue;
    }

    // Await activity stabilization
    if let Some(diff) = wait_for_activity_to_stabilize(
      worker,
      &stats,
      &filter,
      &top_level,
      before,
      desc.sanitize_ops,
      desc.sanitize_resources,
    )
    .await?
    {
      let (formatted, trailer_notes) = format_sanitizer_diff(diff);
      if !formatted.is_empty() {
        let failure = TestFailure::Leaked(formatted, trailer_notes);
        fail_fast_tracker.add_failure();
        let elapsed = earlier.elapsed().as_millis();
        send_test_event(
          &state_rc,
          TestEvent::Result(
            desc.id,
            TestResult::Failed(failure),
            elapsed as u64,
          ),
        )?;
        continue;
      }
    }

    let elapsed = earlier.elapsed().as_millis();
    send_test_event(
      &state_rc,
      TestEvent::Result(desc.id, result, elapsed as u64),
    )?;
  }
  Ok(())
}

/// The sanitizer must ignore ops, resources and timers that were started at the top-level, but
/// completed and restarted, replacing themselves with the same "thing". For example, if you run a
/// `Deno.serve` server at the top level and make fetch requests to it during the test, those ops
/// should not count as completed during the test because they are immediately replaced.
fn is_empty(
  top_level: &TopLevelSanitizerStats,
  diff: &RuntimeActivityDiff,
) -> bool {
  // If the diff is empty, return empty
  if diff.is_empty() {
    return true;
  }

  // If the # of appeared != # of disappeared, we can exit fast with not empty
  if diff.appeared.len() != diff.disappeared.len() {
    return false;
  }

  // If there are no top-level ops and !diff.is_empty(), we can exit fast with not empty
  if top_level.map.is_empty() {
    return false;
  }

  // Otherwise we need to calculate replacement for top-level stats. Sanitizers will not fire
  // if an op, resource or timer is replaced and has a corresponding top-level op.
  let mut map = HashMap::new();
  for item in &diff.appeared {
    let item = get_sanitizer_item_ref(item);
    let Some(n1) = top_level.map.get(&item) else {
      return false;
    };
    let n2 = map.entry(item).and_modify(|n| *n += 1).or_insert(1);
    // If more ops appeared than were created at the top-level, return false
    if *n2 > *n1 {
      return false;
    }
  }

  // We know that we replaced no more things than were created at the top-level. So now we just want
  // to make sure that whatever thing was created has a corresponding disappearance record.
  for item in &diff.disappeared {
    let item = get_sanitizer_item_ref(item);
    // If more things of this type disappeared than appeared, return false
    let Some(n1) = map.get_mut(&item) else {
      return false;
    };
    *n1 -= 1;
    if *n1 == 0 {
      map.remove(&item);
    }
  }

  // If everything is accounted for, we are empty
  map.is_empty()
}

async fn wait_for_activity_to_stabilize(
  worker: &mut MainWorker,
  stats: &RuntimeActivityStatsFactory,
  filter: &RuntimeActivityStatsFilter,
  top_level: &TopLevelSanitizerStats,
  before: RuntimeActivityStats,
  sanitize_ops: bool,
  sanitize_resources: bool,
) -> Result<Option<RuntimeActivityDiff>, CoreError> {
  // First, check to see if there's any diff at all. If not, just continue.
  let after = stats.clone().capture(filter);
  let mut diff = RuntimeActivityStats::diff(&before, &after);
  if is_empty(top_level, &diff) {
    // No activity, so we return early
    return Ok(None);
  }

  // We allow for up to MAX_SANITIZER_LOOP_SPINS to get to a point where there is no difference.
  // TODO(mmastrac): We could be much smarter about this if we had the concept of "progress" in
  // an event loop tick. Ideally we'd be able to tell if we were spinning and doing nothing, or
  // spinning and resolving ops.
  for _ in 0..MAX_SANITIZER_LOOP_SPINS {
    // There was a diff, so let the event loop run once
    poll_event_loop(worker).await?;

    let after = stats.clone().capture(filter);
    diff = RuntimeActivityStats::diff(&before, &after);
    if is_empty(top_level, &diff) {
      return Ok(None);
    }
  }

  if !sanitize_ops {
    diff
      .appeared
      .retain(|activity| !matches!(activity, RuntimeActivity::AsyncOp(..)));
    diff
      .disappeared
      .retain(|activity| !matches!(activity, RuntimeActivity::AsyncOp(..)));
  }
  if !sanitize_resources {
    diff
      .appeared
      .retain(|activity| !matches!(activity, RuntimeActivity::Resource(..)));
    diff
      .disappeared
      .retain(|activity| !matches!(activity, RuntimeActivity::Resource(..)));
  }

  // Since we don't have an option to disable timer sanitization, we use sanitize_ops == false &&
  // sanitize_resources == false to disable those.
  if !sanitize_ops && !sanitize_resources {
    diff.appeared.retain(|activity| {
      !matches!(
        activity,
        RuntimeActivity::Timer(..) | RuntimeActivity::Interval(..)
      )
    });
    diff.disappeared.retain(|activity| {
      !matches!(
        activity,
        RuntimeActivity::Timer(..) | RuntimeActivity::Interval(..)
      )
    });
  }

  Ok(if is_empty(top_level, &diff) {
    None
  } else {
    Some(diff)
  })
}

static HAS_TEST_RUN_SIGINT_HANDLER: AtomicBool = AtomicBool::new(false);

/// Test a collection of specifiers with test modes concurrently.
async fn test_specifiers(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: &Permissions,
  permission_desc_parser: &Arc<RuntimePermissionDescriptorParser<CliSys>>,
  specifiers: Vec<ModuleSpecifier>,
  options: TestSpecifiersOptions,
) -> Result<(), AnyError> {
  let specifiers = if let Some(seed) = options.specifier.shuffle {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut specifiers = specifiers;
    specifiers.sort();
    specifiers.shuffle(&mut rng);
    specifiers
  } else {
    specifiers
  };

  let (test_event_sender_factory, receiver) = create_test_event_channel();
  let concurrent_jobs = options.concurrent_jobs;

  let mut cancel_sender = test_event_sender_factory.weak_sender();
  let sigint_handler_handle = spawn(async move {
    signal::ctrl_c().await.unwrap();
    cancel_sender.send(TestEvent::Sigint).ok();
  });
  HAS_TEST_RUN_SIGINT_HANDLER.store(true, Ordering::Relaxed);
  let reporter = get_test_reporter(&options);
  let fail_fast_tracker = FailFastTracker::new(options.fail_fast);

  let join_handles = specifiers.into_iter().map(move |specifier| {
    let worker_factory = worker_factory.clone();
    let permissions_container = PermissionsContainer::new(
      permission_desc_parser.clone(),
      permissions.clone(),
    );
    let worker_sender = test_event_sender_factory.worker();
    let fail_fast_tracker = fail_fast_tracker.clone();
    let specifier_options = options.specifier.clone();
    spawn_blocking(move || {
      create_and_run_current_thread(test_specifier(
        worker_factory,
        permissions_container,
        specifier,
        worker_sender,
        fail_fast_tracker,
        specifier_options,
      ))
    })
  });

  let join_stream = stream::iter(join_handles)
    .buffer_unordered(concurrent_jobs.get())
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let handler = spawn(async move { report_tests(receiver, reporter).await.0 });

  let (join_results, result) = future::join(join_stream, handler).await;
  sigint_handler_handle.abort();
  HAS_TEST_RUN_SIGINT_HANDLER.store(false, Ordering::Relaxed);
  for join_result in join_results {
    join_result??;
  }
  result??;

  Ok(())
}

/// Gives receiver back in case it was ended with `TestEvent::ForceEndReport`.
pub async fn report_tests(
  mut receiver: TestEventReceiver,
  mut reporter: Box<dyn TestReporter>,
) -> (Result<(), AnyError>, TestEventReceiver) {
  let mut tests = IndexMap::new();
  let mut test_steps = IndexMap::new();
  let mut tests_started = HashSet::new();
  let mut tests_with_result = HashSet::new();
  let mut start_time = None;
  let mut had_plan = false;
  let mut used_only = false;
  let mut failed = false;

  while let Some((_, event)) = receiver.recv().await {
    match event {
      TestEvent::Register(description) => {
        for (_, description) in description.into_iter() {
          reporter.report_register(description);
          // TODO(mmastrac): We shouldn't need to clone here -- we can reuse the descriptions everywhere
          tests.insert(description.id, description.clone());
        }
      }
      TestEvent::Plan(plan) => {
        if !had_plan {
          start_time = Some(Instant::now());
          had_plan = true;
        }
        if plan.used_only {
          used_only = true;
        }
        reporter.report_plan(&plan);
      }
      TestEvent::Wait(id) => {
        if tests_started.insert(id) {
          reporter.report_wait(tests.get(&id).unwrap());
        }
      }
      TestEvent::Output(output) => {
        reporter.report_output(&output);
      }
      TestEvent::Slow(id, elapsed) => {
        reporter.report_slow(tests.get(&id).unwrap(), elapsed);
      }
      TestEvent::Result(id, result, elapsed) => {
        if tests_with_result.insert(id) {
          match result {
            TestResult::Failed(_) | TestResult::Cancelled => {
              failed = true;
            }
            _ => (),
          }
          reporter.report_result(tests.get(&id).unwrap(), &result, elapsed);
        }
      }
      TestEvent::UncaughtError(origin, error) => {
        failed = true;
        reporter.report_uncaught_error(&origin, error);
      }
      TestEvent::StepRegister(description) => {
        reporter.report_step_register(&description);
        test_steps.insert(description.id, description);
      }
      TestEvent::StepWait(id) => {
        if tests_started.insert(id) {
          reporter.report_step_wait(test_steps.get(&id).unwrap());
        }
      }
      TestEvent::StepResult(id, result, duration) => {
        if tests_with_result.insert(id) {
          reporter.report_step_result(
            test_steps.get(&id).unwrap(),
            &result,
            duration,
            &tests,
            &test_steps,
          );
        }
      }
      TestEvent::ForceEndReport => {
        break;
      }
      TestEvent::Completed => {
        reporter.report_completed();
      }
      TestEvent::Sigint => {
        let elapsed = start_time
          .map(|t| Instant::now().duration_since(t))
          .unwrap_or_default();
        reporter.report_sigint(
          &tests_started
            .difference(&tests_with_result)
            .copied()
            .collect(),
          &tests,
          &test_steps,
        );

        #[allow(clippy::print_stderr)]
        if let Err(err) = reporter.flush_report(&elapsed, &tests, &test_steps) {
          eprint!("Test reporter failed to flush: {}", err)
        }
        #[allow(clippy::disallowed_methods)]
        std::process::exit(130);
      }
    }
  }

  let elapsed = start_time
    .map(|t| Instant::now().duration_since(t))
    .unwrap_or_default();
  reporter.report_summary(&elapsed, &tests, &test_steps);
  if let Err(err) = reporter.flush_report(&elapsed, &tests, &test_steps) {
    return (
      Err(anyhow!("Test reporter failed to flush: {}", err)),
      receiver,
    );
  }

  if used_only {
    return (
      Err(anyhow!("Test failed because the \"only\" option was used",)),
      receiver,
    );
  }

  if failed {
    return (Err(anyhow!("Test failed")), receiver);
  }

  (Ok(()), receiver)
}

fn is_supported_test_path_predicate(entry: WalkEntry) -> bool {
  if !is_script_ext(entry.path) {
    false
  } else if has_supported_test_path_name(entry.path) {
    true
  } else if let Some(include) = &entry.patterns.include {
    // allow someone to explicitly specify a path
    matches_pattern_or_exact_path(include, entry.path)
  } else {
    false
  }
}

/// Checks if the path has a basename and extension Deno supports for tests.
pub(crate) fn is_supported_test_path(path: &Path) -> bool {
  has_supported_test_path_name(path) && is_script_ext(path)
}

fn has_supported_test_path_name(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    if basename.ends_with("_test")
      || basename.ends_with(".test")
      || basename == "test"
    {
      return true;
    }

    path
      .components()
      .any(|seg| seg.as_os_str().to_str() == Some("__tests__"))
  } else {
    false
  }
}

/// Checks if the path has an extension Deno supports for tests.
fn is_supported_test_ext(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(
      ext.as_str(),
      "ts"
        | "tsx"
        | "js"
        | "jsx"
        | "mjs"
        | "mts"
        | "cjs"
        | "cts"
        | "md"
        | "mkd"
        | "mkdn"
        | "mdwn"
        | "mdown"
        | "markdown"
    )
  } else {
    false
  }
}

/// Collects specifiers marking them with the appropriate test mode while maintaining the natural
/// input order.
///
/// - Specifiers matching the `is_supported_test_ext` predicate are marked as
///   `TestMode::Documentation`.
/// - Specifiers matching the `is_supported_test_path` are marked as `TestMode::Executable`.
/// - Specifiers matching both predicates are marked as `TestMode::Both`
fn collect_specifiers_with_test_mode(
  cli_options: &CliOptions,
  files: FilePatterns,
  include_inline: &bool,
) -> Result<Vec<(ModuleSpecifier, TestMode)>, AnyError> {
  // todo(dsherret): there's no need to collect twice as it's slow
  let vendor_folder = cli_options.vendor_dir_path();
  let module_specifiers = collect_specifiers(
    files.clone(),
    vendor_folder.map(ToOwned::to_owned),
    is_supported_test_path_predicate,
  )?;

  if *include_inline {
    return collect_specifiers(
      files,
      vendor_folder.map(ToOwned::to_owned),
      |e| is_supported_test_ext(e.path),
    )
    .map(|specifiers| {
      specifiers
        .into_iter()
        .map(|specifier| {
          let mode = if module_specifiers.contains(&specifier) {
            TestMode::Both
          } else {
            TestMode::Documentation
          };

          (specifier, mode)
        })
        .collect()
    });
  }

  let specifiers_with_mode = module_specifiers
    .into_iter()
    .map(|specifier| (specifier, TestMode::Executable))
    .collect();

  Ok(specifiers_with_mode)
}

/// Collects module and document specifiers with test modes via
/// `collect_specifiers_with_test_mode` which are then pre-fetched and adjusted
/// based on the media type.
///
/// Specifiers that do not have a known media type that can be executed as a
/// module are marked as `TestMode::Documentation`. Type definition files
/// cannot be run, and therefore need to be marked as `TestMode::Documentation`
/// as well.
async fn fetch_specifiers_with_test_mode(
  cli_options: &CliOptions,
  file_fetcher: &CliFileFetcher,
  member_patterns: impl Iterator<Item = FilePatterns>,
  doc: &bool,
) -> Result<Vec<(ModuleSpecifier, TestMode)>, AnyError> {
  let mut specifiers_with_mode = member_patterns
    .map(|files| {
      collect_specifiers_with_test_mode(cli_options, files.clone(), doc)
    })
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

  for (specifier, mode) in &mut specifiers_with_mode {
    let file = file_fetcher.fetch_bypass_permissions(specifier).await?;

    let (media_type, _) = file.resolve_media_type_and_charset();
    if matches!(media_type, MediaType::Unknown | MediaType::Dts) {
      *mode = TestMode::Documentation
    }
  }

  Ok(specifiers_with_mode)
}

pub async fn run_tests(
  flags: Arc<Flags>,
  test_flags: TestFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let workspace_test_options =
    cli_options.resolve_workspace_test_options(&test_flags);
  let file_fetcher = factory.file_fetcher()?;
  // Various test files should not share the same permissions in terms of
  // `PermissionsContainer` - otherwise granting/revoking permissions in one
  // file would have impact on other files, which is undesirable.
  let permission_desc_parser = factory.permission_desc_parser()?;
  let permissions = Permissions::from_options(
    permission_desc_parser.as_ref(),
    &cli_options.permissions_options(),
  )?;
  let log_level = cli_options.log_level();

  let members_with_test_options =
    cli_options.resolve_test_options_for_members(&test_flags)?;
  let specifiers_with_mode = fetch_specifiers_with_test_mode(
    cli_options,
    file_fetcher,
    members_with_test_options.into_iter().map(|(_, v)| v.files),
    &workspace_test_options.doc,
  )
  .await?;

  if !workspace_test_options.permit_no_files && specifiers_with_mode.is_empty()
  {
    return Err(anyhow!("No test modules found"));
  }

  let doc_tests = get_doc_tests(&specifiers_with_mode, file_fetcher).await?;
  let specifiers_for_typecheck_and_test =
    get_target_specifiers(specifiers_with_mode, &doc_tests);
  for doc_test in doc_tests {
    file_fetcher.insert_memory_files(doc_test);
  }

  let main_graph_container = factory.main_module_graph_container().await?;

  // Typecheck
  main_graph_container
    .check_specifiers(
      &specifiers_for_typecheck_and_test,
      CheckSpecifiersOptions {
        ext_overwrite: cli_options.ext_flag().as_ref(),
        ..Default::default()
      },
    )
    .await?;

  if workspace_test_options.no_run {
    return Ok(());
  }

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);

  // Run tests
  test_specifiers(
    worker_factory,
    &permissions,
    permission_desc_parser,
    specifiers_for_typecheck_and_test,
    TestSpecifiersOptions {
      cwd: Url::from_directory_path(cli_options.initial_cwd()).map_err(
        |_| {
          anyhow!(
            "Unable to construct URL from the path of cwd: {}",
            cli_options.initial_cwd().to_string_lossy(),
          )
        },
      )?,
      concurrent_jobs: workspace_test_options.concurrent_jobs,
      fail_fast: workspace_test_options.fail_fast,
      log_level,
      filter: workspace_test_options.filter.is_some(),
      reporter: workspace_test_options.reporter,
      junit_path: workspace_test_options.junit_path,
      hide_stacktraces: workspace_test_options.hide_stacktraces,
      specifier: TestSpecifierOptions {
        filter: TestFilter::from_flag(&workspace_test_options.filter),
        shuffle: workspace_test_options.shuffle,
        trace_leaks: workspace_test_options.trace_leaks,
      },
    },
  )
  .await?;

  if test_flags.coverage_raw_data_only {
    return Ok(());
  }

  if let Some(ref coverage) = test_flags.coverage_dir {
    let reporters: [&dyn reporter::CoverageReporter; 3] = [
      &reporter::SummaryCoverageReporter::new(),
      &reporter::LcovCoverageReporter::new(),
      &reporter::HtmlCoverageReporter::new(),
    ];
    if let Err(err) = cover_files(
      flags,
      vec![coverage.clone()],
      vec![],
      vec![],
      vec![],
      Some(
        PathBuf::from(coverage)
          .join("lcov.info")
          .to_string_lossy()
          .to_string(),
      ),
      &reporters,
    ) {
      log::info!("Error generating coverage report: {}", err);
    }
  }

  Ok(())
}

pub async fn run_tests_with_watch(
  flags: Arc<Flags>,
  test_flags: TestFlags,
) -> Result<(), AnyError> {
  // On top of the sigint handlers which are added and unbound for each test
  // run, a process-scoped basic exit handler is required due to a tokio
  // limitation where it doesn't unbind its own handler for the entire process
  // once a user adds one.
  spawn(async move {
    loop {
      signal::ctrl_c().await.unwrap();
      if !HAS_TEST_RUN_SIGINT_HANDLER.load(Ordering::Relaxed) {
        #[allow(clippy::disallowed_methods)]
        std::process::exit(130);
      }
    }
  });

  file_watcher::watch_func(
    flags,
    file_watcher::PrintConfig::new(
      "Test",
      test_flags
        .watch
        .as_ref()
        .map(|w| !w.no_clear_screen)
        .unwrap_or(true),
    ),
    move |flags, watcher_communicator, changed_paths| {
      let test_flags = test_flags.clone();
      watcher_communicator.show_path_changed(changed_paths.clone());
      Ok(async move {
        let factory = CliFactory::from_flags_for_watcher(
          flags,
          watcher_communicator.clone(),
        );
        let cli_options = factory.cli_options()?;
        let workspace_test_options =
          cli_options.resolve_workspace_test_options(&test_flags);

        let _ = watcher_communicator.watch_paths(cli_options.watch_paths());
        let graph_kind = cli_options.type_check_mode().as_graph_kind();
        let log_level = cli_options.log_level();
        let cli_options = cli_options.clone();
        let module_graph_creator = factory.module_graph_creator().await?;
        let file_fetcher = factory.file_fetcher()?;
        let members_with_test_options =
          cli_options.resolve_test_options_for_members(&test_flags)?;
        let watch_paths = members_with_test_options
          .iter()
          .filter_map(|(_, test_options)| {
            test_options
              .files
              .include
              .as_ref()
              .map(|set| set.base_paths())
          })
          .flatten()
          .collect::<Vec<_>>();
        let _ = watcher_communicator.watch_paths(watch_paths);
        let test_modules = members_with_test_options
          .iter()
          .map(|(_, test_options)| {
            collect_specifiers(
              test_options.files.clone(),
              cli_options.vendor_dir_path().map(ToOwned::to_owned),
              if workspace_test_options.doc {
                Box::new(|e: WalkEntry| is_supported_test_ext(e.path))
                  as Box<dyn Fn(WalkEntry) -> bool>
              } else {
                Box::new(is_supported_test_path_predicate)
              },
            )
          })
          .collect::<Result<Vec<_>, _>>()?
          .into_iter()
          .flatten()
          .collect::<Vec<_>>();

        let permission_desc_parser = factory.permission_desc_parser()?;
        let permissions = Permissions::from_options(
          permission_desc_parser.as_ref(),
          &cli_options.permissions_options(),
        )?;
        let graph = module_graph_creator
          .create_graph(
            graph_kind,
            test_modules,
            crate::graph_util::NpmCachingStrategy::Eager,
          )
          .await?;
        module_graph_creator.graph_valid(&graph)?;
        let test_modules = &graph.roots;

        let test_modules_to_reload = if let Some(changed_paths) = changed_paths
        {
          let mut result = IndexSet::with_capacity(test_modules.len());
          let changed_paths = changed_paths.into_iter().collect::<HashSet<_>>();
          for test_module_specifier in test_modules {
            if has_graph_root_local_dependent_changed(
              &graph,
              test_module_specifier,
              &changed_paths,
            ) {
              result.insert(test_module_specifier.clone());
            }
          }
          result
        } else {
          test_modules.clone()
        };

        let specifiers_with_mode = fetch_specifiers_with_test_mode(
          &cli_options,
          file_fetcher,
          members_with_test_options.into_iter().map(|(_, v)| v.files),
          &workspace_test_options.doc,
        )
        .await?
        .into_iter()
        .filter(|(specifier, _)| test_modules_to_reload.contains(specifier))
        .collect::<Vec<(ModuleSpecifier, TestMode)>>();

        let doc_tests =
          get_doc_tests(&specifiers_with_mode, file_fetcher).await?;
        let specifiers_for_typecheck_and_test =
          get_target_specifiers(specifiers_with_mode, &doc_tests);
        for doc_test in doc_tests {
          file_fetcher.insert_memory_files(doc_test);
        }

        let main_graph_container =
          factory.main_module_graph_container().await?;

        // Typecheck
        main_graph_container
          .check_specifiers(
            &specifiers_for_typecheck_and_test,
            crate::graph_container::CheckSpecifiersOptions {
              ext_overwrite: cli_options.ext_flag().as_ref(),
              ..Default::default()
            },
          )
          .await?;

        if workspace_test_options.no_run {
          return Ok(());
        }

        let worker_factory =
          Arc::new(factory.create_cli_main_worker_factory().await?);

        test_specifiers(
          worker_factory,
          &permissions,
          permission_desc_parser,
          specifiers_for_typecheck_and_test,
          TestSpecifiersOptions {
            cwd: Url::from_directory_path(cli_options.initial_cwd()).map_err(
              |_| {
                anyhow!(
                  "Unable to construct URL from the path of cwd: {}",
                  cli_options.initial_cwd().to_string_lossy(),
                )
              },
            )?,
            concurrent_jobs: workspace_test_options.concurrent_jobs,
            fail_fast: workspace_test_options.fail_fast,
            log_level,
            filter: workspace_test_options.filter.is_some(),
            reporter: workspace_test_options.reporter,
            junit_path: workspace_test_options.junit_path,
            hide_stacktraces: workspace_test_options.hide_stacktraces,
            specifier: TestSpecifierOptions {
              filter: TestFilter::from_flag(&workspace_test_options.filter),
              shuffle: workspace_test_options.shuffle,
              trace_leaks: workspace_test_options.trace_leaks,
            },
          },
        )
        .await?;

        Ok(())
      })
    },
  )
  .await?;

  Ok(())
}

/// Extracts doc tests from files specified by the given specifiers.
async fn get_doc_tests(
  specifiers_with_mode: &[(Url, TestMode)],
  file_fetcher: &CliFileFetcher,
) -> Result<Vec<File>, AnyError> {
  let specifiers_needing_extraction = specifiers_with_mode
    .iter()
    .filter(|(_, mode)| mode.needs_test_extraction())
    .map(|(s, _)| s);

  let mut doc_tests = Vec::new();
  for s in specifiers_needing_extraction {
    let file = file_fetcher.fetch_bypass_permissions(s).await?;
    doc_tests.extend(extract_doc_tests(file)?);
  }

  Ok(doc_tests)
}

/// Get a list of specifiers that we need to perform typecheck and run tests on.
/// The result includes "pseudo specifiers" for doc tests.
fn get_target_specifiers(
  specifiers_with_mode: Vec<(Url, TestMode)>,
  doc_tests: &[File],
) -> Vec<Url> {
  specifiers_with_mode
    .into_iter()
    .filter_map(|(s, mode)| mode.needs_test_run().then_some(s))
    .chain(doc_tests.iter().map(|d| d.url.clone()))
    .collect()
}

/// Tracks failures for the `--fail-fast` argument in
/// order to tell when to stop running tests.
#[derive(Clone, Default)]
pub struct FailFastTracker {
  max_count: Option<usize>,
  failure_count: Arc<AtomicUsize>,
}

impl FailFastTracker {
  pub fn new(fail_fast: Option<NonZeroUsize>) -> Self {
    Self {
      max_count: fail_fast.map(|v| v.into()),
      failure_count: Default::default(),
    }
  }

  pub fn add_failure(&self) -> bool {
    if let Some(max_count) = &self.max_count {
      self
        .failure_count
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        >= *max_count
    } else {
      false
    }
  }

  pub fn should_stop(&self) -> bool {
    if let Some(max_count) = &self.max_count {
      self.failure_count.load(std::sync::atomic::Ordering::SeqCst) >= *max_count
    } else {
      false
    }
  }
}

#[cfg(test)]
mod inner_test {
  use std::path::Path;

  use super::*;

  #[test]
  fn test_is_supported_test_ext() {
    assert!(!is_supported_test_ext(Path::new("tests/subdir/redirects")));
    assert!(is_supported_test_ext(Path::new("README.md")));
    assert!(is_supported_test_ext(Path::new("readme.MD")));
    assert!(is_supported_test_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_test_ext(Path::new(
      "testdata/run/001_hello.js"
    )));
    assert!(is_supported_test_ext(Path::new(
      "testdata/run/002_hello.ts"
    )));
    assert!(is_supported_test_ext(Path::new("foo.jsx")));
    assert!(is_supported_test_ext(Path::new("foo.tsx")));
    assert!(is_supported_test_ext(Path::new("foo.TS")));
    assert!(is_supported_test_ext(Path::new("foo.TSX")));
    assert!(is_supported_test_ext(Path::new("foo.JS")));
    assert!(is_supported_test_ext(Path::new("foo.JSX")));
    assert!(is_supported_test_ext(Path::new("foo.mjs")));
    assert!(is_supported_test_ext(Path::new("foo.mts")));
    assert!(is_supported_test_ext(Path::new("foo.cjs")));
    assert!(is_supported_test_ext(Path::new("foo.cts")));
    assert!(!is_supported_test_ext(Path::new("foo.mjsx")));
    assert!(!is_supported_test_ext(Path::new("foo.jsonc")));
    assert!(!is_supported_test_ext(Path::new("foo.JSONC")));
    assert!(!is_supported_test_ext(Path::new("foo.json")));
    assert!(!is_supported_test_ext(Path::new("foo.JsON")));
  }

  #[test]
  fn test_is_supported_test_path() {
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.ts"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.tsx"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.js"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.jsx"
    )));
    assert!(is_supported_test_path(Path::new("bar/foo.test.ts")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.tsx")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.js")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.jsx")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.js")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.jsx")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.ts")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.tsx")));
    assert!(is_supported_test_path(Path::new(
      "foo/bar/__tests__/foo.js"
    )));
    assert!(is_supported_test_path(Path::new(
      "foo/bar/__tests__/foo.jsx"
    )));
    assert!(is_supported_test_path(Path::new(
      "foo/bar/__tests__/foo.ts"
    )));
    assert!(is_supported_test_path(Path::new(
      "foo/bar/__tests__/foo.tsx"
    )));
    assert!(!is_supported_test_path(Path::new("README.md")));
    assert!(!is_supported_test_path(Path::new("lib/typescript.d.ts")));
    assert!(!is_supported_test_path(Path::new("notatest.js")));
    assert!(!is_supported_test_path(Path::new("NotAtest.ts")));
  }
}
