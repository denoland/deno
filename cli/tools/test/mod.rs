// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::FilesConfig;
use crate::args::Flags;
use crate::args::TestFlags;
use crate::args::TestReporterConfig;
use crate::colors;
use crate::display;
use crate::factory::CliFactory;
use crate::factory::CliFactoryBuilder;
use crate::file_fetcher::File;
use crate::file_fetcher::FileFetcher;
use crate::graph_util::graph_valid_with_cli_options;
use crate::graph_util::has_graph_root_local_dependent_changed;
use crate::module_loader::ModuleLoadPreparer;
use crate::ops;
use crate::util::file_watcher;
use crate::util::fs::collect_specifiers;
use crate::util::path::get_extension;
use crate::util::path::is_script_ext;
use crate::util::path::mapped_specifier_for_tsc;
use crate::worker::CliMainWorkerFactory;

use deno_ast::swc::common::comments::CommentKind;
use deno_ast::MediaType;
use deno_ast::SourceRangedForSpanned;
use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context as _;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::task::noop_waker;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::located_script_name;
use deno_core::parking_lot::Mutex;
use deno_core::serde_v8;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::PollEventLoopOptions;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::tokio_util::create_and_run_current_thread;
use deno_runtime::worker::MainWorker;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::Level;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::Read;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use tokio::signal;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::WeakUnboundedSender;

pub mod fmt;
pub mod reporters;

pub use fmt::format_test_error;
use reporters::CompoundTestReporter;
use reporters::DotTestReporter;
use reporters::JunitTestReporter;
use reporters::PrettyTestReporter;
use reporters::TapTestReporter;
use reporters::TestReporter;

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

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
  pub id: usize,
  pub name: String,
  pub ignore: bool,
  pub only: bool,
  pub origin: String,
  pub location: TestLocation,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestFailure {
  JsError(Box<JsError>),
  FailedSteps(usize),
  IncompleteSteps,
  LeakedOps(Vec<String>, bool), // Details, isOpCallTracingEnabled
  LeakedResources(Vec<String>), // Details
  // The rest are for steps only.
  Incomplete,
  OverlapsWithSanitizers(IndexSet<String>), // Long names of overlapped tests
  HasSanitizersAndOverlaps(IndexSet<String>), // Long names of overlapped tests
}

impl ToString for TestFailure {
  fn to_string(&self) -> String {
    match self {
      TestFailure::JsError(js_error) => format_test_error(js_error),
      TestFailure::FailedSteps(1) => "1 test step failed.".to_string(),
      TestFailure::FailedSteps(n) => format!("{} test steps failed.", n),
      TestFailure::IncompleteSteps => "Completed while steps were still running. Ensure all steps are awaited with `await t.step(...)`.".to_string(),
      TestFailure::Incomplete => "Didn't complete before parent. Await step with `await t.step(...)`.".to_string(),
      TestFailure::LeakedOps(details, is_op_call_tracing_enabled) => {
        let mut string = "Leaking async ops:".to_string();
        for detail in details {
          string.push_str(&format!("\n  - {}", detail));
        }
        if !is_op_call_tracing_enabled {
          string.push_str("\nTo get more details where ops were leaked, run again with --trace-ops flag.");
        }
        string
      }
      TestFailure::LeakedResources(details) => {
        let mut string = "Leaking resources:".to_string();
        for detail in details {
          string.push_str(&format!("\n  - {}", detail));
        }
        string
      }
      TestFailure::OverlapsWithSanitizers(long_names) => {
        let mut string = "Started test step while another test step with sanitizers was running:".to_string();
        for long_name in long_names {
          string.push_str(&format!("\n  * {}", long_name));
        }
        string
      }
      TestFailure::HasSanitizersAndOverlaps(long_names) => {
        let mut string = "Started test step with sanitizers while another test step was running:".to_string();
        for long_name in long_names {
          string.push_str(&format!("\n  * {}", long_name));
        }
        string
      }
    }
  }
}

impl TestFailure {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestEvent {
  Register(TestDescription),
  Plan(TestPlan),
  Wait(usize),
  Output(Vec<u8>),
  Result(usize, TestResult, u64),
  UncaughtError(String, Box<JsError>),
  StepRegister(TestStepDescription),
  StepWait(usize),
  StepResult(usize, TestStepResult, u64),
  ForceEndReport,
  Sigint,
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
  pub failures: Vec<(TestDescription, TestFailure)>,
  pub uncaught_errors: Vec<(String, Box<JsError>)>,
}

#[derive(Debug, Clone)]
struct TestSpecifiersOptions {
  concurrent_jobs: NonZeroUsize,
  fail_fast: Option<NonZeroUsize>,
  log_level: Option<log::Level>,
  filter: bool,
  specifier: TestSpecifierOptions,
  reporter: TestReporterConfig,
  junit_path: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct TestSpecifierOptions {
  pub shuffle: Option<u64>,
  pub filter: TestFilter,
  pub trace_ops: bool,
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
  let reporter: Box<dyn TestReporter> = match &options.reporter {
    TestReporterConfig::Dot => Box::new(DotTestReporter::new()),
    TestReporterConfig::Pretty => Box::new(PrettyTestReporter::new(
      parallel,
      options.log_level != Some(Level::Error),
      options.filter,
      false,
    )),
    TestReporterConfig::Junit => {
      Box::new(JunitTestReporter::new("-".to_string()))
    }
    TestReporterConfig::Tap => Box::new(TapTestReporter::new(
      options.concurrent_jobs > NonZeroUsize::new(1).unwrap(),
    )),
  };

  if let Some(junit_path) = &options.junit_path {
    let junit = Box::new(JunitTestReporter::new(junit_path.to_string()));
    return Box::new(CompoundTestReporter::new(vec![reporter, junit]));
  }

  reporter
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
pub async fn test_specifier(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  mut sender: TestEventSender,
  fail_fast_tracker: FailFastTracker,
  options: TestSpecifierOptions,
) -> Result<(), AnyError> {
  match test_specifier_inner(
    worker_factory,
    permissions,
    specifier.clone(),
    &sender,
    fail_fast_tracker,
    options,
  )
  .await
  {
    Ok(()) => Ok(()),
    Err(error) => {
      if error.is::<JsError>() {
        sender.send(TestEvent::UncaughtError(
          specifier.to_string(),
          Box::new(error.downcast::<JsError>().unwrap()),
        ))?;
        Ok(())
      } else {
        Err(error)
      }
    }
  }
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
async fn test_specifier_inner(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  sender: &TestEventSender,
  fail_fast_tracker: FailFastTracker,
  options: TestSpecifierOptions,
) -> Result<(), AnyError> {
  if fail_fast_tracker.should_stop() {
    return Ok(());
  }
  let stdout = StdioPipe::File(sender.stdout());
  let stderr = StdioPipe::File(sender.stderr());
  let mut worker = worker_factory
    .create_custom_worker(
      specifier.clone(),
      PermissionsContainer::new(permissions),
      vec![ops::testing::deno_test::init_ops(sender.clone())],
      Stdio {
        stdin: StdioPipe::Inherit,
        stdout,
        stderr,
      },
    )
    .await?;

  let mut coverage_collector = worker.maybe_setup_coverage_collector().await?;

  if options.trace_ops {
    worker.execute_script_static(
      located_script_name!(),
      "Deno[Deno.internal].core.enableOpCallTracing();",
    )?;
  }

  // We execute the main module as a side module so that import.meta.main is not set.
  worker.execute_side_module_possibly_with_npm().await?;

  let mut worker = worker.into_main_worker();

  // Ensure that there are no pending exceptions before we start running tests
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  worker.dispatch_load_event(located_script_name!())?;

  run_tests_for_worker(&mut worker, &specifier, &options, &fail_fast_tracker)
    .await?;

  // Ignore `defaultPrevented` of the `beforeunload` event. We don't allow the
  // event loop to continue beyond what's needed to await results.
  worker.dispatch_beforeunload_event(located_script_name!())?;
  worker.dispatch_unload_event(located_script_name!())?;

  // Ensure the worker has settled so we can catch any remaining unhandled rejections. We don't
  // want to wait forever here.
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  if let Some(coverage_collector) = coverage_collector.as_mut() {
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
  !state.borrow::<ops::testing::TestContainer>().0.is_empty()
}

pub async fn run_tests_for_worker(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
  options: &TestSpecifierOptions,
  fail_fast_tracker: &FailFastTracker,
) -> Result<(), AnyError> {
  let (tests, mut sender) = {
    let state_rc = worker.js_runtime.op_state();
    let mut state = state_rc.borrow_mut();
    (
      std::mem::take(&mut state.borrow_mut::<ops::testing::TestContainer>().0),
      state.borrow::<TestEventSender>().clone(),
    )
  };
  let unfiltered = tests.len();
  let tests = tests
    .into_iter()
    .filter(|(d, _)| options.filter.includes(&d.name))
    .collect::<Vec<_>>();
  let (only, no_only): (Vec<_>, Vec<_>) =
    tests.into_iter().partition(|(d, _)| d.only);
  let used_only = !only.is_empty();
  let mut tests = if used_only { only } else { no_only };
  if let Some(seed) = options.shuffle {
    tests.shuffle(&mut SmallRng::seed_from_u64(seed));
  }
  sender.send(TestEvent::Plan(TestPlan {
    origin: specifier.to_string(),
    total: tests.len(),
    filtered_out: unfiltered - tests.len(),
    used_only,
  }))?;
  let mut had_uncaught_error = false;
  for (desc, function) in tests {
    if fail_fast_tracker.should_stop() {
      break;
    }
    if desc.ignore {
      sender.send(TestEvent::Result(desc.id, TestResult::Ignored, 0))?;
      continue;
    }
    if had_uncaught_error {
      sender.send(TestEvent::Result(desc.id, TestResult::Cancelled, 0))?;
      continue;
    }
    sender.send(TestEvent::Wait(desc.id))?;

    // TODO(bartlomieju): this is a nasty (beautiful) hack, that was required
    // when switching `JsRuntime` from `FuturesUnordered` to `JoinSet`. With
    // `JoinSet` all pending ops are immediately polled and that caused a problem
    // when some async ops were fired and canceled before running tests (giving
    // false positives in the ops sanitizer). We should probably rewrite sanitizers
    // to be done in Rust instead of in JS (40_testing.js).
    {
      // Poll event loop once, this will allow all ops that are already resolved,
      // but haven't responded to settle.
      let waker = noop_waker();
      let mut cx = Context::from_waker(&waker);
      let _ = worker
        .js_runtime
        .poll_event_loop(&mut cx, PollEventLoopOptions::default());
    }

    let earlier = SystemTime::now();
    let call = worker.js_runtime.call(&function);
    let result = match worker
      .js_runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await
    {
      Ok(r) => r,
      Err(error) => {
        if error.is::<JsError>() {
          sender.send(TestEvent::UncaughtError(
            specifier.to_string(),
            Box::new(error.downcast::<JsError>().unwrap()),
          ))?;
          fail_fast_tracker.add_failure();
          sender.send(TestEvent::Result(desc.id, TestResult::Cancelled, 0))?;
          had_uncaught_error = true;
          continue;
        } else {
          return Err(error);
        }
      }
    };
    let scope = &mut worker.js_runtime.handle_scope();
    let result = v8::Local::new(scope, result);
    let result = serde_v8::from_v8::<TestResult>(scope, result)?;
    if matches!(result, TestResult::Failed(_)) {
      fail_fast_tracker.add_failure();
    }
    let elapsed = SystemTime::now().duration_since(earlier)?.as_millis();
    sender.send(TestEvent::Result(desc.id, result, elapsed as u64))?;
  }
  Ok(())
}

fn extract_files_from_regex_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  file_line_index: usize,
  blocks_regex: &Regex,
  lines_regex: &Regex,
) -> Result<Vec<File>, AnyError> {
  let files = blocks_regex
    .captures_iter(source)
    .filter_map(|block| {
      block.get(1)?;

      let maybe_attributes: Option<Vec<_>> = block
        .get(1)
        .map(|attributes| attributes.as_str().split(' ').collect());

      let file_media_type = if let Some(attributes) = maybe_attributes {
        if attributes.contains(&"ignore") {
          return None;
        }

        match attributes.first() {
          Some(&"js") => MediaType::JavaScript,
          Some(&"javascript") => MediaType::JavaScript,
          Some(&"mjs") => MediaType::Mjs,
          Some(&"cjs") => MediaType::Cjs,
          Some(&"jsx") => MediaType::Jsx,
          Some(&"ts") => MediaType::TypeScript,
          Some(&"typescript") => MediaType::TypeScript,
          Some(&"mts") => MediaType::Mts,
          Some(&"cts") => MediaType::Cts,
          Some(&"tsx") => MediaType::Tsx,
          _ => MediaType::Unknown,
        }
      } else {
        media_type
      };

      if file_media_type == MediaType::Unknown {
        return None;
      }

      let line_offset = source[0..block.get(0).unwrap().start()]
        .chars()
        .filter(|c| *c == '\n')
        .count();

      let line_count = block.get(0).unwrap().as_str().split('\n').count();

      let body = block.get(2).unwrap();
      let text = body.as_str();

      // TODO(caspervonb) generate an inline source map
      let mut file_source = String::new();
      for line in lines_regex.captures_iter(text) {
        let text = line.get(1).unwrap();
        writeln!(file_source, "{}", text.as_str()).unwrap();
      }

      let file_specifier = ModuleSpecifier::parse(&format!(
        "{}${}-{}",
        specifier,
        file_line_index + line_offset + 1,
        file_line_index + line_offset + line_count + 1,
      ))
      .unwrap();
      let file_specifier =
        mapped_specifier_for_tsc(&file_specifier, file_media_type)
          .map(|s| ModuleSpecifier::parse(&s).unwrap())
          .unwrap_or(file_specifier);

      Some(File {
        maybe_types: None,
        media_type: file_media_type,
        source: file_source.into(),
        specifier: file_specifier,
        maybe_headers: None,
      })
    })
    .collect();

  Ok(files)
}

fn extract_files_from_source_comments(
  specifier: &ModuleSpecifier,
  source: Arc<str>,
  media_type: MediaType,
) -> Result<Vec<File>, AnyError> {
  let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info: deno_ast::SourceTextInfo::new(source),
    media_type,
    capture_tokens: false,
    maybe_syntax: None,
    scope_analysis: false,
  })?;
  let comments = parsed_source.comments().get_vec();
  let blocks_regex = lazy_regex::regex!(r"```([^\r\n]*)\r?\n([\S\s]*?)```");
  let lines_regex = lazy_regex::regex!(r"(?:\* ?)(?:\# ?)?(.*)");

  let files = comments
    .iter()
    .filter(|comment| {
      if comment.kind != CommentKind::Block || !comment.text.starts_with('*') {
        return false;
      }

      true
    })
    .flat_map(|comment| {
      extract_files_from_regex_blocks(
        specifier,
        &comment.text,
        media_type,
        parsed_source.text_info().line_index(comment.start()),
        blocks_regex,
        lines_regex,
      )
    })
    .flatten()
    .collect();

  Ok(files)
}

fn extract_files_from_fenced_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
) -> Result<Vec<File>, AnyError> {
  // The pattern matches code blocks as well as anything in HTML comment syntax,
  // but it stores the latter without any capturing groups. This way, a simple
  // check can be done to see if a block is inside a comment (and skip typechecking)
  // or not by checking for the presence of capturing groups in the matches.
  let blocks_regex =
    lazy_regex::regex!(r"(?s)<!--.*?-->|```([^\r\n]*)\r?\n([\S\s]*?)```");
  let lines_regex = lazy_regex::regex!(r"(?:\# ?)?(.*)");

  extract_files_from_regex_blocks(
    specifier,
    source,
    media_type,
    /* file line index */ 0,
    blocks_regex,
    lines_regex,
  )
}

async fn fetch_inline_files(
  file_fetcher: &FileFetcher,
  specifiers: Vec<ModuleSpecifier>,
) -> Result<Vec<File>, AnyError> {
  let mut files = Vec::new();
  for specifier in specifiers {
    let fetch_permissions = PermissionsContainer::allow_all();
    let file = file_fetcher.fetch(&specifier, fetch_permissions).await?;

    let inline_files = if file.media_type == MediaType::Unknown {
      extract_files_from_fenced_blocks(
        &file.specifier,
        &file.source,
        file.media_type,
      )
    } else {
      extract_files_from_source_comments(
        &file.specifier,
        file.source,
        file.media_type,
      )
    };

    files.extend(inline_files?);
  }

  Ok(files)
}

/// Type check a collection of module and document specifiers.
pub async fn check_specifiers(
  cli_options: &CliOptions,
  file_fetcher: &FileFetcher,
  module_load_preparer: &ModuleLoadPreparer,
  specifiers: Vec<(ModuleSpecifier, TestMode)>,
) -> Result<(), AnyError> {
  let lib = cli_options.ts_type_lib_window();
  let inline_files = fetch_inline_files(
    file_fetcher,
    specifiers
      .iter()
      .filter_map(|(specifier, mode)| {
        if *mode != TestMode::Executable {
          Some(specifier.clone())
        } else {
          None
        }
      })
      .collect(),
  )
  .await?;

  if !inline_files.is_empty() {
    let specifiers = inline_files
      .iter()
      .map(|file| file.specifier.clone())
      .collect();

    for file in inline_files {
      file_fetcher.insert_cached(file);
    }

    module_load_preparer
      .prepare_module_load(
        specifiers,
        false,
        lib,
        PermissionsContainer::new(Permissions::allow_all()),
      )
      .await?;
  }

  let module_specifiers = specifiers
    .into_iter()
    .filter_map(|(specifier, mode)| {
      if mode != TestMode::Documentation {
        Some(specifier)
      } else {
        None
      }
    })
    .collect();

  module_load_preparer
    .prepare_module_load(
      module_specifiers,
      false,
      lib,
      PermissionsContainer::allow_all(),
    )
    .await?;

  Ok(())
}

static HAS_TEST_RUN_SIGINT_HANDLER: AtomicBool = AtomicBool::new(false);

/// Test a collection of specifiers with test modes concurrently.
async fn test_specifiers(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: &Permissions,
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

  let (sender, receiver) = unbounded_channel::<TestEvent>();
  let sender = TestEventSender::new(sender);
  let concurrent_jobs = options.concurrent_jobs;

  let sender_ = sender.downgrade();
  let sigint_handler_handle = spawn(async move {
    signal::ctrl_c().await.unwrap();
    sender_.upgrade().map(|s| s.send(TestEvent::Sigint).ok());
  });
  HAS_TEST_RUN_SIGINT_HANDLER.store(true, Ordering::Relaxed);
  let reporter = get_test_reporter(&options);
  let fail_fast_tracker = FailFastTracker::new(options.fail_fast);

  let join_handles = specifiers.into_iter().map(move |specifier| {
    let worker_factory = worker_factory.clone();
    let permissions = permissions.clone();
    let sender = sender.clone();
    let fail_fast_tracker = fail_fast_tracker.clone();
    let specifier_options = options.specifier.clone();
    spawn_blocking(move || {
      create_and_run_current_thread(test_specifier(
        worker_factory,
        permissions,
        specifier,
        sender.clone(),
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
  mut receiver: UnboundedReceiver<TestEvent>,
  mut reporter: Box<dyn TestReporter>,
) -> (Result<(), AnyError>, UnboundedReceiver<TestEvent>) {
  let mut tests = IndexMap::new();
  let mut test_steps = IndexMap::new();
  let mut tests_started = HashSet::new();
  let mut tests_with_result = HashSet::new();
  let mut start_time = None;
  let mut had_plan = false;
  let mut used_only = false;
  let mut failed = false;

  while let Some(event) = receiver.recv().await {
    match event {
      TestEvent::Register(description) => {
        reporter.report_register(&description);
        tests.insert(description.id, description);
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
        if let Err(err) = reporter.flush_report(&elapsed, &tests, &test_steps) {
          eprint!("Test reporter failed to flush: {}", err)
        }
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
      Err(generic_error(format!(
        "Test reporter failed to flush: {}",
        err
      ))),
      receiver,
    );
  }

  if used_only {
    return (
      Err(generic_error(
        "Test failed because the \"only\" option was used",
      )),
      receiver,
    );
  }

  if failed {
    return (Err(generic_error("Test failed")), receiver);
  }

  (Ok(()), receiver)
}

/// Checks if the path has a basename and extension Deno supports for tests.
pub(crate) fn is_supported_test_path(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    (basename.ends_with("_test")
      || basename.ends_with(".test")
      || basename == "test")
      && is_script_ext(path)
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
/// `TestMode::Documentation`.
/// - Specifiers matching the `is_supported_test_path` are marked as `TestMode::Executable`.
/// - Specifiers matching both predicates are marked as `TestMode::Both`
fn collect_specifiers_with_test_mode(
  files: &FilesConfig,
  include_inline: &bool,
) -> Result<Vec<(ModuleSpecifier, TestMode)>, AnyError> {
  let module_specifiers = collect_specifiers(files, is_supported_test_path)?;

  if *include_inline {
    return collect_specifiers(files, is_supported_test_ext).map(
      |specifiers| {
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
      },
    );
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
  file_fetcher: &FileFetcher,
  files: &FilesConfig,
  doc: &bool,
) -> Result<Vec<(ModuleSpecifier, TestMode)>, AnyError> {
  let mut specifiers_with_mode = collect_specifiers_with_test_mode(files, doc)?;

  for (specifier, mode) in &mut specifiers_with_mode {
    let file = file_fetcher
      .fetch(specifier, PermissionsContainer::allow_all())
      .await?;

    if file.media_type == MediaType::Unknown
      || file.media_type == MediaType::Dts
    {
      *mode = TestMode::Documentation
    }
  }

  Ok(specifiers_with_mode)
}

pub async fn run_tests(
  flags: Flags,
  test_flags: TestFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let test_options = cli_options.resolve_test_options(test_flags)?;
  let file_fetcher = factory.file_fetcher()?;
  let module_load_preparer = factory.module_load_preparer().await?;
  // Various test files should not share the same permissions in terms of
  // `PermissionsContainer` - otherwise granting/revoking permissions in one
  // file would have impact on other files, which is undesirable.
  let permissions =
    Permissions::from_options(&cli_options.permissions_options())?;
  let log_level = cli_options.log_level();

  let specifiers_with_mode = fetch_specifiers_with_test_mode(
    file_fetcher,
    &test_options.files,
    &test_options.doc,
  )
  .await?;

  if !test_options.allow_none && specifiers_with_mode.is_empty() {
    return Err(generic_error("No test modules found"));
  }

  check_specifiers(
    cli_options,
    file_fetcher,
    module_load_preparer,
    specifiers_with_mode.clone(),
  )
  .await?;

  if test_options.no_run {
    return Ok(());
  }

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);

  test_specifiers(
    worker_factory,
    &permissions,
    specifiers_with_mode
      .into_iter()
      .filter_map(|(s, m)| match m {
        TestMode::Documentation => None,
        _ => Some(s),
      })
      .collect(),
    TestSpecifiersOptions {
      concurrent_jobs: test_options.concurrent_jobs,
      fail_fast: test_options.fail_fast,
      log_level,
      filter: test_options.filter.is_some(),
      reporter: test_options.reporter,
      junit_path: test_options.junit_path,
      specifier: TestSpecifierOptions {
        filter: TestFilter::from_flag(&test_options.filter),
        shuffle: test_options.shuffle,
        trace_ops: test_options.trace_ops,
      },
    },
  )
  .await?;

  Ok(())
}

pub async fn run_tests_with_watch(
  flags: Flags,
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
      Ok(async move {
        let factory = CliFactoryBuilder::new()
          .build_from_flags_for_watcher(flags, watcher_communicator.clone())
          .await?;
        let cli_options = factory.cli_options();
        let test_options = cli_options.resolve_test_options(test_flags)?;

        let _ = watcher_communicator.watch_paths(cli_options.watch_paths());
        if let Some(include) = &test_options.files.include {
          let _ = watcher_communicator.watch_paths(include.clone());
        }

        let graph_kind = cli_options.type_check_mode().as_graph_kind();
        let log_level = cli_options.log_level();
        let cli_options = cli_options.clone();
        let module_graph_builder = factory.module_graph_builder().await?;
        let file_fetcher = factory.file_fetcher()?;
        let test_modules = if test_options.doc {
          collect_specifiers(&test_options.files, is_supported_test_ext)
        } else {
          collect_specifiers(&test_options.files, is_supported_test_path)
        }?;
        let permissions =
          Permissions::from_options(&cli_options.permissions_options())?;

        let graph = module_graph_builder
          .create_graph(graph_kind, test_modules.clone())
          .await?;
        graph_valid_with_cli_options(
          &graph,
          factory.fs().as_ref(),
          &test_modules,
          &cli_options,
        )?;

        let test_modules_to_reload = if let Some(changed_paths) = changed_paths
        {
          let changed_specifiers = changed_paths
            .into_iter()
            .filter_map(|p| ModuleSpecifier::from_file_path(p).ok())
            .collect::<HashSet<_>>();
          let mut result = Vec::new();
          for test_module_specifier in test_modules {
            if has_graph_root_local_dependent_changed(
              &graph,
              &test_module_specifier,
              &changed_specifiers,
            ) {
              result.push(test_module_specifier.clone());
            }
          }
          result
        } else {
          test_modules.clone()
        };

        let worker_factory =
          Arc::new(factory.create_cli_main_worker_factory().await?);
        let module_load_preparer = factory.module_load_preparer().await?;
        let specifiers_with_mode = fetch_specifiers_with_test_mode(
          file_fetcher,
          &test_options.files,
          &test_options.doc,
        )
        .await?
        .into_iter()
        .filter(|(specifier, _)| test_modules_to_reload.contains(specifier))
        .collect::<Vec<(ModuleSpecifier, TestMode)>>();

        check_specifiers(
          &cli_options,
          file_fetcher,
          module_load_preparer,
          specifiers_with_mode.clone(),
        )
        .await?;

        if test_options.no_run {
          return Ok(());
        }

        test_specifiers(
          worker_factory,
          &permissions,
          specifiers_with_mode
            .into_iter()
            .filter_map(|(s, m)| match m {
              TestMode::Documentation => None,
              _ => Some(s),
            })
            .collect(),
          TestSpecifiersOptions {
            concurrent_jobs: test_options.concurrent_jobs,
            fail_fast: test_options.fail_fast,
            log_level,
            filter: test_options.filter.is_some(),
            reporter: test_options.reporter,
            junit_path: test_options.junit_path,
            specifier: TestSpecifierOptions {
              filter: TestFilter::from_flag(&test_options.filter),
              shuffle: test_options.shuffle,
              trace_ops: test_options.trace_ops,
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

#[derive(Clone)]
pub struct TestEventSender {
  sender: UnboundedSender<TestEvent>,
  stdout_writer: TestOutputPipe,
  stderr_writer: TestOutputPipe,
}

impl TestEventSender {
  pub fn new(sender: UnboundedSender<TestEvent>) -> Self {
    Self {
      stdout_writer: TestOutputPipe::new(sender.clone()),
      stderr_writer: TestOutputPipe::new(sender.clone()),
      sender,
    }
  }

  pub fn stdout(&self) -> std::fs::File {
    self.stdout_writer.as_file()
  }

  pub fn stderr(&self) -> std::fs::File {
    self.stderr_writer.as_file()
  }

  pub fn send(&mut self, message: TestEvent) -> Result<(), AnyError> {
    // for any event that finishes collecting output, we need to
    // ensure that the collected stdout and stderr pipes are flushed
    if matches!(
      message,
      TestEvent::Result(_, _, _)
        | TestEvent::StepWait(_)
        | TestEvent::StepResult(_, _, _)
        | TestEvent::UncaughtError(_, _)
    ) {
      self.flush_stdout_and_stderr()?;
    }

    self.sender.send(message)?;
    Ok(())
  }

  fn downgrade(&self) -> WeakUnboundedSender<TestEvent> {
    self.sender.downgrade()
  }

  fn flush_stdout_and_stderr(&mut self) -> Result<(), AnyError> {
    self.stdout_writer.flush()?;
    self.stderr_writer.flush()?;

    Ok(())
  }
}

// use a string that if it ends up in the output won't affect how things are displayed
const ZERO_WIDTH_SPACE: &str = "\u{200B}";

struct TestOutputPipe {
  writer: os_pipe::PipeWriter,
  state: Arc<Mutex<Option<std::sync::mpsc::Sender<()>>>>,
}

impl Clone for TestOutputPipe {
  fn clone(&self) -> Self {
    Self {
      writer: self.writer.try_clone().unwrap(),
      state: self.state.clone(),
    }
  }
}

impl TestOutputPipe {
  pub fn new(sender: UnboundedSender<TestEvent>) -> Self {
    let (reader, writer) = os_pipe::pipe().unwrap();
    let state = Arc::new(Mutex::new(None));

    start_output_redirect_thread(reader, sender, state.clone());

    Self { writer, state }
  }

  pub fn flush(&mut self) -> Result<(), AnyError> {
    // We want to wake up the other thread and have it respond back
    // that it's done clearing out its pipe before returning.
    let (sender, receiver) = std::sync::mpsc::channel();
    if let Some(sender) = self.state.lock().replace(sender) {
      let _ = sender.send(()); // just in case
    }
    // Bit of a hack to send a zero width space in order to wake
    // the thread up. It seems that sending zero bytes here does
    // not work on windows.
    self.writer.write_all(ZERO_WIDTH_SPACE.as_bytes())?;
    self.writer.flush()?;
    // ignore the error as it might have been picked up and closed
    let _ = receiver.recv();

    Ok(())
  }

  pub fn as_file(&self) -> std::fs::File {
    pipe_writer_to_file(self.writer.try_clone().unwrap())
  }
}

#[cfg(windows)]
fn pipe_writer_to_file(writer: os_pipe::PipeWriter) -> std::fs::File {
  use std::os::windows::prelude::FromRawHandle;
  use std::os::windows::prelude::IntoRawHandle;
  // SAFETY: Requires consuming ownership of the provided handle
  unsafe { std::fs::File::from_raw_handle(writer.into_raw_handle()) }
}

#[cfg(unix)]
fn pipe_writer_to_file(writer: os_pipe::PipeWriter) -> std::fs::File {
  use std::os::unix::io::FromRawFd;
  use std::os::unix::io::IntoRawFd;
  // SAFETY: Requires consuming ownership of the provided handle
  unsafe { std::fs::File::from_raw_fd(writer.into_raw_fd()) }
}

fn start_output_redirect_thread(
  mut pipe_reader: os_pipe::PipeReader,
  sender: UnboundedSender<TestEvent>,
  flush_state: Arc<Mutex<Option<std::sync::mpsc::Sender<()>>>>,
) {
  spawn_blocking(move || loop {
    let mut buffer = [0; 512];
    let size = match pipe_reader.read(&mut buffer) {
      Ok(0) | Err(_) => break,
      Ok(size) => size,
    };
    let oneshot_sender = flush_state.lock().take();
    let mut data = &buffer[0..size];
    if data.ends_with(ZERO_WIDTH_SPACE.as_bytes()) {
      data = &data[0..data.len() - ZERO_WIDTH_SPACE.len()];
    }

    if !data.is_empty()
      && sender
        .send(TestEvent::Output(buffer[0..size].to_vec()))
        .is_err()
    {
      break;
    }

    // Always respond back if this was set. Ideally we would also check to
    // ensure the pipe reader is empty before sending back this response.
    if let Some(sender) = oneshot_sender {
      let _ignore = sender.send(());
    }
  });
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
    assert!(!is_supported_test_path(Path::new("README.md")));
    assert!(!is_supported_test_path(Path::new("lib/typescript.d.ts")));
    assert!(!is_supported_test_path(Path::new("notatest.js")));
    assert!(!is_supported_test_path(Path::new("NotAtest.ts")));
  }
}
