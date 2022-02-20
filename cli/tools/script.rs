// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::flags::Flags;
use deno_core::error::AnyError;

pub async fn list_available_scripts(flags: Flags) -> Result<(), AnyError> {
  //   let ps = ProcState::build(flags.clone()).await?;
  //   let maybe_lint_config = if let Some(config_file) = &ps.maybe_config_file {
  //     config_file.to_lint_config()?
  //   } else {
  //     None
  //   };
  eprintln!("listing all available scripts");
  Ok(())
}

pub async fn execute_script(
  flags: Flags,
  script_name: &str,
) -> Result<(), AnyError> {
  eprintln!("trying to execute {}", script_name);
  Ok(())
}
