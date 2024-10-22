// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_config::deno_json::Task;
use deno_config::workspace::TaskOrScript;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceTasksConfig;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::stream::futures_unordered;
use deno_core::futures::StreamExt;
use deno_path_util::normalize_path;
use deno_runtime::deno_node::NodeResolver;
use deno_task_shell::ShellCommand;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::util::fs::canonicalize_path;

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
      return Ok(0);
    }
  };

  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let env_vars = task_runner::real_env_vars();

  let task_runner = TaskRunner {
    tasks_config,
    task_flags: &task_flags,
    npm_resolver: npm_resolver.as_ref(),
    node_resolver: node_resolver.as_ref(),
    env_vars,
    cli_options,
  };
  task_runner.run_task(task_name).await
}

struct RunSingleOptions<'a> {
  task_name: &'a str,
  script: &'a str,
  cwd: &'a Path,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
}

struct TaskRunner<'a> {
  tasks_config: WorkspaceTasksConfig,
  task_flags: &'a TaskFlags,
  npm_resolver: &'a dyn CliNpmResolver,
  node_resolver: &'a NodeResolver,
  env_vars: HashMap<String, String>,
  cli_options: &'a CliOptions,
}

impl<'a> TaskRunner<'a> {
  async fn run_task(
    &self,
    task_name: &String,
  ) -> Result<i32, deno_core::anyhow::Error> {
    match self.tasks_config.task(task_name) {
      Some((dir_url, task_or_script)) => match task_or_script {
        TaskOrScript::Task(_tasks, definition) => {
          let mut futures_unordered =
            futures_unordered::FuturesUnordered::new();
          for dep in &definition.dependencies {
            let dep = dep.clone();
            futures_unordered.push(async move { self.run_task(&dep).await })
          }
          while let Some(result) = futures_unordered.next().await {
            let exit_code = result?;
            if exit_code > 0 {
              return Ok(exit_code);
            }
          }
          let cwd = match &self.task_flags.cwd {
            Some(path) => canonicalize_path(&PathBuf::from(path))
              .context("failed canonicalizing --cwd")?,
            None => normalize_path(dir_url.to_file_path().unwrap()),
          };

          let custom_commands = task_runner::resolve_custom_commands(
            self.npm_resolver,
            self.node_resolver,
          )?;
          self
            .run_single(RunSingleOptions {
              task_name,
              script: &definition.command,
              cwd: &cwd,
              custom_commands,
            })
            .await
        }
        TaskOrScript::Script(scripts, _script) => {
          // ensure the npm packages are installed if using a managed resolver
          if let Some(npm_resolver) = self.npm_resolver.as_managed() {
            npm_resolver.ensure_top_level_package_json_install().await?;
          }

          let cwd = match &self.task_flags.cwd {
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
            self.npm_resolver,
            self.node_resolver,
          )?;
          for task_name in &task_names {
            if let Some(script) = scripts.get(task_name) {
              let exit_code = self
                .run_single(RunSingleOptions {
                  task_name,
                  script,
                  cwd: &cwd,
                  custom_commands: custom_commands.clone(),
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
        if self.task_flags.is_run {
          return Err(anyhow!("Task not found: {}", task_name));
        }
        log::error!("Task not found: {}", task_name);
        if log::log_enabled!(log::Level::Error) {
          print_available_tasks(
            &mut std::io::stderr(),
            &self.cli_options.start_dir,
            &self.tasks_config,
          )?;
        }
        Ok(1)
      }
    }
  }

  async fn run_single(
    &self,
    opts: RunSingleOptions<'_>,
  ) -> Result<i32, AnyError> {
    let RunSingleOptions {
      task_name,
      script,
      cwd,
      custom_commands,
    } = opts;

    output_task(
      opts.task_name,
      &task_runner::get_script_with_args(script, self.cli_options.argv()),
    );

    Ok(
      task_runner::run_task(task_runner::RunTaskOptions {
        task_name,
        script,
        cwd,
        env_vars: self.env_vars.clone(),
        custom_commands,
        init_cwd: self.cli_options.initial_cwd(),
        argv: self.cli_options.argv(),
        root_node_modules_dir: self.npm_resolver.root_node_modules_path(),
        stdio: None,
      })
      .await?
      .exit_code,
    )
  }
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
                (
                  is_root,
                  false,
                  (
                    k,
                    Cow::Owned(Task::Definition(
                      deno_config::deno_json::TaskDefinition {
                        command: v.clone(),
                        dependencies: vec![],
                      },
                    )),
                  ),
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
        writeln!(writer, "    {}", definition.command)?;
      }
    }
  }

  Ok(())
}
