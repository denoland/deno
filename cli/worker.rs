// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::futures::task::LocalFutureObj;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::Extension;
use deno_core::ModuleId;
use deno_graph::npm::NpmPackageReqReference;
use deno_runtime::colors;
use deno_runtime::deno_node;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::ops::worker_host::WorkerEventCb;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use indexmap::IndexSet;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::sync::mpsc::UnboundedSender;

use crate::args::DenoSubcommand;
use crate::errors;
use crate::module_loader::CliModuleLoader;
use crate::node;
use crate::ops;
use crate::proc_state::ProcState;
use crate::tools;
use crate::tools::bench::BenchEvent;
use crate::tools::bench::BenchPlan;
use crate::tools::bench::BenchResult;
use crate::tools::coverage::CoverageCollector;
use crate::tools::test::FailFastTracker;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFilter;
use crate::tools::test::TestMode;
use crate::tools::test::TestPlan;
use crate::tools::test::TestResult;
use crate::util::checksum;
use crate::version;

pub struct CliMainWorker {
  main_module: ModuleSpecifier,
  is_main_cjs: bool,
  worker: MainWorker,
  ps: ProcState,
}

impl CliMainWorker {
  pub fn into_main_worker(self) -> MainWorker {
    self.worker
  }

  pub async fn setup_repl(&mut self) -> Result<(), AnyError> {
    self.worker.run_event_loop(false).await?;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<i32, AnyError> {
    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;
    log::debug!("main_module {}", self.main_module);

    if self.is_main_cjs {
      self.initialize_main_module_for_node()?;
      deno_node::load_cjs_module(
        &mut self.worker.js_runtime,
        &self.main_module.to_file_path().unwrap().to_string_lossy(),
        true,
        self.ps.options.inspect_brk().is_some(),
      )?;
    } else {
      self.execute_main_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(located_script_name!())?;

    loop {
      self
        .worker
        .run_event_loop(maybe_coverage_collector.is_none())
        .await?;
      if !self
        .worker
        .dispatch_beforeunload_event(located_script_name!())?
      {
        break;
      }
    }

    self.worker.dispatch_unload_event(located_script_name!())?;

    if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
      self
        .worker
        .with_event_loop(coverage_collector.stop_collecting().boxed_local())
        .await?;
    }

    Ok(self.worker.exit_code())
  }

  pub async fn run_for_watcher(self) -> Result<(), AnyError> {
    /// The FileWatcherModuleExecutor provides module execution with safe dispatching of life-cycle events by tracking the
    /// state of any pending events and emitting accordingly on drop in the case of a future
    /// cancellation.
    struct FileWatcherModuleExecutor {
      inner: CliMainWorker,
      pending_unload: bool,
    }

    impl FileWatcherModuleExecutor {
      pub fn new(worker: CliMainWorker) -> FileWatcherModuleExecutor {
        FileWatcherModuleExecutor {
          inner: worker,
          pending_unload: false,
        }
      }

      /// Execute the given main module emitting load and unload events before and after execution
      /// respectively.
      pub async fn execute(&mut self) -> Result<(), AnyError> {
        self.inner.execute_main_module_possibly_with_npm().await?;
        self
          .inner
          .worker
          .dispatch_load_event(located_script_name!())?;
        self.pending_unload = true;

        let result = loop {
          match self.inner.worker.run_event_loop(false).await {
            Ok(()) => {}
            Err(error) => break Err(error),
          }
          match self
            .inner
            .worker
            .dispatch_beforeunload_event(located_script_name!())
          {
            Ok(default_prevented) if default_prevented => {} // continue loop
            Ok(_) => break Ok(()),
            Err(error) => break Err(error),
          }
        };
        self.pending_unload = false;

        result?;

        self
          .inner
          .worker
          .dispatch_unload_event(located_script_name!())?;

        Ok(())
      }
    }

    impl Drop for FileWatcherModuleExecutor {
      fn drop(&mut self) {
        if self.pending_unload {
          let _ = self
            .inner
            .worker
            .dispatch_unload_event(located_script_name!());
        }
      }
    }

    let mut executor = FileWatcherModuleExecutor::new(self);
    executor.execute().await
  }

  pub async fn run_test_specifier(
    &mut self,
    mode: TestMode,
    filter: TestFilter,
    fail_fast_tracker: FailFastTracker,
  ) -> Result<(), AnyError> {
    // Enable op call tracing in core to enable better debugging of op sanitizer
    // failures.
    if self.ps.options.trace_ops() {
      self.worker.js_runtime.execute_script(
        located_script_name!(),
        "Deno[Deno.internal].core.enableOpCallTracing();",
      )?;
    }

    let mut maybe_coverage_collector =
      self.maybe_setup_coverage_collector().await?;

    // We only execute the specifier as a module if it is tagged with TestMode::Module or
    // TestMode::Both.
    if mode != TestMode::Documentation {
      // We execute the module module as a side module so that import.meta.main is not set.
      self.execute_side_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(located_script_name!())?;
    self
      .run_tests(&self.ps.options.shuffle_tests(), filter, fail_fast_tracker)
      .await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }

    self.worker.dispatch_unload_event(located_script_name!())?;

    if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
      self
        .worker
        .with_event_loop(coverage_collector.stop_collecting().boxed_local())
        .await?;
    }
    Ok(())
  }

  pub async fn run_lsp_test_specifier(
    &mut self,
    mode: TestMode,
    filter: TestFilter,
    fail_fast_tracker: FailFastTracker,
  ) -> Result<(), AnyError> {
    self.worker.execute_script(
      located_script_name!(),
      "Deno[Deno.internal].core.enableOpCallTracing();",
    )?;

    if mode != TestMode::Documentation {
      // We execute the module module as a side module so that import.meta.main is not set.
      self.execute_side_module_possibly_with_npm().await?;
    }

    self.worker.dispatch_load_event(located_script_name!())?;
    self.run_tests(&None, filter, fail_fast_tracker).await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }
    self.worker.dispatch_unload_event(located_script_name!())?;
    Ok(())
  }

  pub async fn run_bench_specifier(
    &mut self,
    filter: TestFilter,
  ) -> Result<(), AnyError> {
    // We execute the module module as a side module so that import.meta.main is not set.
    self.execute_side_module_possibly_with_npm().await?;

    self.worker.dispatch_load_event(located_script_name!())?;
    self.run_benchmarks(filter).await?;
    loop {
      if !self
        .worker
        .dispatch_beforeunload_event(located_script_name!())?
      {
        break;
      }
      self.worker.run_event_loop(false).await?;
    }
    self.worker.dispatch_unload_event(located_script_name!())?;
    Ok(())
  }

  async fn execute_main_module_possibly_with_npm(
    &mut self,
  ) -> Result<(), AnyError> {
    let id = self.worker.preload_main_module(&self.main_module).await?;
    self.evaluate_module_possibly_with_npm(id).await
  }

  async fn execute_side_module_possibly_with_npm(
    &mut self,
  ) -> Result<(), AnyError> {
    let id = self.worker.preload_side_module(&self.main_module).await?;
    self.evaluate_module_possibly_with_npm(id).await
  }

  async fn evaluate_module_possibly_with_npm(
    &mut self,
    id: ModuleId,
  ) -> Result<(), AnyError> {
    if self.ps.npm_resolver.has_packages() || self.ps.graph().has_node_specifier
    {
      self.initialize_main_module_for_node()?;
    }
    self.worker.evaluate_module(id).await
  }

  fn initialize_main_module_for_node(&mut self) -> Result<(), AnyError> {
    let mut maybe_binary_command_name = None;

    if let DenoSubcommand::Run(flags) = self.ps.options.sub_command() {
      if let Ok(pkg_ref) = NpmPackageReqReference::from_str(&flags.script) {
        // if the user ran a binary command, we'll need to set process.argv[0]
        // to be the name of the binary command instead of deno
        let binary_name = pkg_ref
          .sub_path
          .as_deref()
          .unwrap_or(pkg_ref.req.name.as_str());
        maybe_binary_command_name = Some(binary_name.to_string());
      }
    }

    deno_node::initialize_runtime(
      &mut self.worker.js_runtime,
      self.ps.options.has_node_modules_dir(),
      maybe_binary_command_name,
    )?;

    Ok(())
  }

  async fn maybe_setup_coverage_collector(
    &mut self,
  ) -> Result<Option<CoverageCollector>, AnyError> {
    if let Some(ref coverage_dir) = self.ps.options.coverage_dir() {
      let session = self.worker.create_inspector_session().await;

      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
      self
        .worker
        .with_event_loop(coverage_collector.start_collecting().boxed_local())
        .await?;
      Ok(Some(coverage_collector))
    } else {
      Ok(None)
    }
  }

  /// Run tests declared with `Deno.test()`. Test events will be dispatched
  /// by calling ops which are currently only implemented in the CLI crate.
  pub async fn run_tests(
    &mut self,
    shuffle: &Option<u64>,
    filter: TestFilter,
    fail_fast_tracker: FailFastTracker,
  ) -> Result<(), AnyError> {
    let (tests, origin, mut sender) = {
      let state_rc = self.worker.js_runtime.op_state();
      let mut state = state_rc.borrow_mut();
      let tests = std::mem::take(
        &mut state.borrow_mut::<ops::testing::TestContainer>().0,
      );
      let origin = state.borrow::<ModuleSpecifier>().clone();
      let sender = state.borrow::<TestEventSender>().clone();
      (tests, origin, sender)
    };
    let unfiltered = tests.len();
    let (only, no_only): (Vec<_>, Vec<_>) =
      tests.into_iter().partition(|(d, _)| d.only);
    let used_only = !only.is_empty();
    let tests = if used_only { only } else { no_only };
    let mut tests = tests
      .into_iter()
      .filter(|(d, _)| filter.includes(&d.name))
      .collect::<Vec<_>>();
    if let Some(seed) = shuffle {
      tests.shuffle(&mut StdRng::seed_from_u64(*seed));
    }
    sender.send(TestEvent::Plan(TestPlan {
      origin: origin.to_string(),
      total: tests.len(),
      filtered_out: unfiltered - tests.len(),
      used_only,
    }))?;
    for (desc, function) in tests {
      if fail_fast_tracker.should_stop() {
        break;
      }
      if desc.ignore {
        sender.send(TestEvent::Result(desc.id, TestResult::Ignored, 0))?;
        continue;
      }
      sender.send(TestEvent::Wait(desc.id))?;
      let earlier = SystemTime::now();
      let promise = {
        let scope = &mut self.worker.js_runtime.handle_scope();
        let cb = function.open(scope);
        let this = v8::undefined(scope).into();
        let promise = cb.call(scope, this, &[]).unwrap();
        v8::Global::new(scope, promise)
      };
      let result = self.worker.js_runtime.resolve_value(promise).await?;
      let scope = &mut self.worker.js_runtime.handle_scope();
      let result = v8::Local::new(scope, result);
      let result = serde_v8::from_v8::<TestResult>(scope, result)?;
      if matches!(result, TestResult::Cancelled | TestResult::Failed(_)) {
        fail_fast_tracker.add_failure();
      }
      let elapsed = SystemTime::now().duration_since(earlier)?.as_millis();
      sender.send(TestEvent::Result(desc.id, result, elapsed as u64))?;
    }
    Ok(())
  }

  /// Run benches declared with `Deno.bench()`. Bench events will be dispatched
  /// by calling ops which are currently only implemented in the CLI crate.
  pub async fn run_benchmarks(
    &mut self,
    filter: TestFilter,
  ) -> Result<(), AnyError> {
    let (benchmarks, origin, sender) = {
      let state_rc = self.worker.js_runtime.op_state();
      let mut state = state_rc.borrow_mut();
      let benchmarks =
        std::mem::take(&mut state.borrow_mut::<ops::bench::BenchContainer>().0);
      let origin = state.borrow::<ModuleSpecifier>().clone();
      let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
      (benchmarks, origin, sender)
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
      origin: origin.to_string(),
      total: benchmarks.len(),
      used_only,
      names: benchmarks.iter().map(|(d, _)| d.name.clone()).collect(),
    }))?;
    for (desc, function) in benchmarks {
      sender.send(BenchEvent::Wait(desc.id))?;
      let promise = {
        let scope = &mut self.worker.js_runtime.handle_scope();
        let cb = function.open(scope);
        let this = v8::undefined(scope).into();
        let promise = cb.call(scope, this, &[]).unwrap();
        v8::Global::new(scope, promise)
      };
      let result = self.worker.js_runtime.resolve_value(promise).await?;
      let scope = &mut self.worker.js_runtime.handle_scope();
      let result = v8::Local::new(scope, result);
      let result = serde_v8::from_v8::<BenchResult>(scope, result)?;
      sender.send(BenchEvent::Result(desc.id, result))?;
    }
    Ok(())
  }
}

pub async fn create_main_worker(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: PermissionsContainer,
) -> Result<CliMainWorker, AnyError> {
  create_main_worker_internal(
    ps,
    main_module,
    permissions,
    vec![],
    Default::default(),
  )
  .await
}

pub async fn create_main_worker_for_test_or_bench(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: PermissionsContainer,
  custom_extensions: Vec<Extension>,
  stdio: deno_runtime::deno_io::Stdio,
) -> Result<CliMainWorker, AnyError> {
  create_main_worker_internal(
    ps,
    main_module,
    permissions,
    custom_extensions,
    stdio,
  )
  .await
}

async fn create_main_worker_internal(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: PermissionsContainer,
  mut custom_extensions: Vec<Extension>,
  stdio: deno_runtime::deno_io::Stdio,
) -> Result<CliMainWorker, AnyError> {
  let (main_module, is_main_cjs) = if let Ok(package_ref) =
    NpmPackageReqReference::from_specifier(&main_module)
  {
    ps.npm_resolver
      .add_package_reqs(vec![package_ref.req.clone()])
      .await?;
    let pkg_nv = ps
      .npm_resolution
      .resolve_pkg_id_from_pkg_req(&package_ref.req)?
      .nv;
    let node_resolution = node::node_resolve_binary_export(
      &pkg_nv,
      package_ref.sub_path.as_deref(),
      &ps.npm_resolver,
    )?;
    let is_main_cjs =
      matches!(node_resolution, node::NodeResolution::CommonJs(_));
    (node_resolution.into_url(), is_main_cjs)
  } else if ps.options.is_npm_main() {
    let node_resolution =
      node::url_to_node_resolution(main_module, &ps.npm_resolver)?;
    let is_main_cjs =
      matches!(node_resolution, node::NodeResolution::CommonJs(_));
    (node_resolution.into_url(), is_main_cjs)
  } else {
    (main_module, false)
  };

  let module_loader = CliModuleLoader::new(
    ps.clone(),
    PermissionsContainer::allow_all(),
    permissions.clone(),
  );

  let maybe_inspector_server = ps.maybe_inspector_server.clone();

  let create_web_worker_cb =
    create_web_worker_callback(ps.clone(), stdio.clone());
  let web_worker_preload_module_cb =
    create_web_worker_preload_module_callback(ps.clone());
  let web_worker_pre_execute_module_cb =
    create_web_worker_pre_execute_module_callback(ps.clone());

  let maybe_storage_key = ps.options.resolve_storage_key(&main_module);
  let origin_storage_dir = maybe_storage_key.as_ref().map(|key| {
    ps.dir
      .origin_data_folder_path()
      .join(checksum::gen(&[key.as_bytes()]))
  });
  let cache_storage_dir = maybe_storage_key.map(|key| {
    // TODO(@satyarohith): storage quota management
    // Note: we currently use temp_dir() to avoid managing storage size.
    std::env::temp_dir()
      .join("deno_cache")
      .join(checksum::gen(&[key.as_bytes()]))
  });

  let mut extensions = ops::cli_exts(ps.clone());
  extensions.append(&mut custom_extensions);

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: ps.options.argv().clone(),
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      debug_flag: ps
        .options
        .log_level()
        .map(|l| l == log::Level::Debug)
        .unwrap_or(false),
      enable_testing_features: ps.options.enable_testing_features(),
      locale: deno_core::v8::icu::get_language_tag(),
      location: ps.options.location_flag().clone(),
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno().to_string(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: ps.options.unstable(),
      user_agent: version::get_user_agent().to_string(),
      inspect: ps.options.is_inspecting(),
    },
    extensions,
    startup_snapshot: Some(crate::js::deno_isolate_init()),
    unsafely_ignore_certificate_errors: ps
      .options
      .unsafely_ignore_certificate_errors()
      .clone(),
    root_cert_store: Some(ps.root_cert_store.clone()),
    seed: ps.options.seed(),
    source_map_getter: Some(Box::new(module_loader.clone())),
    format_js_error_fn: Some(Arc::new(format_js_error)),
    create_web_worker_cb,
    web_worker_preload_module_cb,
    web_worker_pre_execute_module_cb,
    maybe_inspector_server,
    should_break_on_first_statement: ps.options.inspect_brk().is_some(),
    should_wait_for_inspector_session: ps.options.inspect_wait().is_some(),
    module_loader,
    npm_resolver: Some(Rc::new(ps.npm_resolver.clone())),
    get_error_class_fn: Some(&errors::get_error_class_name),
    cache_storage_dir,
    origin_storage_dir,
    blob_store: ps.blob_store.clone(),
    broadcast_channel: ps.broadcast_channel.clone(),
    shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
    compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
    stdio,
  };

  let worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    options,
  );

  Ok(CliMainWorker {
    main_module,
    is_main_cjs,
    worker,
    ps: ps.clone(),
  })
}

// TODO(bartlomieju): this callback could have default value
// and not be required
fn create_web_worker_preload_module_callback(
  _ps: ProcState,
) -> Arc<WorkerEventCb> {
  Arc::new(move |worker| {
    let fut = async move { Ok(worker) };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_pre_execute_module_callback(
  ps: ProcState,
) -> Arc<WorkerEventCb> {
  Arc::new(move |mut worker| {
    let ps = ps.clone();
    let fut = async move {
      // this will be up to date after pre-load
      if ps.npm_resolver.has_packages() {
        deno_node::initialize_runtime(
          &mut worker.js_runtime,
          ps.options.has_node_modules_dir(),
          None,
        )?;
      }

      Ok(worker)
    };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_callback(
  ps: ProcState,
  stdio: deno_runtime::deno_io::Stdio,
) -> Arc<CreateWebWorkerCb> {
  Arc::new(move |args| {
    let maybe_inspector_server = ps.maybe_inspector_server.clone();

    let module_loader = CliModuleLoader::new_for_worker(
      ps.clone(),
      args.parent_permissions.clone(),
      args.permissions.clone(),
    );
    let create_web_worker_cb =
      create_web_worker_callback(ps.clone(), stdio.clone());
    let preload_module_cb =
      create_web_worker_preload_module_callback(ps.clone());
    let pre_execute_module_cb =
      create_web_worker_pre_execute_module_callback(ps.clone());

    let extensions = ops::cli_exts(ps.clone());

    let maybe_storage_key = ps.options.resolve_storage_key(&args.main_module);
    let cache_storage_dir = maybe_storage_key.map(|key| {
      // TODO(@satyarohith): storage quota management
      // Note: we currently use temp_dir() to avoid managing storage size.
      std::env::temp_dir()
        .join("deno_cache")
        .join(checksum::gen(&[key.as_bytes()]))
    });

    let options = WebWorkerOptions {
      bootstrap: BootstrapOptions {
        args: ps.options.argv().clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        debug_flag: ps
          .options
          .log_level()
          .map(|l| l == log::Level::Debug)
          .unwrap_or(false),
        enable_testing_features: ps.options.enable_testing_features(),
        locale: deno_core::v8::icu::get_language_tag(),
        location: Some(args.main_module.clone()),
        no_color: !colors::use_color(),
        is_tty: colors::is_tty(),
        runtime_version: version::deno().to_string(),
        ts_version: version::TYPESCRIPT.to_string(),
        unstable: ps.options.unstable(),
        user_agent: version::get_user_agent().to_string(),
        inspect: ps.options.is_inspecting(),
      },
      extensions,
      startup_snapshot: Some(crate::js::deno_isolate_init()),
      unsafely_ignore_certificate_errors: ps
        .options
        .unsafely_ignore_certificate_errors()
        .clone(),
      root_cert_store: Some(ps.root_cert_store.clone()),
      seed: ps.options.seed(),
      create_web_worker_cb,
      preload_module_cb,
      pre_execute_module_cb,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      source_map_getter: Some(Box::new(module_loader.clone())),
      module_loader,
      npm_resolver: Some(Rc::new(ps.npm_resolver.clone())),
      worker_type: args.worker_type,
      maybe_inspector_server,
      get_error_class_fn: Some(&errors::get_error_class_name),
      blob_store: ps.blob_store.clone(),
      broadcast_channel: ps.broadcast_channel.clone(),
      shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
      stdio: stdio.clone(),
      cache_storage_dir,
    };

    WebWorker::bootstrap_from_options(
      args.name,
      args.permissions,
      args.main_module,
      args.worker_id,
      options,
    )
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_path;
  use deno_core::FsModuleLoader;
  use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
  use deno_runtime::deno_web::BlobStore;
  use deno_runtime::permissions::Permissions;

  fn create_test_worker() -> MainWorker {
    let main_module =
      resolve_path("./hello.js", &std::env::current_dir().unwrap()).unwrap();
    let permissions = PermissionsContainer::new(Permissions::default());

    let options = WorkerOptions {
      bootstrap: BootstrapOptions::default(),
      extensions: vec![],
      startup_snapshot: Some(crate::js::deno_isolate_init()),
      unsafely_ignore_certificate_errors: None,
      root_cert_store: None,
      seed: None,
      format_js_error_fn: None,
      source_map_getter: None,
      web_worker_preload_module_cb: Arc::new(|_| unreachable!()),
      web_worker_pre_execute_module_cb: Arc::new(|_| unreachable!()),
      create_web_worker_cb: Arc::new(|_| unreachable!()),
      maybe_inspector_server: None,
      should_break_on_first_statement: false,
      should_wait_for_inspector_session: false,
      module_loader: Rc::new(FsModuleLoader),
      npm_resolver: None,
      get_error_class_fn: None,
      cache_storage_dir: None,
      origin_storage_dir: None,
      blob_store: BlobStore::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      stdio: Default::default(),
    };

    MainWorker::bootstrap_from_options(main_module, permissions, options)
  }

  #[tokio::test]
  async fn execute_mod_esm_imports_a() {
    let p = test_util::testdata_path().join("runtime/esm_imports_a.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {err:?}");
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {e:?}");
    }
  }

  #[tokio::test]
  async fn execute_mod_circular() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .join("tests/circular1.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let mut worker = create_test_worker();
    let result = worker.execute_main_module(&module_specifier).await;
    if let Err(err) = result {
      eprintln!("execute_mod err {err:?}");
    }
    if let Err(e) = worker.run_event_loop(false).await {
      panic!("Future got unexpected error: {e:?}");
    }
  }

  #[tokio::test]
  async fn execute_mod_resolve_error() {
    // "foo" is not a valid module specifier so this should return an error.
    let mut worker = create_test_worker();
    let module_specifier =
      resolve_path("./does-not-exist", &std::env::current_dir().unwrap())
        .unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn execute_mod_002_hello() {
    // This assumes cwd is project root (an assumption made throughout the
    // tests).
    let mut worker = create_test_worker();
    let p = test_util::testdata_path().join("run/001_hello.js");
    let module_specifier = ModuleSpecifier::from_file_path(&p).unwrap();
    let result = worker.execute_main_module(&module_specifier).await;
    assert!(result.is_ok());
  }
}
