// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::DevFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;

pub async fn dev(
  flags: Arc<Flags>,
  dev_flags: DevFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  log::info!(
    "{} dev server on {}:{}",
    colors::green("Starting"),
    dev_flags.host,
    dev_flags.port,
  );

  // TODO: Read build configuration from deno.json
  // TODO: Build per-environment module graphs
  // TODO: Start HTTP server for serving bundled assets
  // TODO: Start WebSocket server for HMR
  // TODO: Start file watcher

  let _ = cli_options;

  log::error!("deno dev server is not yet fully implemented");
  Ok(())
}
