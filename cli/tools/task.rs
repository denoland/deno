// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::config_file::ConfigFile;
use crate::flags::Flags;
use crate::flags::TaskFlags;
use crate::proc_state::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::collections::HashMap;
use std::sync::Arc;

fn get_tasks_config(
  maybe_config_file: Option<&ConfigFile>,
) -> Result<HashMap<String, String>, AnyError> {
  if let Some(config_file) = maybe_config_file {
    let maybe_tasks_config = config_file.to_tasks_config()?;
    if let Some(tasks_config) = maybe_tasks_config {
      Ok(tasks_config)
    } else {
      bail!("No configured tasks")
    }
  } else {
    bail!("No config file found")
  }
}

fn print_available_tasks(tasks_config: HashMap<String, String>) {
  eprintln!("{}", colors::green("Available tasks:"));

  let mut task_names: Vec<String> = tasks_config.clone().into_keys().collect();
  task_names.sort();

  for name in task_names {
    eprintln!("- {}", colors::cyan(&name));
    eprintln!("    {}", tasks_config[&name])
  }
}

pub async fn execute_script(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let flags = Arc::new(flags);
  let ps = ProcState::build(flags.clone()).await?;
  let tasks_config = get_tasks_config(ps.maybe_config_file.as_ref())?;
  let config_file_url = &ps.maybe_config_file.as_ref().unwrap().specifier;
  let config_file_path = if config_file_url.scheme() == "file" {
    config_file_url.to_file_path().unwrap()
  } else {
    bail!("Only local configuration files are supported")
  };

  if task_flags.task.is_empty() {
    print_available_tasks(tasks_config);
    return Ok(1);
  }

  let cwd = config_file_path.parent().unwrap();
  let task_name = task_flags.task;
  let maybe_script = tasks_config.get(&task_name);

  if let Some(script) = maybe_script {
    let seq_list = deno_task_shell::parser::parse(script)
      .with_context(|| format!("Error parsing script '{}'.", task_name))?;
    let env_vars = std::env::vars().collect::<HashMap<String, String>>();
    let additional_cli_args = Vec::new(); // todo
    let exit_code = deno_task_shell::execute(
      seq_list,
      env_vars,
      cwd.to_path_buf(),
      additional_cli_args,
    )
    .await?;
    Ok(exit_code)
  } else {
    eprintln!("Task not found: {}", task_name);
    print_available_tasks(tasks_config);
    Ok(1)
  }
}
