// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::BenchFlags;
use crate::args::BenchReporterFormat;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::colors;
use crate::display::write_json_to_stdout;
use crate::factory::CliFactory;
use crate::factory::CliFactoryBuilder;
use crate::graph_util::has_graph_root_local_dependent_changed;
use crate::ops;
use crate::tools::test::format_test_error;
use crate::tools::test::TestFilter;
use crate::util::file_watcher;
use crate::util::fs::collect_specifiers;
use crate::util::path::is_script_ext;
use crate::util::path::matches_pattern_or_exact_path;
use crate::version::get_user_agent;
use crate::worker::CliMainWorkerFactory;

use deno_config::glob::WalkEntry;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::serde_v8;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::PollEventLoopOptions;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::tokio_util::create_and_run_current_thread;
use deno_runtime::WorkerExecutionMode;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::Level;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedSender;

mod mitata;
mod reporters;

use reporters::{BenchReporter, ConsoleReporter, CsvReporter, JsonReporter};

#[derive(Debug, Clone)]
struct BenchSpecifierOptions {
  filter: TestFilter,
  format: BenchReporterFormat,
  log_level: Option<log::Level>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchPlan {
  pub total: usize,
  pub origin: String,
  pub used_only: bool,
  pub names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BenchEvent {
  Plan(BenchPlan),
  Output(String),
  Register(BenchDescription),
  Wait(usize),
  Result(usize, BenchResult),
  UncaughtError(String, Box<JsError>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BenchResult {
  Ok(BenchStats),
  Failed(Box<JsError>),
}

#[derive(Debug, Clone)]
pub struct BenchReport {
  pub total: usize,
  pub failed: usize,
  pub failures: Vec<(BenchDescription, Box<JsError>)>,
  pub measurements: Vec<(BenchDescription, BenchStats)>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
pub struct BenchDescription {
  pub id: usize,
  pub name: String,
  pub origin: String,
  pub baseline: bool,
  pub group: Option<String>,
  pub ignore: bool,
  pub only: bool,
  pub warmup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchStats {
  pub n: u64,
  pub min: f64,
  pub max: f64,
  pub avg: f64,
  pub p75: f64,
  pub p99: f64,
  pub p995: f64,
  pub p999: f64,
  pub high_precision: bool,
  pub used_explicit_timers: bool,
}

impl BenchReport {
  pub fn new() -> Self {
    Self {
      total: 0,
      failed: 0,
      failures: Vec::new(),
      measurements: Vec::new(),
    }
  }
}

fn create_reporter(
  show_output: bool,
  format: BenchReporterFormat,
) -> Box<dyn BenchReporter + Send> {
  match format {
    BenchReporterFormat::Console => Box::new(ConsoleReporter::new(show_output)),
    BenchReporterFormat::Json => Box::new(JsonReporter::new()),
    BenchReporterFormat::Csv => Box::new(CsvReporter::new()),
  }
}

/// Run a single specifier as an executable bench module.
async fn bench_specifier(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  sender: UnboundedSender<BenchEvent>,
  filter: TestFilter,
) -> Result<(), AnyError> {
  match bench_specifier_inner(
    worker_factory,
    permissions,
    specifier.clone(),
    &sender,
    filter,
  )
  .await
  {
    Ok(()) => Ok(()),
    Err(error) => {
      if error.is::<JsError>() {
        sender.send(BenchEvent::UncaughtError(
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

/// Run a single specifier as an executable bench module.
async fn bench_specifier_inner(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  sender: &UnboundedSender<BenchEvent>,
  filter: TestFilter,
) -> Result<(), AnyError> {
  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Bench,
      specifier.clone(),
      PermissionsContainer::new(permissions),
      vec![ops::bench::deno_bench::init_ops(sender.clone())],
      Default::default(),
    )
    .await?;

  // We execute the main module as a side module so that import.meta.main is not set.
  worker.execute_side_module_possibly_with_npm().await?;

  let mut worker = worker.into_main_worker();

  // Ensure that there are no pending exceptions before we start running tests
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  worker.dispatch_load_event()?;

  let benchmarks = {
    let state_rc = worker.js_runtime.op_state();
    let mut state = state_rc.borrow_mut();
    std::mem::take(&mut state.borrow_mut::<ops::bench::BenchContainer>().0)
  };
  let (only, no_only): (Vec<_>, Vec<_>) =
    benchmarks.into_iter().partition(|(d, _)| d.only);
  let used_only = !only.is_empty();
  let benchmarks = if used_only { only } else { no_only };
  let mut benchmarks = benchmarks
    .into_iter()
    .filter(|(d, _)| d.warmup || filter.includes(&d.name) && !d.ignore)
    .collect::<Vec<_>>();
  let mut groups = IndexSet::<Option<String>>::new();
  // make sure ungrouped benchmarks are placed above grouped
  groups.insert(None);
  for (desc, _) in &benchmarks {
    groups.insert(desc.group.clone());
  }
  benchmarks.sort_by(|(d1, _), (d2, _)| {
    groups
      .get_index_of(&d1.group)
      .unwrap()
      .partial_cmp(&groups.get_index_of(&d2.group).unwrap())
      .unwrap()
  });
  sender.send(BenchEvent::Plan(BenchPlan {
    origin: specifier.to_string(),
    total: benchmarks.len(),
    used_only,
    names: benchmarks.iter().map(|(d, _)| d.name.clone()).collect(),
  }))?;
  for (desc, function) in benchmarks {
    sender.send(BenchEvent::Wait(desc.id))?;
    let call = worker.js_runtime.call(&function);
    let result = worker
      .js_runtime
      .with_event_loop_promise(call, PollEventLoopOptions::default())
      .await?;
    let scope = &mut worker.js_runtime.handle_scope();
    let result = v8::Local::new(scope, result);
    let result = serde_v8::from_v8::<BenchResult>(scope, result)?;
    sender.send(BenchEvent::Result(desc.id, result))?;
  }

  // Ignore `defaultPrevented` of the `beforeunload` event. We don't allow the
  // event loop to continue beyond what's needed to await results.
  worker.dispatch_beforeunload_event()?;
  worker.dispatch_process_beforeexit_event()?;
  worker.dispatch_unload_event()?;
  worker.dispatch_process_exit_event()?;

  // Ensure the worker has settled so we can catch any remaining unhandled rejections. We don't
  // want to wait forever here.
  worker.run_up_to_duration(Duration::from_millis(0)).await?;

  Ok(())
}

/// Test a collection of specifiers with test modes concurrently.
async fn bench_specifiers(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: &Permissions,
  specifiers: Vec<ModuleSpecifier>,
  options: BenchSpecifierOptions,
) -> Result<(), AnyError> {
  let (sender, mut receiver) = unbounded_channel::<BenchEvent>();
  let log_level = options.log_level;
  let option_for_handles = options.clone();

  let join_handles = specifiers.into_iter().map(move |specifier| {
    let worker_factory = worker_factory.clone();
    let permissions = permissions.clone();
    let sender = sender.clone();
    let options = option_for_handles.clone();
    spawn_blocking(move || {
      let future = bench_specifier(
        worker_factory,
        permissions,
        specifier,
        sender,
        options.filter,
      );
      create_and_run_current_thread(future)
    })
  });

  let join_stream = stream::iter(join_handles)
    .buffer_unordered(1)
    .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

  let handler = {
    spawn(async move {
      let mut used_only = false;
      let mut report = BenchReport::new();
      let mut reporter =
        create_reporter(log_level != Some(Level::Error), options.format);
      let mut benches = IndexMap::new();

      while let Some(event) = receiver.recv().await {
        match event {
          BenchEvent::Plan(plan) => {
            report.total += plan.total;
            if plan.used_only {
              used_only = true;
            }

            reporter.report_plan(&plan);
          }

          BenchEvent::Register(desc) => {
            reporter.report_register(&desc);
            benches.insert(desc.id, desc);
          }

          BenchEvent::Wait(id) => {
            reporter.report_wait(benches.get(&id).unwrap());
          }

          BenchEvent::Output(output) => {
            reporter.report_output(&output);
          }

          BenchEvent::Result(id, result) => {
            let desc = benches.get(&id).unwrap();
            reporter.report_result(desc, &result);
            match result {
              BenchResult::Ok(stats) => {
                report.measurements.push((desc.clone(), stats));
              }

              BenchResult::Failed(failure) => {
                report.failed += 1;
                report.failures.push((desc.clone(), failure));
              }
            };
          }

          BenchEvent::UncaughtError(origin, error) => {
            report.failed += 1;
            reporter.report_uncaught_error(&origin, error);
          }
        }
      }

      reporter.report_end(&report);

      if used_only {
        return Err(generic_error(
          "Bench failed because the \"only\" option was used",
        ));
      }

      if report.failed > 0 {
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

/// Checks if the path has a basename and extension Deno supports for benches.
fn is_supported_bench_path(entry: WalkEntry) -> bool {
  if !is_script_ext(entry.path) {
    false
  } else if has_supported_bench_path_name(entry.path) {
    true
  } else if let Some(include) = &entry.patterns.include {
    // allow someone to explicitly specify a path
    matches_pattern_or_exact_path(include, entry.path)
  } else {
    false
  }
}

fn has_supported_bench_path_name(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    basename.ends_with("_bench")
      || basename.ends_with(".bench")
      || basename == "bench"
  } else {
    false
  }
}

pub async fn run_benchmarks(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<(), AnyError> {
  let cli_options = CliOptions::from_flags(flags)?;
  let workspace_bench_options =
    cli_options.resolve_workspace_bench_options(&bench_flags);
  let factory = CliFactory::from_cli_options(Arc::new(cli_options));
  let cli_options = factory.cli_options();
  // Various bench files should not share the same permissions in terms of
  // `PermissionsContainer` - otherwise granting/revoking permissions in one
  // file would have impact on other files, which is undesirable.
  let permissions =
    Permissions::from_options(&cli_options.permissions_options()?)?;

  let members_with_bench_options =
    cli_options.resolve_bench_options_for_members(&bench_flags)?;
  let specifiers = members_with_bench_options
    .iter()
    .map(|(_, bench_options)| {
      collect_specifiers(
        bench_options.files.clone(),
        cli_options.vendor_dir_path().map(ToOwned::to_owned),
        is_supported_bench_path,
      )
    })
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

  if specifiers.is_empty() {
    return Err(generic_error("No bench modules found"));
  }

  let main_graph_container = factory.main_module_graph_container().await?;
  main_graph_container.check_specifiers(&specifiers).await?;

  if workspace_bench_options.no_run {
    return Ok(());
  }

  let log_level = cli_options.log_level();
  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);
  bench_specifiers(
    worker_factory,
    &permissions,
    specifiers,
    BenchSpecifierOptions {
      filter: TestFilter::from_flag(&workspace_bench_options.filter),
      format: workspace_bench_options.format,
      log_level,
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
  file_watcher::watch_func(
    flags,
    file_watcher::PrintConfig::new(
      "Bench",
      bench_flags
        .watch
        .as_ref()
        .map(|w| !w.no_clear_screen)
        .unwrap_or(true),
    ),
    move |flags, watcher_communicator, changed_paths| {
      let bench_flags = bench_flags.clone();
      Ok(async move {
        let factory = CliFactoryBuilder::new()
          .build_from_flags_for_watcher(flags, watcher_communicator.clone())?;
        let cli_options = factory.cli_options();
        let workspace_bench_options =
          cli_options.resolve_workspace_bench_options(&bench_flags);

        let _ = watcher_communicator.watch_paths(cli_options.watch_paths());

        let graph_kind = cli_options.type_check_mode().as_graph_kind();
        let module_graph_creator = factory.module_graph_creator().await?;
        let members_with_bench_options =
          cli_options.resolve_bench_options_for_members(&bench_flags)?;
        let watch_paths = members_with_bench_options
          .iter()
          .filter_map(|(_, bench_options)| {
            bench_options
              .files
              .include
              .as_ref()
              .map(|set| set.base_paths())
          })
          .flatten()
          .collect::<Vec<_>>();
        let _ = watcher_communicator.watch_paths(watch_paths);
        let collected_bench_modules = members_with_bench_options
          .iter()
          .map(|(_, bench_options)| {
            collect_specifiers(
              bench_options.files.clone(),
              cli_options.vendor_dir_path().map(ToOwned::to_owned),
              is_supported_bench_path,
            )
          })
          .collect::<Result<Vec<_>, _>>()?
          .into_iter()
          .flatten()
          .collect::<Vec<_>>();

        // Various bench files should not share the same permissions in terms of
        // `PermissionsContainer` - otherwise granting/revoking permissions in one
        // file would have impact on other files, which is undesirable.
        let permissions =
          Permissions::from_options(&cli_options.permissions_options()?)?;

        let graph = module_graph_creator
          .create_graph(graph_kind, collected_bench_modules.clone())
          .await?;
        module_graph_creator.graph_valid(&graph)?;
        let bench_modules = &graph.roots;

        let bench_modules_to_reload = if let Some(changed_paths) = changed_paths
        {
          let changed_paths = changed_paths.into_iter().collect::<HashSet<_>>();
          let mut result = IndexSet::with_capacity(bench_modules.len());
          for bench_module_specifier in bench_modules {
            if has_graph_root_local_dependent_changed(
              &graph,
              bench_module_specifier,
              &changed_paths,
            ) {
              result.insert(bench_module_specifier.clone());
            }
          }
          result
        } else {
          bench_modules.clone()
        };

        let worker_factory =
          Arc::new(factory.create_cli_main_worker_factory().await?);

        let specifiers = collected_bench_modules
          .into_iter()
          .filter(|specifier| bench_modules_to_reload.contains(specifier))
          .collect::<Vec<ModuleSpecifier>>();

        factory
          .main_module_graph_container()
          .await?
          .check_specifiers(&specifiers)
          .await?;

        if workspace_bench_options.no_run {
          return Ok(());
        }

        let log_level = cli_options.log_level();
        bench_specifiers(
          worker_factory,
          &permissions,
          specifiers,
          BenchSpecifierOptions {
            filter: TestFilter::from_flag(&workspace_bench_options.filter),
            format: workspace_bench_options.format,
            log_level,
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
