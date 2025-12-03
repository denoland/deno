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
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::File;
use deno_config::glob::FilePatterns;
use deno_config::glob::WalkEntry;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::anyhow;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::error::CoreErrorKind;
use deno_core::error::JsError;
use deno_core::futures::StreamExt;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::located_script_name;
use deno_core::serde_v8;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::url::Url;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::coverage::CoverageCollector;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::tokio_util::create_and_run_current_thread;
use deno_runtime::worker::MainWorker;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::Level;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use regex::Regex;
use serde::Deserialize;
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
use crate::util::fs::CollectSpecifiersOptions;
use crate::util::fs::collect_specifiers;
use crate::util::path::get_extension;
use crate::util::path::is_script_ext;
use crate::util::path::matches_pattern_or_exact_path;
use crate::worker::CliMainWorkerFactory;
use crate::worker::CreateCustomWorkerError;

mod channel;
pub mod fmt;
pub mod reporters;
mod sanitizers;

pub use channel::TestEventReceiver;
pub use channel::TestEventSender;
pub use channel::TestEventWorkerSender;
pub use channel::create_single_test_event_channel;
pub use channel::create_test_event_channel;
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

static SLOW_TEST_TIMEOUT: LazyLock<u64> = LazyLock::new(|| {
  let base_timeout = env::var("DENO_SLOW_TEST_TIMEOUT").unwrap_or_default();
  base_timeout.parse().unwrap_or(60).max(1)
});

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
    if let Some(substring) = &self.substring
      && !name.contains(substring)
    {
      return false;
    }
    if let Some(regex) = &self.regex
      && !regex.is_match(name)
    {
      return false;
    }
    if let Some(include) = &self.include
      && !include.contains(name)
    {
      return false;
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
pub(crate) struct TestContainer {
  descriptions: TestDescriptions,
  test_functions: Vec<v8::Global<v8::Function>>,
  test_hooks: TestHooks,
}

#[derive(Default)]
pub(crate) struct TestHooks {
  pub before_all: Vec<v8::Global<v8::Function>>,
  pub before_each: Vec<v8::Global<v8::Function>>,
  pub after_each: Vec<v8::Global<v8::Function>>,
  pub after_all: Vec<v8::Global<v8::Function>>,
}

impl TestContainer {
  pub fn register(
    &mut self,
    description: TestDescription,
    function: v8::Global<v8::Function>,
  ) {
    self.descriptions.tests.insert(description.id, description);
    self.test_functions.push(function)
  }

  pub fn register_hook(
    &mut self,
    hook_type: String,
    function: v8::Global<v8::Function>,
  ) {
    match hook_type.as_str() {
      "beforeAll" => self.test_hooks.before_all.push(function),
      "beforeEach" => self.test_hooks.before_each.push(function),
      "afterEach" => self.test_hooks.after_each.push(function),
      "afterAll" => self.test_hooks.after_all.push(function),
      _ => {}
    }
  }

  pub fn is_empty(&self) -> bool {
    self.test_functions.is_empty()
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
  pub strip_ascii_color: bool,
  pub initial_cwd: Option<Url>,
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
      TestFailure::IncompleteSteps => Cow::Borrowed(
        "Completed while steps were still running. Ensure all steps are awaited with `await t.step(...)`.",
      ),
      TestFailure::Incomplete => Cow::Borrowed(
        "Didn't complete before parent. Await step with `await t.step(...)`.",
      ),
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

  pub fn error_location(&self) -> Option<TestLocation> {
    let TestFailure::JsError(js_error) = self else {
      return None;
    };
    // The first line of user code comes above the test file.
    // The call stack usually contains the top 10 frames, and cuts off after that.
    // We need to explicitly check for the test runner here.
    // - Checking for a `ext:` is not enough, since other Deno `ext:`s can appear in the call stack.
    // - This check guarantees that the next frame is inside of the Deno.test(),
    //   and not somewhere else.
    const TEST_RUNNER: &str = "ext:cli/40_test.js";
    let runner_frame_index = js_error
      .frames
      .iter()
      .position(|f| f.file_name.as_deref() == Some(TEST_RUNNER))?;
    let frame = js_error
      .frames
      .split_at(runner_frame_index)
      .0
      .iter()
      .rfind(|f| {
        f.file_name.as_ref().is_some_and(|f| {
          f.starts_with("file:") && !f.contains("node_modules")
        })
      })?;
    let file_name = frame.file_name.as_ref()?.clone();
    // Turn into zero based indices
    let line_number = frame.line_number.map(|v| v - 1)? as u32;
    let column_number = frame.column_number.map(|v| v - 1).unwrap_or(0) as u32;
    Some(TestLocation {
      file_name,
      line_number,
      column_number,
    })
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

// TODO(bartlomieju): in Rust 1.90 some structs started getting flagged as not used
#[allow(dead_code)]
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
    strip_ascii_color: false,
    initial_cwd: Some(options.cwd.clone()),
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
      TestFailureFormatOptions {
        strip_ascii_color: true,
        ..failure_format_options
      },
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
        strip_ascii_color: true,
        initial_cwd: Some(options.cwd.clone()),
      },
    ));
    return Box::new(CompoundTestReporter::new(vec![reporter, junit]));
  }

  reporter
}

#[allow(clippy::too_many_arguments)]
async fn configure_main_worker(
  worker_factory: Arc<CliMainWorkerFactory>,
  specifier: &Url,
  preload_modules: Vec<Url>,
  require_modules: Vec<Url>,
  permissions_container: PermissionsContainer,
  worker_sender: TestEventWorkerSender,
  options: &TestSpecifierOptions,
  sender: UnboundedSender<jupyter_protocol::messaging::StreamContent>,
) -> Result<(Option<CoverageCollector>, MainWorker), CreateCustomWorkerError> {
  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Test,
      specifier.clone(),
      preload_modules,
      require_modules,
      permissions_container,
      vec![
        ops::testing::deno_test::init(worker_sender.sender),
        ops::lint::deno_lint_ext_for_test::init(),
        ops::jupyter::deno_jupyter_for_test::init(sender),
      ],
      Stdio {
        stdin: StdioPipe::inherit(),
        stdout: StdioPipe::file(worker_sender.stdout),
        stderr: StdioPipe::file(worker_sender.stderr),
      },
      None,
    )
    .await?;
  let coverage_collector = worker.maybe_setup_coverage_collector();
  if options.trace_leaks {
    worker
      .execute_script_static(
        located_script_name!(),
        "Deno[Deno.internal].core.setLeakTracingEnabled(true);",
      )
      .map_err(|e| CoreErrorKind::Js(e).into_box())?;
  }

  let op_state = worker.op_state();

  let check_res =
    |res: Result<(), CoreError>| match res.map_err(|err| err.into_kind()) {
      Ok(()) => Ok(()),
      Err(CoreErrorKind::Js(err)) => TestEventTracker::new(op_state.clone())
        .uncaught_error(specifier.to_string(), err)
        .map_err(|e| CoreErrorKind::JsBox(JsErrorBox::from_err(e)).into_box()),
      Err(err) => Err(err.into_box()),
    };

  check_res(worker.execute_preload_modules().await)?;
  check_res(worker.execute_side_module().await)?;

  let worker = worker.into_main_worker();

  Ok((coverage_collector, worker))
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
#[allow(clippy::too_many_arguments)]
pub async fn test_specifier(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions_container: PermissionsContainer,
  specifier: ModuleSpecifier,
  preload_modules: Vec<ModuleSpecifier>,
  require_modules: Vec<ModuleSpecifier>,
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
    preload_modules,
    require_modules,
    permissions_container,
    worker_sender,
    &options,
    jupyter_channel.0,
  )
  .await?;
  let event_tracker = TestEventTracker::new(worker.js_runtime.op_state());

  match test_specifier_inner(
    &mut worker,
    coverage_collector,
    specifier.clone(),
    fail_fast_tracker,
    &event_tracker,
    options,
  )
  .await
  {
    Ok(()) => Ok(()),
    Err(TestSpecifierError::Core(err)) => match err.into_kind() {
      CoreErrorKind::Js(err) => {
        event_tracker.uncaught_error(specifier.to_string(), err)?;
        Ok(())
      }
      err => Err(err.into_box().into()),
    },
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
async fn test_specifier_inner(
  worker: &mut MainWorker,
  mut coverage_collector: Option<CoverageCollector>,
  specifier: ModuleSpecifier,
  fail_fast_tracker: FailFastTracker,
  event_tracker: &TestEventTracker,
  options: TestSpecifierOptions,
) -> Result<(), TestSpecifierError> {
  // Ensure that there are no pending exceptions before we start running tests
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  worker
    .dispatch_load_event()
    .map_err(|e| CoreErrorKind::Js(e).into_box())?;

  run_tests_for_worker(
    worker,
    &specifier,
    &options,
    &fail_fast_tracker,
    event_tracker,
  )
  .await?;

  // Ignore `defaultPrevented` of the `beforeunload` event. We don't allow the
  // event loop to continue beyond what's needed to await results.
  worker
    .dispatch_beforeunload_event()
    .map_err(|e| CoreErrorKind::Js(e).into_box())?;
  worker
    .dispatch_unload_event()
    .map_err(|e| CoreErrorKind::Js(e).into_box())?;

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
    coverage_collector.stop_collecting()?;
  }
  Ok(())
}

pub fn worker_has_tests(worker: &mut MainWorker) -> bool {
  let state_rc = worker.js_runtime.op_state();
  let state = state_rc.borrow();
  !state.borrow::<TestContainer>().is_empty()
}

// Each test needs a fresh reqwest connection pool to avoid inter-test weirdness with connections
// failing. If we don't do this, a connection to a test server we just tore down might be re-used in
// the next test.
// TODO(mmastrac): this should be some sort of callback that we can implement for any subsystem
pub fn worker_prepare_for_test(worker: &mut MainWorker) {
  worker
    .js_runtime
    .op_state()
    .borrow_mut()
    .try_take::<deno_runtime::deno_fetch::Client>();
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

async fn slow_test_watchdog(event_tracker: TestEventTracker, test_id: usize) {
  // The slow test warning should pop up every DENO_SLOW_TEST_TIMEOUT*(2**n) seconds,
  // with a duration that is doubling each time. So for a warning time of 60s,
  // we should get a warning at 60s, 120s, 240s, etc.
  let base_timeout = *SLOW_TEST_TIMEOUT;
  let mut multiplier = 1;
  let mut elapsed = 0;
  loop {
    tokio::time::sleep(Duration::from_secs(
      base_timeout * (multiplier - elapsed),
    ))
    .await;
    if event_tracker
      .slow(test_id, Duration::from_secs(base_timeout * multiplier))
      .is_err()
    {
      break;
    }
    multiplier *= 2;
    elapsed += 1;
  }
}

pub async fn run_tests_for_worker(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
  options: &TestSpecifierOptions,
  fail_fast_tracker: &FailFastTracker,
  event_tracker: &TestEventTracker,
) -> Result<(), RunTestsForWorkerErr> {
  let state_rc = worker.js_runtime.op_state();

  // Take whatever tests have been registered
  let container =
    std::mem::take(&mut *state_rc.borrow_mut().borrow_mut::<TestContainer>());

  let descriptions = Arc::new(container.descriptions);
  event_tracker.register(descriptions.clone())?;
  run_tests_for_worker_inner(
    worker,
    specifier,
    descriptions,
    container.test_functions,
    container.test_hooks,
    options,
    event_tracker,
    fail_fast_tracker,
  )
  .await
}

fn compute_tests_to_run(
  descs: &TestDescriptions,
  test_functions: Vec<v8::Global<v8::Function>>,
  filter: TestFilter,
) -> (Vec<(&TestDescription, v8::Global<v8::Function>)>, bool) {
  let mut tests_to_run = Vec::with_capacity(descs.len());
  let mut used_only = false;
  for ((_, d), f) in descs.tests.iter().zip(test_functions) {
    if !filter.includes(&d.name) {
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
  (tests_to_run, used_only)
}

async fn call_hooks<H>(
  worker: &mut MainWorker,
  hook_fns: impl Iterator<Item = &v8::Global<v8::Function>>,
  mut error_handler: H,
) -> Result<(), RunTestsForWorkerErr>
where
  H: FnMut(CoreErrorKind) -> Result<(), RunTestsForWorkerErr>,
{
  for hook_fn in hook_fns {
    let call = worker.js_runtime.call(hook_fn);
    let result = worker
      .js_runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await;
    let Err(err) = result else {
      continue;
    };
    error_handler(err.into_kind())?;
    break;
  }
  Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_tests_for_worker_inner(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
  descs: Arc<TestDescriptions>,
  test_functions: Vec<v8::Global<v8::Function>>,
  test_hooks: TestHooks,
  options: &TestSpecifierOptions,
  event_tracker: &TestEventTracker,
  fail_fast_tracker: &FailFastTracker,
) -> Result<(), RunTestsForWorkerErr> {
  let unfiltered = descs.len();

  let (mut tests_to_run, used_only) =
    compute_tests_to_run(&descs, test_functions, options.filter.clone());

  if let Some(seed) = options.shuffle {
    tests_to_run.shuffle(&mut SmallRng::seed_from_u64(seed));
  }

  event_tracker.plan(TestPlan {
    origin: specifier.to_string(),
    total: tests_to_run.len(),
    filtered_out: unfiltered - tests_to_run.len(),
    used_only,
  })?;

  let mut had_uncaught_error = false;
  let sanitizer_helper = sanitizers::create_test_sanitizer_helper(worker);

  // Execute beforeAll hooks (FIFO order)
  call_hooks(worker, test_hooks.before_all.iter(), |core_error| {
    tests_to_run = vec![];
    match core_error {
      CoreErrorKind::Js(err) => {
        event_tracker.uncaught_error(specifier.to_string(), err)?;
        Ok(())
      }
      err => Err(err.into_box().into()),
    }
  })
  .await?;

  for (desc, function) in tests_to_run.into_iter() {
    worker_prepare_for_test(worker);

    if fail_fast_tracker.should_stop() {
      break;
    }

    if desc.ignore {
      event_tracker.ignored(desc)?;
      continue;
    }
    if had_uncaught_error {
      event_tracker.cancelled(desc)?;
      continue;
    }
    event_tracker.wait(desc)?;

    // Poll event loop once, to allow all ops that are already resolved, but haven't
    // responded to settle.
    // TODO(mmastrac): we should provide an API to poll the event loop until no further
    // progress is made.
    poll_event_loop(worker).await?;

    // We always capture stats, regardless of sanitization state
    let before_test_stats = sanitizer_helper.capture_stats();

    let earlier = Instant::now();

    // Execute beforeEach hooks (FIFO order)
    let mut before_each_hook_errored = false;

    call_hooks(worker, test_hooks.before_each.iter(), |core_error| {
      match core_error {
        CoreErrorKind::Js(err) => {
          before_each_hook_errored = true;
          let test_result = TestResult::Failed(TestFailure::JsError(err));
          fail_fast_tracker.add_failure();
          event_tracker.result(desc, test_result, earlier.elapsed())?;
          Ok(())
        }
        err => Err(err.into_box().into()),
      }
    })
    .await?;

    // TODO(bartlomieju): this whole block/binding could be reworked into something better
    let result = if !before_each_hook_errored {
      let call = worker.js_runtime.call(&function);

      let slow_test_warning =
        spawn(slow_test_watchdog(event_tracker.clone(), desc.id));

      let result = worker
        .js_runtime
        .with_event_loop_promise(call, PollEventLoopOptions::default())
        .await;
      slow_test_warning.abort();
      let result = match result {
        Ok(r) => r,
        Err(error) => match error.into_kind() {
          CoreErrorKind::Js(js_error) => {
            event_tracker.uncaught_error(specifier.to_string(), js_error)?;
            fail_fast_tracker.add_failure();
            event_tracker.cancelled(desc)?;
            had_uncaught_error = true;
            continue;
          }
          err => return Err(err.into_box().into()),
        },
      };

      // Check the result before we check for leaks
      deno_core::scope!(scope, &mut worker.js_runtime);
      let result = v8::Local::new(scope, result);
      serde_v8::from_v8::<TestResult>(scope, result)?
    } else {
      TestResult::Ignored
    };

    if matches!(result, TestResult::Failed(_)) {
      fail_fast_tracker.add_failure();
      event_tracker.result(desc, result.clone(), earlier.elapsed())?;
    }

    // Execute afterEach hooks (LIFO order)
    call_hooks(worker, test_hooks.after_each.iter().rev(), |core_error| {
      match core_error {
        CoreErrorKind::Js(err) => {
          let test_result = TestResult::Failed(TestFailure::JsError(err));
          fail_fast_tracker.add_failure();
          event_tracker.result(desc, test_result, earlier.elapsed())?;
          Ok(())
        }
        err => Err(err.into_box().into()),
      }
    })
    .await?;

    if matches!(result, TestResult::Failed(_)) {
      continue;
    }

    // Await activity stabilization
    if let Some(diff) = sanitizers::wait_for_activity_to_stabilize(
      worker,
      &sanitizer_helper,
      before_test_stats,
      desc.sanitize_ops,
      desc.sanitize_resources,
    )
    .await?
    {
      let (formatted, trailer_notes) = format_sanitizer_diff(diff);
      if !formatted.is_empty() {
        let failure = TestFailure::Leaked(formatted, trailer_notes);
        fail_fast_tracker.add_failure();
        event_tracker.result(
          desc,
          TestResult::Failed(failure),
          earlier.elapsed(),
        )?;
        continue;
      }
    }

    // TODO(bartlomieju): using `before_each_hook_errored` is fishy
    if !before_each_hook_errored {
      event_tracker.result(desc, result, earlier.elapsed())?;
    }
  }

  event_tracker.completed()?;

  // Execute afterAll hooks (LIFO order)
  call_hooks(worker, test_hooks.after_all.iter().rev(), |core_error| {
    match core_error {
      CoreErrorKind::Js(err) => {
        event_tracker.uncaught_error(specifier.to_string(), err)?;
        Ok(())
      }
      err => Err(err.into_box().into()),
    }
  })
  .await?;

  Ok(())
}

static HAS_TEST_RUN_SIGINT_HANDLER: AtomicBool = AtomicBool::new(false);

/// Test a collection of specifiers with test modes concurrently.
async fn test_specifiers(
  worker_factory: Arc<CliMainWorkerFactory>,
  cli_options: &Arc<CliOptions>,
  permission_desc_parser: &Arc<RuntimePermissionDescriptorParser<CliSys>>,
  specifiers: Vec<ModuleSpecifier>,
  preload_modules: Vec<ModuleSpecifier>,
  require_modules: Vec<ModuleSpecifier>,
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
    deno_signals::ctrl_c().await.unwrap();
    cancel_sender.send(TestEvent::Sigint).ok();
  });
  HAS_TEST_RUN_SIGINT_HANDLER.store(true, Ordering::Relaxed);
  let reporter = get_test_reporter(&options);
  let fail_fast_tracker = FailFastTracker::new(options.fail_fast);

  let join_handles = specifiers.into_iter().map(move |specifier| {
    let worker_factory = worker_factory.clone();
    let specifier_dir = cli_options.workspace().resolve_member_dir(&specifier);
    let preload_modules = preload_modules.clone();
    let require_modules = require_modules.clone();
    let worker_sender = test_event_sender_factory.worker();
    let fail_fast_tracker = fail_fast_tracker.clone();
    let specifier_options = options.specifier.clone();
    let cli_options = cli_options.clone();
    let permission_desc_parser = permission_desc_parser.clone();
    spawn_blocking(move || {
      // Various test files should not share the same permissions in terms of
      // `PermissionsContainer` - otherwise granting/revoking permissions in one
      // file would have impact on other files, which is undesirable.
      let permissions =
        cli_options.permissions_options_for_dir(&specifier_dir)?;
      let permissions_container = PermissionsContainer::new(
        permission_desc_parser.clone(),
        Permissions::from_options(
          permission_desc_parser.as_ref(),
          &permissions,
        )?,
      );
      create_and_run_current_thread(test_specifier(
        worker_factory,
        permissions_container,
        specifier,
        preload_modules,
        require_modules,
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
    CollectSpecifiersOptions {
      file_patterns: files.clone(),
      vendor_folder: vendor_folder.map(ToOwned::to_owned),
      include_ignored_specified: false,
    },
    is_supported_test_path_predicate,
  )?;

  if *include_inline {
    return collect_specifiers(
      CollectSpecifiersOptions {
        file_patterns: files,
        vendor_folder: vendor_folder.map(ToOwned::to_owned),
        include_ignored_specified: false,
      },
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
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  // Run tests
  test_specifiers(
    worker_factory,
    cli_options,
    factory.permission_desc_parser()?,
    specifiers_for_typecheck_and_test,
    preload_modules,
    require_modules,
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
          .into_owned(),
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
      deno_signals::ctrl_c().await.unwrap();
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
              CollectSpecifiersOptions {
                file_patterns: test_options.files.clone(),
                vendor_folder: cli_options
                  .vendor_dir_path()
                  .map(ToOwned::to_owned),
                include_ignored_specified: false,
              },
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

        let graph = module_graph_creator
          .create_graph(graph_kind, test_modules, NpmCachingStrategy::Eager)
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
        let preload_modules = cli_options.preload_modules()?;
        let require_modules = cli_options.require_modules()?;

        test_specifiers(
          worker_factory,
          &cli_options,
          factory.permission_desc_parser()?,
          specifiers_for_typecheck_and_test,
          preload_modules,
          require_modules,
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

#[derive(Clone)]
pub struct TestEventTracker {
  op_state: Rc<RefCell<OpState>>,
}

impl TestEventTracker {
  pub fn new(op_state: Rc<RefCell<OpState>>) -> Self {
    Self { op_state }
  }

  fn send_event(&self, event: TestEvent) -> Result<(), ChannelClosedError> {
    self
      .op_state
      .borrow_mut()
      .borrow_mut::<TestEventSender>()
      .send(event)
  }

  fn slow(
    &self,
    test_id: usize,
    duration: Duration,
  ) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Slow(test_id, duration.as_millis() as _))
  }

  fn wait(&self, desc: &TestDescription) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Wait(desc.id))
  }

  fn ignored(&self, desc: &TestDescription) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Result(desc.id, TestResult::Ignored, 0))
  }

  fn cancelled(
    &self,
    desc: &TestDescription,
  ) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Result(desc.id, TestResult::Cancelled, 0))
  }

  fn register(
    &self,
    descriptions: Arc<TestDescriptions>,
  ) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Register(descriptions))
  }

  fn completed(&self) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Completed)
  }

  fn uncaught_error(
    &self,
    specifier: String,
    error: Box<JsError>,
  ) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::UncaughtError(specifier, error))
  }

  fn plan(&self, plan: TestPlan) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Plan(plan))
  }

  fn result(
    &self,
    desc: &TestDescription,
    test_result: TestResult,
    duration: Duration,
  ) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::Result(
      desc.id,
      test_result,
      duration.as_millis() as u64,
    ))
  }

  pub(crate) fn force_end_report(&self) -> Result<(), ChannelClosedError> {
    self.send_event(TestEvent::ForceEndReport)
  }
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

  pub fn add_failure(&self) {
    self
      .failure_count
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
