// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Read;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::file_fetcher::File;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_npm_installer::PackageCaching;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_path_util::resolve_url_or_path;
use deno_runtime::WorkerExecutionMode;
use deno_semver::npm::NpmPackageReqReference;

use crate::args::EvalFlags;
use crate::args::Flags;
use crate::args::RunFlags;
use crate::args::WatchFlagsWithPaths;
use crate::factory::CliFactory;
use crate::util;
use crate::util::file_watcher::WatcherRestartMode;

pub mod hmr;

pub fn check_permission_before_script(flags: &Flags) {
  if !flags.has_permission() && flags.has_permission_in_argv() {
    log::warn!(
      "{}",
      crate::colors::yellow(
        r#"Permission flags have likely been incorrectly set after the script argument.
To grant permissions, set them before the script argument. For example:
    deno run --allow-read=. main.js"#
      )
    );
  }
}

pub fn set_npm_user_agent() {
  static ONCE: std::sync::Once = std::sync::Once::new();
  ONCE.call_once(|| {
    // SAFETY: guarded by Once so only called once from a single thread
    unsafe {
      std::env::set_var(
        crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR,
        crate::npm::get_npm_config_user_agent(),
      )
    };
  });
}

/// If the main module is an `npm:` specifier whose resolved bin entry is a
/// native executable (Mach-O / ELF / PE), returns the resolved `BinValue` so
/// callers can decide what to do with it. Returns `Ok(None)` when the bin
/// should fall through to the JS module loader (or no bin can be resolved).
async fn resolve_npm_native_bin(
  factory: &CliFactory,
  main_module: &ModuleSpecifier,
) -> Result<Option<node_resolver::BinValue>, AnyError> {
  if main_module.scheme() != "npm" {
    return Ok(None);
  }
  let req_ref = match NpmPackageReqReference::from_specifier(main_module) {
    Ok(req_ref) => req_ref,
    Err(_) => return Ok(None),
  };
  // If the user explicitly asked for a sub-path, don't intercept.
  if req_ref.sub_path().is_some() {
    return Ok(None);
  }

  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;

  let cwd_url =
    match deno_path_util::url_from_directory_path(cli_options.initial_cwd()) {
      Ok(url) => url,
      Err(_) => return Ok(None),
    };

  let package_folder = match npm_resolver
    .resolve_pkg_folder_from_deno_module_req(req_ref.req(), &cwd_url)
  {
    Ok(folder) => folder,
    Err(_) => return Ok(None),
  };

  let bins = match node_resolver
    .resolve_npm_binary_commands_for_package(&package_folder)
  {
    Ok(bins) if !bins.is_empty() => bins,
    _ => return Ok(None),
  };

  let bin_value =
    match crate::tools::x::find_bin_value(&bins, req_ref.req().name.as_str()) {
      Some(bin_value) => bin_value,
      None => return Ok(None),
    };

  // `read_bin_value` (used to build `bins`) classifies anything without a
  // `#!` shebang as `BinValue::Executable`, so a plain `cli.mjs` ends up
  // here too. Re-check the actual file magic so we only intercept real
  // ELF / Mach-O / PE binaries and let JS bins fall through to the module
  // loader as before.
  if !is_native_binary(bin_value.path()) {
    return Ok(None);
  }

  Ok(Some(bin_value))
}

/// If the main module resolves to a native npm bin, spawn it as a subprocess
/// and return its exit code. Otherwise returns `Ok(None)` so the caller
/// proceeds with the normal JS module-loading path.
async fn try_run_npm_bin_executable(
  factory: &CliFactory,
  flags: &Flags,
  main_module: &ModuleSpecifier,
) -> Result<Option<i32>, AnyError> {
  let Some(bin_value) = resolve_npm_native_bin(factory, main_module).await?
  else {
    return Ok(None);
  };

  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let unstable_args = cli_options.unstable_args();
  let npm_process_state = crate::tools::x::get_npm_process_state(npm_resolver);
  let exit_code = crate::tools::x::run_bin_value(
    factory,
    flags,
    bin_value,
    npm_process_state,
    &unstable_args,
  )?;
  Ok(Some(exit_code))
}

async fn is_npm_native_bin(
  factory: &CliFactory,
  main_module: &ModuleSpecifier,
) -> Result<bool, AnyError> {
  Ok(
    resolve_npm_native_bin(factory, main_module)
      .await?
      .is_some(),
  )
}

fn is_native_binary(path: &std::path::Path) -> bool {
  let Ok(mut file) = std::fs::File::open(path) else {
    return false;
  };
  let mut buf = [0u8; 4];
  if file.read_exact(&mut buf).is_err() {
    return false;
  }
  node_resolver::is_binary(&buf)
}

pub async fn run_script(
  mode: WorkerExecutionMode,
  flags: Arc<Flags>,
  watch: Option<WatchFlagsWithPaths>,
  unconfigured_runtime: Option<deno_runtime::UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  check_permission_before_script(&flags);

  if let Some(watch_flags) = watch {
    return run_with_watch(mode, flags, watch_flags).boxed_local().await;
  }

  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line
  crate::boot_phase("run_script start");
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  crate::boot_phase("after cli_options");
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client_provider();
  let workspace_resolver = factory.workspace_resolver().await?;
  crate::boot_phase("after workspace_resolver");
  let node_resolver = factory.node_resolver().await?;
  crate::boot_phase("after node_resolver");
  // Run a background task that checks for available upgrades or output
  // if an earlier run of this background task found a new version of Deno.
  #[cfg(feature = "upgrade")]
  super::upgrade::check_for_upgrades(
    http_client.clone(),
    deno_dir.upgrade_check_file_path(),
  );

  let main_module_resolver = crate::args::WorkspaceMainModuleResolver::new(
    workspace_resolver.clone(),
    node_resolver.clone(),
  );
  let main_module = cli_options
    .resolve_main_module_with_resolver(Some(&main_module_resolver))?;
  let preload_modules =
    cli_options.preload_modules_with_resolver(Some(&main_module_resolver))?;
  let require_modules = cli_options.require_modules()?;

  if main_module.scheme() == "npm" {
    set_npm_user_agent();
  }

  maybe_npm_install(&factory).await?;

  crate::boot_phase("after resolve main_module");

  if let Some(exit_code) =
    try_run_npm_bin_executable(&factory, &flags, main_module).await?
  {
    return Ok(exit_code);
  }

  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(roots)
    .await?;
  crate::boot_phase("after worker_factory");
  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      mode,
      main_module.clone(),
      preload_modules,
      require_modules,
      unconfigured_runtime,
    )
    .await
    .inspect_err(|e| deno_telemetry::report_event("boot_failure", e))?;
  crate::boot_phase("after create_main_worker");

  let exit_code = worker
    .run()
    .await
    .inspect_err(|e| deno_telemetry::report_event("uncaught_exception", e))?;
  crate::boot_phase("after worker.run (exit)");
  Ok(exit_code)
}

pub async fn run_from_stdin(
  flags: Arc<Flags>,
  unconfigured_runtime: Option<deno_runtime::UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let main_module = cli_options.resolve_main_module()?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  maybe_npm_install(&factory).await?;

  let file_fetcher = factory.file_fetcher()?;
  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(roots)
    .await?;
  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Save a fake file into file fetcher cache
  // to allow module access by TS compiler
  file_fetcher.insert_memory_files(File {
    url: main_module.clone(),
    mtime: None,
    maybe_headers: None,
    source: source.into(),
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
  });

  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      WorkerExecutionMode::Run,
      main_module.clone(),
      preload_modules,
      require_modules,
      unconfigured_runtime,
    )
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

// TODO(bartlomieju): this function is not handling `exit_code` set by the runtime
// code properly.
async fn run_with_watch(
  mode: WorkerExecutionMode,
  flags: Arc<Flags>,
  watch_flags: WatchFlagsWithPaths,
) -> Result<i32, AnyError> {
  util::file_watcher::watch_recv(
    flags,
    util::file_watcher::PrintConfig::new_with_banner(
      if watch_flags.hmr { "HMR" } else { "Watcher" },
      "Process",
      !watch_flags.no_clear_screen,
    ),
    WatcherRestartMode::Automatic,
    move |flags, watcher_communicator, changed_paths| {
      watcher_communicator.show_path_changed(changed_paths.clone());
      Ok(async move {
        let factory = CliFactory::from_flags_for_watcher(
          flags,
          watcher_communicator.clone(),
        );
        let cli_options = factory.cli_options()?;
        let main_module = cli_options.resolve_main_module()?;
        let preload_modules = cli_options.preload_modules()?;
        let require_modules = cli_options.require_modules()?;

        if main_module.scheme() == "npm" {
          set_npm_user_agent();
        }

        maybe_npm_install(&factory).await?;

        if is_npm_native_bin(&factory, main_module).await? {
          return Err(deno_core::anyhow::anyhow!(
            "Cannot use --watch with an npm package whose bin entry is a native executable: {main_module}"
          ));
        }

        let _ = watcher_communicator.watch_paths(cli_options.watch_paths());

        let mut worker = factory
          .create_cli_main_worker_factory()
          .await?
          .create_main_worker(
            mode,
            main_module.clone(),
            preload_modules,
            require_modules,
          )
          .await?;

        let exit_code = if watch_flags.hmr {
          worker.run().await?
        } else {
          worker.run_for_watcher().await?
        };

        Ok(exit_code)
      })
    },
  )
  .boxed_local()
  .await?;

  Ok(0)
}

pub async fn eval_command(
  flags: Arc<Flags>,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  // Auto-detect CJS vs ESM if --ext was not explicitly provided.
  // Default is ESM (preserving existing behavior). Only switch to CJS if
  // the code contains CJS-specific patterns like require() calls.
  // We check for import/export declarations first — if present, it's
  // definitely ESM. If absent, we look for CJS patterns to decide.
  let flags = if flags.ext.is_none() {
    let source_code = if eval_flags.print {
      format!("console.log({})", &eval_flags.code)
    } else {
      eval_flags.code.clone()
    };
    let specifier = deno_core::url::Url::parse("file:///eval.js").unwrap();
    let is_script = deno_ast::parse_program(deno_ast::ParseParams {
      specifier,
      text: source_code.clone().into(),
      media_type: deno_ast::MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map(|parsed| parsed.compute_is_script())
    .unwrap_or(true);
    // Only treat as CJS if it parses as a script AND contains CJS patterns.
    // This preserves backward compatibility: code without imports/exports
    // defaults to ESM (the longstanding deno eval behavior).
    let has_cjs_patterns = is_script
      && (source_code.contains("require(")
        || source_code.contains("module.exports")
        || source_code.contains("exports.")
        || source_code.contains("__dirname")
        || source_code.contains("__filename"));
    if has_cjs_patterns {
      let mut flags = (*flags).clone();
      flags.ext = Some("cjs".to_string());
      Arc::new(flags)
    } else {
      flags
    }
  } else {
    flags
  };
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let file_fetcher = factory.file_fetcher()?;
  let main_module = cli_options.resolve_main_module()?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  maybe_npm_install(&factory).await?;

  // Create a dummy source file.
  let source_code = if eval_flags.print {
    format!("console.log({})", eval_flags.code)
  } else {
    eval_flags.code
  };
  // Match Node's `[eval]` URL for `-e` scripts so inspector clients (and
  // Node compat tests) see the same script URL. V8 honors the
  // `//# sourceURL=` comment for both classic scripts and ES modules.
  // Appended (not prepended) so user line numbers are preserved.
  let source_code = format!("{source_code}\n//# sourceURL=[eval]\n");

  // Save a fake file into file fetcher cache
  // to allow module access by TS compiler.
  file_fetcher.insert_memory_files(File {
    url: main_module.clone(),
    mtime: None,
    maybe_headers: None,
    source: source_code.into_bytes().into(),
    loaded_from: deno_cache_dir::file_fetcher::LoadedFrom::Local,
  });

  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(
      WorkerExecutionMode::Eval,
      main_module.clone(),
      preload_modules,
      require_modules,
    )
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

pub async fn maybe_npm_install(factory: &CliFactory) -> Result<(), AnyError> {
  let cli_options = factory.cli_options()?;
  // ensure an "npm install" is done if the user has explicitly
  // opted into using a managed node_modules directory
  if cli_options.specified_node_modules_dir()? == Some(NodeModulesDirMode::Auto)
    && let Some(npm_installer) = factory.npm_installer_if_managed().await?
  {
    let _clear_guard = factory
      .text_only_progress_bar()
      .deferred_keep_initialize_alive();
    let already_done = npm_installer
      .ensure_top_level_package_json_install()
      .await?;
    if !already_done
      && matches!(
        cli_options.default_npm_caching_strategy(),
        NpmCachingStrategy::Eager
      )
    {
      npm_installer.cache_packages(PackageCaching::All).await?;
    }
  }
  Ok(())
}

pub async fn run_eszip(
  flags: Arc<Flags>,
  run_flags: RunFlags,
  unconfigured_runtime: Option<deno_runtime::UnconfiguredRuntime>,
  roots: LibWorkerFactoryRoots,
) -> Result<i32, AnyError> {
  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;

  // entrypoint#path1,path2,...
  let (entrypoint, _files) = run_flags
    .script
    .split_once("#")
    .with_context(|| "eszip: invalid script string")?;

  let mode = WorkerExecutionMode::Run;
  let main_module = resolve_url_or_path(entrypoint, cli_options.initial_cwd())?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  let worker_factory = factory
    .create_cli_main_worker_factory_with_roots(roots)
    .await?;
  let mut worker = worker_factory
    .create_main_worker_with_unconfigured_runtime(
      mode,
      main_module.clone(),
      preload_modules,
      require_modules,
      unconfigured_runtime,
    )
    .await?;

  let exit_code = worker.run().await?;
  Ok(exit_code)
}
