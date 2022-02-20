// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::config_file::ConfigFile;
use crate::flags::Flags;
use crate::proc_state::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;

fn get_scripts_config(
  maybe_config_file: Option<&ConfigFile>,
) -> Result<HashMap<String, String>, AnyError> {
  if let Some(config_file) = maybe_config_file {
    let maybe_scripts_config = config_file.to_scripts_config()?;
    if let Some(scripts_config) = maybe_scripts_config {
      Ok(scripts_config)
    } else {
      bail!("No configured scripts")
    }
  } else {
    bail!("No config file found")
  }
}

// TODO: make it return error instead
fn print_available_scripts(scripts_config: HashMap<String, String>) {
  eprintln!("{}", colors::green("Available scripts:"));

  let mut script_names: Vec<String> =
    scripts_config.clone().into_keys().collect();
  script_names.sort();

  for name in script_names {
    eprintln!("- {}", colors::cyan(&name));
    eprintln!("    {}", scripts_config[&name])
  }
}

pub async fn list_available_scripts(flags: Flags) -> Result<(), AnyError> {
  let flags = Arc::new(flags);
  let ps = ProcState::build(flags.clone()).await?;
  let scripts_config = get_scripts_config(ps.maybe_config_file.as_ref())?;
  print_available_scripts(scripts_config);
  Ok(())
}

pub async fn execute_script(
  flags: Flags,
  script_name: &str,
) -> Result<i32, AnyError> {
  let flags = Arc::new(flags);
  let ps = ProcState::build(flags.clone()).await?;
  let scripts_config = get_scripts_config(ps.maybe_config_file.as_ref())?;
  let maybe_script = scripts_config.get(script_name);

  if let Some(script) = maybe_script {
    let (shell_name, shell_flag) = if cfg!(windows) {
      // TODO: allow to use PowerShell?
      ("cmd", "/C")
    } else {
      // TODO: allow to use other shells?
      ("sh", "-c")
    };
    let shell_arg = format!("{} {}", script, flags.argv.join(" "));

    let status = Command::new(shell_name)
      .arg(shell_flag)
      .arg(shell_arg)
      .status()
      .await
      .with_context(|| format!("Failed to execute command: {}", script_name))?;
    // TODO: Is unwrapping to 1 ok here?
    Ok(status.code().unwrap_or(1))
  } else {
    eprintln!("Script not found: {}", script_name);
    print_available_scripts(scripts_config);
    Ok(1)
  }
}
