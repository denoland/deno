// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::io::Read;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;

use crate::args::EvalFlags;
use crate::args::Flags;
use crate::args::RunFlags;
use crate::file_fetcher::File;
use crate::npm::NpmPackageReference;
use crate::proc_state::ProcState;
use crate::util;
use crate::worker::create_main_worker;

pub async fn run_script(
  flags: Flags,
  run_flags: RunFlags,
) -> Result<i32, AnyError> {
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
    return run_with_watch(flags, run_flags.script).await;
  }

  // TODO(bartlomieju): actually I think it will also fail if there's an import
  // map specified and bare specifier is used on the command line - this should
  // probably call `ProcState::resolve` instead
  let ps = ProcState::build(flags).await?;

  // Run a background task that checks for available upgrades. If an earlier
  // run of this background task found a new version of Deno.
  super::upgrade::check_for_upgrades(
    ps.http_client.clone(),
    ps.dir.upgrade_check_file_path(),
  );

  let main_module = if NpmPackageReference::from_str(&run_flags.script).is_ok()
  {
    ModuleSpecifier::parse(&run_flags.script)?
  } else {
    resolve_url_or_path(&run_flags.script)?
  };
  let permissions = PermissionsContainer::new(Permissions::from_options(
    &ps.options.permissions_options(),
  )?);
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions).await?;

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

pub async fn run_from_stdin(flags: Flags) -> Result<i32, AnyError> {
  let ps = ProcState::build(flags).await?;
  let main_module = resolve_url_or_path("./$deno$stdin.ts").unwrap();
  let mut worker = create_main_worker(
    &ps.clone(),
    main_module.clone(),
    PermissionsContainer::new(Permissions::from_options(
      &ps.options.permissions_options(),
    )?),
  )
  .await?;

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
  ps.file_fetcher.insert_cached(source_file);

  let exit_code = worker.run().await?;
  Ok(exit_code)
}

// TODO(bartlomieju): this function is not handling `exit_code` set by the runtime
// code properly.
async fn run_with_watch(flags: Flags, script: String) -> Result<i32, AnyError> {
  let flags = Arc::new(flags);
  let main_module = resolve_url_or_path(&script)?;
  let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
  let mut ps =
    ProcState::build_for_file_watcher((*flags).clone(), sender.clone()).await?;

  let operation = |main_module: ModuleSpecifier| {
    ps.reset_for_file_watcher();
    let ps = ps.clone();
    Ok(async move {
      let permissions = PermissionsContainer::new(Permissions::from_options(
        &ps.options.permissions_options(),
      )?);
      let worker =
        create_main_worker(&ps, main_module.clone(), permissions).await?;
      worker.run_for_watcher().await?;

      Ok(())
    })
  };

  util::file_watcher::watch_func2(
    receiver,
    operation,
    main_module,
    util::file_watcher::PrintConfig {
      job_name: "Process".to_string(),
      clear_screen: !flags.no_clear_screen,
    },
  )
  .await?;

  Ok(0)
}

pub async fn eval_command(
  flags: Flags,
  eval_flags: EvalFlags,
) -> Result<i32, AnyError> {
  // deno_graph works off of extensions for local files to determine the media
  // type, and so our "fake" specifier needs to have the proper extension.
  let main_module =
    resolve_url_or_path(&format!("./$deno$eval.{}", eval_flags.ext))?;
  let ps = ProcState::build(flags).await?;
  let permissions = PermissionsContainer::new(Permissions::from_options(
    &ps.options.permissions_options(),
  )?);
  let mut worker =
    create_main_worker(&ps, main_module.clone(), permissions).await?;
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
  ps.file_fetcher.insert_cached(file);
  let exit_code = worker.run().await?;
  Ok(exit_code)
}
