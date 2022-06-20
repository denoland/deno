// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache;
use crate::colors;
use crate::compat;
use crate::create_main_worker;
use crate::emit;
use crate::file_watcher;
use crate::file_watcher::ResolutionResult;
use crate::flags::BenchFlags;
use crate::flags::Flags;
use crate::flags::TypeCheckMode;
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
use crate::tools::test::format_test_error;
use crate::tools::test::TestFilter;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::ModuleSpecifier;
use deno_graph::ModuleKind;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_basic;
use indexmap::IndexMap;
use log::Level;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Deserialize)]
struct BenchSpecifierOptions {
  compat_mode: bool,
  filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

fn create_reporter(show_output: bool) -> Box<dyn BenchReporter + Send> {
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

        if None == self.group || group != self.group.as_ref().unwrap() {
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
  let filter = TestFilter::from_flag(&options.filter);
  let mut worker = create_main_worker(
    &ps,
    specifier.clone(),
    permissions,
    vec![ops::bench::init(channel.clone(), filter, ps.flags.unstable)],
    Default::default(),
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
    r#"Deno[Deno.internal].runBenchmarks()"#,
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

  let handler = {
    tokio::task::spawn(async move {
      let mut used_only = false;
      let mut report = BenchReport::new();
      let mut reporter = create_reporter(log_level != Some(Level::Error));
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
  let no_check = ps.flags.type_check_mode == TypeCheckMode::None;

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
        &mut cache,
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
