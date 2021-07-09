// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
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
use deno_core::error::AnyError;
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
use std::sync::{Arc, Mutex};
use std::task::Poll;
use swc_common::comments::CommentKind;

// Expression used to get the array containing the actual test definitions in the runtime.
static TEST_REGISTRY: &str = "(Deno[Deno.internal].tests)";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDescription {
  pub name: String,
  pub ignore: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestPlan {
  pub origin: ModuleSpecifier,
  pub pending: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TestResult {
  Ok,
  Ignored,
  Failed(String),
}

struct TestSummary {
  total: usize,
  passed: usize,
  failed: usize,
  ignored: usize,
  allowed_fail: usize,
  filtered_out: usize,
  measured: usize,
  failures: Vec<(TestDescription, String)>,
  not_failures: Vec<(TestDescription, String)>,
}

impl TestSummary {
  fn new() -> Self {
    Self {
      total: 0,
      passed: 0,
      failed: 0,
      ignored: 0,
      allowed_fail: 0,
      filtered_out: 0,
      measured: 0,
      failures: Vec::new(),
      not_failures: Vec::new(),
    }
  }
}

trait TestReporter {
  fn visit_plan(&mut self, plan: TestPlan);
  fn visit_wait(&mut self, description: TestDescription);
  fn visit_result(&mut self, description: TestDescription, result: TestResult);
  fn visit_summary(&mut self, _summary: &TestSummary);
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
  fn visit_plan(&mut self, plan: TestPlan) {
    println!(
      "running {} tests from {}",
      plan.pending,
      plan.origin.to_string()
    );
  }

  fn visit_wait(&mut self, description: TestDescription) {
    if !self.concurrent {
      print!("test {} ...", description.name);
    }
  }

  fn visit_result(&mut self, description: TestDescription, result: TestResult) {
    if self.concurrent {
      print!("test {} ...", description.name);
    }

    let duration = 0;

    match result {
      TestResult::Ok => {
        println!(
          " {} {}",
          colors::green("ok"),
          colors::gray(format!("({}ms)", duration))
        );
      }

      TestResult::Ignored => {
        println!(
          " {} {}",
          colors::yellow("ignored"),
          colors::gray(format!("({}ms)", duration))
        );
      }

      TestResult::Failed(_) => {
        println!(
          " {} {}",
          colors::red("FAILED"),
          colors::gray(format!("({}ms)", duration))
        );
      }
    }
  }

  fn visit_summary(&mut self, summary: &TestSummary) {
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

    let status = if summary.failed > 0 {
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
      colors::gray(format!("({}ms)", 0)),
    );
  }
}

fn create_reporter(concurrent: bool) -> Box<dyn TestReporter + Send> {
  Box::new(PrettyTestReporter::new(concurrent))
}

enum TestEvent {
  Plan(TestPlan),
  Wait(TestDescription),
  Result(TestDescription, TestResult),
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
      let test_files_as_urls = test_files
        .iter()
        .map(|f| Url::from_file_path(f).unwrap())
        .collect::<Vec<Url>>();
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

async fn test_module<F>(
  program_state: Arc<ProgramState>,
  module_specifier: ModuleSpecifier,
  permissions: Permissions,
  process_event: F,
) -> Result<(), AnyError>
where
  F: Fn(TestEvent) + Send + 'static + Clone,
{
  let mut worker = create_main_worker(
    &program_state,
    module_specifier.clone(),
    permissions,
    true,
  );

  let (registry, descriptions) = {
    let execute_result = worker.execute_module(&module_specifier).await;
    execute_result?;

    let registry = worker
      .js_runtime
      .execute_script("deno:test_module", TEST_REGISTRY)?;

    let mut scope = worker.js_runtime.handle_scope();
    let registry_local =
      v8::Local::<v8::Value>::new(&mut scope, registry.clone());
    let descriptions: Vec<TestDescription> =
      serde_v8::from_v8(&mut scope, registry_local).unwrap();

    (registry, descriptions)
  };

  let iterator = descriptions
    .iter()
    .enumerate()
    .filter(|(_, _description)| true);

  process_event(TestEvent::Plan(TestPlan {
    origin: module_specifier,
    pending: iterator.clone().count(),
  }));

  for (index, description) in iterator {
    if description.ignore {
      process_event(TestEvent::Result(
        description.clone(),
        TestResult::Ignored,
      ));
      continue;
    }

    process_event(TestEvent::Wait(description.clone()));

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
      let fn_function = v8::Local::<v8::Function>::try_from(fn_value).unwrap();

      let result = fn_function.call(&mut scope, value, &[]).unwrap();
      let result = v8::Local::<v8::Promise>::try_from(result).unwrap();

      v8::Global::<v8::Promise>::new(&mut scope, result)
    };

    let result = future::poll_fn(|cx| {
      worker.poll_event_loop(cx, false);

      let state = {
        let mut scope = worker.js_runtime.handle_scope();
        let result_promise = result_promise.get(&mut scope);

        result_promise.state()
      };

      match state {
        v8::PromiseState::Pending => Poll::Pending,
        v8::PromiseState::Fulfilled => Poll::Ready(TestResult::Ok),
        v8::PromiseState::Rejected => {
          Poll::Ready(TestResult::Failed("TODO".to_string()))
        }
      }
    })
    .await;

    process_event(TestEvent::Result(description.clone(), result.clone()));
  }

  Ok(())
}

/// Runs tests.
///
/// Returns a boolean indicating whether the tests failed.
#[allow(clippy::too_many_arguments)]
pub async fn run_tests(
  program_state: Arc<ProgramState>,
  permissions: Permissions,
  lib: module_graph::TypeLib,
  doc_modules: Vec<ModuleSpecifier>,
  test_modules: Vec<ModuleSpecifier>,
  no_run: bool,
  fail_fast: bool,
  quiet: bool,
  allow_none: bool,
  filter: Option<String>,
  shuffle: Option<u64>,
  concurrent_jobs: usize,
) -> Result<bool, AnyError> {
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
    let mut test_programs = Vec::new();

    let blocks_regex = Regex::new(r"```([^\n]*)\n([\S\s]*?)```")?;
    let lines_regex = Regex::new(r"(?:\* ?)(?:\# ?)?(.*)")?;

    for specifier in &doc_modules {
      let mut fetch_permissions = Permissions::allow_all();
      let file = program_state
        .file_fetcher
        .fetch(&specifier, &mut fetch_permissions)
        .await?;

      let parsed_module =
        ast::parse(&file.specifier.as_str(), &file.source, &file.media_type)?;

      let mut comments = parsed_module.get_comments();
      comments.sort_by_key(|comment| {
        let location = parsed_module.get_location(&comment.span);
        location.line
      });

      for comment in comments {
        if comment.kind != CommentKind::Block || !comment.text.starts_with('*')
        {
          continue;
        }

        for block in blocks_regex.captures_iter(&comment.text) {
          let body = block.get(2).unwrap();
          let text = body.as_str();

          // TODO(caspervonb) generate an inline source map
          let mut source = String::new();
          for line in lines_regex.captures_iter(&text) {
            let text = line.get(1).unwrap();
            source.push_str(&format!("{}\n", text.as_str()));
          }

          source.push_str("export {};");

          let element = block.get(0).unwrap();
          let span = comment
            .span
            .from_inner_byte_pos(element.start(), element.end());
          let location = parsed_module.get_location(&span);

          let specifier = deno_core::resolve_url_or_path(&format!(
            "{}${}-{}",
            location.filename,
            location.line,
            location.line + element.as_str().split('\n').count(),
          ))?;

          let file = File {
            local: specifier.to_file_path().unwrap(),
            maybe_types: None,
            media_type: MediaType::TypeScript, // media_type.clone(),
            source: source.clone(),
            specifier: specifier.clone(),
          };

          program_state.file_fetcher.insert_cached(file.clone());
          test_programs.push(file.specifier.clone());
        }
      }
    }

    program_state
      .prepare_module_graph(
        test_programs.clone(),
        lib.clone(),
        Permissions::allow_all(),
        permissions.clone(),
        program_state.maybe_import_map.clone(),
      )
      .await?;
  } else if test_modules.is_empty() {
    println!("No matching test modules found");
    if !allow_none {
      std::process::exit(1);
    }

    return Ok(false);
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
    return Ok(false);
  }

  let concurrent = concurrent_jobs > 0;
  let reporter_lock = Arc::new(Mutex::new(create_reporter(concurrent)));
  let summary_lock = Arc::new(Mutex::new(TestSummary::new()));

  let process_event = {
    let reporter_lock = reporter_lock.clone();
    let summary_lock = summary_lock.clone();

    move |event: TestEvent| {
      let mut reporter = reporter_lock.lock().unwrap();
      let mut summary = summary_lock.lock().unwrap();

      match event {
        TestEvent::Plan(plan) => {
          reporter.visit_plan(plan);
        }

        TestEvent::Wait(description) => {
          summary.total += 1;
          reporter.visit_wait(description);
        }

        TestEvent::Result(description, result) => {
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

          reporter.visit_result(description, result);
        }
      }
    }
  };

  let join_handles = test_modules.iter().map(move |main_module| {
    let program_state = program_state.clone();
    let main_module = main_module.clone();
    let permissions = permissions.clone();
    let process_event = process_event.clone();

    tokio::task::spawn_blocking(move || {
      std::thread::spawn(move || {
        tokio_util::run_basic(test_module(
          program_state,
          main_module,
          permissions,
          process_event,
        ))
      })
      .join()
      .unwrap()
    })
  });

  let _join_results = stream::iter(join_handles)
    .buffer_unordered(concurrent_jobs)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>()
    .await;

  let summary = summary_lock.lock().unwrap();
  reporter_lock.lock().unwrap().visit_summary(&summary);

  Ok(summary.failed > 0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_collect_test_module_specifiers() {
    let test_data_path = test_util::root_path().join("cli/tests/subdir");
    let mut matched_urls = collect_test_module_specifiers(
      vec![
        "https://example.com/colors_test.ts".to_string(),
        "./mod1.ts".to_string(),
        "./mod3.js".to_string(),
        "subdir2/mod2.ts".to_string(),
        "http://example.com/printf_test.ts".to_string(),
      ],
      &test_data_path,
      is_supported,
    )
    .unwrap();
    let test_data_url =
      Url::from_file_path(test_data_path).unwrap().to_string();

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
    let mut matched_urls = collect_test_module_specifiers(
      vec![".".to_string()],
      &root,
      is_supported,
    )
    .unwrap();
    matched_urls.sort();
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
