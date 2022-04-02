// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::cache::CacherLoader;
use crate::colors;
use crate::compat;
use crate::create_main_worker;
use crate::display;
use crate::emit;
use crate::file_watcher;
use crate::file_watcher::ResolutionResult;
use crate::flags::BenchFlags;
use crate::flags::Flags;
use crate::flags::TypecheckMode;
use crate::fs_util::collect_specifiers;
use crate::fs_util::is_supported_bench_path;
use crate::graph_util::contains_specifier;
use crate::graph_util::graph_valid;
use crate::located_script_name;
use crate::lockfile;
use crate::ops;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

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
use num_format::Locale;
use num_format::ToFormattedString;
use serde::Deserialize;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedSender;

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
  IterationTime(u64),
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
    print!(
      "bench {} ... {} iterations ",
      description.name, description.iterations
    );
    // flush for faster feedback when line buffered
    std::io::stdout().flush().unwrap();
  }
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
          "{} ns/iter ({}..{} ns/iter) {}",
          ns_op.to_formatted_string(&Locale::en),
          min_op.to_formatted_string(&Locale::en),
          max_op.to_formatted_string(&Locale::en),
          colors::green("ok")
        )
      }
      BenchResult::Ignored => colors::yellow("ignored").to_string(),
      BenchResult::Failed(_) => colors::red("FAILED").to_string(),
    };

    println!(
      "{} {}",
      status,
      colors::gray(format!("({})", display::human_elapsed(elapsed.into())))
    );
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
      colors::gray(format!("({})", display::human_elapsed(elapsed.as_millis()))),
    );
  }
}

fn create_reporter(echo_output: bool) -> Box<dyn BenchReporter + Send> {
  Box::new(PrettyBenchReporter::new(echo_output))
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

/// Run a single specifier as an executable bench module.
async fn bench_specifier(
  ps: ProcState,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  channel: UnboundedSender<BenchEvent>,
  options: BenchSpecifierOptions,
) -> Result<(), AnyError> {
  let mut worker = create_main_worker(
    &ps,
    specifier.clone(),
    permissions,
    vec![ops::bench::init(channel.clone(), ps.flags.unstable)],
  );

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

  let bench_result = worker.js_runtime.execute_script(
    &located_script_name!(),
    &format!(
      r#"Deno[Deno.internal].runBenchmarks({})"#,
      json!({
        "filter": options.filter,
      }),
    ),
  )?;

  worker.js_runtime.resolve_value(bench_result).await?;

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

  let (sender, mut receiver) = unbounded_channel::<BenchEvent>();

  let join_handles = specifiers.iter().map(move |specifier| {
    let ps = ps.clone();
    let permissions = permissions.clone();
    let specifier = specifier.clone();
    let sender = sender.clone();
    let options = options.clone();

    tokio::task::spawn_blocking(move || {
      let future = bench_specifier(ps, permissions, specifier, sender, options);

      run_basic(future)
    })
  });

  let join_stream = stream::iter(join_handles)
    .buffer_unordered(1)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let mut reporter = create_reporter(log_level != Some(Level::Error));

  let handler = {
    tokio::task::spawn(async move {
      let earlier = Instant::now();
      let mut summary = BenchSummary::new();
      let mut used_only = false;

      while let Some(event) = receiver.recv().await {
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

          BenchEvent::IterationTime(iter_time) => {
            summary.current_bench.measures.push(iter_time.into())
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
  let specifiers = collect_specifiers(
    bench_flags.include.unwrap_or_else(|| vec![".".to_string()]),
    &bench_flags.ignore.clone(),
    is_supported_bench_path,
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

// TODO(bartlomieju): heavy duplication of code with `cli/tools/test.rs`
pub async fn run_benchmarks_with_watch(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<(), AnyError> {
  let flags = Arc::new(flags);
  let ps = ProcState::build(flags.clone()).await?;
  let permissions = Permissions::from_options(&flags.permissions_options());

  let lib = if flags.unstable {
    emit::TypeLib::UnstableDenoWindow
  } else {
    emit::TypeLib::DenoWindow
  };

  let include = bench_flags.include.unwrap_or_else(|| vec![".".to_string()]);
  let ignore = bench_flags.ignore.clone();
  let paths_to_watch: Vec<_> = include.iter().map(PathBuf::from).collect();
  let no_check = ps.flags.typecheck_mode == TypecheckMode::None;

  let resolver = |changed: Option<Vec<PathBuf>>| {
    let mut cache = cache::FetchCacher::new(
      ps.dir.gen_cache.clone(),
      ps.file_fetcher.clone(),
      Permissions::allow_all(),
      Permissions::allow_all(),
    );

    let paths_to_watch = paths_to_watch.clone();
    let paths_to_watch_clone = paths_to_watch.clone();

    let maybe_import_map_resolver =
      ps.maybe_import_map.clone().map(ImportMapResolver::new);
    let maybe_jsx_resolver = ps.maybe_config_file.as_ref().and_then(|cf| {
      cf.to_maybe_jsx_import_source_module()
        .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
    });
    let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
    let maybe_imports = ps
      .maybe_config_file
      .as_ref()
      .map(|cf| cf.to_maybe_imports());
    let files_changed = changed.is_some();
    let include = include.clone();
    let ignore = ignore.clone();
    let check_js = ps
      .maybe_config_file
      .as_ref()
      .map(|cf| cf.get_check_js())
      .unwrap_or(false);

    async move {
      let bench_modules =
        collect_specifiers(include.clone(), &ignore, is_supported_bench_path)?;

      let mut paths_to_watch = paths_to_watch_clone;
      let mut modules_to_reload = if files_changed {
        Vec::new()
      } else {
        bench_modules
          .iter()
          .map(|url| (url.clone(), ModuleKind::Esm))
          .collect()
      };
      let maybe_imports = if let Some(result) = maybe_imports {
        result?
      } else {
        None
      };
      let maybe_resolver = if maybe_jsx_resolver.is_some() {
        maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
      } else {
        maybe_import_map_resolver
          .as_ref()
          .map(|im| im.as_resolver())
      };
      let graph = deno_graph::create_graph(
        bench_modules
          .iter()
          .map(|s| (s.clone(), ModuleKind::Esm))
          .collect(),
        false,
        maybe_imports,
        cache.as_mut_loader(),
        maybe_resolver,
        maybe_locker,
        None,
        None,
      )
      .await;
      graph_valid(&graph, !no_check, check_js)?;

      // TODO(@kitsonk) - This should be totally derivable from the graph.
      for specifier in bench_modules {
        fn get_dependencies<'a>(
          graph: &'a deno_graph::ModuleGraph,
          maybe_module: Option<&'a deno_graph::Module>,
          // This needs to be accessible to skip getting dependencies if they're already there,
          // otherwise this will cause a stack overflow with circular dependencies
          output: &mut HashSet<&'a ModuleSpecifier>,
          no_check: bool,
        ) {
          if let Some(module) = maybe_module {
            for dep in module.dependencies.values() {
              if let Some(specifier) = &dep.get_code() {
                if !output.contains(specifier) {
                  output.insert(specifier);
                  get_dependencies(
                    graph,
                    graph.get(specifier),
                    output,
                    no_check,
                  );
                }
              }
              if !no_check {
                if let Some(specifier) = &dep.get_type() {
                  if !output.contains(specifier) {
                    output.insert(specifier);
                    get_dependencies(
                      graph,
                      graph.get(specifier),
                      output,
                      no_check,
                    );
                  }
                }
              }
            }
          }
        }

        // This bench module and all it's dependencies
        let mut modules = HashSet::new();
        modules.insert(&specifier);
        get_dependencies(&graph, graph.get(&specifier), &mut modules, no_check);

        paths_to_watch.extend(
          modules
            .iter()
            .filter_map(|specifier| specifier.to_file_path().ok()),
        );

        if let Some(changed) = &changed {
          for path in changed.iter().filter_map(|path| {
            deno_core::resolve_url_or_path(&path.to_string_lossy()).ok()
          }) {
            if modules.contains(&&path) {
              modules_to_reload.push((specifier, ModuleKind::Esm));
              break;
            }
          }
        }
      }

      Ok((paths_to_watch, modules_to_reload))
    }
    .map(move |result| {
      if files_changed
        && matches!(result, Ok((_, ref modules)) if modules.is_empty())
      {
        ResolutionResult::Ignore
      } else {
        match result {
          Ok((paths_to_watch, modules_to_reload)) => {
            ResolutionResult::Restart {
              paths_to_watch,
              result: Ok(modules_to_reload),
            }
          }
          Err(e) => ResolutionResult::Restart {
            paths_to_watch,
            result: Err(e),
          },
        }
      }
    })
  };

  let operation = |modules_to_reload: Vec<(ModuleSpecifier, ModuleKind)>| {
    let flags = flags.clone();
    let filter = bench_flags.filter.clone();
    let include = include.clone();
    let ignore = ignore.clone();
    let lib = lib.clone();
    let permissions = permissions.clone();
    let ps = ps.clone();

    async move {
      let specifiers =
        collect_specifiers(include.clone(), &ignore, is_supported_bench_path)?
          .iter()
          .filter(|specifier| contains_specifier(&modules_to_reload, specifier))
          .cloned()
          .collect::<Vec<ModuleSpecifier>>();

      check_specifiers(&ps, permissions.clone(), specifiers.clone(), lib)
        .await?;

      bench_specifiers(
        ps,
        permissions.clone(),
        specifiers,
        BenchSpecifierOptions {
          compat_mode: flags.compat,
          filter: filter.clone(),
        },
      )
      .await?;

      Ok(())
    }
  };

  file_watcher::watch_func(
    resolver,
    operation,
    file_watcher::PrintConfig {
      job_name: "Bench".to_string(),
      clear_screen: !flags.no_clear_screen,
    },
  )
  .await?;

  Ok(())
}
