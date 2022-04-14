// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod auth_tokens;
mod cache;
mod cdp;
mod checksum;
mod compat;
mod config_file;
mod deno_dir;
mod diagnostics;
mod diff;
mod disk_cache;
mod display;
mod emit;
mod errors;
mod file_fetcher;
mod file_watcher;
mod flags;
mod flags_allow_net;
mod fmt_errors;
mod fs_util;
mod graph_util;
mod http_cache;
mod http_util;
mod lockfile;
mod logger;
mod lsp;
mod module_loader;
mod ops;
mod proc_state;
mod resolver;
mod source_maps;
mod standalone;
mod text_encoding;
mod tools;
mod tsc;
mod unix_util;
mod version;
mod windows_util;

use crate::file_fetcher::File;
use crate::file_watcher::ResolutionResult;
use crate::flags::BenchFlags;
use crate::flags::BundleFlags;
use crate::flags::CacheFlags;
use crate::flags::CompileFlags;
use crate::flags::CompletionsFlags;
use crate::flags::CoverageFlags;
use crate::flags::DenoSubcommand;
use crate::flags::DocFlags;
use crate::flags::EvalFlags;
use crate::flags::Flags;
use crate::flags::FmtFlags;
use crate::flags::InfoFlags;
use crate::flags::InstallFlags;
use crate::flags::LintFlags;
use crate::flags::ReplFlags;
use crate::flags::RunFlags;
use crate::flags::TaskFlags;
use crate::flags::TestFlags;
use crate::flags::TypecheckMode;
use crate::flags::UninstallFlags;
use crate::flags::UpgradeFlags;
use crate::flags::VendorFlags;
use crate::fmt_errors::PrettyJsError;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::graph_valid;
use crate::module_loader::CliModuleLoader;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;
use crate::source_maps::apply_source_map;
use deno_ast::MediaType;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::future::LocalFutureObj;
use deno_core::futures::Future;
use deno_core::located_script_name;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::v8_set_flags;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_runtime::colors;
use deno_runtime::ops::worker_host::CreateWebWorkerCb;
use deno_runtime::ops::worker_host::PreloadModuleCb;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_basic;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use log::debug;
use log::info;
use std::env;
use std::io::Read;
use std::io::Write;
use std::iter::once;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

fn create_web_worker_preload_module_callback(
  ps: ProcState,
) -> Arc<PreloadModuleCb> {
  let compat = ps.flags.compat;

  Arc::new(move |mut worker| {
    let fut = async move {
      if compat {
        worker.execute_side_module(&compat::GLOBAL_URL).await?;
        worker.execute_side_module(&compat::MODULE_URL).await?;
      }

      Ok(worker)
    };
    LocalFutureObj::new(Box::new(fut))
  })
}

fn create_web_worker_callback(ps: ProcState) -> Arc<CreateWebWorkerCb> {
  Arc::new(move |args| {
    let global_state_ = ps.clone();
    let js_error_create_fn = Rc::new(move |core_js_error| {
      let source_mapped_error =
        apply_source_map(&core_js_error, global_state_.clone());
      PrettyJsError::create(source_mapped_error)
    });

    let maybe_inspector_server = ps.maybe_inspector_server.clone();

    let module_loader = CliModuleLoader::new_for_worker(
      ps.clone(),
      args.parent_permissions.clone(),
    );
    let create_web_worker_cb = create_web_worker_callback(ps.clone());
    let preload_module_cb =
      create_web_worker_preload_module_callback(ps.clone());

    let extensions = ops::cli_exts(ps.clone(), args.use_deno_namespace);

    let options = WebWorkerOptions {
      bootstrap: BootstrapOptions {
        args: ps.flags.argv.clone(),
        apply_source_maps: true,
        cpu_count: std::thread::available_parallelism()
          .map(|p| p.get())
          .unwrap_or(1),
        debug_flag: ps
          .flags
          .log_level
          .map_or(false, |l| l == log::Level::Debug),
        enable_testing_features: ps.flags.enable_testing_features,
        location: Some(args.main_module.clone()),
        no_color: !colors::use_color(),
        is_tty: colors::is_tty(),
        runtime_version: version::deno(),
        ts_version: version::TYPESCRIPT.to_string(),
        unstable: ps.flags.unstable,
      },
      extensions,
      unsafely_ignore_certificate_errors: ps
        .flags
        .unsafely_ignore_certificate_errors
        .clone(),
      root_cert_store: ps.root_cert_store.clone(),
      user_agent: version::get_user_agent(),
      seed: ps.flags.seed,
      module_loader,
      create_web_worker_cb,
      preload_module_cb,
      js_error_create_fn: Some(js_error_create_fn),
      use_deno_namespace: args.use_deno_namespace,
      worker_type: args.worker_type,
      maybe_inspector_server,
      get_error_class_fn: Some(&crate::errors::get_error_class_name),
      blob_store: ps.blob_store.clone(),
      broadcast_channel: ps.broadcast_channel.clone(),
      shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
      maybe_exit_code: args.maybe_exit_code,
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

pub fn create_main_worker(
  ps: &ProcState,
  main_module: ModuleSpecifier,
  permissions: Permissions,
  mut custom_extensions: Vec<Extension>,
) -> MainWorker {
  let module_loader = CliModuleLoader::new(ps.clone());

  let global_state_ = ps.clone();

  let js_error_create_fn = Rc::new(move |core_js_error| {
    let source_mapped_error =
      apply_source_map(&core_js_error, global_state_.clone());
    PrettyJsError::create(source_mapped_error)
  });

  let maybe_inspector_server = ps.maybe_inspector_server.clone();
  let should_break_on_first_statement = ps.flags.inspect_brk.is_some();

  let create_web_worker_cb = create_web_worker_callback(ps.clone());
  let web_worker_preload_module_cb =
    create_web_worker_preload_module_callback(ps.clone());

  let maybe_storage_key = if let Some(location) = &ps.flags.location {
    // if a location is set, then the ascii serialization of the location is
    // used, unless the origin is opaque, and then no storage origin is set, as
    // we can't expect the origin to be reproducible
    let storage_origin = location.origin().ascii_serialization();
    if storage_origin == "null" {
      None
    } else {
      Some(storage_origin)
    }
  } else if let Some(config_file) = &ps.maybe_config_file {
    // otherwise we will use the path to the config file
    Some(config_file.specifier.to_string())
  } else {
    // otherwise we will use the path to the main module
    Some(main_module.to_string())
  };

  let origin_storage_dir = maybe_storage_key.map(|key| {
    ps.dir
      .root
      // TODO(@crowlKats): change to origin_data for 2.0
      .join("location_data")
      .join(checksum::gen(&[key.as_bytes()]))
  });

  let mut extensions = ops::cli_exts(ps.clone(), true);
  extensions.append(&mut custom_extensions);

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      apply_source_maps: true,
      args: ps.flags.argv.clone(),
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      debug_flag: ps.flags.log_level.map_or(false, |l| l == log::Level::Debug),
      enable_testing_features: ps.flags.enable_testing_features,
      location: ps.flags.location.clone(),
      no_color: !colors::use_color(),
      is_tty: colors::is_tty(),
      runtime_version: version::deno(),
      ts_version: version::TYPESCRIPT.to_string(),
      unstable: ps.flags.unstable,
    },
    extensions,
    unsafely_ignore_certificate_errors: ps
      .flags
      .unsafely_ignore_certificate_errors
      .clone(),
    root_cert_store: ps.root_cert_store.clone(),
    user_agent: version::get_user_agent(),
    seed: ps.flags.seed,
    js_error_create_fn: Some(js_error_create_fn),
    create_web_worker_cb,
    web_worker_preload_module_cb,
    maybe_inspector_server,
    should_break_on_first_statement,
    module_loader,
    get_error_class_fn: Some(&crate::errors::get_error_class_name),
    origin_storage_dir,
    blob_store: ps.blob_store.clone(),
    broadcast_channel: ps.broadcast_channel.clone(),
    shared_array_buffer_store: Some(ps.shared_array_buffer_store.clone()),
    compiled_wasm_module_store: Some(ps.compiled_wasm_module_store.clone()),
  };

  MainWorker::bootstrap_from_options(main_module, permissions, options)
}

pub fn write_to_stdout_ignore_sigpipe(
  bytes: &[u8],
) -> Result<(), std::io::Error> {
  use std::io::ErrorKind;

  match std::io::stdout().write_all(bytes) {
    Ok(()) => Ok(()),
    Err(e) => match e.kind() {
      ErrorKind::BrokenPipe => Ok(()),
      _ => Err(e),
    },
  }
}

pub fn write_json_to_stdout<T>(value: &T) -> Result<(), AnyError>
where
  T: ?Sized + serde::ser::Serialize,
{
  let mut writer = std::io::BufWriter::new(std::io::stdout());
  serde_json::to_writer_pretty(&mut writer, value)?;
  writeln!(&mut writer)?;
  Ok(())
}

fn print_cache_info(
  state: &ProcState,
  json: bool,
  location: Option<&deno_core::url::Url>,
) -> Result<(), AnyError> {
  let deno_dir = &state.dir.root;
  let modules_cache = &state.file_fetcher.get_http_cache_location();
  let typescript_cache = &state.dir.gen_cache.location;
  let registry_cache =
    &state.dir.root.join(lsp::language_server::REGISTRIES_PATH);
  let mut origin_dir = state.dir.root.join("location_data");

  if let Some(location) = &location {
    origin_dir =
      origin_dir.join(&checksum::gen(&[location.to_string().as_bytes()]));
  }

  let local_storage_dir = origin_dir.join("local_storage");

  if json {
    let mut output = json!({
      "denoDir": deno_dir,
      "modulesCache": modules_cache,
      "typescriptCache": typescript_cache,
      "registryCache": registry_cache,
      "originStorage": origin_dir,
    });

    if location.is_some() {
      output["localStorage"] = serde_json::to_value(local_storage_dir)?;
    }

    write_json_to_stdout(&output)
  } else {
    println!(
      "{} {}",
      colors::bold("DENO_DIR location:"),
      deno_dir.display()
    );
    println!(
      "{} {}",
      colors::bold("Remote modules cache:"),
      modules_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Emitted modules cache:"),
      typescript_cache.display()
    );
    println!(
      "{} {}",
      colors::bold("Language server registries cache:"),
      registry_cache.display(),
    );
    println!(
      "{} {}",
      colors::bold("Origin storage:"),
      origin_dir.display()
    );
    if location.is_some() {
      println!(
        "{} {}",
        colors::bold("Local Storage:"),
        local_storage_dir.display(),
      );
    }
    Ok(())
  }
}

pub fn get_types(unstable: bool) -> String {
  let mut types = vec![
    crate::tsc::DENO_NS_LIB,
    crate::tsc::DENO_CONSOLE_LIB,
    crate::tsc::DENO_URL_LIB,
    crate::tsc::DENO_WEB_LIB,
    crate::tsc::DENO_FETCH_LIB,
    crate::tsc::DENO_WEBGPU_LIB,
    crate::tsc::DENO_WEBSOCKET_LIB,
    crate::tsc::DENO_WEBSTORAGE_LIB,
    crate::tsc::DENO_CRYPTO_LIB,
    crate::tsc::DENO_BROADCAST_CHANNEL_LIB,
    crate::tsc::DENO_NET_LIB,
    crate::tsc::SHARED_GLOBALS_LIB,
    crate::tsc::WINDOW_LIB,
  ];

  if unstable {
    types.push(crate::tsc::UNSTABLE_NS_LIB);
  }

  types.join("\n")
}

async fn compile_command(
  flags: Flags,
  compile_flags: CompileFlags,
) -> Result<i32, AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);

  let run_flags = tools::standalone::compile_to_runtime_flags(
    &flags,
    compile_flags.args.clone(),
  )?;

  let module_specifier = resolve_url_or_path(&compile_flags.source_file)?;
  let ps = ProcState::build(Arc::new(flags)).await?;
  let deno_dir = &ps.dir;

  let output_path =
    tools::standalone::resolve_compile_executable_output_path(&compile_flags)?;

  let graph = Arc::try_unwrap(
    create_graph_and_maybe_check(module_specifier.clone(), &ps, debug).await?,
  )
  .map_err(|_| {
    generic_error("There should only be one reference to ModuleGraph")
  })?;

  graph.valid().unwrap();

  let eszip = eszip::EszipV2::from_graph(graph, Default::default())?;

  info!(
    "{} {}",
    colors::green("Compile"),
    module_specifier.to_string()
  );

  // Select base binary based on target
  let original_binary =
    tools::standalone::get_base_binary(deno_dir, compile_flags.target.clone())
      .await?;

  let final_bin = tools::standalone::create_standalone_binary(
    original_binary,
    eszip,
    module_specifier.clone(),
    run_flags,
    ps,
  )
  .await?;

  info!("{} {}", colors::green("Emit"), output_path.display());

  tools::standalone::write_standalone_binary(output_path, final_bin).await?;

  Ok(0)
}

async fn info_command(
  flags: Flags,
  info_flags: InfoFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  if let Some(specifier) = info_flags.file {
    let specifier = resolve_url_or_path(&specifier)?;
    let mut cache = cache::FetchCacher::new(
      ps.dir.gen_cache.clone(),
      ps.file_fetcher.clone(),
      Permissions::allow_all(),
      Permissions::allow_all(),
    );
    let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
    let maybe_import_map_resolver =
      ps.maybe_import_map.clone().map(ImportMapResolver::new);
    let maybe_jsx_resolver = ps.maybe_config_file.as_ref().and_then(|cf| {
      cf.to_maybe_jsx_import_source_module()
        .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
    });
    let maybe_resolver = if maybe_jsx_resolver.is_some() {
      maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
    } else {
      maybe_import_map_resolver
        .as_ref()
        .map(|im| im.as_resolver())
    };
    let graph = deno_graph::create_graph(
      vec![(specifier, deno_graph::ModuleKind::Esm)],
      false,
      None,
      &mut cache,
      maybe_resolver,
      maybe_locker,
      None,
      None,
    )
    .await;

    if info_flags.json {
      write_json_to_stdout(&json!(graph))?;
    } else {
      write_to_stdout_ignore_sigpipe(graph.to_string().as_bytes())?;
    }
  } else {
    // If it was just "deno info" print location of caches and exit
    print_cache_info(&ps, info_flags.json, ps.flags.location.as_ref())?;
  }
  Ok(0)
}

async fn install_command(
  flags: Flags,
  install_flags: InstallFlags,
) -> Result<i32, AnyError> {
  let mut preload_flags = flags.clone();
  preload_flags.inspect = None;
  preload_flags.inspect_brk = None;
  let permissions =
    Permissions::from_options(&preload_flags.permissions_options());
  let ps = ProcState::build(Arc::new(preload_flags)).await?;
  let main_module = resolve_url_or_path(&install_flags.module_url)?;
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions, vec![]);
  // First, fetch and compile the module; this step ensures that the module exists.
  worker.preload_module(&main_module, true).await?;
  tools::installer::install(flags, install_flags)?;
  Ok(0)
}

async fn uninstall_command(
  uninstall_flags: UninstallFlags,
) -> Result<i32, AnyError> {
  tools::installer::uninstall(uninstall_flags.name, uninstall_flags.root)?;
  Ok(0)
}

async fn lsp_command() -> Result<i32, AnyError> {
  lsp::start().await?;
  Ok(0)
}

async fn lint_command(
  flags: Flags,
  lint_flags: LintFlags,
) -> Result<i32, AnyError> {
  if lint_flags.rules {
    tools::lint::print_rules_list(lint_flags.json);
    return Ok(0);
  }

  tools::lint::lint(flags, lint_flags).await?;
  Ok(0)
}

async fn cache_command(
  flags: Flags,
  cache_flags: CacheFlags,
) -> Result<i32, AnyError> {
  let lib = if flags.unstable {
    emit::TypeLib::UnstableDenoWindow
  } else {
    emit::TypeLib::DenoWindow
  };
  let ps = ProcState::build(Arc::new(flags)).await?;

  for file in cache_flags.files {
    let specifier = resolve_url_or_path(&file)?;
    ps.prepare_module_load(
      vec![specifier],
      false,
      lib.clone(),
      Permissions::allow_all(),
      Permissions::allow_all(),
      false,
    )
    .await?;
  }

  Ok(0)
}

async fn eval_command(
  flags: Flags,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  // deno_graph works off of extensions for local files to determine the media
  // type, and so our "fake" specifier needs to have the proper extension.
  let main_module =
    resolve_url_or_path(&format!("./$deno$eval.{}", eval_flags.ext)).unwrap();
  let permissions = Permissions::from_options(&flags.permissions_options());
  let ps = ProcState::build(Arc::new(flags)).await?;
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions, vec![]);
  // Create a dummy source file.
  let source_code = if eval_flags.print {
    format!("console.log({})", eval_flags.code)
  } else {
    eval_flags.code
  }
  .into_bytes();

  let file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::Unknown,
    source: Arc::new(String::from_utf8(source_code)?),
    specifier: main_module.clone(),
    maybe_headers: None,
  };

  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler.
  ps.file_fetcher.insert_cached(file);
  debug!("main_module {}", &main_module);
  if ps.flags.compat {
    worker.execute_side_module(&compat::GLOBAL_URL).await?;
  }
  worker.execute_main_module(&main_module).await?;
  worker.dispatch_load_event(&located_script_name!())?;
  worker.run_event_loop(false).await?;
  worker.dispatch_unload_event(&located_script_name!())?;
  Ok(0)
}

async fn create_graph_and_maybe_check(
  root: ModuleSpecifier,
  ps: &ProcState,
  debug: bool,
) -> Result<Arc<deno_graph::ModuleGraph>, AnyError> {
  let mut cache = cache::FetchCacher::new(
    ps.dir.gen_cache.clone(),
    ps.file_fetcher.clone(),
    Permissions::allow_all(),
    Permissions::allow_all(),
  );
  let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
  let maybe_imports = if let Some(config_file) = &ps.maybe_config_file {
    config_file.to_maybe_imports()?
  } else {
    None
  };
  let maybe_import_map_resolver =
    ps.maybe_import_map.clone().map(ImportMapResolver::new);
  let maybe_jsx_resolver = ps.maybe_config_file.as_ref().and_then(|cf| {
    cf.to_maybe_jsx_import_source_module()
      .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
  });
  let maybe_resolver = if maybe_jsx_resolver.is_some() {
    maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
  } else {
    maybe_import_map_resolver
      .as_ref()
      .map(|im| im.as_resolver())
  };
  let graph = Arc::new(
    deno_graph::create_graph(
      vec![(root, deno_graph::ModuleKind::Esm)],
      false,
      maybe_imports,
      &mut cache,
      maybe_resolver,
      maybe_locker,
      None,
      None,
    )
    .await,
  );

  let check_js = ps
    .maybe_config_file
    .as_ref()
    .map(|cf| cf.get_check_js())
    .unwrap_or(false);
  graph_valid(
    &graph,
    ps.flags.typecheck_mode != TypecheckMode::None,
    check_js,
  )?;
  graph_lock_or_exit(&graph);

  if ps.flags.typecheck_mode != TypecheckMode::None {
    let lib = if ps.flags.unstable {
      emit::TypeLib::UnstableDenoWindow
    } else {
      emit::TypeLib::DenoWindow
    };
    let (ts_config, maybe_ignored_options) = emit::get_ts_config(
      emit::ConfigType::Check {
        tsc_emit: false,
        lib,
      },
      ps.maybe_config_file.as_ref(),
      None,
    )?;
    if let Some(ignored_options) = maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
    let maybe_config_specifier =
      ps.maybe_config_file.as_ref().map(|cf| cf.specifier.clone());
    let check_result = emit::check_and_maybe_emit(
      &graph.roots,
      Arc::new(RwLock::new(graph.as_ref().into())),
      &mut cache,
      emit::CheckOptions {
        typecheck_mode: ps.flags.typecheck_mode.clone(),
        debug,
        emit_with_diagnostics: false,
        maybe_config_specifier,
        ts_config,
        log_checks: true,
        reload: ps.flags.reload,
        reload_exclusions: Default::default(),
      },
    )?;
    debug!("{}", check_result.stats);
    if !check_result.diagnostics.is_empty() {
      return Err(check_result.diagnostics.into());
    }
  }

  Ok(graph)
}

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  ps: &ProcState,
  flags: &Flags,
) -> Result<(String, Option<String>), AnyError> {
  info!("{} {}", colors::green("Bundle"), graph.roots[0].0);

  let (ts_config, maybe_ignored_options) = emit::get_ts_config(
    emit::ConfigType::Bundle,
    ps.maybe_config_file.as_ref(),
    None,
  )?;
  if flags.typecheck_mode == TypecheckMode::None {
    if let Some(ignored_options) = maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
  }

  emit::bundle(
    graph,
    emit::BundleOptions {
      bundle_type: emit::BundleType::Module,
      ts_config,
      emit_ignore_directives: true,
    },
  )
}

async fn bundle_command(
  flags: Flags,
  bundle_flags: BundleFlags,
) -> Result<i32, AnyError> {
  let debug = flags.log_level == Some(log::Level::Debug);
  let flags = Arc::new(flags);
  let resolver = |_| {
    let flags = flags.clone();
    let source_file1 = bundle_flags.source_file.clone();
    let source_file2 = bundle_flags.source_file.clone();
    async move {
      let module_specifier = resolve_url_or_path(&source_file1)?;

      debug!(">>>>> bundle START");
      let ps = ProcState::build(flags).await?;

      let graph =
        create_graph_and_maybe_check(module_specifier, &ps, debug).await?;

      let mut paths_to_watch: Vec<PathBuf> = graph
        .specifiers()
        .iter()
        .filter_map(|(_, r)| {
          r.as_ref().ok().and_then(|(s, _, _)| s.to_file_path().ok())
        })
        .collect();

      if let Ok(Some(import_map_path)) =
        config_file::resolve_import_map_specifier(
          ps.flags.import_map_path.as_deref(),
          ps.maybe_config_file.as_ref(),
        )
        .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
      {
        paths_to_watch.push(import_map_path);
      }

      Ok((paths_to_watch, graph, ps))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, graph, ps)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok((ps, graph)),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(source_file2)],
        result: Err(e),
      },
    })
  };

  let operation = |(ps, graph): (ProcState, Arc<deno_graph::ModuleGraph>)| {
    let out_file = bundle_flags.out_file.clone();
    async move {
      let (bundle_emit, maybe_bundle_map) =
        bundle_module_graph(graph.as_ref(), &ps, &ps.flags)?;
      debug!(">>>>> bundle END");

      if let Some(out_file) = out_file.as_ref() {
        let output_bytes = bundle_emit.as_bytes();
        let output_len = output_bytes.len();
        fs_util::write_file(out_file, output_bytes, 0o644)?;
        info!(
          "{} {:?} ({})",
          colors::green("Emit"),
          out_file,
          colors::gray(display::human_size(output_len as f64))
        );
        if let Some(bundle_map) = maybe_bundle_map {
          let map_bytes = bundle_map.as_bytes();
          let map_len = map_bytes.len();
          let ext = if let Some(curr_ext) = out_file.extension() {
            format!("{}.map", curr_ext.to_string_lossy())
          } else {
            "map".to_string()
          };
          let map_out_file = out_file.with_extension(ext);
          fs_util::write_file(&map_out_file, map_bytes, 0o644)?;
          info!(
            "{} {:?} ({})",
            colors::green("Emit"),
            map_out_file,
            colors::gray(display::human_size(map_len as f64))
          );
        }
      } else {
        println!("{}", bundle_emit);
      }

      Ok(())
    }
  };

  if flags.watch.is_some() {
    file_watcher::watch_func(
      resolver,
      operation,
      file_watcher::PrintConfig {
        job_name: "Bundle".to_string(),
        clear_screen: !flags.no_clear_screen,
      },
    )
    .await?;
  } else {
    let module_graph =
      if let ResolutionResult::Restart { result, .. } = resolver(None).await {
        result?
      } else {
        unreachable!();
      };
    operation(module_graph).await?;
  }

  Ok(0)
}

async fn doc_command(
  flags: Flags,
  doc_flags: DocFlags,
) -> Result<i32, AnyError> {
  tools::doc::print_docs(flags, doc_flags).await?;
  Ok(0)
}

async fn format_command(
  flags: Flags,
  fmt_flags: FmtFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  let maybe_fmt_config = if let Some(config_file) = &ps.maybe_config_file {
    config_file.to_fmt_config()?
  } else {
    None
  };

  if fmt_flags.files.len() == 1 && fmt_flags.files[0].to_string_lossy() == "-" {
    tools::fmt::format_stdin(
      fmt_flags,
      maybe_fmt_config.map(|c| c.options).unwrap_or_default(),
    )?;
    return Ok(0);
  }

  tools::fmt::format(ps.flags.as_ref(), fmt_flags, maybe_fmt_config).await?;
  Ok(0)
}

async fn repl_command(
  flags: Flags,
  repl_flags: ReplFlags,
) -> Result<i32, AnyError> {
  let main_module = resolve_url_or_path("./$deno$repl.ts").unwrap();
  let permissions = Permissions::from_options(&flags.permissions_options());
  let ps = ProcState::build(Arc::new(flags)).await?;
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions, vec![]);
  if ps.flags.compat {
    worker.execute_side_module(&compat::GLOBAL_URL).await?;
    compat::add_global_require(&mut worker.js_runtime, main_module.as_str())?;
    worker.run_event_loop(false).await?;
    compat::setup_builtin_modules(&mut worker.js_runtime)?;
  }
  worker.run_event_loop(false).await?;

  tools::repl::run(&ps, worker, repl_flags.eval_files, repl_flags.eval).await
}

async fn run_from_stdin(flags: Flags) -> Result<i32, AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  let permissions = Permissions::from_options(&ps.flags.permissions_options());
  let main_module = resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker =
    create_main_worker(&ps.clone(), main_module.clone(), permissions, vec![]);

  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::TypeScript,
    source: Arc::new(String::from_utf8(source)?),
    specifier: main_module.clone(),
    maybe_headers: None,
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  ps.file_fetcher.insert_cached(source_file);

  debug!("main_module {}", main_module);
  if ps.flags.compat {
    worker.execute_side_module(&compat::GLOBAL_URL).await?;
  }
  worker.execute_main_module(&main_module).await?;
  worker.dispatch_load_event(&located_script_name!())?;
  worker.run_event_loop(false).await?;
  worker.dispatch_unload_event(&located_script_name!())?;
  Ok(worker.get_exit_code())
}

// TODO(bartlomieju): this function is not handling `exit_code` set by the runtime
// code properly.
async fn run_with_watch(flags: Flags, script: String) -> Result<i32, AnyError> {
  let flags = Arc::new(flags);
  let resolver = |_| {
    let script1 = script.clone();
    let script2 = script.clone();
    let flags = flags.clone();
    let watch_flag = flags.watch.clone();
    async move {
      let main_module = resolve_url_or_path(&script1)?;
      let ps = ProcState::build(flags).await?;
      let mut cache = cache::FetchCacher::new(
        ps.dir.gen_cache.clone(),
        ps.file_fetcher.clone(),
        Permissions::allow_all(),
        Permissions::allow_all(),
      );
      let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
      let maybe_imports = if let Some(config_file) = &ps.maybe_config_file {
        config_file.to_maybe_imports()?
      } else {
        None
      };
      let maybe_import_map_resolver =
        ps.maybe_import_map.clone().map(ImportMapResolver::new);
      let maybe_jsx_resolver = ps.maybe_config_file.as_ref().and_then(|cf| {
        cf.to_maybe_jsx_import_source_module()
          .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
      });
      let maybe_resolver = if maybe_jsx_resolver.is_some() {
        maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
      } else {
        maybe_import_map_resolver
          .as_ref()
          .map(|im| im.as_resolver())
      };
      let graph = deno_graph::create_graph(
        vec![(main_module.clone(), deno_graph::ModuleKind::Esm)],
        false,
        maybe_imports,
        &mut cache,
        maybe_resolver,
        maybe_locker,
        None,
        None,
      )
      .await;
      let check_js = ps
        .maybe_config_file
        .as_ref()
        .map(|cf| cf.get_check_js())
        .unwrap_or(false);
      graph_valid(
        &graph,
        ps.flags.typecheck_mode != flags::TypecheckMode::None,
        check_js,
      )?;

      // Find all local files in graph
      let mut paths_to_watch: Vec<PathBuf> = graph
        .specifiers()
        .iter()
        .filter_map(|(_, r)| {
          r.as_ref().ok().and_then(|(s, _, _)| s.to_file_path().ok())
        })
        .collect();

      // Add the extra files listed in the watch flag
      if let Some(watch_paths) = watch_flag {
        paths_to_watch.extend(watch_paths);
      }

      if let Ok(Some(import_map_path)) =
        config_file::resolve_import_map_specifier(
          ps.flags.import_map_path.as_deref(),
          ps.maybe_config_file.as_ref(),
        )
        .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
      {
        paths_to_watch.push(import_map_path);
      }

      Ok((paths_to_watch, main_module, ps))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, module_info, ps)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok((ps, module_info)),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(script2)],
        result: Err(e),
      },
    })
  };

  /// The FileWatcherModuleExecutor provides module execution with safe dispatching of life-cycle events by tracking the
  /// state of any pending events and emitting accordingly on drop in the case of a future
  /// cancellation.
  struct FileWatcherModuleExecutor {
    worker: MainWorker,
    pending_unload: bool,
    compat: bool,
  }

  impl FileWatcherModuleExecutor {
    pub fn new(worker: MainWorker, compat: bool) -> FileWatcherModuleExecutor {
      FileWatcherModuleExecutor {
        worker,
        pending_unload: false,
        compat,
      }
    }

    /// Execute the given main module emitting load and unload events before and after execution
    /// respectively.
    pub async fn execute(
      &mut self,
      main_module: &ModuleSpecifier,
    ) -> Result<(), AnyError> {
      if self.compat {
        self.worker.execute_side_module(&compat::GLOBAL_URL).await?;
      }
      self.worker.execute_main_module(main_module).await?;
      self.worker.dispatch_load_event(&located_script_name!())?;
      self.pending_unload = true;

      let result = self.worker.run_event_loop(false).await;
      self.pending_unload = false;

      if let Err(err) = result {
        return Err(err);
      }

      self.worker.dispatch_unload_event(&located_script_name!())?;

      Ok(())
    }
  }

  impl Drop for FileWatcherModuleExecutor {
    fn drop(&mut self) {
      if self.pending_unload {
        self
          .worker
          .dispatch_unload_event(&located_script_name!())
          .unwrap();
      }
    }
  }

  let operation = |(ps, main_module): (ProcState, ModuleSpecifier)| {
    let flags = flags.clone();
    let permissions = Permissions::from_options(&flags.permissions_options());
    async move {
      // We make use an module executor guard to ensure that unload is always fired when an
      // operation is called.
      let mut executor = FileWatcherModuleExecutor::new(
        create_main_worker(&ps, main_module.clone(), permissions, vec![]),
        flags.compat,
      );

      executor.execute(&main_module).await?;

      Ok(())
    }
  };

  file_watcher::watch_func(
    resolver,
    operation,
    file_watcher::PrintConfig {
      job_name: "Process".to_string(),
      clear_screen: !flags.no_clear_screen,
    },
  )
  .await?;
  Ok(0)
}

async fn run_command(
  flags: Flags,
  run_flags: RunFlags,
) -> Result<i32, AnyError> {
  // Read script content from stdin
  if run_flags.script == "-" {
    return run_from_stdin(flags).await;
  }

  if flags.watch.is_some() {
    return run_with_watch(flags, run_flags.script).await;
  }

  // TODO(bartlomieju): it should not be resolved here if we're in compat mode
  // because it might be a bare specifier
  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line - this should
  // probably call `ProcState::resolve` instead
  let main_module = resolve_url_or_path(&run_flags.script)?;
  let ps = ProcState::build(Arc::new(flags)).await?;
  let permissions = Permissions::from_options(&ps.flags.permissions_options());
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions, vec![]);

  let mut maybe_coverage_collector =
    if let Some(ref coverage_dir) = ps.coverage_dir {
      let session = worker.create_inspector_session().await;

      let coverage_dir = PathBuf::from(coverage_dir);
      let mut coverage_collector =
        tools::coverage::CoverageCollector::new(coverage_dir, session);
      worker
        .with_event_loop(coverage_collector.start_collecting().boxed_local())
        .await?;
      Some(coverage_collector)
    } else {
      None
    };

  debug!("main_module {}", main_module);

  if ps.flags.compat {
    // TODO(bartlomieju): fix me
    assert_eq!(main_module.scheme(), "file");

    // Set up Node globals
    worker.execute_side_module(&compat::GLOBAL_URL).await?;
    // And `module` module that we'll use for checking which
    // loader to use and potentially load CJS module with.
    // This allows to skip permission check for `--allow-net`
    // which would otherwise be requested by dynamically importing
    // this file.
    worker.execute_side_module(&compat::MODULE_URL).await?;

    let use_esm_loader = compat::check_if_should_use_esm_loader(&main_module)?;

    if use_esm_loader {
      // ES module execution in Node compatiblity mode
      worker.execute_main_module(&main_module).await?;
    } else {
      // CJS module execution in Node compatiblity mode
      compat::load_cjs_module(
        &mut worker.js_runtime,
        &main_module.to_file_path().unwrap().display().to_string(),
        true,
      )?;
    }
  } else {
    // Regular ES module execution
    worker.execute_main_module(&main_module).await?;
  }

  worker.dispatch_load_event(&located_script_name!())?;
  worker
    .run_event_loop(maybe_coverage_collector.is_none())
    .await?;
  worker.dispatch_unload_event(&located_script_name!())?;

  if let Some(coverage_collector) = maybe_coverage_collector.as_mut() {
    worker
      .with_event_loop(coverage_collector.stop_collecting().boxed_local())
      .await?;
  }
  Ok(worker.get_exit_code())
}

async fn task_command(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  tools::task::execute_script(flags, task_flags).await
}

async fn coverage_command(
  flags: Flags,
  coverage_flags: CoverageFlags,
) -> Result<i32, AnyError> {
  if coverage_flags.files.is_empty() {
    return Err(generic_error("No matching coverage profiles found"));
  }

  tools::coverage::cover_files(flags, coverage_flags).await?;
  Ok(0)
}

async fn bench_command(
  flags: Flags,
  bench_flags: BenchFlags,
) -> Result<i32, AnyError> {
  if flags.watch.is_some() {
    tools::bench::run_benchmarks_with_watch(flags, bench_flags).await?;
  } else {
    tools::bench::run_benchmarks(flags, bench_flags).await?;
  }

  Ok(0)
}

async fn test_command(
  flags: Flags,
  test_flags: TestFlags,
) -> Result<i32, AnyError> {
  if let Some(ref coverage_dir) = flags.coverage_dir {
    std::fs::create_dir_all(&coverage_dir)?;
    env::set_var(
      "DENO_UNSTABLE_COVERAGE_DIR",
      PathBuf::from(coverage_dir).canonicalize()?,
    );
  }

  if flags.watch.is_some() {
    tools::test::run_tests_with_watch(flags, test_flags).await?;
  } else {
    tools::test::run_tests(flags, test_flags).await?;
  }

  Ok(0)
}

async fn completions_command(
  _flags: Flags,
  completions_flags: CompletionsFlags,
) -> Result<i32, AnyError> {
  write_to_stdout_ignore_sigpipe(&completions_flags.buf)?;
  Ok(0)
}

async fn types_command(flags: Flags) -> Result<i32, AnyError> {
  let types = get_types(flags.unstable);
  write_to_stdout_ignore_sigpipe(types.as_bytes())?;
  Ok(0)
}

async fn upgrade_command(
  _flags: Flags,
  upgrade_flags: UpgradeFlags,
) -> Result<i32, AnyError> {
  tools::upgrade::upgrade(upgrade_flags).await?;
  Ok(0)
}

async fn vendor_command(
  flags: Flags,
  vendor_flags: VendorFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  tools::vendor::vendor(ps, vendor_flags).await?;
  Ok(0)
}

fn init_v8_flags(v8_flags: &[String]) {
  let v8_flags_includes_help = v8_flags
    .iter()
    .any(|flag| flag == "-help" || flag == "--help");
  // Keep in sync with `standalone.rs`.
  let v8_flags = once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
    .chain(v8_flags.iter().cloned())
    .collect::<Vec<_>>();
  let unrecognized_v8_flags = v8_set_flags(v8_flags)
    .into_iter()
    .skip(1)
    .collect::<Vec<_>>();
  if !unrecognized_v8_flags.is_empty() {
    for f in unrecognized_v8_flags {
      eprintln!("error: V8 did not recognize flag '{}'", f);
    }
    eprintln!("\nFor a list of V8 flags, use '--v8-flags=--help'");
    std::process::exit(1);
  }
  if v8_flags_includes_help {
    std::process::exit(0);
  }
}

fn get_subcommand(
  flags: Flags,
) -> Pin<Box<dyn Future<Output = Result<i32, AnyError>>>> {
  match flags.subcommand.clone() {
    DenoSubcommand::Bench(bench_flags) => {
      bench_command(flags, bench_flags).boxed_local()
    }
    DenoSubcommand::Bundle(bundle_flags) => {
      bundle_command(flags, bundle_flags).boxed_local()
    }
    DenoSubcommand::Doc(doc_flags) => {
      doc_command(flags, doc_flags).boxed_local()
    }
    DenoSubcommand::Eval(eval_flags) => {
      eval_command(flags, eval_flags).boxed_local()
    }
    DenoSubcommand::Cache(cache_flags) => {
      cache_command(flags, cache_flags).boxed_local()
    }
    DenoSubcommand::Compile(compile_flags) => {
      compile_command(flags, compile_flags).boxed_local()
    }
    DenoSubcommand::Coverage(coverage_flags) => {
      coverage_command(flags, coverage_flags).boxed_local()
    }
    DenoSubcommand::Fmt(fmt_flags) => {
      format_command(flags, fmt_flags).boxed_local()
    }
    DenoSubcommand::Info(info_flags) => {
      info_command(flags, info_flags).boxed_local()
    }
    DenoSubcommand::Install(install_flags) => {
      install_command(flags, install_flags).boxed_local()
    }
    DenoSubcommand::Uninstall(uninstall_flags) => {
      uninstall_command(uninstall_flags).boxed_local()
    }
    DenoSubcommand::Lsp => lsp_command().boxed_local(),
    DenoSubcommand::Lint(lint_flags) => {
      lint_command(flags, lint_flags).boxed_local()
    }
    DenoSubcommand::Repl(repl_flags) => {
      repl_command(flags, repl_flags).boxed_local()
    }
    DenoSubcommand::Run(run_flags) => {
      run_command(flags, run_flags).boxed_local()
    }
    DenoSubcommand::Task(task_flags) => {
      task_command(flags, task_flags).boxed_local()
    }
    DenoSubcommand::Test(test_flags) => {
      test_command(flags, test_flags).boxed_local()
    }
    DenoSubcommand::Completions(completions_flags) => {
      completions_command(flags, completions_flags).boxed_local()
    }
    DenoSubcommand::Types => types_command(flags).boxed_local(),
    DenoSubcommand::Upgrade(upgrade_flags) => {
      upgrade_command(flags, upgrade_flags).boxed_local()
    }
    DenoSubcommand::Vendor(vendor_flags) => {
      vendor_command(flags, vendor_flags).boxed_local()
    }
  }
}

fn setup_panic_hook() {
  // This function does two things inside of the panic hook:
  // - Tokio does not exit the process when a task panics, so we define a custom
  //   panic hook to implement this behaviour.
  // - We print a message to stderr to indicate that this is a bug in Deno, and
  //   should be reported to us.
  let orig_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |panic_info| {
    eprintln!("\n============================================================");
    eprintln!("Deno has panicked. This is a bug in Deno. Please report this");
    eprintln!("at https://github.com/denoland/deno/issues/new.");
    eprintln!("If you can reliably reproduce this panic, include the");
    eprintln!("reproduction steps and re-run with the RUST_BACKTRACE=1 env");
    eprintln!("var set and include the backtrace in your report.");
    eprintln!();
    eprintln!(
      "Platform: {} {}",
      std::env::consts::OS,
      std::env::consts::ARCH
    );
    eprintln!("Version: {}", version::deno());
    eprintln!("Args: {:?}", std::env::args().collect::<Vec<_>>());
    eprintln!();
    orig_hook(panic_info);
    std::process::exit(1);
  }));
}

fn unwrap_or_exit<T>(result: Result<T, AnyError>) -> T {
  match result {
    Ok(value) => value,
    Err(error) => {
      eprintln!(
        "{}: {}",
        colors::red_bold("error"),
        format!("{:?}", error).trim_start_matches("error: ")
      );
      std::process::exit(1);
    }
  }
}

pub fn main() {
  setup_panic_hook();

  unix_util::raise_fd_limit();
  windows_util::ensure_stdio_open();
  #[cfg(windows)]
  colors::enable_ansi(); // For Windows 10

  let args: Vec<String> = env::args().collect();

  let exit_code = async move {
    let standalone_res =
      match standalone::extract_standalone(args.clone()).await {
        Ok(Some((metadata, eszip))) => standalone::run(eszip, metadata).await,
        Ok(None) => Ok(()),
        Err(err) => Err(err),
      };
    // TODO(bartlomieju): doesn't handle exit code set by the runtime properly
    unwrap_or_exit(standalone_res);

    let flags = match flags::flags_from_vec(args) {
      Ok(flags) => flags,
      Err(err @ clap::Error { .. })
        if err.kind() == clap::ErrorKind::DisplayHelp
          || err.kind() == clap::ErrorKind::DisplayVersion =>
      {
        err.print().unwrap();
        std::process::exit(0);
      }
      Err(err) => unwrap_or_exit(Err(AnyError::from(err))),
    };
    if !flags.v8_flags.is_empty() {
      init_v8_flags(&*flags.v8_flags);
    }

    logger::init(flags.log_level);

    let exit_code = get_subcommand(flags).await;

    exit_code
  };

  let exit_code = unwrap_or_exit(run_basic(exit_code));

  std::process::exit(exit_code);
}
