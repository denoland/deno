// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::compat;
use crate::create_main_worker;
use crate::emit;
// use crate::file_fetcher::File;
// use crate::file_watcher;
// use crate::file_watcher::ResolutionResult;
use crate::flags::BenchFlags;
use crate::flags::Flags;
use crate::fs_util::collect_specifiers;
use crate::fs_util::is_supported_bench_path;
use crate::located_script_name;
use crate::ops;
use crate::proc_state::ProcState;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleKind;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_basic;
use log::Level;
use num_format::Locale;
use num_format::ToFormattedString;
use serde::Deserialize;
use std::io::Write;
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

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BenchDescription {
  pub origin: String,
  pub name: String,
  pub iterations: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BenchOutput {
  Console(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BenchResult {
  Ok,
  Ignored,
  Failed(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchPlan {
  pub origin: String,
  pub total: usize,
  pub filtered_out: usize,
  pub used_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BenchEvent {
  Plan(BenchPlan),
  Wait(BenchDescription),
  Output(BenchOutput),
  IterationStart(u64),
  IterationFinish(u64),
  Result(BenchDescription, BenchResult, u64),
}

#[derive(Debug, Clone)]
pub struct BenchMeasures {
  pub iterations: u64,
  pub current_start: Instant,
  pub measures: Vec<u128>,
}

#[derive(Debug, Clone)]
pub struct BenchSummary {
  pub total: usize,
  pub passed: usize,
  pub failed: usize,
  pub ignored: usize,
  pub filtered_out: usize,
  pub measured: usize,
  pub measures: Vec<BenchMeasures>,
  pub current_bench: BenchMeasures,
  pub failures: Vec<(BenchDescription, String)>,
}

impl BenchSummary {
  pub fn new() -> Self {
    Self {
      total: 0,
      passed: 0,
      failed: 0,
      ignored: 0,
      filtered_out: 0,
      measured: 0,
      measures: Vec::new(),
      current_bench: BenchMeasures {
        iterations: 0,
        current_start: Instant::now(),
        measures: vec![],
      },
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

pub trait BenchReporter {
  fn report_plan(&mut self, plan: &BenchPlan);
  fn report_wait(&mut self, description: &BenchDescription);
  fn report_output(&mut self, output: &BenchOutput);
  fn report_result(
    &mut self,
    description: &BenchDescription,
    result: &BenchResult,
    elapsed: u64,
    current_bench: &BenchMeasures,
  );
  fn report_summary(&mut self, summary: &BenchSummary, elapsed: &Duration);
}

struct PrettyBenchReporter {
  echo_output: bool,
}

impl PrettyBenchReporter {
  fn new(echo_output: bool) -> Self {
    Self { echo_output }
  }

  fn force_report_wait(&mut self, description: &BenchDescription) {
    print!("bench {} ... ", description.name);
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
  }
}

/// A function that converts a milisecond elapsed time to a string that
/// represents a human readable version of that time.
fn human_elapsed(elapsed: u128) -> String {
  if elapsed < 1_000 {
    return format!("({}ms)", elapsed);
  }
  if elapsed < 1_000 * 60 {
    return format!("({}s)", elapsed / 1000);
  }

  let seconds = elapsed / 1_000;
  let minutes = seconds / 60;
  let seconds_remainder = seconds % 60;
  format!("({}m{}s)", minutes, seconds_remainder)
}

impl BenchReporter for PrettyBenchReporter {
  fn report_plan(&mut self, plan: &BenchPlan) {
    let inflection = if plan.total == 1 { "bench" } else { "benches" };
    println!("running {} {} from {}", plan.total, inflection, plan.origin);
  }

  fn report_wait(&mut self, description: &BenchDescription) {
    self.force_report_wait(description);
  }

  fn report_output(&mut self, output: &BenchOutput) {
    if self.echo_output {
      match output {
        BenchOutput::Console(line) => print!("{}", line),
      }
    }
  }

  fn report_result(
    &mut self,
    _description: &BenchDescription,
    result: &BenchResult,
    elapsed: u64,
    current_bench: &BenchMeasures,
  ) {
    let status = match result {
      BenchResult::Ok => {
        let ns_op = current_bench.measures.iter().sum::<u128>()
          / current_bench.iterations as u128;
        let min_op = current_bench.measures.iter().min().unwrap_or(&0);
        let max_op = current_bench.measures.iter().max().unwrap_or(&0);
        format!(
          "{} iterations {} ns/iter ({}..{} ns/iter) {}",
          current_bench.iterations,
          ns_op.to_formatted_string(&Locale::en),
          min_op.to_formatted_string(&Locale::en),
          max_op.to_formatted_string(&Locale::en),
          colors::green("ok")
        )
      }
      BenchResult::Ignored => colors::yellow("ignored").to_string(),
      BenchResult::Failed(_) => colors::red("FAILED").to_string(),
    };

    println!("{} {}", status, colors::gray(human_elapsed(elapsed.into())));
  }

  fn report_summary(&mut self, summary: &BenchSummary, elapsed: &Duration) {
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
      "\nbench result: {}. {} passed; {} failed; {} ignored; {} measured; {} filtered out {}\n",
      status,
      summary.passed,
      summary.failed,
      summary.ignored,
      summary.measured,
      summary.filtered_out,
      colors::gray(human_elapsed(elapsed.as_millis())),
    );
  }
}

fn create_reporter(echo_output: bool) -> Box<dyn BenchReporter + Send> {
  Box::new(PrettyBenchReporter::new(echo_output))
}

fn fetch_specifiers(
  include: Vec<String>,
  ignore: Vec<PathBuf>,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  collect_specifiers(include, &ignore, is_supported_bench_path)
}

/// Type check a collection of module and document specifiers.
async fn check_specifiers(
  ps: &ProcState,
  permissions: Permissions,
  specifiers: Vec<ModuleSpecifier>,
  lib: emit::TypeLib,
) -> Result<(), AnyError> {
  let module_specifiers = specifiers
    .into_iter()
    .map(|s| (s, ModuleKind::Esm))
    .collect();
  ps.prepare_module_load(
    module_specifiers,
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
  channel: Sender<BenchEvent>,
  options: BenchSpecifierOptions,
) -> Result<(), AnyError> {
  let mut worker = create_main_worker(
    &ps,
    specifier.clone(),
    permissions,
    vec![ops::bench::init(channel.clone())],
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

  let (sender, receiver) = channel::<BenchEvent>();

  let join_handles = specifiers.iter().map(move |specifier| {
    let ps = ps.clone();
    let permissions = permissions.clone();
    let specifier = specifier.clone();
    let sender = sender.clone();
    let options = options.clone();

    tokio::task::spawn_blocking(move || {
      let join_handle = std::thread::spawn(move || {
        let future =
          bench_specifier(ps, permissions, specifier, sender, options);

        run_basic(future)
      });

      join_handle.join().unwrap()
    })
  });

  let join_stream = stream::iter(join_handles)
    .buffer_unordered(1)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let mut reporter = create_reporter(log_level != Some(Level::Error));

  let handler = {
    tokio::task::spawn_blocking(move || {
      let earlier = Instant::now();
      let mut summary = BenchSummary::new();
      let mut used_only = false;

      for event in receiver.iter() {
        match event {
          BenchEvent::Plan(plan) => {
            summary.total += plan.total;
            summary.filtered_out += plan.filtered_out;

            if plan.used_only {
              used_only = true;
            }

            reporter.report_plan(&plan);
          }

          BenchEvent::Wait(description) => {
            reporter.report_wait(&description);
            summary.current_bench = BenchMeasures {
              iterations: description.iterations,
              current_start: Instant::now(),
              measures: Vec::with_capacity(
                description.iterations.try_into().unwrap(),
              ),
            };
          }

          BenchEvent::Output(output) => {
            reporter.report_output(&output);
          }

          BenchEvent::IterationStart(_iteration) => {
            summary.current_bench.current_start = Instant::now();
          }

          BenchEvent::IterationFinish(_iteration) => {
            let duration_of_iter =
              Instant::now() - summary.current_bench.current_start;
            summary
              .current_bench
              .measures
              .push(duration_of_iter.as_nanos())
          }

          BenchEvent::Result(description, result, elapsed) => {
            match &result {
              BenchResult::Ok => {
                summary.passed += 1;
              }
              BenchResult::Ignored => {
                summary.ignored += 1;
              }
              BenchResult::Failed(error) => {
                summary.failed += 1;
                summary.failures.push((description.clone(), error.clone()));
              }
            }

            reporter.report_result(
              &description,
              &result,
              elapsed,
              &summary.current_bench,
            );
          }
        }
      }

      let elapsed = Instant::now().duration_since(earlier);
      reporter.report_summary(&summary, &elapsed);

      if used_only {
        return Err(generic_error(
          "Bench failed because the \"only\" option was used",
        ));
      }

      if summary.failed > 0 {
        return Err(generic_error("Bench failed"));
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
    bench_flags.include.unwrap_or_else(|| vec![".".to_string()]),
    bench_flags.ignore.clone(),
  )?;

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
  _flags: Flags,
  _bench_flags: BenchFlags,
) -> Result<(), AnyError> {
  todo!()
}
