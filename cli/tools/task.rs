// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::fs_util;
use crate::proc_state::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

fn print_available_tasks(tasks_config: BTreeMap<String, String>) {
  eprintln!("{}", colors::green("Available tasks:"));

  for name in tasks_config.keys() {
    eprintln!("- {}", colors::cyan(name));
    eprintln!("    {}", tasks_config[name])
  }
}

pub async fn execute_script(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  log::warn!(
    "{} deno task is unstable and may drastically change in the future",
    crate::colors::yellow("Warning"),
  );
  let ps = ProcState::build(flags).await?;
  let tasks_config = ps.options.resolve_tasks_config()?;
  let config_file_url = ps.options.maybe_config_file_specifier().unwrap();
  let config_file_path = if config_file_url.scheme() == "file" {
    config_file_url.to_file_path().unwrap()
  } else {
    bail!("Only local configuration files are supported")
  };

  if task_flags.task.is_empty() {
    print_available_tasks(tasks_config);
    return Ok(1);
  }

  let cwd = match task_flags.cwd {
    Some(path) => fs_util::canonicalize_path(&PathBuf::from(path))?,
    None => config_file_path.parent().unwrap().to_owned(),
  };
  let task_name = task_flags.task;
  let maybe_script = tasks_config.get(&task_name);

  if let Some(script) = maybe_script {
    let additional_args = ps
      .options
      .argv()
      .iter()
      // surround all the additional arguments in double quotes
      // and santize any command substition
      .map(|a| format!("\"{}\"", a.replace('"', "\\\"").replace('$', "\\$")))
      .collect::<Vec<_>>()
      .join(" ");
    let script = format!("{} {}", script, additional_args);
    let script = script.trim();
    log::info!(
      "{} {} {}",
      colors::green("Task"),
      colors::cyan(&task_name),
      script,
    );
    let seq_list = deno_task_shell::parser::parse(script)
      .with_context(|| format!("Error parsing script '{}'.", task_name))?;

    // get the starting env vars (the PWD env var will be set by deno_task_shell)
    let mut env_vars = std::env::vars().collect::<HashMap<String, String>>();
    const INIT_CWD_NAME: &str = "INIT_CWD";
    if !env_vars.contains_key(INIT_CWD_NAME) {
      if let Ok(cwd) = std::env::current_dir() {
        // if not set, set an INIT_CWD env var that has the cwd
        env_vars
          .insert(INIT_CWD_NAME.to_string(), cwd.to_string_lossy().to_string());
      }
    }

    let exit_code = deno_task_shell::execute(seq_list, env_vars, &cwd).await;
    Ok(exit_code)
  } else {
    eprintln!("Task not found: {}", task_name);
    print_available_tasks(tasks_config);
    Ok(1)
  }
}
