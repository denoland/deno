// Copyright 2018-2025 the Deno authors. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_cache::CacheImpl;
use deno_cache::CreateCache;
use deno_cache::SqliteBackedCache;
use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_core::merge_op_metrics;
use deno_core::v8;
use deno_core::CompiledWasmModuleStore;
use deno_core::Extension;
use deno_core::InspectorSessionKind;
use deno_core::InspectorSessionOptions;
use deno_core::JsRuntime;
use deno_core::LocalInspectorSession;
use deno_core::ModuleCodeString;
use deno_core::ModuleId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpMetricsFactoryFn;
use deno_core::OpMetricsSummaryTracker;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;
use deno_core::SharedArrayBufferStore;
use deno_core::SourceCodeCacheInfo;
use deno_cron::local::LocalCronHandler;
use deno_fs::FileSystem;
use deno_io::Stdio;
use deno_kv::dynamic::MultiBackendDbHandler;
use deno_napi::DenoRtNativeAddonLoaderRc;
use deno_node::ExtNodeSys;
use deno_node::NodeExtInitServices;
use deno_os::ExitCode;
use deno_permissions::PermissionsContainer;
use deno_process::NpmProcessStateProviderRc;
use deno_tls::RootCertStoreProvider;
use deno_tls::TlsKeys;
use deno_web::BlobStore;
use log::debug;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::code_cache::CodeCache;
use crate::code_cache::CodeCacheType;
use crate::inspector_server::InspectorServer;
use crate::ops;
use crate::shared::runtime;
use crate::BootstrapOptions;
use crate::FeatureChecker;

pub type FormatJsErrorFn = dyn Fn(&JsError) -> String + Sync + Send;

#[cfg(target_os = "linux")]
pub(crate) static MEMORY_TRIM_HANDLER_ENABLED: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("DENO_USR2_MEMORY_TRIM").is_some());

#[cfg(target_os = "linux")]
pub(crate) static SIGUSR2_RX: LazyLock<tokio::sync::watch::Receiver<()>> =
  LazyLock::new(|| {
    let (tx, rx) = tokio::sync::watch::channel(());

    tokio::spawn(async move {
      let mut sigusr2 = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::user_defined2(),
      )
      .unwrap();

      loop {
        sigusr2.recv().await;

        // SAFETY: calling into libc, nothing relevant on the Rust side.
        unsafe {
          libc::malloc_trim(0);
        }

        if tx.send(()).is_err() {
          break;
        }
      }
    });

    rx
  });

pub fn import_meta_resolve_callback(
  loader: &dyn ModuleLoader,
  specifier: String,
  referrer: String,
) -> Result<ModuleSpecifier, deno_core::error::ModuleLoaderError> {
  loader.resolve(
    &specifier,
    &referrer,
    deno_core::ResolutionKind::DynamicImport,
  )
}

// TODO(bartlomieju): temporary measurement until we start supporting more
// module types
pub fn validate_import_attributes_callback(
  scope: &mut v8::HandleScope,
  attributes: &HashMap<String, String>,
) {
  for (key, value) in attributes {
    let msg = if key != "type" {
      Some(format!("\"{key}\" attribute is not supported."))
    } else if value != "json" && value != "$$deno-core-internal-wasm-module" {
      Some(format!("\"{value}\" is not a valid module type."))
    } else {
      None
    };

    let Some(msg) = msg else {
      continue;
    };

    let message = v8::String::new(scope, &msg).unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
    return;
  }
}

pub fn make_wait_for_inspector_disconnect_callback() -> Box<dyn Fn()> {
  let has_notified_of_inspector_disconnect = AtomicBool::new(false);
  Box::new(move || {
    if !has_notified_of_inspector_disconnect
      .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
      log::info!("Program finished. Waiting for inspector to disconnect to exit the process...");
    }
  })
}

/// This worker is created and used by almost all
/// subcommands in Deno executable.
///
/// It provides ops available in the `Deno` namespace.
///
/// All `WebWorker`s created during program execution
/// are descendants of this worker.
pub struct MainWorker {
  pub js_runtime: JsRuntime,
  should_break_on_first_statement: bool,
  should_wait_for_inspector_session: bool,
  exit_code: ExitCode,
  bootstrap_fn_global: Option<v8::Global<v8::Function>>,
  dispatch_load_event_fn_global: v8::Global<v8::Function>,
  dispatch_beforeunload_event_fn_global: v8::Global<v8::Function>,
  dispatch_unload_event_fn_global: v8::Global<v8::Function>,
  dispatch_process_beforeexit_event_fn_global: v8::Global<v8::Function>,
  dispatch_process_exit_event_fn_global: v8::Global<v8::Function>,
  memory_trim_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for MainWorker {
  fn drop(&mut self) {
    if let Some(memory_trim_handle) = self.memory_trim_handle.take() {
      memory_trim_handle.abort();
    }
  }
}

pub struct WorkerServiceOptions<
  TInNpmPackageChecker: InNpmPackageChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TExtNodeSys: ExtNodeSys,
> {
  pub blob_store: Arc<BlobStore>,
  pub broadcast_channel: InMemoryBroadcastChannel,
  pub deno_rt_native_addon_loader: Option<DenoRtNativeAddonLoaderRc>,
  pub feature_checker: Arc<FeatureChecker>,
  pub fs: Arc<dyn FileSystem>,
  /// Implementation of `ModuleLoader` which will be
  /// called when V8 requests to load ES modules.
  ///
  /// If not provided runtime will error if code being
  /// executed tries to load modules.
  pub module_loader: Rc<dyn ModuleLoader>,
  pub node_services: Option<
    NodeExtInitServices<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TExtNodeSys,
    >,
  >,
  pub npm_process_state_provider: Option<NpmProcessStateProviderRc>,
  pub permissions: PermissionsContainer,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub fetch_dns_resolver: deno_fetch::dns::Resolver,

  /// The store to use for transferring SharedArrayBuffers between isolates.
  /// If multiple isolates should have the possibility of sharing
  /// SharedArrayBuffers, they should use the same [SharedArrayBufferStore]. If
  /// no [SharedArrayBufferStore] is specified, SharedArrayBuffer can not be
  /// serialized.
  pub shared_array_buffer_store: Option<SharedArrayBufferStore>,

  /// The store to use for transferring `WebAssembly.Module` objects between
  /// isolates.
  /// If multiple isolates should have the possibility of sharing
  /// `WebAssembly.Module` objects, they should use the same
  /// [CompiledWasmModuleStore]. If no [CompiledWasmModuleStore] is specified,
  /// `WebAssembly.Module` objects cannot be serialized.
  pub compiled_wasm_module_store: Option<CompiledWasmModuleStore>,

  /// V8 code cache for module and script source code.
  pub v8_code_cache: Option<Arc<dyn CodeCache>>,
}

pub struct WorkerOptions {
  pub bootstrap: BootstrapOptions,

  /// JsRuntime extensions, not to be confused with ES modules.
  ///
  /// Extensions register "ops" and JavaScript sources provided in `js` or `esm`
  /// configuration. If you are using a snapshot, then extensions shouldn't
  /// provide JavaScript sources that were already snapshotted.
  pub extensions: Vec<Extension>,

  /// V8 snapshot that should be loaded on startup.
  pub startup_snapshot: Option<&'static [u8]>,

  /// Should op registration be skipped?
  pub skip_op_registration: bool,

  /// Optional isolate creation parameters, such as heap limits.
  pub create_params: Option<v8::CreateParams>,

  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub seed: Option<u64>,

  // Callbacks invoked when creating new instance of WebWorker
  pub create_web_worker_cb: Arc<ops::worker_host::CreateWebWorkerCb>,
  pub format_js_error_fn: Option<Arc<FormatJsErrorFn>>,

  pub maybe_inspector_server: Option<Arc<InspectorServer>>,
  // If true, the worker will wait for inspector session and break on first
  // statement of user code. Takes higher precedence than
  // `should_wait_for_inspector_session`.
  pub should_break_on_first_statement: bool,
  // If true, the worker will wait for inspector session before executing
  // user code.
  pub should_wait_for_inspector_session: bool,
  /// If Some, print a low-level trace output for ops matching the given patterns.
  pub strace_ops: Option<Vec<String>>,

  pub cache_storage_dir: Option<std::path::PathBuf>,
  pub origin_storage_dir: Option<std::path::PathBuf>,
  pub stdio: Stdio,
  pub enable_stack_trace_arg_in_ops: bool,

  pub unconfigured_runtime: Option<UnconfiguredRuntime>,
}

impl Default for WorkerOptions {
  fn default() -> Self {
    Self {
      create_web_worker_cb: Arc::new(|_| {
        unimplemented!("web workers are not supported")
      }),
      skip_op_registration: false,
      seed: None,
      unsafely_ignore_certificate_errors: Default::default(),
      should_break_on_first_statement: Default::default(),
      should_wait_for_inspector_session: Default::default(),
      strace_ops: Default::default(),
      maybe_inspector_server: Default::default(),
      format_js_error_fn: Default::default(),
      origin_storage_dir: Default::default(),
      cache_storage_dir: Default::default(),
      extensions: Default::default(),
      startup_snapshot: Default::default(),
      create_params: Default::default(),
      bootstrap: Default::default(),
      stdio: Default::default(),
      enable_stack_trace_arg_in_ops: false,
      unconfigured_runtime: None,
    }
  }
}

pub fn create_op_metrics(
  enable_op_summary_metrics: bool,
  strace_ops: Option<Vec<String>>,
) -> (
  Option<Rc<OpMetricsSummaryTracker>>,
  Option<OpMetricsFactoryFn>,
) {
  let mut op_summary_metrics = None;
  let mut op_metrics_factory_fn: Option<OpMetricsFactoryFn> = None;
  let now = Instant::now();
  let max_len: Rc<std::cell::Cell<usize>> = Default::default();
  if let Some(patterns) = strace_ops {
    /// Match an op name against a list of patterns
    fn matches_pattern(patterns: &[String], name: &str) -> bool {
      let mut found_match = false;
      let mut found_nomatch = false;
      for pattern in patterns.iter() {
        if let Some(pattern) = pattern.strip_prefix('-') {
          if name.contains(pattern) {
            return false;
          }
        } else if name.contains(pattern.as_str()) {
          found_match = true;
        } else {
          found_nomatch = true;
        }
      }

      found_match || !found_nomatch
    }

    op_metrics_factory_fn = Some(Box::new(move |_, _, decl| {
      // If we don't match a requested pattern, or we match a negative pattern, bail
      if !matches_pattern(&patterns, decl.name) {
        return None;
      }

      max_len.set(max_len.get().max(decl.name.len()));
      let max_len = max_len.clone();
      Some(Rc::new(
        #[allow(clippy::print_stderr)]
        move |op: &deno_core::_ops::OpCtx, event, source| {
          eprintln!(
            "[{: >10.3}] {name:max_len$}: {event:?} {source:?}",
            now.elapsed().as_secs_f64(),
            name = op.decl().name,
            max_len = max_len.get()
          );
        },
      ))
    }));
  }

  if enable_op_summary_metrics {
    let summary = Rc::new(OpMetricsSummaryTracker::default());
    let summary_metrics = summary.clone().op_metrics_factory_fn(|_| true);
    op_metrics_factory_fn = Some(match op_metrics_factory_fn {
      Some(f) => merge_op_metrics(f, summary_metrics),
      None => summary_metrics,
    });
    op_summary_metrics = Some(summary);
  }

  (op_summary_metrics, op_metrics_factory_fn)
}

impl MainWorker {
  pub fn bootstrap_from_options<
    TInNpmPackageChecker: InNpmPackageChecker + 'static,
    TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
    TExtNodeSys: ExtNodeSys + 'static,
  >(
    main_module: &ModuleSpecifier,
    services: WorkerServiceOptions<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TExtNodeSys,
    >,
    options: WorkerOptions,
  ) -> Self {
    let (mut worker, bootstrap_options) =
      Self::from_options(main_module, services, options);
    worker.bootstrap(bootstrap_options);
    worker
  }

  fn from_options<
    TInNpmPackageChecker: InNpmPackageChecker + 'static,
    TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
    TExtNodeSys: ExtNodeSys + 'static,
  >(
    main_module: &ModuleSpecifier,
    services: WorkerServiceOptions<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TExtNodeSys,
    >,
    mut options: WorkerOptions,
  ) -> (Self, BootstrapOptions) {
    fn create_cache_inner(options: &WorkerOptions) -> Option<CreateCache> {
      if let Ok(var) = std::env::var("DENO_CACHE_LSC_ENDPOINT") {
        let elems: Vec<_> = var.split(",").collect();
        if elems.len() == 2 {
          let endpoint = elems[0];
          let token = elems[1];
          use deno_cache::CacheShard;

          let shard =
            Rc::new(CacheShard::new(endpoint.to_string(), token.to_string()));
          let create_cache_fn = move || {
            let x = deno_cache::LscBackend::default();
            x.set_shard(shard.clone());

            Ok(CacheImpl::Lsc(x))
          };
          #[allow(clippy::arc_with_non_send_sync)]
          return Some(CreateCache(Arc::new(create_cache_fn)));
        }
      }

      if let Some(storage_dir) = &options.cache_storage_dir {
        let storage_dir = storage_dir.clone();
        let create_cache_fn = move || {
          let s = SqliteBackedCache::new(storage_dir.clone())?;
          Ok(CacheImpl::Sqlite(s))
        };
        return Some(CreateCache(Arc::new(create_cache_fn)));
      }

      None
    }
    let create_cache = create_cache_inner(&options);

    // Get our op metrics
    let (op_summary_metrics, op_metrics_factory_fn) = create_op_metrics(
      options.bootstrap.enable_op_summary_metrics,
      options.strace_ops,
    );

    // Permissions: many ops depend on this
    let enable_testing_features = options.bootstrap.enable_testing_features;
    let exit_code = ExitCode::default();

    // check options that require configuring a new jsruntime
    if options.unconfigured_runtime.is_some()
      && (options.enable_stack_trace_arg_in_ops
        || op_metrics_factory_fn.is_some())
    {
      options.unconfigured_runtime = None;
    }

    #[cfg(feature = "hmr")]
    assert!(
      cfg!(not(feature = "only_snapshotted_js_sources")),
      "'hmr' is incompatible with 'only_snapshotted_js_sources'."
    );

    #[cfg(feature = "only_snapshotted_js_sources")]
    options.startup_snapshot.as_ref().expect("A user snapshot was not provided, even though 'only_snapshotted_js_sources' is used.");

    let mut js_runtime = if let Some(u) = options.unconfigured_runtime {
      u.hydrate(services.module_loader)
    } else {
      let mut extensions = common_extensions::<
        TInNpmPackageChecker,
        TNpmPackageFolderResolver,
        TExtNodeSys,
      >(options.startup_snapshot.is_some(), false);

      extensions.extend(std::mem::take(&mut options.extensions));

      common_runtime(
        services.module_loader.clone(),
        options.startup_snapshot,
        options.create_params,
        options.skip_op_registration,
        services.shared_array_buffer_store,
        services.compiled_wasm_module_store,
        extensions,
        op_metrics_factory_fn,
        options.enable_stack_trace_arg_in_ops,
      )
    };

    js_runtime.set_eval_context_code_cache_cbs(services.v8_code_cache.map(
      |cache| {
        let cache_clone = cache.clone();
        (
          Box::new(move |specifier: &ModuleSpecifier, code: &v8::String| {
            let source_hash = {
              use std::hash::Hash;
              use std::hash::Hasher;
              let mut hasher = twox_hash::XxHash64::default();
              code.hash(&mut hasher);
              hasher.finish()
            };
            let data = cache
              .get_sync(specifier, CodeCacheType::Script, source_hash)
              .inspect(|_| {
                // This log line is also used by tests.
                log::debug!(
                  "V8 code cache hit for script: {specifier}, [{source_hash}]"
                );
              })
              .map(Cow::Owned);
            Ok(SourceCodeCacheInfo {
              data,
              hash: source_hash,
            })
          }) as Box<dyn Fn(&_, &_) -> _>,
          Box::new(
            move |specifier: ModuleSpecifier, source_hash: u64, data: &[u8]| {
              // This log line is also used by tests.
              log::debug!(
              "Updating V8 code cache for script: {specifier}, [{source_hash}]"
            );
              cache_clone.set_sync(
                specifier,
                CodeCacheType::Script,
                source_hash,
                data,
              );
            },
          ) as Box<dyn Fn(_, _, &_)>,
        )
      },
    ));

    js_runtime
      .lazy_init_extensions(vec![
        deno_web::deno_web::args::<PermissionsContainer>(
          services.blob_store.clone(),
          options.bootstrap.location.clone(),
        ),
        deno_fetch::deno_fetch::args::<PermissionsContainer>(
          deno_fetch::Options {
            user_agent: options.bootstrap.user_agent.clone(),
            root_cert_store_provider: services.root_cert_store_provider.clone(),
            unsafely_ignore_certificate_errors: options
              .unsafely_ignore_certificate_errors
              .clone(),
            file_fetch_handler: Rc::new(deno_fetch::FsFetchHandler),
            resolver: services.fetch_dns_resolver,
            ..Default::default()
          },
        ),
        deno_cache::deno_cache::args(create_cache),
        deno_websocket::deno_websocket::args::<PermissionsContainer>(
          options.bootstrap.user_agent.clone(),
          services.root_cert_store_provider.clone(),
          options.unsafely_ignore_certificate_errors.clone(),
        ),
        deno_webstorage::deno_webstorage::args(
          options.origin_storage_dir.clone(),
        ),
        deno_crypto::deno_crypto::args(options.seed),
        deno_broadcast_channel::deno_broadcast_channel::args(
          services.broadcast_channel.clone(),
        ),
        deno_ffi::deno_ffi::args::<PermissionsContainer>(
          services.deno_rt_native_addon_loader.clone(),
        ),
        deno_net::deno_net::args::<PermissionsContainer>(
          services.root_cert_store_provider.clone(),
          options.unsafely_ignore_certificate_errors.clone(),
        ),
        deno_kv::deno_kv::args(
          MultiBackendDbHandler::remote_or_sqlite::<PermissionsContainer>(
            options.origin_storage_dir.clone(),
            options.seed,
            deno_kv::remote::HttpOptions {
              user_agent: options.bootstrap.user_agent.clone(),
              root_cert_store_provider: services
                .root_cert_store_provider
                .clone(),
              unsafely_ignore_certificate_errors: options
                .unsafely_ignore_certificate_errors
                .clone(),
              client_cert_chain_and_key: TlsKeys::Null,
              proxy: None,
            },
          ),
          deno_kv::KvConfig::builder().build(),
        ),
        deno_napi::deno_napi::args::<PermissionsContainer>(
          services.deno_rt_native_addon_loader.clone(),
        ),
        deno_http::deno_http::args(deno_http::Options {
          no_legacy_abort: options.bootstrap.no_legacy_abort,
          ..Default::default()
        }),
        deno_io::deno_io::args(Some(options.stdio)),
        deno_fs::deno_fs::args::<PermissionsContainer>(services.fs.clone()),
        deno_os::deno_os::args(Some(exit_code.clone())),
        deno_process::deno_process::args(services.npm_process_state_provider),
        deno_node::deno_node::args::<
          PermissionsContainer,
          TInNpmPackageChecker,
          TNpmPackageFolderResolver,
          TExtNodeSys,
        >(services.node_services, services.fs.clone()),
        ops::runtime::deno_runtime::args(main_module.clone()),
        ops::worker_host::deno_worker_host::args(
          options.create_web_worker_cb.clone(),
          options.format_js_error_fn.clone(),
        ),
      ])
      .unwrap();

    if let Some(op_summary_metrics) = op_summary_metrics {
      js_runtime.op_state().borrow_mut().put(op_summary_metrics);
    }

    {
      let state = js_runtime.op_state();
      let mut state = state.borrow_mut();

      // Put inspector handle into the op state so we can put a breakpoint when
      // executing a CJS entrypoint.
      state.put(js_runtime.inspector());

      state.put::<PermissionsContainer>(services.permissions);
      state.put(ops::TestingFeaturesEnabled(enable_testing_features));
      state.put(services.feature_checker);
    }

    if let Some(server) = options.maybe_inspector_server.clone() {
      server.register_inspector(
        main_module.to_string(),
        &mut js_runtime,
        options.should_break_on_first_statement
          || options.should_wait_for_inspector_session,
      );
    }

    let (
      bootstrap_fn_global,
      dispatch_load_event_fn_global,
      dispatch_beforeunload_event_fn_global,
      dispatch_unload_event_fn_global,
      dispatch_process_beforeexit_event_fn_global,
      dispatch_process_exit_event_fn_global,
    ) = {
      let context = js_runtime.main_context();
      let scope = &mut js_runtime.handle_scope();
      let context_local = v8::Local::new(scope, context);
      let global_obj = context_local.global(scope);
      let bootstrap_str =
        v8::String::new_external_onebyte_static(scope, b"bootstrap").unwrap();
      let bootstrap_ns: v8::Local<v8::Object> = global_obj
        .get(scope, bootstrap_str.into())
        .unwrap()
        .try_into()
        .unwrap();
      let main_runtime_str =
        v8::String::new_external_onebyte_static(scope, b"mainRuntime").unwrap();
      let bootstrap_fn =
        bootstrap_ns.get(scope, main_runtime_str.into()).unwrap();
      let bootstrap_fn =
        v8::Local::<v8::Function>::try_from(bootstrap_fn).unwrap();
      let dispatch_load_event_fn_str =
        v8::String::new_external_onebyte_static(scope, b"dispatchLoadEvent")
          .unwrap();
      let dispatch_load_event_fn = bootstrap_ns
        .get(scope, dispatch_load_event_fn_str.into())
        .unwrap();
      let dispatch_load_event_fn =
        v8::Local::<v8::Function>::try_from(dispatch_load_event_fn).unwrap();
      let dispatch_beforeunload_event_fn_str =
        v8::String::new_external_onebyte_static(
          scope,
          b"dispatchBeforeUnloadEvent",
        )
        .unwrap();
      let dispatch_beforeunload_event_fn = bootstrap_ns
        .get(scope, dispatch_beforeunload_event_fn_str.into())
        .unwrap();
      let dispatch_beforeunload_event_fn =
        v8::Local::<v8::Function>::try_from(dispatch_beforeunload_event_fn)
          .unwrap();
      let dispatch_unload_event_fn_str =
        v8::String::new_external_onebyte_static(scope, b"dispatchUnloadEvent")
          .unwrap();
      let dispatch_unload_event_fn = bootstrap_ns
        .get(scope, dispatch_unload_event_fn_str.into())
        .unwrap();
      let dispatch_unload_event_fn =
        v8::Local::<v8::Function>::try_from(dispatch_unload_event_fn).unwrap();
      let dispatch_process_beforeexit_event =
        v8::String::new_external_onebyte_static(
          scope,
          b"dispatchProcessBeforeExitEvent",
        )
        .unwrap();
      let dispatch_process_beforeexit_event_fn = bootstrap_ns
        .get(scope, dispatch_process_beforeexit_event.into())
        .unwrap();
      let dispatch_process_beforeexit_event_fn =
        v8::Local::<v8::Function>::try_from(
          dispatch_process_beforeexit_event_fn,
        )
        .unwrap();
      let dispatch_process_exit_event =
        v8::String::new_external_onebyte_static(
          scope,
          b"dispatchProcessExitEvent",
        )
        .unwrap();
      let dispatch_process_exit_event_fn = bootstrap_ns
        .get(scope, dispatch_process_exit_event.into())
        .unwrap();
      let dispatch_process_exit_event_fn =
        v8::Local::<v8::Function>::try_from(dispatch_process_exit_event_fn)
          .unwrap();
      (
        v8::Global::new(scope, bootstrap_fn),
        v8::Global::new(scope, dispatch_load_event_fn),
        v8::Global::new(scope, dispatch_beforeunload_event_fn),
        v8::Global::new(scope, dispatch_unload_event_fn),
        v8::Global::new(scope, dispatch_process_beforeexit_event_fn),
        v8::Global::new(scope, dispatch_process_exit_event_fn),
      )
    };

    let worker = Self {
      js_runtime,
      should_break_on_first_statement: options.should_break_on_first_statement,
      should_wait_for_inspector_session: options
        .should_wait_for_inspector_session,
      exit_code,
      bootstrap_fn_global: Some(bootstrap_fn_global),
      dispatch_load_event_fn_global,
      dispatch_beforeunload_event_fn_global,
      dispatch_unload_event_fn_global,
      dispatch_process_beforeexit_event_fn_global,
      dispatch_process_exit_event_fn_global,
      memory_trim_handle: None,
    };
    (worker, options.bootstrap)
  }

  pub fn bootstrap(&mut self, options: BootstrapOptions) {
    // Setup bootstrap options for ops.
    {
      let op_state = self.js_runtime.op_state();
      let mut state = op_state.borrow_mut();
      state.put(options.clone());
      if let Some(node_ipc_fd) = options.node_ipc_fd {
        state.put(deno_node::ChildPipeFd(node_ipc_fd));
      }
    }

    let scope = &mut self.js_runtime.handle_scope();
    let scope = &mut v8::TryCatch::new(scope);
    let args = options.as_v8(scope);
    let bootstrap_fn = self.bootstrap_fn_global.take().unwrap();
    let bootstrap_fn = v8::Local::new(scope, bootstrap_fn);
    let undefined = v8::undefined(scope);
    bootstrap_fn.call(scope, undefined.into(), &[args]);
    if let Some(exception) = scope.exception() {
      let error = JsError::from_v8_exception(scope, exception);
      panic!("Bootstrap exception: {error}");
    }
  }

  #[cfg(not(target_os = "linux"))]
  pub fn setup_memory_trim_handler(&mut self) {
    // Noop
  }

  /// Sets up a handler that responds to SIGUSR2 signals by trimming unused
  /// memory and notifying V8 of low memory conditions.
  /// Note that this must be called within a tokio runtime.
  /// Calling this method multiple times will be a no-op.
  #[cfg(target_os = "linux")]
  pub fn setup_memory_trim_handler(&mut self) {
    if self.memory_trim_handle.is_some() {
      return;
    }

    if !*MEMORY_TRIM_HANDLER_ENABLED {
      return;
    }

    let mut sigusr2_rx = SIGUSR2_RX.clone();

    let spawner = self
      .js_runtime
      .op_state()
      .borrow()
      .borrow::<deno_core::V8CrossThreadTaskSpawner>()
      .clone();

    let memory_trim_handle = tokio::spawn(async move {
      loop {
        if sigusr2_rx.changed().await.is_err() {
          break;
        }

        spawner.spawn(move |isolate| {
          isolate.low_memory_notification();
        });
      }
    });

    self.memory_trim_handle = Some(memory_trim_handle);
  }

  /// See [JsRuntime::execute_script](deno_core::JsRuntime::execute_script)
  pub fn execute_script(
    &mut self,
    script_name: &'static str,
    source_code: ModuleCodeString,
  ) -> Result<v8::Global<v8::Value>, CoreError> {
    self.js_runtime.execute_script(script_name, source_code)
  }

  /// Loads and instantiates specified JavaScript module as "main" module.
  pub async fn preload_main_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, CoreError> {
    self.js_runtime.load_main_es_module(module_specifier).await
  }

  /// Loads and instantiates specified JavaScript module as "side" module.
  pub async fn preload_side_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleId, CoreError> {
    self.js_runtime.load_side_es_module(module_specifier).await
  }

  /// Executes specified JavaScript module.
  pub async fn evaluate_module(
    &mut self,
    id: ModuleId,
  ) -> Result<(), CoreError> {
    self.wait_for_inspector_session();
    let mut receiver = self.js_runtime.mod_evaluate(id);
    tokio::select! {
      // Not using biased mode leads to non-determinism for relatively simple
      // programs.
      biased;

      maybe_result = &mut receiver => {
        debug!("received module evaluate {:#?}", maybe_result);
        maybe_result
      }

      event_loop_result = self.run_event_loop(false) => {
        event_loop_result?;
        receiver.await
      }
    }
  }

  /// Run the event loop up to a given duration. If the runtime resolves early, returns
  /// early. Will always poll the runtime at least once.
  pub async fn run_up_to_duration(
    &mut self,
    duration: Duration,
  ) -> Result<(), CoreError> {
    match tokio::time::timeout(
      duration,
      self
        .js_runtime
        .run_event_loop(PollEventLoopOptions::default()),
    )
    .await
    {
      Ok(Ok(_)) => Ok(()),
      Err(_) => Ok(()),
      Ok(Err(e)) => Err(e),
    }
  }

  /// Loads, instantiates and executes specified JavaScript module.
  pub async fn execute_side_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), CoreError> {
    let id = self.preload_side_module(module_specifier).await?;
    self.evaluate_module(id).await
  }

  /// Loads, instantiates and executes specified JavaScript module.
  ///
  /// This module will have "import.meta.main" equal to true.
  pub async fn execute_main_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), CoreError> {
    let id = self.preload_main_module(module_specifier).await?;
    self.evaluate_module(id).await
  }

  fn wait_for_inspector_session(&mut self) {
    if self.should_break_on_first_statement {
      self
        .js_runtime
        .inspector()
        .borrow_mut()
        .wait_for_session_and_break_on_next_statement();
    } else if self.should_wait_for_inspector_session {
      self.js_runtime.inspector().borrow_mut().wait_for_session();
    }
  }

  /// Create new inspector session. This function panics if Worker
  /// was not configured to create inspector.
  pub fn create_inspector_session(&mut self) -> LocalInspectorSession {
    self.js_runtime.maybe_init_inspector();
    self.js_runtime.inspector().borrow().create_local_session(
      InspectorSessionOptions {
        kind: InspectorSessionKind::Blocking,
      },
    )
  }

  pub async fn run_event_loop(
    &mut self,
    wait_for_inspector: bool,
  ) -> Result<(), CoreError> {
    self
      .js_runtime
      .run_event_loop(PollEventLoopOptions {
        wait_for_inspector,
        ..Default::default()
      })
      .await
  }

  /// Return exit code set by the executed code (either in main worker
  /// or one of child web workers).
  pub fn exit_code(&self) -> i32 {
    self.exit_code.get()
  }

  /// Dispatches "load" event to the JavaScript runtime.
  ///
  /// Does not poll event loop, and thus not await any of the "load" event handlers.
  pub fn dispatch_load_event(&mut self) -> Result<(), JsError> {
    let scope = &mut self.js_runtime.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let dispatch_load_event_fn =
      v8::Local::new(tc_scope, &self.dispatch_load_event_fn_global);
    let undefined = v8::undefined(tc_scope);
    dispatch_load_event_fn.call(tc_scope, undefined.into(), &[]);
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(tc_scope, exception);
      return Err(error);
    }
    Ok(())
  }

  /// Dispatches "unload" event to the JavaScript runtime.
  ///
  /// Does not poll event loop, and thus not await any of the "unload" event handlers.
  pub fn dispatch_unload_event(&mut self) -> Result<(), JsError> {
    let scope = &mut self.js_runtime.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let dispatch_unload_event_fn =
      v8::Local::new(tc_scope, &self.dispatch_unload_event_fn_global);
    let undefined = v8::undefined(tc_scope);
    dispatch_unload_event_fn.call(tc_scope, undefined.into(), &[]);
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(tc_scope, exception);
      return Err(error);
    }
    Ok(())
  }

  /// Dispatches process.emit("exit") event for node compat.
  pub fn dispatch_process_exit_event(&mut self) -> Result<(), JsError> {
    let scope = &mut self.js_runtime.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let dispatch_process_exit_event_fn =
      v8::Local::new(tc_scope, &self.dispatch_process_exit_event_fn_global);
    let undefined = v8::undefined(tc_scope);
    dispatch_process_exit_event_fn.call(tc_scope, undefined.into(), &[]);
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(tc_scope, exception);
      return Err(error);
    }
    Ok(())
  }

  /// Dispatches "beforeunload" event to the JavaScript runtime. Returns a boolean
  /// indicating if the event was prevented and thus event loop should continue
  /// running.
  pub fn dispatch_beforeunload_event(&mut self) -> Result<bool, JsError> {
    let scope = &mut self.js_runtime.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let dispatch_beforeunload_event_fn =
      v8::Local::new(tc_scope, &self.dispatch_beforeunload_event_fn_global);
    let undefined = v8::undefined(tc_scope);
    let ret_val =
      dispatch_beforeunload_event_fn.call(tc_scope, undefined.into(), &[]);
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(tc_scope, exception);
      return Err(error);
    }
    let ret_val = ret_val.unwrap();
    Ok(ret_val.is_false())
  }

  /// Dispatches process.emit("beforeExit") event for node compat.
  pub fn dispatch_process_beforeexit_event(&mut self) -> Result<bool, JsError> {
    let scope = &mut self.js_runtime.handle_scope();
    let tc_scope = &mut v8::TryCatch::new(scope);
    let dispatch_process_beforeexit_event_fn = v8::Local::new(
      tc_scope,
      &self.dispatch_process_beforeexit_event_fn_global,
    );
    let undefined = v8::undefined(tc_scope);
    let ret_val = dispatch_process_beforeexit_event_fn.call(
      tc_scope,
      undefined.into(),
      &[],
    );
    if let Some(exception) = tc_scope.exception() {
      let error = JsError::from_v8_exception(tc_scope, exception);
      return Err(error);
    }
    let ret_val = ret_val.unwrap();
    Ok(ret_val.is_true())
  }
}

fn common_extensions<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TExtNodeSys: ExtNodeSys + 'static,
>(
  has_snapshot: bool,
  unconfigured_runtime: bool,
) -> Vec<Extension> {
  // NOTE(bartlomieju): ordering is important here, keep it in sync with
  // `runtime/worker.rs`, `runtime/web_worker.rs`, `runtime/snapshot_info.rs`
  // and `runtime/snapshot.rs`!
  vec![
    deno_telemetry::deno_telemetry::init(),
    // Web APIs
    deno_webidl::deno_webidl::init(),
    deno_console::deno_console::init(),
    deno_url::deno_url::init(),
    deno_web::deno_web::lazy_init::<PermissionsContainer>(),
    deno_webgpu::deno_webgpu::init(),
    deno_canvas::deno_canvas::init(),
    deno_fetch::deno_fetch::lazy_init::<PermissionsContainer>(),
    deno_cache::deno_cache::lazy_init(),
    deno_websocket::deno_websocket::lazy_init::<PermissionsContainer>(),
    deno_webstorage::deno_webstorage::lazy_init(),
    deno_crypto::deno_crypto::lazy_init(),
    deno_broadcast_channel::deno_broadcast_channel::lazy_init::<
      InMemoryBroadcastChannel,
    >(),
    deno_ffi::deno_ffi::lazy_init::<PermissionsContainer>(),
    deno_net::deno_net::lazy_init::<PermissionsContainer>(),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::lazy_init::<MultiBackendDbHandler>(),
    deno_cron::deno_cron::init(LocalCronHandler::new()),
    deno_napi::deno_napi::lazy_init::<PermissionsContainer>(),
    deno_http::deno_http::lazy_init(),
    deno_io::deno_io::lazy_init(),
    deno_fs::deno_fs::lazy_init::<PermissionsContainer>(),
    deno_os::deno_os::lazy_init(),
    deno_process::deno_process::lazy_init(),
    deno_node::deno_node::lazy_init::<
      PermissionsContainer,
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TExtNodeSys,
    >(),
    // Ops from this crate
    ops::runtime::deno_runtime::lazy_init(),
    ops::worker_host::deno_worker_host::lazy_init(),
    ops::fs_events::deno_fs_events::init(),
    ops::permissions::deno_permissions::init(),
    ops::tty::deno_tty::init(),
    ops::http::deno_http_runtime::init(),
    ops::bootstrap::deno_bootstrap::init(
      has_snapshot.then(Default::default),
      unconfigured_runtime,
    ),
    runtime::init(),
    // NOTE(bartlomieju): this is done, just so that ops from this extension
    // are available and importing them in `99_main.js` doesn't cause an
    // error because they're not defined. Trying to use these ops in non-worker
    // context will cause a panic.
    ops::web_worker::deno_web_worker::init().disable(),
  ]
}

#[allow(clippy::too_many_arguments)]
fn common_runtime(
  module_loader: Rc<dyn ModuleLoader>,
  startup_snapshot: Option<&'static [u8]>,
  create_params: Option<v8::CreateParams>,
  skip_op_registration: bool,
  shared_array_buffer_store: Option<SharedArrayBufferStore>,
  compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
  extensions: Vec<Extension>,
  op_metrics_factory_fn: Option<OpMetricsFactoryFn>,
  enable_stack_trace_arg_in_ops: bool,
) -> JsRuntime {
  JsRuntime::new(RuntimeOptions {
    module_loader: Some(module_loader),
    startup_snapshot,
    create_params,
    skip_op_registration,
    shared_array_buffer_store,
    compiled_wasm_module_store,
    extensions,
    #[cfg(feature = "transpile")]
    extension_transpiler: Some(Rc::new(|specifier, source| {
      crate::transpile::maybe_transpile_source(specifier, source)
    })),
    #[cfg(not(feature = "transpile"))]
    extension_transpiler: None,
    inspector: true,
    is_main: true,
    op_metrics_factory_fn,
    wait_for_inspector_disconnect_callback: Some(
      make_wait_for_inspector_disconnect_callback(),
    ),
    import_meta_resolve_callback: Some(Box::new(import_meta_resolve_callback)),
    validate_import_attributes_cb: Some(Box::new(
      validate_import_attributes_callback,
    )),
    import_assertions_support: deno_core::ImportAssertionsSupport::Error,
    maybe_op_stack_trace_callback: enable_stack_trace_arg_in_ops.then(|| {
      Box::new(|stack| {
        deno_permissions::prompter::set_current_stacktrace(stack)
      }) as _
    }),
    ..Default::default()
  })
}

pub struct UnconfiguredRuntime {
  module_loader: Rc<PlaceholderModuleLoader>,
  js_runtime: JsRuntime,
}

impl UnconfiguredRuntime {
  pub fn new<
    TInNpmPackageChecker: InNpmPackageChecker + 'static,
    TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
    TExtNodeSys: ExtNodeSys + 'static,
  >(
    startup_snapshot: &'static [u8],
    create_params: Option<v8::CreateParams>,
    shared_array_buffer_store: Option<SharedArrayBufferStore>,
    compiled_wasm_module_store: Option<CompiledWasmModuleStore>,
    additional_extensions: Vec<Extension>,
  ) -> Self {
    let mut extensions = common_extensions::<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TExtNodeSys,
    >(true, true);

    extensions.extend(additional_extensions);

    let module_loader =
      Rc::new(PlaceholderModuleLoader(std::cell::RefCell::new(None)));

    let js_runtime = common_runtime(
      module_loader.clone(),
      Some(startup_snapshot),
      create_params,
      true,
      shared_array_buffer_store,
      compiled_wasm_module_store,
      extensions,
      None,
      false,
    );

    UnconfiguredRuntime {
      module_loader,
      js_runtime,
    }
  }

  fn hydrate(self, module_loader: Rc<dyn ModuleLoader>) -> JsRuntime {
    let _ = self.module_loader.0.borrow_mut().insert(module_loader);
    self.js_runtime
  }
}

struct PlaceholderModuleLoader(
  std::cell::RefCell<Option<Rc<dyn ModuleLoader>>>,
);

impl ModuleLoader for PlaceholderModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: deno_core::ResolutionKind,
  ) -> Result<ModuleSpecifier, deno_core::error::ModuleLoaderError> {
    self
      .0
      .borrow_mut()
      .clone()
      .unwrap()
      .resolve(specifier, referrer, kind)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dyn_import: bool,
    requested_module_type: deno_core::RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    self.0.borrow_mut().clone().unwrap().load(
      module_specifier,
      maybe_referrer,
      is_dyn_import,
      requested_module_type,
    )
  }

  fn prepare_load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<String>,
    is_dyn_import: bool,
  ) -> std::pin::Pin<
    Box<
      dyn std::prelude::rust_2024::Future<
        Output = Result<(), deno_core::error::ModuleLoaderError>,
      >,
    >,
  > {
    self.0.borrow_mut().clone().unwrap().prepare_load(
      module_specifier,
      maybe_referrer,
      is_dyn_import,
    )
  }

  fn finish_load(&self) {
    self.0.borrow_mut().clone().unwrap().finish_load()
  }

  fn purge_and_prevent_code_cache(&self, module_specifier: &str) {
    self
      .0
      .borrow_mut()
      .clone()
      .unwrap()
      .purge_and_prevent_code_cache(module_specifier)
  }

  fn get_source_map(&self, file_name: &str) -> Option<Cow<[u8]>> {
    let v = self.0.borrow_mut().clone().unwrap();
    let v = v.get_source_map(file_name);
    v.map(|c| Cow::from(c.into_owned()))
  }

  fn get_source_mapped_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    self
      .0
      .borrow_mut()
      .clone()
      .unwrap()
      .get_source_mapped_source_line(file_name, line_number)
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    name: &str,
  ) -> Option<v8::Local<'s, v8::Data>> {
    self
      .0
      .borrow_mut()
      .clone()
      .unwrap()
      .get_host_defined_options(scope, name)
  }
}
