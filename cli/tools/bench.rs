// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::BenchOptions;
use crate::args::CliOptions;
use crate::args::TypeCheckMode;
use crate::colors;
use crate::display::write_json_to_stdout;
use crate::factory::CliFactory;
use crate::graph_util::graph_valid_with_cli_options;
use crate::module_loader::ModuleLoadPreparer;
use crate::ops;
use crate::tools::test::format_test_error;
use crate::tools::test::TestFilter;
use crate::util::file_watcher;
use crate::util::file_watcher::ResolutionResult;
use crate::util::fs::collect_specifiers;
use crate::util::path::is_supported_ext;
use crate::version::get_user_agent;
use crate::worker::CliMainWorkerFactory;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::located_script_name;
use deno_core::serde_v8;
use deno_core::task::spawn;
use deno_core::task::spawn_blocking;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_graph::GraphKind;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::tokio_util::create_and_run_current_thread;
use indexmap::IndexMap;
use indexmap::IndexSet;
use log::Level;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
struct BenchSpecifierOptions {
  filter: TestFilter,
  json: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchStats {
  pub n: u64,
  pub min: f64,
  pub max: f64,
  pub avg: f64,
  pub p75: f64,
  pub p99: f64,
  pub p995: f64,
  pub p999: f64,
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
  json: bool,
) -> Box<dyn BenchReporter + Send> {
  if json {
    return Box::new(JsonReporter::new());
  }
  Box::new(ConsoleReporter::new(show_output))
}

pub trait BenchReporter {
  fn report_group_summary(&mut self);
  fn report_plan(&mut self, plan: &BenchPlan);
  fn report_end(&mut self, report: &BenchReport);
  fn report_register(&mut self, desc: &BenchDescription);
  fn report_wait(&mut self, desc: &BenchDescription);
  fn report_output(&mut self, output: &str);
  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult);
}

#[derive(Debug, Serialize)]
struct JsonReporterOutput {
  runtime: String,
  cpu: String,
  benches: Vec<JsonReporterBench>,
}

impl Default for JsonReporterOutput {
  fn default() -> Self {
    Self {
      runtime: format!("{} {}", get_user_agent(), env!("TARGET")),
      cpu: mitata::cpu::name(),
      benches: vec![],
    }
  }
}

#[derive(Debug, Serialize)]
struct JsonReporterBench {
  origin: String,
  group: Option<String>,
  name: String,
  baseline: bool,
  results: Vec<BenchResult>,
}

#[derive(Debug, Serialize)]
struct JsonReporter(JsonReporterOutput);

impl JsonReporter {
  fn new() -> Self {
    Self(Default::default())
  }
}

impl BenchReporter for JsonReporter {
  fn report_group_summary(&mut self) {}
  #[cold]
  fn report_plan(&mut self, _plan: &BenchPlan) {}

  fn report_end(&mut self, _report: &BenchReport) {
    match write_json_to_stdout(self) {
      Ok(_) => (),
      Err(e) => println!("{e}"),
    }
  }

  fn report_register(&mut self, _desc: &BenchDescription) {}

  fn report_wait(&mut self, _desc: &BenchDescription) {}

  fn report_output(&mut self, _output: &str) {}

  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult) {
    let maybe_bench = self.0.benches.iter_mut().find(|bench| {
      bench.origin == desc.origin
        && bench.group == desc.group
        && bench.name == desc.name
        && bench.baseline == desc.baseline
    });

    if let Some(bench) = maybe_bench {
      bench.results.push(result.clone());
    } else {
      self.0.benches.push(JsonReporterBench {
        origin: desc.origin.clone(),
        group: desc.group.clone(),
        name: desc.name.clone(),
        baseline: desc.baseline,
        results: vec![result.clone()],
      });
    }
  }
}

struct ConsoleReporter {
  name: String,
  show_output: bool,
  has_ungrouped: bool,
  group: Option<String>,
  baseline: bool,
  group_measurements: Vec<(BenchDescription, BenchStats)>,
  options: Option<mitata::reporter::Options>,
}

impl ConsoleReporter {
  fn new(show_output: bool) -> Self {
    Self {
      show_output,
      group: None,
      options: None,
      baseline: false,
      name: String::new(),
      has_ungrouped: false,
      group_measurements: Vec::new(),
    }
  }
}

impl BenchReporter for ConsoleReporter {
  #[cold]
  fn report_plan(&mut self, plan: &BenchPlan) {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    static FIRST_PLAN: AtomicBool = AtomicBool::new(true);

    self.report_group_summary();

    self.group = None;
    self.baseline = false;
    self.name = String::new();
    self.group_measurements.clear();
    self.options = Some(mitata::reporter::Options::new(
      &plan.names.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
    ));

    let options = self.options.as_mut().unwrap();

    options.percentiles = true;
    options.colors = colors::use_color();

    if FIRST_PLAN
      .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      println!("{}", colors::gray(format!("cpu: {}", mitata::cpu::name())));
      println!(
        "{}\n",
        colors::gray(format!(
          "runtime: deno {} ({})",
          crate::version::deno(),
          env!("TARGET")
        ))
      );
    } else {
      println!();
    }

    println!(
      "{}\n{}\n{}",
      colors::gray(&plan.origin),
      mitata::reporter::header(options),
      mitata::reporter::br(options)
    );
  }

  fn report_register(&mut self, _desc: &BenchDescription) {}

  fn report_wait(&mut self, desc: &BenchDescription) {
    self.name = desc.name.clone();

    match &desc.group {
      None => {
        self.has_ungrouped = true;
      }

      Some(group) => {
        if self.group.is_none()
          && self.has_ungrouped
          && self.group_measurements.is_empty()
        {
          println!();
        }

        if self.group.is_none() || group != self.group.as_ref().unwrap() {
          self.report_group_summary();
        }

        if (self.group.is_none() && self.has_ungrouped)
          || (self.group.is_some() && self.group_measurements.is_empty())
        {
          println!();
        }

        self.group = Some(group.clone());
      }
    }
  }

  fn report_output(&mut self, output: &str) {
    if self.show_output {
      print!("{} {}", colors::gray(format!("{}:", self.name)), output)
    }
  }

  fn report_result(&mut self, desc: &BenchDescription, result: &BenchResult) {
    let options = self.options.as_ref().unwrap();

    match result {
      BenchResult::Ok(stats) => {
        let mut desc = desc.clone();

        if desc.baseline && !self.baseline {
          self.baseline = true;
        } else {
          desc.baseline = false;
        }

        println!(
          "{}",
          mitata::reporter::benchmark(
            &desc.name,
            &mitata::reporter::BenchmarkStats {
              avg: stats.avg,
              min: stats.min,
              max: stats.max,
              p75: stats.p75,
              p99: stats.p99,
              p995: stats.p995,
            },
            options
          )
        );

        self.group_measurements.push((desc, stats.clone()));
      }

      BenchResult::Failed(js_error) => {
        println!(
          "{}",
          mitata::reporter::benchmark_error(
            &desc.name,
            &mitata::reporter::Error {
              stack: None,
              message: format_test_error(js_error),
            },
            options
          )
        )
      }
    };
  }

  fn report_group_summary(&mut self) {
    let options = match self.options.as_ref() {
      None => return,
      Some(options) => options,
    };

    if 2 <= self.group_measurements.len()
      && (self.group.is_some() || (self.group.is_none() && self.baseline))
    {
      println!(
        "\n{}",
        mitata::reporter::summary(
          &self
            .group_measurements
            .iter()
            .map(|(d, s)| mitata::reporter::GroupBenchmark {
              name: d.name.clone(),
              baseline: d.baseline,
              group: d.group.as_deref().unwrap_or("").to_owned(),

              stats: mitata::reporter::BenchmarkStats {
                avg: s.avg,
                min: s.min,
                max: s.max,
                p75: s.p75,
                p99: s.p99,
                p995: s.p995,
              },
            })
            .collect::<Vec<mitata::reporter::GroupBenchmark>>(),
          options
        )
      );
    }

    self.baseline = false;
    self.group_measurements.clear();
  }

  fn report_end(&mut self, _: &BenchReport) {
    self.report_group_summary();
  }
}

/// Type check a collection of module and document specifiers.
async fn check_specifiers(
  cli_options: &CliOptions,
  module_load_preparer: &ModuleLoadPreparer,
  specifiers: Vec<ModuleSpecifier>,
) -> Result<(), AnyError> {
  let lib = cli_options.ts_type_lib_window();
  module_load_preparer
    .prepare_module_load(
      specifiers,
      false,
      lib,
      PermissionsContainer::allow_all(),
    )
    .await?;
  Ok(())
}

/// Run a single specifier as an executable bench module.
async fn bench_specifier(
  worker_factory: Arc<CliMainWorkerFactory>,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  sender: UnboundedSender<BenchEvent>,
  filter: TestFilter,
) -> Result<(), AnyError> {
  let mut worker = worker_factory
    .create_custom_worker(
      specifier.clone(),
      PermissionsContainer::new(permissions),
      vec![ops::bench::deno_bench::init_ops(sender.clone())],
      Default::default(),
    )
    .await?;

  // We execute the main module as a side module so that import.meta.main is not set.
  worker.execute_side_module_possibly_with_npm().await?;

  let mut worker = worker.into_main_worker();
  worker.dispatch_load_event(located_script_name!())?;

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
    .filter(|(d, _)| filter.includes(&d.name) && !d.ignore)
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
    let result = worker.js_runtime.call_and_await(&function).await?;
    let scope = &mut worker.js_runtime.handle_scope();
    let result = v8::Local::new(scope, result);
    let result = serde_v8::from_v8::<BenchResult>(scope, result)?;
    sender.send(BenchEvent::Result(desc.id, result))?;
  }

  // Ignore `defaultPrevented` of the `beforeunload` event. We don't allow the
  // event loop to continue beyond what's needed to await results.
  worker.dispatch_beforeunload_event(located_script_name!())?;
  worker.dispatch_unload_event(located_script_name!())?;
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
    let specifier = specifier;
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
        create_reporter(log_level != Some(Level::Error), options.json);
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
fn is_supported_bench_path(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    (basename.ends_with("_bench")
      || basename.ends_with(".bench")
      || basename == "bench")
      && is_supported_ext(path)
  } else {
    false
  }
}

pub async fn run_benchmarks(
  cli_options: CliOptions,
  bench_options: BenchOptions,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_cli_options(Arc::new(cli_options));
  let cli_options = factory.cli_options();
  // Various bench files should not share the same permissions in terms of
  // `PermissionsContainer` - otherwise granting/revoking permissions in one
  // file would have impact on other files, which is undesirable.
  let permissions =
    Permissions::from_options(&cli_options.permissions_options())?;

  let specifiers =
    collect_specifiers(&bench_options.files, is_supported_bench_path)?;

  if specifiers.is_empty() {
    return Err(generic_error("No bench modules found"));
  }

  check_specifiers(
    cli_options,
    factory.module_load_preparer().await?,
    specifiers.clone(),
  )
  .await?;

  if bench_options.no_run {
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
      filter: TestFilter::from_flag(&bench_options.filter),
      json: bench_options.json,
      log_level,
    },
  )
  .await?;

  Ok(())
}

// TODO(bartlomieju): heavy duplication of code with `cli/tools/test.rs`
pub async fn run_benchmarks_with_watch(
  cli_options: CliOptions,
  bench_options: BenchOptions,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_cli_options(Arc::new(cli_options));
  let cli_options = factory.cli_options();
  let module_graph_builder = factory.module_graph_builder().await?;
  let file_watcher = factory.file_watcher()?;
  let module_load_preparer = factory.module_load_preparer().await?;
  // Various bench files should not share the same permissions in terms of
  // `PermissionsContainer` - otherwise granting/revoking permissions in one
  // file would have impact on other files, which is undesirable.
  let permissions =
    Permissions::from_options(&cli_options.permissions_options())?;
  let type_check = cli_options.type_check_mode() != TypeCheckMode::None;
  let graph_kind = match type_check {
    true => GraphKind::All,
    false => GraphKind::CodeOnly,
  };

  let resolver = |changed: Option<Vec<PathBuf>>| {
    let paths_to_watch = bench_options.files.include.clone();
    let paths_to_watch_clone = paths_to_watch.clone();
    let files_changed = changed.is_some();
    let bench_options = &bench_options;
    let module_graph_builder = module_graph_builder.clone();
    let cli_options = cli_options.clone();

    async move {
      let bench_modules =
        collect_specifiers(&bench_options.files, is_supported_bench_path)?;

      let mut paths_to_watch = paths_to_watch_clone;
      let mut modules_to_reload = if files_changed {
        Vec::new()
      } else {
        bench_modules.clone()
      };
      let graph = module_graph_builder
        .create_graph(graph_kind, bench_modules.clone())
        .await?;
      graph_valid_with_cli_options(&graph, &bench_modules, &cli_options)?;

      // TODO(@kitsonk) - This should be totally derivable from the graph.
      for specifier in bench_modules {
        fn get_dependencies<'a>(
          graph: &'a deno_graph::ModuleGraph,
          maybe_module: Option<&'a deno_graph::Module>,
          // This needs to be accessible to skip getting dependencies if they're already there,
          // otherwise this will cause a stack overflow with circular dependencies
          output: &mut HashSet<&'a ModuleSpecifier>,
        ) {
          if let Some(module) = maybe_module.and_then(|m| m.esm()) {
            for dep in module.dependencies.values() {
              if let Some(specifier) = &dep.get_code() {
                if !output.contains(specifier) {
                  output.insert(specifier);
                  get_dependencies(graph, graph.get(specifier), output);
                }
              }
              if let Some(specifier) = &dep.get_type() {
                if !output.contains(specifier) {
                  output.insert(specifier);
                  get_dependencies(graph, graph.get(specifier), output);
                }
              }
            }
          }
        }

        // This bench module and all it's dependencies
        let mut modules = HashSet::new();
        modules.insert(&specifier);
        get_dependencies(&graph, graph.get(&specifier), &mut modules);

        paths_to_watch.extend(
          modules
            .iter()
            .filter_map(|specifier| specifier.to_file_path().ok()),
        );

        if let Some(changed) = &changed {
          for path in changed
            .iter()
            .filter_map(|path| ModuleSpecifier::from_file_path(path).ok())
          {
            if modules.contains(&path) {
              modules_to_reload.push(specifier);
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

  let create_cli_main_worker_factory =
    factory.create_cli_main_worker_factory_func().await?;
  let operation = |modules_to_reload: Vec<ModuleSpecifier>| {
    let permissions = &permissions;
    let bench_options = &bench_options;
    file_watcher.reset();
    let module_load_preparer = module_load_preparer.clone();
    let cli_options = cli_options.clone();
    let create_cli_main_worker_factory = create_cli_main_worker_factory.clone();

    async move {
      let worker_factory = Arc::new(create_cli_main_worker_factory());
      let specifiers =
        collect_specifiers(&bench_options.files, is_supported_bench_path)?
          .into_iter()
          .filter(|specifier| modules_to_reload.contains(specifier))
          .collect::<Vec<ModuleSpecifier>>();

      check_specifiers(&cli_options, &module_load_preparer, specifiers.clone())
        .await?;

      if bench_options.no_run {
        return Ok(());
      }

      let log_level = cli_options.log_level();
      bench_specifiers(
        worker_factory,
        permissions,
        specifiers,
        BenchSpecifierOptions {
          filter: TestFilter::from_flag(&bench_options.filter),
          json: bench_options.json,
          log_level,
        },
      )
      .await?;

      Ok(())
    }
  };

  let clear_screen = !cli_options.no_clear_screen();
  file_watcher::watch_func(
    resolver,
    operation,
    file_watcher::PrintConfig {
      job_name: "Bench".to_string(),
      clear_screen,
    },
  )
  .await?;

  Ok(())
}

mod mitata {
  // Copyright 2022 evanwashere
  //
  // Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
  //
  // The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
  //
  // THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

  use crate::colors;
  use std::str::FromStr;

  fn fmt_duration(time: f64) -> String {
    // SAFETY: this is safe since its just reformatting numbers
    unsafe {
      if time < 1e0 {
        return format!(
          "{} ps",
          f64::from_str(&format!("{:.2}", time * 1e3)).unwrap_unchecked()
        );
      }

      if time < 1e3 {
        return format!(
          "{} ns",
          f64::from_str(&format!("{:.2}", time)).unwrap_unchecked()
        );
      }
      if time < 1e6 {
        return format!(
          "{} µs",
          f64::from_str(&format!("{:.2}", time / 1e3)).unwrap_unchecked()
        );
      }
      if time < 1e9 {
        return format!(
          "{} ms",
          f64::from_str(&format!("{:.2}", time / 1e6)).unwrap_unchecked()
        );
      }
      if time < 1e12 {
        return format!(
          "{} s",
          f64::from_str(&format!("{:.2}", time / 1e9)).unwrap_unchecked()
        );
      }
      if time < 36e11 {
        return format!(
          "{} m",
          f64::from_str(&format!("{:.2}", time / 60e9)).unwrap_unchecked()
        );
      }

      format!(
        "{} h",
        f64::from_str(&format!("{:.2}", time / 36e11)).unwrap_unchecked()
      )
    }
  }

  pub mod cpu {
    #![allow(dead_code)]

    pub fn name() -> String {
      #[cfg(target_os = "linux")]
      return linux();
      #[cfg(target_os = "macos")]
      return macos();
      #[cfg(target_os = "windows")]
      return windows();

      #[allow(unreachable_code)]
      {
        "unknown".to_string()
      }
    }

    pub fn macos() -> String {
      let mut sysctl = std::process::Command::new("sysctl");

      sysctl.arg("-n");
      sysctl.arg("machdep.cpu.brand_string");
      return std::str::from_utf8(
        &sysctl
          .output()
          .map(|x| x.stdout)
          .unwrap_or(Vec::from("unknown")),
      )
      .unwrap()
      .trim()
      .to_string();
    }

    pub fn windows() -> String {
      let mut wmi = std::process::Command::new("wmic");

      wmi.arg("cpu");
      wmi.arg("get");
      wmi.arg("name");

      return match wmi.output() {
        Err(_) => String::from("unknown"),

        Ok(x) => {
          let x = String::from_utf8_lossy(&x.stdout);
          return x.lines().nth(1).unwrap_or("unknown").trim().to_string();
        }
      };
    }

    pub fn linux() -> String {
      let info =
        std::fs::read_to_string("/proc/cpuinfo").unwrap_or(String::new());

      for line in info.lines() {
        let mut iter = line.split(':');
        let key = iter.next().unwrap_or("");

        if key.contains("Hardware")
          || key.contains("Processor")
          || key.contains("chip type")
          || key.contains("model name")
          || key.starts_with("cpu type")
          || key.starts_with("cpu model")
        {
          return iter.next().unwrap_or("unknown").trim().to_string();
        }
      }

      String::from("unknown")
    }
  }

  pub mod reporter {
    use super::*;

    #[derive(Clone, PartialEq)]
    pub struct Error {
      pub message: String,
      pub stack: Option<String>,
    }

    #[derive(Clone, PartialEq)]
    pub struct BenchmarkStats {
      pub avg: f64,
      pub min: f64,
      pub max: f64,
      pub p75: f64,
      pub p99: f64,
      pub p995: f64,
    }

    #[derive(Clone, PartialEq)]
    pub struct GroupBenchmark {
      pub name: String,
      pub group: String,
      pub baseline: bool,
      pub stats: BenchmarkStats,
    }

    #[derive(Clone, PartialEq)]
    pub struct Options {
      size: usize,
      pub avg: bool,
      pub colors: bool,
      pub min_max: bool,
      pub percentiles: bool,
    }

    impl Options {
      pub fn new(names: &[&str]) -> Options {
        Options {
          avg: true,
          colors: true,
          min_max: true,
          size: size(names),
          percentiles: true,
        }
      }
    }

    pub fn size(names: &[&str]) -> usize {
      let mut max = 9;

      for name in names {
        if max < name.len() {
          max = name.len();
        }
      }

      2 + max
    }

    pub fn br(options: &Options) -> String {
      let mut s = String::new();

      s.push_str(&"-".repeat(
        options.size
          + 14 * options.avg as usize
          + 24 * options.min_max as usize,
      ));

      if options.percentiles {
        s.push(' ');
        s.push_str(&"-".repeat(9 + 10 + 10));
      }

      s
    }

    pub fn benchmark_error(n: &str, e: &Error, options: &Options) -> String {
      let size = options.size;
      let mut s = String::new();

      s.push_str(&format!("{:<size$}", n));
      s.push_str(&format!(
        "{}: {}",
        &(if !options.colors {
          "error".to_string()
        } else {
          colors::red("error").to_string()
        }),
        e.message
      ));

      if let Some(ref stack) = e.stack {
        s.push('\n');

        match options.colors {
          false => s.push_str(stack),
          true => s.push_str(&colors::gray(stack).to_string()),
        }
      }

      s
    }

    pub fn header(options: &Options) -> String {
      let size = options.size;
      let mut s = String::new();

      s.push_str(&format!("{:<size$}", "benchmark"));
      if options.avg {
        s.push_str(&format!("{:>14}", "time (avg)"));
      }
      if options.min_max {
        s.push_str(&format!("{:>24}", "(min … max)"));
      }
      if options.percentiles {
        s.push_str(&format!(" {:>9} {:>9} {:>9}", "p75", "p99", "p995"));
      }

      s
    }

    pub fn benchmark(
      name: &str,
      stats: &BenchmarkStats,
      options: &Options,
    ) -> String {
      let size = options.size;
      let mut s = String::new();

      s.push_str(&format!("{:<size$}", name));

      if !options.colors {
        if options.avg {
          s.push_str(&format!(
            "{:>14}",
            format!("{}/iter", fmt_duration(stats.avg))
          ));
        }
        if options.min_max {
          s.push_str(&format!(
            "{:>24}",
            format!(
              "({} … {})",
              fmt_duration(stats.min),
              fmt_duration(stats.max)
            )
          ));
        }
        if options.percentiles {
          s.push_str(&format!(
            " {:>9} {:>9} {:>9}",
            fmt_duration(stats.p75),
            fmt_duration(stats.p99),
            fmt_duration(stats.p995)
          ));
        }
      } else {
        if options.avg {
          s.push_str(&format!(
            "{:>23}",
            format!("{}/iter", colors::yellow(fmt_duration(stats.avg)))
          ));
        }
        if options.min_max {
          s.push_str(&format!(
            "{:>42}",
            format!(
              "({} … {})",
              colors::cyan(fmt_duration(stats.min)),
              colors::magenta(fmt_duration(stats.max))
            )
          ));
        }
        if options.percentiles {
          s.push_str(&format!(
            " {:>18} {:>18} {:>18}",
            colors::magenta(fmt_duration(stats.p75)),
            colors::magenta(fmt_duration(stats.p99)),
            colors::magenta(fmt_duration(stats.p995))
          ));
        }
      }

      s
    }

    pub fn summary(benchmarks: &[GroupBenchmark], options: &Options) -> String {
      let mut s = String::new();
      let mut benchmarks = benchmarks.to_owned();
      benchmarks.sort_by(|a, b| a.stats.avg.partial_cmp(&b.stats.avg).unwrap());
      let baseline = benchmarks
        .iter()
        .find(|b| b.baseline)
        .unwrap_or(&benchmarks[0]);

      if !options.colors {
        s.push_str(&format!("summary\n  {}", baseline.name));

        for b in benchmarks.iter().filter(|b| *b != baseline) {
          let faster = b.stats.avg >= baseline.stats.avg;
          let diff = f64::from_str(&format!(
            "{:.2}",
            1.0 / baseline.stats.avg * b.stats.avg
          ))
          .unwrap();
          let inv_diff = f64::from_str(&format!(
            "{:.2}",
            1.0 / b.stats.avg * baseline.stats.avg
          ))
          .unwrap();
          s.push_str(&format!(
            "\n   {}x times {} than {}",
            if faster { diff } else { inv_diff },
            if faster { "faster" } else { "slower" },
            b.name
          ));
        }
      } else {
        s.push_str(&format!(
          "{}\n  {}",
          colors::bold("summary"),
          colors::cyan_bold(&baseline.name)
        ));

        for b in benchmarks.iter().filter(|b| *b != baseline) {
          let faster = b.stats.avg >= baseline.stats.avg;
          let diff = f64::from_str(&format!(
            "{:.2}",
            1.0 / baseline.stats.avg * b.stats.avg
          ))
          .unwrap();
          let inv_diff = f64::from_str(&format!(
            "{:.2}",
            1.0 / b.stats.avg * baseline.stats.avg
          ))
          .unwrap();
          s.push_str(&format!(
            "\n   {}x {} than {}",
            if faster {
              colors::green(diff.to_string()).to_string()
            } else {
              colors::red(inv_diff.to_string()).to_string()
            },
            if faster { "faster" } else { "slower" },
            colors::cyan_bold(&b.name)
          ));
        }
      }

      s
    }
  }
}
