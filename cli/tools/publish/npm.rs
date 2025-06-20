// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::WorkerExecutionMode;

use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::PublishFlags;
use crate::args::RunFlags;
use crate::tools;

const NPM_PACKAGE_REQ: &str = "npm:npm@11.4.2";

pub async fn publish_to_npm(
  flags: Arc<Flags>,
  publish_flags: PublishFlags,
  roots: LibWorkerFactoryRoots,
) -> Result<(), AnyError> {
  let mut flags = Arc::unwrap_or_clone(flags);
  flags.subcommand = DenoSubcommand::Run(RunFlags {
    script: NPM_PACKAGE_REQ.to_string(),
    watch: None,
    bare: false,
  });
  if publish_flags.dry_run {
    flags.argv.insert(0, "--dry-run".to_string());
  }

  let exit_code = tools::run::run_script(
    WorkerExecutionMode::Run,
    Arc::new(flags),
    None,
    None,
    roots,
  )
  .await?;

  if exit_code != 0 {
    deno_runtime::exit(exit_code);
  }

  Ok(())
}
