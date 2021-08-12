// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::Location;
use crate::colors;
use crate::create_main_worker;
use crate::file_fetcher::File;
use crate::fs_util::collect_files;
use crate::fs_util::normalize_path;
use crate::media_type::MediaType;
use crate::module_graph;
use crate::program_state::ProgramState;
use crate::tokio_util;
use crate::tools::coverage::CoverageCollector;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::serde_v8;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::Deserialize;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;
use swc_common::comments::CommentKind;
use uuid::Uuid;

// Expression used to get the array containing the actual test definitions in the runtime.
static TEST_REGISTRY: &str = "(Deno[Deno.internal].tests)";

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
  pub name: String,
  pub ignore: bool,
  pub only: bool,
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
  pub origin: ModuleSpecifier,
  pub total: usize,
  pub filtered_in: usize,
  pub filtered_out: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSummary {
  pub total: usize,
  pub passed: usize,
  pub failed: usize,
  pub ignored: usize,
  pub filtered_in: usize,
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
      filtered_in: 0,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestEvent {
  Plan(TestPlan),
  Wait(TestDescription),
  Result(TestDescription, TestResult, Duration),
}

trait TestReporter {
  fn report_plan(&mut self, plan: &TestPlan);
  fn report_wait(&mut self, description: &TestDescription);
  fn report_result(
    &mut self,
    description: &TestDescription,
    result: &TestResult,
    elapsed: &Duration,
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
    elapsed: &Duration,
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
      colors::gray(format!("({}ms)", elapsed.as_millis()))
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

pub(crate) fn is_supported(p: &Path) -> bool {
  use std::path::Component;
  if let Some(Component::Normal(basename_os_str)) = p.components().next_back() {
    let basename = basename_os_str.to_string_lossy();
    basename.ends_with("_test.ts")
      || basename.ends_with("_test.tsx")
      || basename.ends_with("_test.js")
      || basename.ends_with("_test.mjs")
      || basename.ends_with("_test.jsx")
      || basename.ends_with(".test.ts")
      || basename.ends_with(".test.tsx")
      || basename.ends_with(".test.js")
      || basename.ends_with(".test.mjs")
      || basename.ends_with(".test.jsx")
      || basename == "test.ts"
      || basename == "test.tsx"
      || basename == "test.js"
      || basename == "test.mjs"
      || basename == "test.jsx"
  } else {
    false
  }
}

pub fn is_remote_url(module_url: &str) -> bool {
  let lower = module_url.to_lowercase();
  lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn collect_test_module_specifiers<P>(
  include: Vec<String>,
  root_path: &Path,
  predicate: P,
) -> Result<Vec<Url>, AnyError>
where
  P: Fn(&Path) -> bool,
{
  let (include_paths, include_urls): (Vec<String>, Vec<String>) =
    include.into_iter().partition(|n| !is_remote_url(n));
  let mut prepared = vec![];

  for path in include_paths {
    let p = normalize_path(&root_path.join(path));
    if p.is_dir() {
      let test_files = collect_files(&[p], &[], &predicate).unwrap();
      let mut test_files_as_urls = test_files
        .iter()
        .map(|f| Url::from_file_path(f).unwrap())
        .collect::<Vec<Url>>();

      test_files_as_urls.sort();
      prepared.extend(test_files_as_urls);
    } else {
      let url = Url::from_file_path(p).unwrap();
      prepared.push(url);
    }
  }

  for remote_url in include_urls {
    let url = Url::parse(&remote_url)?;
    prepared.push(url);
  }

  Ok(prepared)
}

async fn test_specifier<F>(
  program_state: Arc<ProgramState>,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  quiet: bool,
  shuffle: Option<u64>,
  process_event: F,
) -> Result<(), AnyError>
where
  F: Fn(TestEvent) + Send + 'static + Clone,
{
  let mut worker =
    create_main_worker(&program_state, main_module.clone(), permissions, true);

  let test_module =
    deno_core::resolve_path(&format!("{}$deno$test.js", Uuid::new_v4()))?;

  let test_source = format!(r#"import "{}";"#, main_module);

  let test_file = File {
    local: test_module.to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::JavaScript,
    source: test_source.clone(),
    specifier: test_module.clone(),
  };

  program_state.file_fetcher.insert_cached(test_file);

  let registry = {
    worker.execute_module(&test_module).await?;
    let registry = worker
      .js_runtime
      .execute_script("deno:test_module", TEST_REGISTRY)?;

    registry
  };

  // TODO(caspervonb): capture stdout/stderr to memory instead of overriding at the javascript
  // layer.
  if quiet {
    worker.js_runtime.execute_script(
      "deno:test_module",
      "globalThis.console = Deno[Deno.internal].disabledConsole;",
    )?;
  }

  let descriptions = {
    let mut scope = worker.js_runtime.handle_scope();
    let registry_local =
      v8::Local::<v8::Value>::new(&mut scope, registry.clone());

    let descriptions: Vec<TestDescription> =
      serde_v8::from_v8(&mut scope, registry_local).unwrap();

    descriptions
  };

  let entries = descriptions
    .iter()
    .enumerate()
    .collect::<Vec<(usize, &TestDescription)>>();

  let filtered_in = {
    let only = entries
      .clone()
      .into_iter()
      .filter(|(_, description)| description.only)
      .collect::<Vec<(usize, &TestDescription)>>();

    if only.is_empty() {
      entries.clone()
    } else {
      only
    }
  };

  let filtered_out = {
    let mut filtered_out = filtered_in
      .clone()
      .into_iter()
      .filter(|(_, _description)| true)
      .collect::<Vec<(usize, &TestDescription)>>();

    if let Some(seed) = shuffle {
      let mut rng = SmallRng::seed_from_u64(seed);
      filtered_out.sort();
      filtered_out.shuffle(&mut rng);
    }

    filtered_out
  };

  process_event(TestEvent::Plan(TestPlan {
    origin: main_module,
    total: filtered_out.len(),
    filtered_in: entries.len() - filtered_in.len(),
    filtered_out: entries.len() - filtered_out.len(),
  }));

  for (index, description) in filtered_out {
    let earlier = Instant::now();
    process_event(TestEvent::Wait(description.clone()));

    let result = if description.ignore {
      TestResult::Ignored
    } else {
      let result_promise = {
        let mut scope = worker.js_runtime.handle_scope();
        let registry_local =
          v8::Local::<v8::Value>::new(&mut scope, registry.clone());
        let registry_local =
          v8::Local::<v8::Array>::try_from(registry_local).unwrap();

        let value = registry_local
          .get_index(&mut scope, index.try_into().unwrap())
          .unwrap();
        let object = v8::Local::<v8::Object>::try_from(value)?;

        let fn_key = v8::String::new(&mut scope, "fn").unwrap();
        let fn_value = object.get(&mut scope, fn_key.into()).unwrap();
        let fn_function =
          v8::Local::<v8::Function>::try_from(fn_value).unwrap();

        let result = fn_function.call(&mut scope, value, &[]).unwrap();
        let result = v8::Local::<v8::Promise>::try_from(result).unwrap();

        v8::Global::<v8::Promise>::new(&mut scope, result)
      };

      let result = future::poll_fn(|cx| {
        worker.poll_event_loop(cx, false);

        let mut scope = worker.js_runtime.handle_scope();
        let result_promise = result_promise.get(&mut scope);

        match result_promise.state() {
          v8::PromiseState::Pending => Poll::Pending,
          v8::PromiseState::Fulfilled => Poll::Ready(TestResult::Ok),
          v8::PromiseState::Rejected => {
            let error = result_promise.result(&mut scope);
            let error = JsError::from_v8_exception(&mut scope, error);
            Poll::Ready(TestResult::Failed(error.to_string()))
          }
        }
      })
      .await;

      result
    };

    let elapsed = Instant::now().duration_since(earlier);
    process_event(TestEvent::Result(description.clone(), result, elapsed));
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
      let maybe_attributes = block
        .get(1)
        .map(|attributes| attributes.as_str().split(' '));

      let file_media_type = if let Some(mut attributes) = maybe_attributes {
        match attributes.next() {
          Some("js") => MediaType::JavaScript,
          Some("jsx") => MediaType::Jsx,
          Some("ts") => MediaType::TypeScript,
          Some("tsx") => MediaType::Tsx,
          Some("") => *media_type,
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
  fail_fast: Option<usize>,
  quiet: bool,
  allow_none: bool,
  filter: Option<String>,
  shuffle: Option<u64>,
  concurrent_jobs: usize,
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

  program_state
    .prepare_module_graph(
      test_modules.clone(),
      lib.clone(),
      Permissions::allow_all(),
      permissions.clone(),
      program_state.maybe_import_map.clone(),
    )
    .await?;

  if no_run {
    return Ok(());
  }

  let earlier = Instant::now();

  let (sender, mut receiver) =
    tokio::sync::mpsc::unbounded_channel::<TestEvent>();

  let join_handles = test_modules.iter().map(move |main_module| {
    let program_state = program_state.clone();
    let main_module = main_module.clone();
    let permissions = permissions.clone();
    let shuffle = shuffle.clone();
    let sender = sender.clone();

    tokio::task::spawn_blocking(move || {
      std::thread::spawn(move || {
        tokio_util::run_basic(test_specifier(
          program_state,
          main_module,
          permissions,
          quiet,
          shuffle,
          move |event| {
            sender.send(event);
          },
        ))
      })
      .join()
      .unwrap()
    })
  });

  let join_future = stream::iter(join_handles)
    .buffer_unordered(concurrent_jobs)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let result_future = tokio::task::spawn(async move {
    let mut summary = TestSummary::new();
    let mut reporter = create_reporter(concurrent_jobs > 0);

    loop {
      let maybe_event = receiver.recv().await;
      if let Some(event) = maybe_event {
        match event {
          TestEvent::Plan(plan) => {
            summary.total += plan.total;
            summary.filtered_in += plan.filtered_in;
            summary.filtered_out += plan.filtered_out;
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

              TestResult::Failed(reason) => {
                summary.failed += 1;
                summary.failures.push((description.clone(), reason.clone()));
              }
            }

            reporter.report_result(&description, &result, &elapsed);
          }
        }

        if let Some(x) = fail_fast {
          if summary.failed >= x {
            break;
          }
        }
      } else {
        break;
      }
    }

    (reporter, summary)
  });

  let (join_results, result) = future::join(join_future, result_future).await;

  let mut join_errors = join_results.into_iter().filter_map(|join_result| {
    join_result
      .ok()
      .map(|handle_result| handle_result.err())
      .flatten()
  });

  if let Some(e) = join_errors.next() {
    return Err(e);
  }

  let (mut reporter, summary) = result.unwrap();
  let elapsed = Instant::now().duration_since(earlier);
  reporter.report_summary(&summary, &elapsed);

  if summary.failed > 0 {
    return Err(generic_error("Test failed"));
  }

  if summary.filtered_in > 0 {
    return Err(generic_error(
      "Test failed because the \"only\" option was used",
    ));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_collect_test_module_specifiers() {
    let sub_dir_path = test_util::testdata_path().join("subdir");
    let mut matched_urls = collect_test_module_specifiers(
      vec![
        "https://example.com/colors_test.ts".to_string(),
        "./mod1.ts".to_string(),
        "./mod3.js".to_string(),
        "subdir2/mod2.ts".to_string(),
        "http://example.com/printf_test.ts".to_string(),
      ],
      &sub_dir_path,
      is_supported,
    )
    .unwrap();
    let test_data_url = Url::from_file_path(sub_dir_path).unwrap().to_string();

    let expected: Vec<Url> = vec![
      format!("{}/mod1.ts", test_data_url),
      format!("{}/mod3.js", test_data_url),
      format!("{}/subdir2/mod2.ts", test_data_url),
      "http://example.com/printf_test.ts".to_string(),
      "https://example.com/colors_test.ts".to_string(),
    ]
    .into_iter()
    .map(|f| Url::parse(&f).unwrap())
    .collect();
    matched_urls.sort();
    assert_eq!(matched_urls, expected);
  }

  #[test]
  fn test_is_supported() {
    assert!(is_supported(Path::new("tests/subdir/foo_test.ts")));
    assert!(is_supported(Path::new("tests/subdir/foo_test.tsx")));
    assert!(is_supported(Path::new("tests/subdir/foo_test.js")));
    assert!(is_supported(Path::new("tests/subdir/foo_test.jsx")));
    assert!(is_supported(Path::new("bar/foo.test.ts")));
    assert!(is_supported(Path::new("bar/foo.test.tsx")));
    assert!(is_supported(Path::new("bar/foo.test.js")));
    assert!(is_supported(Path::new("bar/foo.test.jsx")));
    assert!(is_supported(Path::new("foo/bar/test.js")));
    assert!(is_supported(Path::new("foo/bar/test.jsx")));
    assert!(is_supported(Path::new("foo/bar/test.ts")));
    assert!(is_supported(Path::new("foo/bar/test.tsx")));
    assert!(!is_supported(Path::new("README.md")));
    assert!(!is_supported(Path::new("lib/typescript.d.ts")));
    assert!(!is_supported(Path::new("notatest.js")));
    assert!(!is_supported(Path::new("NotAtest.ts")));
  }

  #[test]
  fn supports_dirs() {
    // TODO(caspervonb) generate some fixtures in a temporary directory instead, there's no need
    // for this to rely on external fixtures.
    let root = test_util::root_path()
      .join("test_util")
      .join("std")
      .join("http");
    println!("root {:?}", root);
    let matched_urls = collect_test_module_specifiers(
      vec![".".to_string()],
      &root,
      is_supported,
    )
    .unwrap();

    let root_url = Url::from_file_path(root).unwrap().to_string();
    println!("root_url {}", root_url);
    let expected: Vec<Url> = vec![
      format!("{}/_io_test.ts", root_url),
      format!("{}/cookie_test.ts", root_url),
      format!("{}/file_server_test.ts", root_url),
      format!("{}/racing_server_test.ts", root_url),
      format!("{}/server_test.ts", root_url),
      format!("{}/test.ts", root_url),
    ]
    .into_iter()
    .map(|f| Url::parse(&f).unwrap())
    .collect();
    assert_eq!(matched_urls, expected);
  }

  #[test]
  fn test_is_remote_url() {
    assert!(is_remote_url("https://deno.land/std/http/file_server.ts"));
    assert!(is_remote_url("http://deno.land/std/http/file_server.ts"));
    assert!(is_remote_url("HTTP://deno.land/std/http/file_server.ts"));
    assert!(is_remote_url("HTTp://deno.land/std/http/file_server.ts"));
    assert!(!is_remote_url("file:///dev/deno_std/http/file_server.ts"));
    assert!(!is_remote_url("./dev/deno_std/http/file_server.ts"));
  }
}
