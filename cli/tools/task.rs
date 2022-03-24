// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::config_file::ConfigFile;
use crate::flags::Flags;
use crate::flags::TaskFlags;
use crate::proc_state::ProcState;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

fn get_tasks_config(
  maybe_config_file: Option<&ConfigFile>,
) -> Result<BTreeMap<String, String>, AnyError> {
  if let Some(config_file) = maybe_config_file {
    let maybe_tasks_config = config_file.to_tasks_config()?;
    if let Some(tasks_config) = maybe_tasks_config {
      for key in tasks_config.keys() {
        if key.is_empty() {
          bail!("Configuration file task names cannot be empty");
        } else if !key
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'))
        {
          bail!("Configuration file task names must only contain alpha-numeric characters, colons (:), underscores (_), or dashes (-). Task: {}", key);
        } else if !key.chars().next().unwrap().is_ascii_alphabetic() {
          bail!("Configuration file task names must start with an alphabetic character. Task: {}", key);
        }
      }
      Ok(tasks_config)
    } else {
      bail!("No tasks found in configuration file")
    }
  } else {
    bail!("No config file found")
  }
}

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
    let additional_args = flags
      .argv
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
    let env_vars = std::env::vars().collect::<HashMap<String, String>>();
    let exit_code = deno_task_shell::execute(seq_list, env_vars, cwd).await;
    Ok(exit_code)
  } else {
    eprintln!("Task not found: {}", task_name);
    print_available_tasks(tasks_config);
    Ok(1)
  }
}

#[cfg(test)]
mod test {
  use deno_ast::ModuleSpecifier;
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn tasks_no_tasks() {
    run_task_error_test(r#"{}"#, "No tasks found in configuration file");
  }

  #[test]
  fn task_name_invalid_chars() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "some%test": "deno bundle mod.ts"
        }
      }"#,
      concat!(
        "Configuration file task names must only contain alpha-numeric ",
        "characters, colons (:), underscores (_), or dashes (-). Task: some%test",
      ),
    );
  }

  #[test]
  fn task_name_non_alpha_starting_char() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "1test": "deno bundle mod.ts"
        }
      }"#,
      concat!(
        "Configuration file task names must start with an ",
        "alphabetic character. Task: 1test",
      ),
    );
  }

  #[test]
  fn task_name_empty() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "": "deno bundle mod.ts"
        }
      }"#,
      "Configuration file task names cannot be empty",
    );
  }

  fn run_task_error_test(config_text: &str, expected_error: &str) {
    let config_dir = ModuleSpecifier::parse("file:///deno/").unwrap();
    let config_specifier = config_dir.join("tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    assert_eq!(
      get_tasks_config(Some(&config_file))
        .err()
        .unwrap()
        .to_string(),
      expected_error,
    );
  }
}
