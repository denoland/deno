// Copyright 2018-2026 the Deno authors. MIT license.

mod dev;
pub mod plugin_host;
mod production;

use std::sync::Arc;

use deno_core::error::AnyError;

use crate::args::BuildFlags;
use crate::args::DevFlags;
use crate::args::Flags;

pub async fn build(
  flags: Arc<Flags>,
  build_flags: BuildFlags,
) -> Result<(), AnyError> {
  production::build(flags, build_flags).await
}

pub async fn dev(
  flags: Arc<Flags>,
  dev_flags: DevFlags,
) -> Result<(), AnyError> {
  dev::dev(flags, dev_flags).await
}
