// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::Location;
use crate::colors;
use crate::create_main_worker;
use crate::file_fetcher::File;
use crate::media_type::MediaType;
use crate::module_graph;
use crate::ops;
use crate::program_state::ProgramState;
use crate::tokio_util;
use crate::tools::coverage::CoverageCollector;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use log::Level;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use swc_common::comments::CommentKind;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
  pub origin: String,
  pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResult {
  Ok,
  Ignored,
  Failed(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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
  Plan(TestPlan),
  Wait(TestDescription),
  Result(TestDescription, TestResult, u64),
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSummary {
  pub total: usize,
  pub passed: usize,
  pub failed: usize,
  pub ignored: usize,
  pub filtered_out: usize,
  pub measured: usize,
  pub failures: Vec<(TestDescription, String)>,
}

impl TestSummary {
  fn new() -> TestSummary {
    TestSummary {
      total: 0,
      passed: 0,
      failed: 0,
      ignored: 0,
      filtered_out: 0,
      measured: 0,
      failures: Vec::new(),
    }
  }

  fn has_failed(&self) -> bool {
    self.failed > 0 || !self.failures.is_empty()
  }

  fn has_pending(&self) -> bool {
    self.total - self.passed - self.failed - self.ignored > 0
  }
}

trait TestReporter {
  fn report_plan(&mut self, plan: &TestPlan);
  fn report_wait(&mut self, description: &TestDescription);
  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  );
  fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration);
}

struct PrettyTestReporter {
  concurrent: bool,
}

impl PrettyTestReporter {
  fn new(concurrent: bool) -> PrettyTestReporter {
    PrettyTestReporter { concurrent }
  }
}

impl TestReporter for PrettyTestReporter {
  fn report_plan(&mut self, plan: &TestPlan) {
    let inflection = if plan.total == 1 { "test" } else { "tests" };
    println!("running {} {} from {}", plan.total, inflection, plan.origin);
  }

  fn report_wait(&mut self, description: &TestDescription) {
    if !self.concurrent {
      print!("test {} ...", description.name);
    }
  }

  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: u64,
  ) {
    if self.concurrent {
      print!("test {} ...", description.name);
    }

    let status = match result {
      TestResult::Ok => colors::green("ok").to_string(),
      TestResult::Ignored => colors::yellow("ignored").to_string(),
      TestResult::Failed(_) => colors::red("FAILED").to_string(),
    };

    println!(
      " {} {}",
      status,
      colors::gray(format!("({}ms)", elapsed)).to_string()
    );
  }

  fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration) {
    if !summary.failures.is_empty() {
      println!("\nfailures:\n");
      for (description, error) in &summary.failures {
        println!("{}", description.name);
        println!("{}", error);
        println!();
      }

      println!("failures:\n");
      for (description, _) in &summary.failures {
        println!("\t{}", description.name);
      }
    }

    let status = if summary.has_failed() || summary.has_pending() {
      colors::red("FAILED").to_string()
    } else {
      colors::green("ok").to_string()
    };

    println!(
      "\ntest result: {}. {} passed; {} failed; {} ignored; {} measured; {} filtered out {}\n",
      status,
      summary.passed,
      summary.failed,
      summary.ignored,
      summary.measured,
      summary.filtered_out,
      colors::gray(format!("({}ms)", elapsed.as_millis())),
    );
  }
}

fn create_reporter(concurrent: bool) -> Box<dyn TestReporter + Send> {
  Box::new(PrettyTestReporter::new(concurrent))
}

pub async fn test_specifier(
  program_state: Arc<ProgramState>,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  filter: Option<String>,
  shuffle: Option<u64>,
  channel: Sender<TestEvent>,
) -> Result<(), AnyError> {
  let mut fetch_permissions = Permissions::allow_all();

  let main_file = program_state
    .file_fetcher
    .fetch(&main_module, &mut fetch_permissions)
    .await?;

  let test_module =
    deno_core::resolve_path(&format!("{}$deno$test.js", Uuid::new_v4()))?;

  let mut test_source = String::new();
  if main_file.media_type != MediaType::Unknown {
    test_source.push_str(&format!("import \"{}\";\n", main_module));
  }

  test_source
    .push_str("await new Promise(resolve => setTimeout(resolve, 0));\n");

  test_source.push_str("window.dispatchEvent(new Event('load'));\n");

  test_source.push_str(&format!(
    "await Deno[Deno.internal].runTests({});\n",
    json!({
      "disableLog": program_state.flags.log_level == Some(Level::Error),
      "filter": filter,
      "shuffle": shuffle,
    }),
  ));

  test_source.push_str("window.dispatchEvent(new Event('unload'));\n");

  let test_file = File {
    local: test_module.to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::JavaScript,
    source: test_source.clone(),
    specifier: test_module.clone(),
  };

  program_state.file_fetcher.insert_cached(test_file);

  let init_ops = |js_runtime: &mut JsRuntime| {
    ops::testing::init(js_runtime);

    js_runtime
      .op_state()
      .borrow_mut()
      .put::<Sender<TestEvent>>(channel.clone());
  };

  let mut worker = create_main_worker(
    &program_state,
    main_module.clone(),
    permissions,
    Some(&init_ops),
  );

  let mut maybe_coverage_collector = if let Some(ref coverage_dir) =
    program_state.coverage_dir
  {
    let session = worker.create_inspector_session().await;
    let coverage_dir = PathBuf::from(coverage_dir);
    let mut coverage_collector = CoverageCollector::new(coverage_dir, session);
    worker
      .with_event_loop(coverage_collector.start_collecting().boxed_local())
      .await?;

    Some(coverage_collector)
  } else {
    None
  };

  worker.execute_module(&test_module).await?;

  worker
    .run_event_loop(maybe_coverage_collector.is_none())
    .await?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    worker
      .with_event_loop(coverage_collector.stop_collecting().boxed_local())
      .await?;
  }

  Ok(())
}

fn extract_files_from_regex_blocks(
  location: &Location,
  source: &str,
  media_type: &MediaType,
  blocks_regex: &Regex,
  lines_regex: &Regex,
) -> Result<Vec<File>, AnyError> {
  let files = blocks_regex
    .captures_iter(source)
    .filter_map(|block| {
      let maybe_attributes: Option<Vec<_>> = block
        .get(1)
        .map(|attributes| attributes.as_str().split(' ').collect());

      let file_media_type = if let Some(attributes) = maybe_attributes {
        if attributes.contains(&"ignore") {
          return None;
        }

        match attributes.get(0) {
          Some(&"js") => MediaType::JavaScript,
          Some(&"jsx") => MediaType::Jsx,
          Some(&"ts") => MediaType::TypeScript,
          Some(&"tsx") => MediaType::Tsx,
          Some(&"") => *media_type,
          _ => MediaType::Unknown,
        }
      } else {
        *media_type
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
        file_source.push_str(&format!("{}\n", text.as_str()));
      }

      file_source.push_str("export {};");

      let file_specifier = deno_core::resolve_url_or_path(&format!(
        "{}${}-{}{}",
        location.specifier,
        location.line + line_offset,
        location.line + line_offset + line_count,
        file_media_type.as_ts_extension(),
      ))
      .unwrap();

      Some(File {
        local: file_specifier.to_file_path().unwrap(),
        maybe_types: None,
        media_type: file_media_type,
        source: file_source,
        specifier: file_specifier,
      })
    })
    .collect();

  Ok(files)
}

fn extract_files_from_source_comments(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
) -> Result<Vec<File>, AnyError> {
  let parsed_module = ast::parse(specifier.as_str(), source, media_type)?;
  let comments = parsed_module.get_comments();
  let blocks_regex = Regex::new(r"```([^\n]*)\n([\S\s]*?)```")?;
  let lines_regex = Regex::new(r"(?:\* ?)(?:\# ?)?(.*)")?;

  let files = comments
    .iter()
    .filter(|comment| {
      if comment.kind != CommentKind::Block || !comment.text.starts_with('*') {
        return false;
      }

      true
    })
    .flat_map(|comment| {
      let location = parsed_module.get_location(comment.span.lo);

      extract_files_from_regex_blocks(
        &location,
        &comment.text,
        media_type,
        &blocks_regex,
        &lines_regex,
      )
    })
    .flatten()
    .collect();

  Ok(files)
}

fn extract_files_from_fenced_blocks(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: &MediaType,
) -> Result<Vec<File>, AnyError> {
  let location = Location {
    specifier: specifier.to_string(),
    line: 1,
    col: 0,
  };

  let blocks_regex = Regex::new(r"```([^\n]*)\n([\S\s]*?)```")?;
  let lines_regex = Regex::new(r"(?:\# ?)?(.*)")?;

  extract_files_from_regex_blocks(
    &location,
    source,
    media_type,
    &blocks_regex,
    &lines_regex,
  )
}

async fn fetch_inline_files(
  program_state: Arc<ProgramState>,
  specifiers: Vec<ModuleSpecifier>,
) -> Result<Vec<File>, AnyError> {
  let mut files = Vec::new();
  for specifier in specifiers {
    let mut fetch_permissions = Permissions::allow_all();
    let file = program_state
      .file_fetcher
      .fetch(&specifier, &mut fetch_permissions)
      .await?;

    let inline_files = if file.media_type == MediaType::Unknown {
      extract_files_from_fenced_blocks(
        &file.specifier,
        &file.source,
        &file.media_type,
      )
    } else {
      extract_files_from_source_comments(
        &file.specifier,
        &file.source,
        &file.media_type,
      )
    };

    files.extend(inline_files?);
  }

  Ok(files)
}

/// Runs tests.
///
#[allow(clippy::too_many_arguments)]
pub async fn run_tests(
  program_state: Arc<ProgramState>,
  permissions: Permissions,
  lib: module_graph::TypeLib,
  doc_modules: Vec<ModuleSpecifier>,
  test_modules: Vec<ModuleSpecifier>,
  no_run: bool,
  fail_fast: Option<NonZeroUsize>,
  allow_none: bool,
  filter: Option<String>,
  shuffle: Option<u64>,
  concurrent_jobs: NonZeroUsize,
) -> Result<(), AnyError> {
  if !allow_none && doc_modules.is_empty() && test_modules.is_empty() {
    return Err(generic_error("No test modules found"));
  }

  let test_modules = if let Some(seed) = shuffle {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut test_modules = test_modules.clone();
    test_modules.sort();
    test_modules.shuffle(&mut rng);
    test_modules
  } else {
    test_modules
  };

  if !doc_modules.is_empty() {
    let files = fetch_inline_files(program_state.clone(), doc_modules).await?;
    let specifiers = files.iter().map(|file| file.specifier.clone()).collect();

    for file in files {
      program_state.file_fetcher.insert_cached(file);
    }

    program_state
      .prepare_module_graph(
        specifiers,
        lib.clone(),
        Permissions::allow_all(),
        permissions.clone(),
        program_state.maybe_import_map.clone(),
      )
      .await?;
  }

  let prepare_roots = {
    let mut files = Vec::new();
    let mut fetch_permissions = Permissions::allow_all();
    for specifier in &test_modules {
      let file = program_state
        .file_fetcher
        .fetch(specifier, &mut fetch_permissions)
        .await?;

      files.push(file);
    }

    let prepare_roots = files
      .iter()
      .filter(|file| file.media_type != MediaType::Unknown)
      .map(|file| file.specifier.clone())
      .collect();

    prepare_roots
  };

  program_state
    .prepare_module_graph(
      prepare_roots,
      lib.clone(),
      Permissions::allow_all(),
      permissions.clone(),
      program_state.maybe_import_map.clone(),
    )
    .await?;

  if no_run {
    return Ok(());
  }

  let (sender, receiver) = channel::<TestEvent>();

  let join_handles = test_modules.iter().map(move |main_module| {
    let program_state = program_state.clone();
    let main_module = main_module.clone();
    let permissions = permissions.clone();
    let filter = filter.clone();
    let sender = sender.clone();

    tokio::task::spawn_blocking(move || {
      let join_handle = std::thread::spawn(move || {
        let future = test_specifier(
          program_state,
          main_module,
          permissions,
          filter,
          shuffle,
          sender,
        );

        tokio_util::run_basic(future)
      });

      join_handle.join().unwrap()
    })
  });

  let join_stream = stream::iter(join_handles)
    .buffer_unordered(concurrent_jobs.get())
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let mut reporter = create_reporter(concurrent_jobs.get() > 1);
  let handler = {
    tokio::task::spawn_blocking(move || {
      let earlier = Instant::now();
      let mut summary = TestSummary::new();
      let mut used_only = false;

      for event in receiver.iter() {
        match event {
          TestEvent::Plan(plan) => {
            summary.total += plan.total;
            summary.filtered_out += plan.filtered_out;

            if plan.used_only {
              used_only = true;
            }

            reporter.report_plan(&plan);
          }

          TestEvent::Wait(description) => {
            reporter.report_wait(&description);
          }

          TestEvent::Result(description, result, elapsed) => {
            match &result {
              TestResult::Ok => {
                summary.passed += 1;
              }

              TestResult::Ignored => {
                summary.ignored += 1;
              }

              TestResult::Failed(error) => {
                summary.failed += 1;
                summary.failures.push((description.clone(), error.clone()));
              }
            }

            reporter.report_result(&description, &result, elapsed);
          }
        }

        if let Some(x) = fail_fast {
          if summary.failed >= x.get() {
            break;
          }
        }
      }

      let elapsed = Instant::now().duration_since(earlier);
      reporter.report_summary(&summary, &elapsed);

      if used_only {
        return Err(generic_error(
          "Test failed because the \"only\" option was used",
        ));
      }

      if summary.failed > 0 {
        return Err(generic_error("Test failed"));
      }

      Ok(())
    })
  };

  let (join_results, result) = future::join(join_stream, handler).await;

  let mut join_errors = join_results.into_iter().filter_map(|join_result| {
    join_result
      .ok()
      .map(|handle_result| handle_result.err())
      .flatten()
  });

  if let Some(e) = join_errors.next() {
    return Err(e);
  }

  match result {
    Ok(result) => {
      if let Some(err) = result.err() {
        return Err(err);
      }
    }

    Err(err) => {
      return Err(err.into());
    }
  }

  Ok(())
}
