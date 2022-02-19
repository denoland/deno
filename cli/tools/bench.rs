// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::cache::CacherLoader;
use crate::colors;
use crate::create_main_worker;
use crate::file_fetcher::File;
use crate::file_watcher;
use crate::file_watcher::ResolutionResult;
use crate::flags::BenchFlags;
use crate::flags::Flags;
use crate::fs_util::collect_specifiers;
use crate::fs_util::is_supported_bench_path;
use crate::fs_util::is_supported_test_ext;
use crate::graph_util::contains_specifier;
use crate::graph_util::graph_valid;
use crate::located_script_name;
use crate::lockfile;
use crate::ops;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;
use crate::tools::coverage::CoverageCollector;

use deno_ast::swc::common::comments::CommentKind;
use deno_ast::MediaType;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleKind;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_basic;
use log::Level;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
struct BenchSpecifierOptions {
  compat_mode: bool,
  filter: Option<String>,
}

fn fetch_specifiers(
  include: Vec<String>,
  ignore: Vec<PathBuf>,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  collect_specifiers(include.clone(), &ignore, is_supported_bench_path)
}

/// Type check a collection of module and document specifiers.
async fn check_specifiers(
  ps: &ProcState,
  permissions: Permissions,
  specifiers: Vec<ModuleSpecifier>,
  lib: emit::TypeLib,
) -> Result<(), AnyError> {
  ps.prepare_module_load(
    specifiers,
    false,
    lib,
    Permissions::allow_all(),
    permissions,
    true,
  )
  .await?;

  Ok(())
}

/// Test a single specifier as documentation containing test programs, an executable test module or
/// both.
async fn bench_specifier(
  ps: ProcState,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  channel: Sender<TestEvent>,
  options: BenchSpecifierOptions,
) -> Result<(), AnyError> {
  let mut worker = create_main_worker(
    &ps,
    specifier.clone(),
    permissions,
    vec![ops::testing::init(channel.clone())],
  );

  // Enable op call tracing in core to enable better debugging of op sanitizer
  // failures.
  worker
    .execute_script(&located_script_name!(), "Deno.core.enableOpCallTracing();")
    .unwrap();

  if options.compat_mode {
    worker.execute_side_module(&compat::GLOBAL_URL).await?;
    worker.execute_side_module(&compat::MODULE_URL).await?;

    let use_esm_loader = compat::check_if_should_use_esm_loader(&specifier)?;

    if use_esm_loader {
      worker.execute_side_module(&specifier).await?;
    } else {
      compat::load_cjs_module(
        &mut worker.js_runtime,
        &specifier.to_file_path().unwrap().display().to_string(),
        false,
      )?;
      worker.run_event_loop(false).await?;
    }
  } else {
    // We execute the module module as a side module so that import.meta.main is not set.
    worker.execute_side_module(&specifier).await?;
  }

  worker.dispatch_load_event(&located_script_name!())?;

  let test_result = worker.js_runtime.execute_script(
    &located_script_name!(),
    &format!(
      r#"Deno[Deno.internal].runBenchmarks({})"#,
      json!({
        "filter": options.filter,
      }),
    ),
  )?;

  worker.js_runtime.resolve_value(test_result).await?;

  worker.dispatch_unload_event(&located_script_name!())?;

  Ok(())
}

/// Test a collection of specifiers with test modes concurrently.
async fn bench_specifiers(
  ps: ProcState,
  permissions: Permissions,
  specifiers: Vec<ModuleSpecifier>,
  options: BenchSpecifierOptions,
) -> Result<(), AnyError> {
  let log_level = ps.flags.log_level;

  let (sender, receiver) = channel::<TestEvent>();

  let join_handles = specifiers.iter().map(move |specifier| {
    let ps = ps.clone();
    let permissions = permissions.clone();
    let specifier = specifier.clone();
    let sender = sender.clone();
    let options = options.clone();

    tokio::task::spawn_blocking(move || {
      let join_handle = std::thread::spawn(move || {
        let future =
          bench_specifier(ps, permissions, specifier, mode, sender, options);

        run_basic(future)
      });

      join_handle.join().unwrap()
    })
  });

  let join_stream = stream::iter(join_handles)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let mut reporter =
    create_reporter(concurrent_jobs.get() > 1, log_level != Some(Level::Error));

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

          TestEvent::Output(output) => {
            reporter.report_output(&output);
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

          TestEvent::StepWait(description) => {
            reporter.report_step_wait(&description);
          }

          TestEvent::StepResult(description, result, duration) => {
            match &result {
              TestStepResult::Ok => {
                summary.passed_steps += 1;
              }
              TestStepResult::Ignored => {
                summary.ignored_steps += 1;
              }
              TestStepResult::Failed(_) => {
                summary.failed_steps += 1;
              }
              TestStepResult::Pending(_) => {
                summary.pending_steps += 1;
              }
            }

            reporter.report_step_result(&description, &result, duration);
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

  // propagate any errors
  for join_result in join_results {
    join_result??;
  }

  result??;

  Ok(())
}

pub async fn run_benchmarks(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  let permissions = Permissions::from_options(&ps.flags.permissions_options());
  let specifiers = fetch_specifiers(
    &ps,
    bench_flags.include.unwrap_or_else(|| vec![".".to_string()]),
    bench_flags.ignore.clone(),
  )
  .await?;

  if specifiers.is_empty() {
    return Err(generic_error("No bench modules found"));
  }

  let lib = if ps.flags.unstable {
    emit::TypeLib::UnstableDenoWindow
  } else {
    emit::TypeLib::DenoWindow
  };

  check_specifiers(&ps, permissions.clone(), specifiers.clone(), lib).await?;

  let compat = ps.flags.compat;
  bench_specifiers(
    ps,
    permissions,
    specifiers,
    BenchSpecifierOptions {
      compat_mode: compat,
      filter: bench_flags.filter,
    },
  )
  .await?;

  Ok(())
}

pub async fn run_benchmarks_with_watch(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<(), AnyError> {
  todo!()
}
