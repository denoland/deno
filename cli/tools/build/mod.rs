// Copyright 2018-2026 the Deno authors. MIT license.

mod dev;

use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::BuildFlags;
use crate::args::DevFlags;
use crate::args::Flags;

pub async fn build(
  _flags: Arc<Flags>,
  _build_flags: BuildFlags,
) -> Result<(), AnyError> {
  log::error!("deno build is not yet implemented");
  std::process::exit(1);
}

pub async fn dev(
  flags: Arc<Flags>,
  dev_flags: DevFlags,
) -> Result<(), AnyError> {
  dev::dev(flags, dev_flags).await
}
