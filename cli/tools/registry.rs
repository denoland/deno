// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::AnyError;

use crate::args::Flags;

pub async fn info(_flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg info is not yet implemented");
  Ok(())
}

pub async fn login(_flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg login is not yet implemented");
  Ok(())
}

pub async fn publish(
  _flags: Flags,
  _directory: PathBuf,
) -> Result<(), AnyError> {
  eprintln!("deno reg publish is not yet implemented");
  Ok(())
}

pub async fn scope(_flags: Flags) -> Result<(), AnyError> {
  eprintln!("deno reg scope is not yet implemented");
  Ok(())
}
