// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::File;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_lib::standalone::binary::SerializedWorkspaceResolverImportMap;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_npm_installer::PackageCaching;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_path_util::resolve_url_or_path;
use deno_runtime::WorkerExecutionMode;
use eszip::EszipV2;
use jsonc_parser::ParseOptions;

use crate::args::EvalFlags;
use crate::args::Flags;
use crate::args::RunFlags;
use crate::args::WatchFlagsWithPaths;
use crate::factory::CliFactory;
use crate::util;
use crate::util::file_watcher::WatcherRestartMode;
use crate::util::watch_env_tracker::WatchEnvTracker;

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
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::set_var(
        crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR,
        crate::npm::get_npm_config_user_agent(),
      )
    };
  });
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
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client_provider();
  let workspace_resolver = factory.workspace_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  // Run a background task that checks for available upgrades or output
  // if an earlier run of this background task found a new version of Deno.
  #[cfg(feature = "upgrade")]
  super::upgrade::check_for_upgrades(
    http_client.clone(),
    deno_dir.upgrade_check_file_path(),
  );

  let main_module = cli_options.resolve_main_module_with_resolver(Some(
    &crate::args::WorkspaceMainModuleResolver::new(
      workspace_resolver.clone(),
      node_resolver.clone(),
    ),
  ))?;
  let preload_modules = cli_options.preload_modules()?;
  let require_modules = cli_options.require_modules()?;

  if main_module.scheme() == "npm" {
    set_npm_user_agent();
  }

  maybe_npm_install(&factory).await?;

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
    .await
    .inspect_err(|e| deno_telemetry::report_event("boot_failure", e))?;

  let exit_code = worker
    .run()
    .await
    .inspect_err(|e| deno_telemetry::report_event("uncaught_exception", e))?;
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
      let env_file_paths: Option<Vec<std::path::PathBuf>> = flags
        .env_file
        .as_ref()
        .map(|files| files.iter().map(PathBuf::from).collect());
      WatchEnvTracker::snapshot().load_env_variables_from_env_files(
        env_file_paths.as_ref(),
        flags.log_level,
      );
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

        if watch_flags.hmr {
          worker.run().await?;
        } else {
          worker.run_for_watcher().await?;
        }

        Ok(())
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

#[allow(unused)]
async fn load_import_map(
  eszips: &[EszipV2],
  specifier: &str,
) -> Result<SerializedWorkspaceResolverImportMap, AnyError> {
  let maybe_module = eszips
    .iter()
    .rev()
    .find_map(|eszip| eszip.get_import_map(specifier));
  let Some(module) = maybe_module else {
    return Err(AnyError::msg(format!("import map not found '{specifier}'")));
  };
  let base_url = deno_core::url::Url::parse(specifier).map_err(|err| {
    AnyError::msg(format!(
      "import map specifier '{specifier}' is not a valid url: {err}"
    ))
  })?;
  let bytes = module
    .source()
    .await
    .ok_or_else(|| AnyError::msg("import map not found '{specifier}'"))?;
  let text = String::from_utf8_lossy(&bytes);
  let json_value =
    jsonc_parser::parse_to_serde_value(&text, &ParseOptions::default())
      .map_err(|err| {
        AnyError::msg(format!("import map failed to parse: {err}"))
      })?
      .ok_or_else(|| AnyError::msg("import map is not valid JSON"))?;
  let import_map = import_map::parse_from_value(base_url, json_value)?;

  Ok(SerializedWorkspaceResolverImportMap {
    specifier: specifier.to_string(),
    json: import_map.import_map.to_json(),
  })
}
