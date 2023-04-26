// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::get_root_cert_store;
use crate::args::CaData;
use crate::args::CacheSetting;
use crate::cache::DenoDir;
use crate::colors;
use crate::file_fetcher::get_source_from_data_url;
use crate::http_util::HttpClient;
use crate::npm::create_npm_fs_resolver;
use crate::npm::CliNpmRegistryApi;
use crate::npm::CliNpmResolver;
use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::ops;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::v8::construct_v8_flags;
use crate::version;
use crate::CliGraphResolver;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::task::LocalFutureObj;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::v8_set_flags;
use deno_core::CompiledWasmModuleStore;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_core::SharedArrayBufferStore;
use deno_graph::source::Resolver;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_node;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::fmt_errors::format_js_error;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::ops::worker_host::WorkerEventCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use import_map::parse_from_json;
use log::Level;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

mod binary;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::Metadata;

#[derive(Clone)]
struct EmbeddedModuleLoader {
  eszip: Arc<eszip::EszipV2>,
  maybe_import_map_resolver: Option<Arc<CliGraphResolver>>,
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    // Try to follow redirects when resolving.
    let referrer = match self.eszip.get_module(referrer) {
      Some(eszip::Module { ref specifier, .. }) => {
        ModuleSpecifier::parse(specifier)?
      }
      None => {
        let cwd = std::env::current_dir().context("Unable to get CWD")?;
        deno_core::resolve_url_or_path(referrer, &cwd)?
      }
    };

    self
      .maybe_import_map_resolver
      .as_ref()
      .map(|r| r.resolve(specifier, &referrer))
      .unwrap_or_else(|| {
        deno_core::resolve_import(specifier, referrer.as_str())
          .map_err(|err| err.into())
      })
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let is_data_uri = get_source_from_data_url(module_specifier).ok();
    let module = self
      .eszip
      .get_module(module_specifier.as_str())
      .ok_or_else(|| type_error("Module not found"));
    // TODO(mmastrac): This clone can probably be removed in the future if ModuleSpecifier is no longer a full-fledged URL
    let module_specifier = module_specifier.clone();

    async move {
      if let Some((source, _)) = is_data_uri {
        return Ok(deno_core::ModuleSource::new(
          deno_core::ModuleType::JavaScript,
          source.into(),
          &module_specifier,
        ));
      }

      let module = module?;
      let code = module.source().await.unwrap_or_default();
      let code = std::str::from_utf8(&code)
        .map_err(|_| type_error("Module source is not utf-8"))?
        .to_owned()
        .into();

      Ok(deno_core::ModuleSource::new(
        match module.kind {
          eszip::ModuleKind::JavaScript => ModuleType::JavaScript,
          eszip::ModuleKind::Json => ModuleType::Json,
        },
        code,
        &module_specifier,
      ))
    }
    .boxed_local()
  }
}

fn web_worker_callback() -> Arc<WorkerEventCb> {
  Arc::new(|worker| {
    let fut = async move { Ok(worker) };
    LocalFutureObj::new(Box::new(fut))
  })
}

struct SharedWorkerState {
  npm_resolver: Arc<CliNpmResolver>,
  root_cert_store: RootCertStore,
  node_fs: Arc<dyn deno_node::NodeFs>,
  blob_store: BlobStore,
  broadcast_channel: InMemoryBroadcastChannel,
  shared_array_buffer_store: SharedArrayBufferStore,
  compiled_wasm_module_store: CompiledWasmModuleStore,
  // options
  argv: Vec<String>,
  seed: Option<u64>,
  unsafely_ignore_certificate_errors: Option<Vec<String>>,
  unstable: bool,
}

fn create_web_worker_callback(
  shared: &Arc<SharedWorkerState>,
  module_loader: &EmbeddedModuleLoader,
) -> Arc<CreateWebWorkerCb> {
  let shared = shared.clone();
  let module_loader = module_loader.clone();
  Arc::new(move |args| {
    let module_loader = Rc::new(module_loader.clone());

    let create_web_worker_cb =
      create_web_worker_callback(&shared, &module_loader);
    let web_worker_cb = web_worker_callback();

    let options = WebWorkerOptions {
      bootstrap: BootstrapOptions {
        args: shared.argv.clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        debug_flag: false,
        enable_testing_features: false,
        locale: deno_core::v8::icu::get_language_tag(),
        location: Some(args.main_module.clone()),
        no_color: !colors::use_color(),
        is_tty: colors::is_tty(),
        runtime_version: version::deno().to_string(),
        ts_version: version::TYPESCRIPT.to_string(),
        unstable: shared.unstable,
        user_agent: version::get_user_agent().to_string(),
        inspect: false,
      },
      extensions: ops::cli_exts(shared.npm_resolver.clone()),
      startup_snapshot: Some(crate::js::deno_isolate_init()),
      unsafely_ignore_certificate_errors: shared
        .unsafely_ignore_certificate_errors
        .clone(),
      root_cert_store: Some(shared.root_cert_store.clone()),
      seed: shared.seed,
      module_loader,
      node_fs: Some(shared.node_fs.clone()),
      npm_resolver: None, // not currently supported
      create_web_worker_cb,
      preload_module_cb: web_worker_cb.clone(),
      pre_execute_module_cb: web_worker_cb,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      source_map_getter: None,
      worker_type: args.worker_type,
      maybe_inspector_server: None,
      get_error_class_fn: Some(&get_error_class_name),
      blob_store: shared.blob_store.clone(),
      broadcast_channel: shared.broadcast_channel.clone(),
      shared_array_buffer_store: Some(shared.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(
        shared.compiled_wasm_module_store.clone(),
      ),
      cache_storage_dir: None,
      stdio: Default::default(),
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

pub async fn run(
  eszip: eszip::EszipV2,
  metadata: Metadata,
) -> Result<(), AnyError> {
  let main_module = &metadata.entrypoint;
  let dir = DenoDir::new(None)?;
  let root_cert_store = get_root_cert_store(
    None,
    metadata.ca_stores,
    metadata.ca_data.map(CaData::Bytes),
  )?;
  let progress_bar = ProgressBar::new(ProgressBarStyle::TextOnly);
  let http_client = HttpClient::new(
    Some(root_cert_store.clone()),
    metadata.unsafely_ignore_certificate_errors.clone(),
  )?;
  let npm_registry_url = CliNpmRegistryApi::default_url().to_owned();
  let npm_cache = Arc::new(NpmCache::new(
    dir.npm_folder_path(),
    CacheSetting::Use,
    http_client.clone(),
    progress_bar.clone(),
  ));
  let npm_api = Arc::new(CliNpmRegistryApi::new(
    npm_registry_url.clone(),
    npm_cache.clone(),
    http_client.clone(),
    progress_bar.clone(),
  ));
  let node_fs = Arc::new(deno_node::RealFs);
  let npm_resolution =
    Arc::new(NpmResolution::from_serialized(npm_api.clone(), None, None));
  let npm_fs_resolver = create_npm_fs_resolver(
    node_fs.clone(),
    npm_cache,
    &progress_bar,
    npm_registry_url,
    npm_resolution.clone(),
    None,
  );
  let npm_resolver = Arc::new(CliNpmResolver::new(
    npm_resolution.clone(),
    npm_fs_resolver,
    None,
  ));

  let shared = Arc::new(SharedWorkerState {
    npm_resolver,
    root_cert_store,
    node_fs,
    blob_store: BlobStore::default(),
    broadcast_channel: InMemoryBroadcastChannel::default(),
    shared_array_buffer_store: SharedArrayBufferStore::default(),
    compiled_wasm_module_store: CompiledWasmModuleStore::default(),
    argv: metadata.argv,
    seed: metadata.seed,
    unsafely_ignore_certificate_errors: metadata
      .unsafely_ignore_certificate_errors,
    unstable: metadata.unstable,
  });

  let permissions = PermissionsContainer::new(Permissions::from_options(
    &metadata.permissions,
  )?);
  let module_loader = EmbeddedModuleLoader {
    eszip: Arc::new(eszip),
    maybe_import_map_resolver: metadata.maybe_import_map.map(
      |(base, source)| {
        Arc::new(CliGraphResolver::new(
          None,
          Some(Arc::new(
            parse_from_json(&base, &source).unwrap().import_map,
          )),
          false,
          npm_api.clone(),
          npm_resolution.clone(),
          Default::default(),
        ))
      },
    ),
  };
  let create_web_worker_cb =
    create_web_worker_callback(&shared, &module_loader);
  let web_worker_cb = web_worker_callback();

  v8_set_flags(construct_v8_flags(&metadata.v8_flags, vec![]));

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: shared.argv.clone(),
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      debug_flag: metadata
        .log_level
        .map(|l| l == Level::Debug)
        .unwrap_or(false),
      enable_testing_features: false,
      locale: deno_core::v8::icu::get_language_tag(),
      location: metadata.location,
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno().to_string(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: metadata.unstable,
      user_agent: version::get_user_agent().to_string(),
      inspect: false,
    },
    extensions: ops::cli_exts(shared.npm_resolver.clone()),
    startup_snapshot: Some(crate::js::deno_isolate_init()),
    unsafely_ignore_certificate_errors: shared
      .unsafely_ignore_certificate_errors
      .clone(),
    root_cert_store: Some(shared.root_cert_store.clone()),
    seed: metadata.seed,
    source_map_getter: None,
    format_js_error_fn: Some(Arc::new(format_js_error)),
    create_web_worker_cb,
    web_worker_preload_module_cb: web_worker_cb.clone(),
    web_worker_pre_execute_module_cb: web_worker_cb,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    should_wait_for_inspector_session: false,
    module_loader: Rc::new(module_loader),
    node_fs: Some(shared.node_fs.clone()),
    npm_resolver: None, // not currently supported
    get_error_class_fn: Some(&get_error_class_name),
    cache_storage_dir: None,
    origin_storage_dir: None,
    blob_store: shared.blob_store.clone(),
    broadcast_channel: shared.broadcast_channel.clone(),
    shared_array_buffer_store: Some(shared.shared_array_buffer_store.clone()),
    compiled_wasm_module_store: Some(shared.compiled_wasm_module_store.clone()),
    stdio: Default::default(),
  };
  let mut worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    options,
  );
  worker.execute_main_module(main_module).await?;
  worker.dispatch_load_event(located_script_name!())?;

  loop {
    worker.run_event_loop(false).await?;
    if !worker.dispatch_beforeunload_event(located_script_name!())? {
      break;
    }
  }

  worker.dispatch_unload_event(located_script_name!())?;
  std::process::exit(0);
}

fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e).unwrap_or_else(|| {
    panic!(
      "Error '{}' contains boxed error of unsupported type:{}",
      e,
      e.chain().map(|e| format!("\n  {e:?}")).collect::<String>()
    );
  })
}
