// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CaData;
use crate::args::Flags;
use crate::args::NodeModulesDirOption;
use crate::colors;
use crate::file_fetcher::get_source_from_data_url;
use crate::module_loader::NpmModuleLoader;
use crate::npm::CliNpmResolver;
use crate::ops;
use crate::proc_state::ProcState;
use crate::util::v8::construct_v8_flags;
use crate::version;
use crate::CliGraphResolver;
use deno_ast::MediaType;
use deno_core::anyhow::Context;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::task::LocalFutureObj;
use deno_core::futures::FutureExt;
use deno_core::located_script_name;
use deno_core::v8_set_flags;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_graph::source::Resolver;
use deno_runtime::deno_node;
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
use deno_semver::npm::NpmPackageReqReference;
use import_map::parse_from_json;
use log::Level;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

mod binary;
mod file_system;
mod virtual_fs;

pub use binary::extract_standalone;
pub use binary::is_standalone_binary;
pub use binary::DenoCompileBinaryWriter;

use self::binary::Metadata;
use self::binary::NPM_VFS;
use self::file_system::DenoCompileFileSystem;

#[derive(Clone)]
struct EmbeddedModuleLoader {
  eszip: Arc<eszip::EszipV2>,
  maybe_import_map_resolver: Option<Arc<CliGraphResolver>>,
  npm_module_loader: Arc<NpmModuleLoader>,
  root_permissions: PermissionsContainer,
  dynamic_permissions: PermissionsContainer,
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
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

    let permissions = if matches!(kind, ResolutionKind::DynamicImport) {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    if let Some(result) = self.npm_module_loader.resolve_if_in_npm_package(
      specifier,
      &referrer,
      permissions,
    ) {
      return result;
    }

    if let Ok(reference) = NpmPackageReqReference::from_str(specifier) {
      return self
        .npm_module_loader
        .resolve_for_req_reference(&reference, permissions);
    }

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
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    let is_data_uri = get_source_from_data_url(module_specifier).ok();
    let permissions = if is_dynamic {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    if let Some(result) = self.npm_module_loader.load_sync_if_in_npm_package(
      &module_specifier,
      maybe_referrer,
      permissions,
    ) {
      return match result {
        Ok(code_source) => Box::pin(deno_core::futures::future::ready(Ok(
          deno_core::ModuleSource::new_with_redirect(
            match code_source.media_type {
              MediaType::Json => ModuleType::Json,
              _ => ModuleType::JavaScript,
            },
            code_source.code,
            &module_specifier,
            &code_source.found_url,
          ),
        ))),
        Err(err) => Box::pin(deno_core::futures::future::ready(Err(err))),
      };
    }

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

fn metadata_to_flags(metadata: &Metadata) -> Flags {
  let permissions = metadata.permissions.clone();
  Flags {
    argv: metadata.argv.clone(),
    unstable: metadata.unstable,
    seed: metadata.seed,
    location: metadata.location.clone(),
    allow_env: permissions.allow_env,
    allow_hrtime: permissions.allow_hrtime,
    allow_net: permissions.allow_net,
    allow_ffi: permissions.allow_ffi,
    allow_read: permissions.allow_read,
    allow_run: permissions.allow_run,
    allow_write: permissions.allow_write,
    v8_flags: metadata.v8_flags.clone(),
    log_level: metadata.log_level,
    ca_stores: metadata.ca_stores.clone(),
    ca_data: metadata.ca_data.clone().map(CaData::Bytes),
    node_modules_dir: if metadata.node_modules_dir {
      Some(NodeModulesDirOption::Path(NPM_VFS.root().to_path_buf()))
    } else {
      None
    },
    npm_cache_dir: if metadata.node_modules_dir {
      None
    } else {
      Some(NPM_VFS.root().to_path_buf())
    },
    npm_snapshot: metadata.npm_snapshot.clone(),
    ..Default::default()
  }
}

fn web_worker_callback() -> Arc<WorkerEventCb> {
  Arc::new(|worker| {
    let fut = async move { Ok(worker) };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_callback(
  ps: &ProcState,
  module_loader: &EmbeddedModuleLoader,
  file_system: &DenoCompileFileSystem,
) -> Arc<CreateWebWorkerCb> {
  let ps = ps.clone();
  let module_loader = module_loader.clone();
  let file_system = file_system.clone();
  Arc::new(move |args| {
    let create_web_worker_cb =
      create_web_worker_callback(&ps, &module_loader, &file_system);
    let module_loader = Rc::new(module_loader.clone());
    let web_worker_cb = web_worker_callback();

    let options = WebWorkerOptions {
      bootstrap: BootstrapOptions {
        args: ps.options.argv().clone(),
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        debug_flag: ps.options.log_level().map_or(false, |l| l == Level::Debug),
        enable_testing_features: false,
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
      extensions: ops::cli_exts(ps.npm_resolver.clone()),
      startup_snapshot: Some(crate::js::deno_isolate_init()),
      unsafely_ignore_certificate_errors: ps
        .options
        .unsafely_ignore_certificate_errors()
        .clone(),
      root_cert_store: Some(ps.root_cert_store.clone()),
      seed: ps.options.seed(),
      module_loader,
      node_fs: Some(ps.node_fs.clone()),
      npm_resolver: None, // not currently supported
      create_web_worker_cb,
      preload_module_cb: web_worker_cb.clone(),
      pre_execute_module_cb: web_worker_cb,
      format_js_error_fn: Some(Arc::new(format_js_error)),
      source_map_getter: None,
      worker_type: args.worker_type,
      maybe_inspector_server: None,
      get_error_class_fn: Some(&get_error_class_name),
      blob_store: ps.blob_store.clone(),
      broadcast_channel: ps.broadcast_channel.clone(),
      shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
      cache_storage_dir: None,
      stdio: Default::default(),
    };

    WebWorker::bootstrap_from_options(
      args.name,
      args.permissions,
      file_system.clone(),
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
  let flags = metadata_to_flags(&metadata);
  let main_module = &metadata.entrypoint;
  let ps =
    ProcState::from_flags_and_node_fs(flags, Arc::new(DenoCompileFileSystem))
      .await?;
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
          ps.npm_api.clone(),
          ps.npm_resolution.clone(),
          ps.package_json_deps_installer.clone(),
        ))
      },
    ),
    npm_module_loader: Arc::new(NpmModuleLoader::new(
      ps.cjs_resolutions.clone(),
      ps.node_code_translator.clone(),
      ps.node_fs.clone(),
      ps.node_resolver.clone(),
    )),
    root_permissions: permissions.clone(),
    // todo(THIS PR): seems wrong :)
    dynamic_permissions: permissions.clone(),
  };
  let file_system = DenoCompileFileSystem;
  let create_web_worker_cb =
    create_web_worker_callback(&ps, &module_loader, &file_system);
  let web_worker_cb = web_worker_callback();

  v8_set_flags(construct_v8_flags(&metadata.v8_flags, vec![]));

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: metadata.argv,
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
      inspect: ps.options.is_inspecting(),
    },
    extensions: ops::cli_exts(ps.npm_resolver.clone()),
    startup_snapshot: Some(crate::js::deno_isolate_init()),
    unsafely_ignore_certificate_errors: metadata
      .unsafely_ignore_certificate_errors,
    root_cert_store: Some(ps.root_cert_store.clone()),
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
    node_fs: Some(ps.node_fs.clone()),
    npm_resolver: None, // not currently supported
    get_error_class_fn: Some(&get_error_class_name),
    cache_storage_dir: None,
    origin_storage_dir: None,
    blob_store: ps.blob_store.clone(),
    broadcast_channel: ps.broadcast_channel.clone(),
    shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
    compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
    stdio: Default::default(),
  };
  let mut worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    file_system,
    options,
  );

  let id = worker.preload_main_module(main_module).await?;
  if metadata.npm_snapshot.is_some() {
    deno_node::initialize_runtime(
      &mut worker.js_runtime,
      metadata.node_modules_dir,
      None,
    )?;
  }
  worker.evaluate_module(id).await?;
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
