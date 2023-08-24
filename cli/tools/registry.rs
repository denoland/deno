// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;

use crate::args::Flags;

pub async fn login(flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg login is not yet implemented");
  Ok(())
}

pub async fn publish(flags: Flags, directory: PathBuf) -> Result<(), AnyError> {
  eprintln!("deno reg publish is not yet implemented");
  Ok(())
}
