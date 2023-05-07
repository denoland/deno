// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io::Read;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;

use crate::args::EvalFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::factory::CliFactoryBuilder;
use crate::file_fetcher::File;
use crate::util;

pub async fn run_script(flags: Flags) -> Result<i32, AnyError> {
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

  if flags.watch.is_some() {
    return run_with_watch(flags).await;
  }

  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line
  let factory = CliFactory::from_flags(flags).await?;
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client();
  let cli_options = factory.cli_options();

  // Run a background task that checks for available upgrades. If an earlier
  // run of this background task found a new version of Deno.
  super::upgrade::check_for_upgrades(
    http_client.clone(),
    deno_dir.upgrade_check_file_path(),
  );

  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  let permissions = PermissionsContainer::new(Permissions::from_options(
    &cli_options.permissions_options(),
  )?);
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(main_module, permissions)
    .await?;

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

pub async fn run_from_stdin(flags: Flags) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  let file_fetcher = factory.file_fetcher()?;
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let permissions = PermissionsContainer::new(Permissions::from_options(
    &cli_options.permissions_options(),
  )?);
  let mut source = Vec::new();
  std::io::stdin().read_to_end(&mut source)?;
  // Create a dummy source file.
  let source_file = File {
    local: main_module.clone().to_file_path().unwrap(),
    maybe_types: None,
    media_type: MediaType::TypeScript,
    source: String::from_utf8(source)?.into(),
    specifier: main_module.clone(),
    maybe_headers: None,
  };
  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler
  file_fetcher.insert_cached(source_file);

  let mut worker = worker_factory
    .create_main_worker(main_module, permissions)
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

// TODO(bartlomieju): this function is not handling `exit_code` set by the runtime
// code properly.
async fn run_with_watch(flags: Flags) -> Result<i32, AnyError> {
  let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
  let factory = CliFactoryBuilder::new()
    .with_watcher(sender.clone())
    .build_from_flags(flags)
    .await?;
  let file_watcher = factory.file_watcher()?;
  let cli_options = factory.cli_options();
  let clear_screen = !cli_options.no_clear_screen();
  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

  let create_cli_main_worker_factory =
    factory.create_cli_main_worker_factory_func().await?;
  let operation = |main_module: ModuleSpecifier| {
    file_watcher.reset();
    let permissions = PermissionsContainer::new(Permissions::from_options(
      &cli_options.permissions_options(),
    )?);
    let create_cli_main_worker_factory = create_cli_main_worker_factory.clone();

    Ok(async move {
      let worker = create_cli_main_worker_factory()
        .create_main_worker(main_module, permissions)
        .await?;
      let reset_env = util::env::reset_env_func();
      worker.run_for_watcher().await?;
      reset_env();

      Ok(())
    })
  };

  util::file_watcher::watch_func2(
    receiver,
    operation,
    main_module,
    util::file_watcher::PrintConfig {
      job_name: "Process".to_string(),
      clear_screen,
    },
  )
  .await?;

  Ok(0)
}

pub async fn eval_command(
  flags: Flags,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let file_fetcher = factory.file_fetcher()?;
  let main_module = cli_options.resolve_main_module()?;

  maybe_npm_install(&factory).await?;

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
    source: String::from_utf8(source_code)?.into(),
    specifier: main_module.clone(),
    maybe_headers: None,
  };

  // Save our fake file into file fetcher cache
  // to allow module access by TS compiler.
  file_fetcher.insert_cached(file);

  let permissions = PermissionsContainer::new(Permissions::from_options(
    &cli_options.permissions_options(),
  )?);
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let mut worker = worker_factory
    .create_main_worker(main_module, permissions)
    .await?;
  let exit_code = worker.run().await?;
  Ok(exit_code)
}

async fn maybe_npm_install(factory: &CliFactory) -> Result<(), AnyError> {
  // ensure an "npm install" is done if the user has explicitly
  // opted into using a node_modules directory
  if factory.cli_options().node_modules_dir_enablement() == Some(true) {
    factory
      .package_json_deps_installer()
      .await?
      .ensure_top_level_install()
      .await?;
  }
  Ok(())
}
