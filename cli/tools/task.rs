// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::proc_state::ProcState;
use crate::util::fs::canonicalize_path;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::LocalBoxFuture;
use deno_semver::npm::NpmPackageNv;
use deno_task_shell::ExecuteResult;
use deno_task_shell::ShellCommand;
use deno_task_shell::ShellCommandContext;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

pub async fn execute_script(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let ps = ProcState::from_flags(flags).await?;
  let tasks_config = ps.options.resolve_tasks_config()?;
  let maybe_package_json = ps.options.maybe_package_json();
  let package_json_scripts = maybe_package_json
    .as_ref()
    .and_then(|p| p.scripts.clone())
    .unwrap_or_default();

  let task_name = match &task_flags.task {
    Some(task) => task,
    None => {
      print_available_tasks(&tasks_config, &package_json_scripts);
      return Ok(1);
    }
  };

  if let Some(script) = tasks_config.get(task_name) {
    let config_file_url = ps.options.maybe_config_file_specifier().unwrap();
    let config_file_path = if config_file_url.scheme() == "file" {
      config_file_url.to_file_path().unwrap()
    } else {
      bail!("Only local configuration files are supported")
    };
    let cwd = match task_flags.cwd {
      Some(path) => canonicalize_path(&PathBuf::from(path))?,
      None => config_file_path.parent().unwrap().to_owned(),
    };
    let script = get_script_with_args(script, &ps);
    output_task(task_name, &script);
    let seq_list = deno_task_shell::parser::parse(&script)
      .with_context(|| format!("Error parsing script '{task_name}'."))?;
    let env_vars = collect_env_vars();
    let exit_code =
      deno_task_shell::execute(seq_list, env_vars, &cwd, Default::default())
        .await;
    Ok(exit_code)
  } else if let Some(script) = package_json_scripts.get(task_name) {
    if let Some(package_deps) = ps.package_json_deps_installer.package_deps() {
      for (key, value) in package_deps {
        if let Err(err) = value {
          log::info!(
            "{} Ignoring dependency '{}' in package.json because its version requirement failed to parse: {:#}",
            colors::yellow("Warning"),
            key,
            err,
          );
        }
      }
    }
    ps.package_json_deps_installer
      .ensure_top_level_install()
      .await?;
    ps.npm_resolver.resolve_pending().await?;

    log::info!(
      "{} Currently only basic package.json `scripts` are supported. Programs like `rimraf` or `cross-env` will not work correctly. This will be fixed in the upcoming release.",
      colors::yellow("Warning"),
    );

    let cwd = match task_flags.cwd {
      Some(path) => canonicalize_path(&PathBuf::from(path))?,
      None => maybe_package_json
        .as_ref()
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_owned(),
    };
    let script = get_script_with_args(script, &ps);
    output_task(task_name, &script);
    let seq_list = deno_task_shell::parser::parse(&script)
      .with_context(|| format!("Error parsing script '{task_name}'."))?;
    let npx_commands = resolve_npm_commands(&ps)?;
    let env_vars = collect_env_vars();
    let exit_code =
      deno_task_shell::execute(seq_list, env_vars, &cwd, npx_commands).await;
    Ok(exit_code)
  } else {
    eprintln!("Task not found: {task_name}");
    print_available_tasks(&tasks_config, &package_json_scripts);
    Ok(1)
  }
}

fn get_script_with_args(script: &str, ps: &ProcState) -> String {
  let additional_args = ps
    .options
    .argv()
    .iter()
    // surround all the additional arguments in double quotes
    // and santize any command substition
    .map(|a| format!("\"{}\"", a.replace('"', "\\\"").replace('$', "\\$")))
    .collect::<Vec<_>>()
    .join(" ");
  let script = format!("{script} {additional_args}");
  script.trim().to_owned()
}

fn output_task(task_name: &str, script: &str) {
  log::info!(
    "{} {} {}",
    colors::green("Task"),
    colors::cyan(&task_name),
    script,
  );
}

fn collect_env_vars() -> HashMap<String, String> {
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
  env_vars
}

fn print_available_tasks(
  // order can be important, so these use an index map
  tasks_config: &IndexMap<String, String>,
  package_json_scripts: &IndexMap<String, String>,
) {
  eprintln!("{}", colors::green("Available tasks:"));

  let mut had_task = false;
  for (is_deno, (key, value)) in tasks_config.iter().map(|e| (true, e)).chain(
    package_json_scripts
      .iter()
      .filter(|(key, _)| !tasks_config.contains_key(*key))
      .map(|e| (false, e)),
  ) {
    eprintln!(
      "- {}{}",
      colors::cyan(key),
      if is_deno {
        "".to_string()
      } else {
        format!(" {}", colors::italic_gray("(package.json)"))
      }
    );
    eprintln!("    {value}");
    had_task = true;
  }
  if !had_task {
    eprintln!("  {}", colors::red("No tasks found in configuration file"));
  }
}

struct NpxCommand;

impl ShellCommand for NpxCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if let Some(first_arg) = context.args.get(0).cloned() {
      if let Some(command) = context.state.resolve_command(&first_arg) {
        let context = ShellCommandContext {
          args: context.args.iter().skip(1).cloned().collect::<Vec<_>>(),
          ..context
        };
        command.execute(context)
      } else {
        let _ = context
          .stderr
          .write_line(&format!("npx: could not resolve command '{first_arg}'"));
        Box::pin(futures::future::ready(ExecuteResult::from_exit_code(1)))
      }
    } else {
      let _ = context.stderr.write_line("npx: missing command");
      Box::pin(futures::future::ready(ExecuteResult::from_exit_code(1)))
    }
  }
}

#[derive(Clone)]
struct NpmPackageBinCommand {
  name: String,
  npm_package: NpmPackageNv,
}

impl ShellCommand for NpmPackageBinCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    let mut args = vec![
      "run".to_string(),
      "-A".to_string(),
      if self.npm_package.name == self.name {
        format!("npm:{}", self.npm_package)
      } else {
        format!("npm:{}/{}", self.npm_package, self.name)
      },
    ];
    args.extend(context.args);
    let executable_command =
      deno_task_shell::ExecutableCommand::new("deno".to_string());
    executable_command.execute(ShellCommandContext { args, ..context })
  }
}

fn resolve_npm_commands(
  ps: &ProcState,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut result = HashMap::new();
  let snapshot = ps.npm_resolver.snapshot();
  for id in snapshot.top_level_packages() {
    let bin_commands =
      crate::node::node_resolve_binary_commands(&id.nv, &ps.npm_resolver)?;
    for bin_command in bin_commands {
      result.insert(
        bin_command.to_string(),
        Rc::new(NpmPackageBinCommand {
          name: bin_command,
          npm_package: id.nv.clone(),
        }) as Rc<dyn ShellCommand>,
      );
    }
  }
  if !result.contains_key("npx") {
    result.insert("npx".to_string(), Rc::new(NpxCommand));
  }
  Ok(result)
}
