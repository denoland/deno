// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::util::fs::canonicalize_path;
use deno_config::deno_json::Task;
use deno_config::workspace::TaskOrScript;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceTasksConfig;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_task_shell::ShellCommand;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub async fn execute_script(
  flags: Arc<Flags>,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let start_dir = &cli_options.start_dir;
  if !start_dir.has_deno_or_pkg_json() {
    bail!("deno task couldn't find deno.json(c). See https://docs.deno.com/go/config")
  }
  let force_use_pkg_json =
    std::env::var_os(crate::task_runner::USE_PKG_JSON_HIDDEN_ENV_VAR_NAME)
      .map(|v| {
        // always remove so sub processes don't inherit this env var
        std::env::remove_var(
          crate::task_runner::USE_PKG_JSON_HIDDEN_ENV_VAR_NAME,
        );
        v == "1"
      })
      .unwrap_or(false);
  let tasks_config = start_dir.to_tasks_config()?;
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
        &cli_options.start_dir,
        &tasks_config,
      )?;
      return Ok(1);
    }
  };

  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let env_vars = task_runner::real_env_vars();

  match tasks_config.task(task_name) {
    Some((dir_url, task_or_script)) => match task_or_script {
      TaskOrScript::Task(_tasks, script) => {
        let cwd = match task_flags.cwd {
          Some(path) => canonicalize_path(&PathBuf::from(path))
            .context("failed canonicalizing --cwd")?,
          None => normalize_path(dir_url.to_file_path().unwrap()),
        };

        let custom_commands = task_runner::resolve_custom_commands(
          npm_resolver.as_ref(),
          node_resolver,
        )?;
        run_task(RunTaskOptions {
          task_name,
          script,
          cwd: &cwd,
          env_vars,
          custom_commands,
          npm_resolver: npm_resolver.as_ref(),
          cli_options,
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
        let custom_commands = task_runner::resolve_custom_commands(
          npm_resolver.as_ref(),
          node_resolver,
        )?;
        for task_name in &task_names {
          if let Some(script) = scripts.get(task_name) {
            let exit_code = run_task(RunTaskOptions {
              task_name,
              script,
              cwd: &cwd,
              env_vars: env_vars.clone(),
              custom_commands: custom_commands.clone(),
              npm_resolver: npm_resolver.as_ref(),
              cli_options,
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
          &cli_options.start_dir,
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
  env_vars: HashMap<String, String>,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  npm_resolver: &'a dyn CliNpmResolver,
  cli_options: &'a CliOptions,
}

async fn run_task(opts: RunTaskOptions<'_>) -> Result<i32, AnyError> {
  let RunTaskOptions {
    task_name,
    script,
    cwd,
    env_vars,
    custom_commands,
    npm_resolver,
    cli_options,
  } = opts;

  output_task(
    opts.task_name,
    &task_runner::get_script_with_args(script, cli_options.argv()),
  );

  task_runner::run_task(task_runner::RunTaskOptions {
    task_name,
    script,
    cwd,
    env_vars,
    custom_commands,
    init_cwd: opts.cli_options.initial_cwd(),
    argv: cli_options.argv(),
    root_node_modules_dir: npm_resolver
      .root_node_modules_path()
      .map(|p| p.as_path()),
  })
  .await
}

fn output_task(task_name: &str, script: &str) {
  log::info!(
    "{} {} {}",
    colors::green("Task"),
    colors::cyan(task_name),
    script,
  );
}

fn print_available_tasks(
  writer: &mut dyn std::io::Write,
  workspace_dir: &Arc<WorkspaceDirectory>,
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
            && config.folder_url
              == *workspace_dir.workspace.root_dir().as_ref();
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
                && config.folder_url
                  == *workspace_dir.workspace.root_dir().as_ref();
              config.tasks.iter().map(move |(k, v)| {
                (is_root, false, (k, Cow::Owned(Task::Definition(v.clone()))))
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
          Task::Definition(definition) => definition,
          Task::Commented { definition, .. } => definition,
        };
        if let Task::Commented { comments, .. } = task.as_ref() {
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
