// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::npm::InnerCliNpmResolverRef;
use crate::npm::ManagedCliNpmResolver;
use crate::util::fs::canonicalize_path;
use deno_config::workspace::TaskOrScript;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceTasksConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::normalize_path;
use deno_runtime::deno_node::NodeResolver;
use deno_semver::package::PackageNv;
use deno_task_shell::ExecutableCommand;
use deno_task_shell::ExecuteResult;
use deno_task_shell::ShellCommand;
use deno_task_shell::ShellCommandContext;
use lazy_regex::Lazy;
use regex::Regex;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::task::LocalSet;

// WARNING: Do not depend on this env var in user code. It's not stable API.
const USE_PKG_JSON_HIDDEN_ENV_VAR_NAME: &str =
  "DENO_INTERNAL_TASK_USE_PKG_JSON";

pub async fn execute_script(
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags)?;
  let cli_options = factory.cli_options();
  let start_ctx = cli_options.workspace.resolve_start_ctx();
  if !start_ctx.has_deno_or_pkg_json() {
    bail!("deno task couldn't find deno.json(c). See https://deno.land/manual@v{}/getting_started/configuration_file", env!("CARGO_PKG_VERSION"))
  }
  let force_use_pkg_json = std::env::var_os(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME)
    .map(|v| {
      // always remove so sub processes don't inherit this env var
      std::env::remove_var(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME);
      v == "1"
    })
    .unwrap_or(false);
  let tasks_config = start_ctx.to_tasks_config()?;
  let tasks_config = if force_use_pkg_json {
    tasks_config.with_only_pkg_json()
  } else {
    tasks_config
  };

  let task_name = match &task_flags.task {
    Some(task) => task,
    None => {
      print_available_tasks(
        &mut std::io::stdout(),
        &cli_options.workspace,
        &tasks_config,
      )?;
      return Ok(1);
    }
  };

  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let env_vars = real_env_vars();

  match tasks_config.task(task_name) {
    Some((dir_url, task_or_script)) => match task_or_script {
      TaskOrScript::Task(_tasks, script) => {
        let cwd = match task_flags.cwd {
          Some(path) => canonicalize_path(&PathBuf::from(path))
            .context("failed canonicalizing --cwd")?,
          None => normalize_path(dir_url.to_file_path().unwrap()),
        };

        let custom_commands =
          resolve_custom_commands(npm_resolver.as_ref(), node_resolver)?;
        run_task(RunTaskOptions {
          task_name,
          script,
          cwd: &cwd,
          init_cwd: cli_options.initial_cwd(),
          env_vars,
          argv: cli_options.argv(),
          custom_commands,
          root_node_modules_dir: npm_resolver
            .root_node_modules_path()
            .map(|p| p.as_path()),
        })
        .await
      }
      TaskOrScript::Script(scripts, _script) => {
        // ensure the npm packages are installed if using a node_modules
        // directory and managed resolver
        if cli_options.has_node_modules_dir() {
          if let Some(npm_resolver) = npm_resolver.as_managed() {
            npm_resolver.ensure_top_level_package_json_install().await?;
          }
        }

        let cwd = match task_flags.cwd {
          Some(path) => canonicalize_path(&PathBuf::from(path))?,
          None => normalize_path(dir_url.to_file_path().unwrap()),
        };

        // At this point we already checked if the task name exists in package.json.
        // We can therefore check for "pre" and "post" scripts too, since we're only
        // dealing with package.json here and not deno.json
        let task_names = vec![
          format!("pre{}", task_name),
          task_name.clone(),
          format!("post{}", task_name),
        ];
        let custom_commands =
          resolve_custom_commands(npm_resolver.as_ref(), node_resolver)?;
        for task_name in &task_names {
          if let Some(script) = scripts.get(task_name) {
            let exit_code = run_task(RunTaskOptions {
              task_name,
              script,
              cwd: &cwd,
              init_cwd: cli_options.initial_cwd(),
              env_vars: env_vars.clone(),
              argv: cli_options.argv(),
              custom_commands: custom_commands.clone(),
              root_node_modules_dir: npm_resolver
                .root_node_modules_path()
                .map(|p| p.as_path()),
            })
            .await?;
            if exit_code > 0 {
              return Ok(exit_code);
            }
          }
        }

        Ok(0)
      }
    },
    None => {
      log::error!("Task not found: {task_name}");
      if log::log_enabled!(log::Level::Error) {
        print_available_tasks(
          &mut std::io::stderr(),
          &cli_options.workspace,
          &tasks_config,
        )?;
      }
      Ok(1)
    }
  }
}

struct RunTaskOptions<'a> {
  task_name: &'a str,
  script: &'a str,
  cwd: &'a Path,
  init_cwd: &'a Path,
  env_vars: HashMap<String, String>,
  argv: &'a [String],
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  root_node_modules_dir: Option<&'a Path>,
}

async fn run_task(opts: RunTaskOptions<'_>) -> Result<i32, AnyError> {
  let script = get_script_with_args(opts.script, opts.argv);
  output_task(opts.task_name, &script);
  let seq_list = deno_task_shell::parser::parse(&script)
    .with_context(|| format!("Error parsing script '{}'.", opts.task_name))?;
  let env_vars =
    prepare_env_vars(opts.env_vars, opts.init_cwd, opts.root_node_modules_dir);
  let local = LocalSet::new();
  let future = deno_task_shell::execute(
    seq_list,
    env_vars,
    opts.cwd,
    opts.custom_commands,
  );
  Ok(local.run_until(future).await)
}

fn get_script_with_args(script: &str, argv: &[String]) -> String {
  let additional_args = argv
    .iter()
    // surround all the additional arguments in double quotes
    // and sanitize any command substitution
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
    colors::cyan(task_name),
    script,
  );
}

fn prepare_env_vars(
  mut env_vars: HashMap<String, String>,
  initial_cwd: &Path,
  node_modules_dir: Option<&Path>,
) -> HashMap<String, String> {
  const INIT_CWD_NAME: &str = "INIT_CWD";
  if !env_vars.contains_key(INIT_CWD_NAME) {
    // if not set, set an INIT_CWD env var that has the cwd
    env_vars.insert(
      INIT_CWD_NAME.to_string(),
      initial_cwd.to_string_lossy().to_string(),
    );
  }
  if let Some(node_modules_dir) = node_modules_dir {
    prepend_to_path(
      &mut env_vars,
      node_modules_dir.join(".bin").to_string_lossy().to_string(),
    );
  }
  env_vars
}

fn prepend_to_path(env_vars: &mut HashMap<String, String>, value: String) {
  match env_vars.get_mut("PATH") {
    Some(path) => {
      if path.is_empty() {
        *path = value;
      } else {
        *path =
          format!("{}{}{}", value, if cfg!(windows) { ";" } else { ":" }, path);
      }
    }
    None => {
      env_vars.insert("PATH".to_string(), value);
    }
  }
}

fn real_env_vars() -> HashMap<String, String> {
  std::env::vars()
    .map(|(k, v)| {
      if cfg!(windows) {
        (k.to_uppercase(), v)
      } else {
        (k, v)
      }
    })
    .collect::<HashMap<String, String>>()
}

fn print_available_tasks(
  writer: &mut dyn std::io::Write,
  workspace: &Arc<Workspace>,
  tasks_config: &WorkspaceTasksConfig,
) -> Result<(), std::io::Error> {
  writeln!(writer, "{}", colors::green("Available tasks:"))?;
  let is_cwd_root_dir = tasks_config.root.is_none();

  if tasks_config.is_empty() {
    writeln!(
      writer,
      "  {}",
      colors::red("No tasks found in configuration file")
    )?;
  } else {
    let mut seen_task_names =
      HashSet::with_capacity(tasks_config.tasks_count());
    for maybe_config in [&tasks_config.member, &tasks_config.root] {
      let Some(config) = maybe_config else {
        continue;
      };
      for (is_root, is_deno, (key, task)) in config
        .deno_json
        .as_ref()
        .map(|config| {
          let is_root = !is_cwd_root_dir
            && config.folder_url == *workspace.root_folder().0.as_ref();
          config
            .tasks
            .iter()
            .map(move |(k, t)| (is_root, true, (k, Cow::Borrowed(t))))
        })
        .into_iter()
        .flatten()
        .chain(
          config
            .package_json
            .as_ref()
            .map(|config| {
              let is_root = !is_cwd_root_dir
                && config.folder_url == *workspace.root_folder().0.as_ref();
              config.tasks.iter().map(move |(k, v)| {
                (
                  is_root,
                  false,
                  (k, Cow::Owned(deno_config::Task::Definition(v.clone()))),
                )
              })
            })
            .into_iter()
            .flatten(),
        )
      {
        if !seen_task_names.insert(key) {
          continue; // already seen
        }
        writeln!(
          writer,
          "- {}{}",
          colors::cyan(key),
          if is_root {
            if is_deno {
              format!(" {}", colors::italic_gray("(workspace)"))
            } else {
              format!(" {}", colors::italic_gray("(workspace package.json)"))
            }
          } else if is_deno {
            "".to_string()
          } else {
            format!(" {}", colors::italic_gray("(package.json)"))
          }
        )?;
        let definition = match task.as_ref() {
          deno_config::Task::Definition(definition) => definition,
          deno_config::Task::Commented { definition, .. } => definition,
        };
        if let deno_config::Task::Commented { comments, .. } = task.as_ref() {
          let slash_slash = colors::italic_gray("//");
          for comment in comments {
            writeln!(
              writer,
              "    {slash_slash} {}",
              colors::italic_gray(comment)
            )?;
          }
        }
        writeln!(writer, "    {definition}")?;
      }
    }
  }

  Ok(())
}

struct NpmCommand;

impl ShellCommand for NpmCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if context.args.first().map(|s| s.as_str()) == Some("run")
      && context.args.len() > 2
      // for now, don't run any npm scripts that have a flag because
      // we don't handle stuff like `--workspaces` properly
      && !context.args.iter().any(|s| s.starts_with('-'))
    {
      // run with deno task instead
      let mut args = Vec::with_capacity(context.args.len());
      args.push("task".to_string());
      args.extend(context.args.iter().skip(1).cloned());

      let mut state = context.state;
      state.apply_env_var(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME, "1");
      return ExecutableCommand::new(
        "deno".to_string(),
        std::env::current_exe().unwrap(),
      )
      .execute(ShellCommandContext {
        args,
        state,
        ..context
      });
    }

    // fallback to running the real npm command
    let npm_path = match context.state.resolve_command_path("npm") {
      Ok(path) => path,
      Err(err) => {
        let _ = context.stderr.write_line(&format!("{}", err));
        return Box::pin(futures::future::ready(
          ExecuteResult::from_exit_code(err.exit_code()),
        ));
      }
    };
    ExecutableCommand::new("npm".to_string(), npm_path).execute(context)
  }
}

struct NpxCommand;

impl ShellCommand for NpxCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if let Some(first_arg) = context.args.first().cloned() {
      if let Some(command) = context.state.resolve_custom_command(&first_arg) {
        let context = ShellCommandContext {
          args: context.args.iter().skip(1).cloned().collect::<Vec<_>>(),
          ..context
        };
        command.execute(context)
      } else {
        // can't find the command, so fallback to running the real npx command
        let npx_path = match context.state.resolve_command_path("npx") {
          Ok(npx) => npx,
          Err(err) => {
            let _ = context.stderr.write_line(&format!("{}", err));
            return Box::pin(futures::future::ready(
              ExecuteResult::from_exit_code(err.exit_code()),
            ));
          }
        };
        ExecutableCommand::new("npx".to_string(), npx_path).execute(context)
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
  npm_package: PackageNv,
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
    let executable_command = deno_task_shell::ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe().unwrap(),
    );
    executable_command.execute(ShellCommandContext { args, ..context })
  }
}

/// Runs a module in the node_modules folder.
#[derive(Clone)]
struct NodeModulesFileRunCommand {
  command_name: String,
  path: PathBuf,
}

impl ShellCommand for NodeModulesFileRunCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    let mut args = vec![
      "run".to_string(),
      "--ext=js".to_string(),
      "-A".to_string(),
      self.path.to_string_lossy().to_string(),
    ];
    args.extend(context.args);
    let executable_command = deno_task_shell::ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe().unwrap(),
    );
    // set this environment variable so that the launched process knows the npm command name
    context
      .state
      .apply_env_var("DENO_INTERNAL_NPM_CMD_NAME", &self.command_name);
    executable_command.execute(ShellCommandContext { args, ..context })
  }
}

fn resolve_custom_commands(
  npm_resolver: &dyn CliNpmResolver,
  node_resolver: &NodeResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut commands = match npm_resolver.as_inner() {
    InnerCliNpmResolverRef::Byonm(npm_resolver) => {
      let node_modules_dir = npm_resolver.root_node_modules_path().unwrap();
      resolve_npm_commands_from_bin_dir(node_modules_dir)
    }
    InnerCliNpmResolverRef::Managed(npm_resolver) => {
      resolve_managed_npm_commands(npm_resolver, node_resolver)?
    }
  };
  commands.insert("npm".to_string(), Rc::new(NpmCommand));
  Ok(commands)
}

fn resolve_npm_commands_from_bin_dir(
  node_modules_dir: &Path,
) -> HashMap<String, Rc<dyn ShellCommand>> {
  let mut result = HashMap::<String, Rc<dyn ShellCommand>>::new();
  let bin_dir = node_modules_dir.join(".bin");
  log::debug!("Resolving commands in '{}'.", bin_dir.display());
  match std::fs::read_dir(&bin_dir) {
    Ok(entries) => {
      for entry in entries {
        let Ok(entry) = entry else {
          continue;
        };
        if let Some(command) = resolve_bin_dir_entry_command(entry) {
          result.insert(command.command_name.clone(), Rc::new(command));
        }
      }
    }
    Err(err) => {
      log::debug!("Failed read_dir for '{}': {:#}", bin_dir.display(), err);
    }
  }
  result
}

fn resolve_bin_dir_entry_command(
  entry: std::fs::DirEntry,
) -> Option<NodeModulesFileRunCommand> {
  if entry.path().extension().is_some() {
    return None; // only look at files without extensions (even on Windows)
  }
  let file_type = entry.file_type().ok()?;
  let path = if file_type.is_file() {
    entry.path()
  } else if file_type.is_symlink() {
    entry.path().canonicalize().ok()?
  } else {
    return None;
  };
  let text = std::fs::read_to_string(&path).ok()?;
  let command_name = entry.file_name().to_string_lossy().to_string();
  if let Some(path) = resolve_execution_path_from_npx_shim(path, &text) {
    log::debug!(
      "Resolved npx command '{}' to '{}'.",
      command_name,
      path.display()
    );
    Some(NodeModulesFileRunCommand { command_name, path })
  } else {
    log::debug!("Failed resolving npx command '{}'.", command_name);
    None
  }
}

/// This is not ideal, but it works ok because it allows us to bypass
/// the shebang and execute the script directly with Deno.
fn resolve_execution_path_from_npx_shim(
  file_path: PathBuf,
  text: &str,
) -> Option<PathBuf> {
  static SCRIPT_PATH_RE: Lazy<Regex> =
    lazy_regex::lazy_regex!(r#""\$basedir\/([^"]+)" "\$@""#);

  if text.starts_with("#!/usr/bin/env node") {
    // launch this file itself because it's a JS file
    Some(file_path)
  } else {
    // Search for...
    // > "$basedir/../next/dist/bin/next" "$@"
    // ...which is what it will look like on Windows
    SCRIPT_PATH_RE
      .captures(text)
      .and_then(|c| c.get(1))
      .map(|relative_path| {
        file_path.parent().unwrap().join(relative_path.as_str())
      })
  }
}

fn resolve_managed_npm_commands(
  npm_resolver: &ManagedCliNpmResolver,
  node_resolver: &NodeResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut result = HashMap::new();
  let snapshot = npm_resolver.snapshot();
  for id in snapshot.top_level_packages() {
    let package_folder = npm_resolver.resolve_pkg_folder_from_pkg_id(id)?;
    let bin_commands =
      node_resolver.resolve_binary_commands(&package_folder)?;
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

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_prepend_to_path() {
    let mut env_vars = HashMap::new();

    prepend_to_path(&mut env_vars, "/example".to_string());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".to_string(), "/example".to_string())])
    );

    prepend_to_path(&mut env_vars, "/example2".to_string());
    let separator = if cfg!(windows) { ";" } else { ":" };
    assert_eq!(
      env_vars,
      HashMap::from([(
        "PATH".to_string(),
        format!("/example2{}/example", separator)
      )])
    );

    env_vars.get_mut("PATH").unwrap().clear();
    prepend_to_path(&mut env_vars, "/example".to_string());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".to_string(), "/example".to_string())])
    );
  }

  #[test]
  fn test_resolve_execution_path_from_npx_shim() {
    // example shim on unix
    let unix_shim = r#"#!/usr/bin/env node
"use strict";
console.log('Hi!');
"#;
    let path = PathBuf::from("/node_modules/.bin/example");
    assert_eq!(
      resolve_execution_path_from_npx_shim(path.clone(), unix_shim).unwrap(),
      path
    );
    // example shim on windows
    let windows_shim = r#"#!/bin/sh
basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

case `uname` in
    *CYGWIN*|*MINGW*|*MSYS*) basedir=`cygpath -w "$basedir"`;;
esac

if [ -x "$basedir/node" ]; then
  exec "$basedir/node"  "$basedir/../example/bin/example" "$@"
else
  exec node  "$basedir/../example/bin/example" "$@"
fi"#;
    assert_eq!(
      resolve_execution_path_from_npx_shim(path.clone(), windows_shim).unwrap(),
      path.parent().unwrap().join("../example/bin/example")
    );
  }
}
