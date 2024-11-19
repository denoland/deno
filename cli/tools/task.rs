// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_config::workspace::TaskDefinition;
use deno_config::workspace::TaskOrScript;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceTasksConfig;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::stream::futures_unordered;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::url::Url;
use deno_path_util::normalize_path;
use deno_runtime::deno_node::NodeResolver;
use deno_task_shell::ShellCommand;
use indexmap::IndexMap;

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
  let mut tasks_config = start_dir.to_tasks_config()?;
  if force_use_pkg_json {
    tasks_config = tasks_config.with_only_pkg_json()
  }

  let Some(task_name) = &task_flags.task else {
    print_available_tasks(
      &mut std::io::stdout(),
      &cli_options.start_dir,
      &tasks_config,
    )?;
    return Ok(0);
  };

  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let env_vars = task_runner::real_env_vars();

  let no_of_concurrent_tasks = if let Ok(value) = std::env::var("DENO_JOBS") {
    value.parse::<NonZeroUsize>().ok()
  } else {
    std::thread::available_parallelism().ok()
  }
  .unwrap_or_else(|| NonZeroUsize::new(2).unwrap());
  let completed = RefCell::new(HashSet::with_capacity(8));
  let running =
    RefCell::new(HashSet::with_capacity(no_of_concurrent_tasks.into()));

  let mut task_runner = TaskRunner {
    tasks_config,
    task_flags: &task_flags,
    npm_resolver: npm_resolver.as_ref(),
    node_resolver: node_resolver.as_ref(),
    env_vars,
    cli_options,
    concurrency: no_of_concurrent_tasks.into(),
    completed,
    running,
    task_names: vec![],
  };

  match task_runner.sort_tasks_topo(task_name) {
    Ok(sorted) => task_runner.run_tasks_in_parallel(sorted).await,
    Err(err) => match err {
      TaskError::NotFound(name) => {
        if task_flags.is_run {
          return Err(anyhow!("Task not found: {}", name));
        }

        log::error!("Task not found: {}", name);
        if log::log_enabled!(log::Level::Error) {
          print_available_tasks(
            &mut std::io::stderr(),
            &task_runner.cli_options.start_dir,
            &task_runner.tasks_config,
          )?;
        }
        Ok(1)
      }
      TaskError::TaskDepCycle(name) => {
        // TODO(bartlomieju): this should be improved to show the cycle
        log::error!("Task cycle detected: {}", name);
        Ok(1)
      }
    },
  }
}

struct RunSingleOptions<'a> {
  task_name: &'a str,
  script: &'a str,
  cwd: &'a Path,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
}

#[derive(Debug)]
enum TaskError {
  NotFound(String),
  TaskDepCycle(String),
}

struct TaskRunner<'a> {
  tasks_config: WorkspaceTasksConfig,
  task_flags: &'a TaskFlags,
  npm_resolver: &'a dyn CliNpmResolver,
  node_resolver: &'a NodeResolver,
  env_vars: HashMap<String, String>,
  cli_options: &'a CliOptions,
  concurrency: usize,
  completed: RefCell<HashSet<String>>,
  running: RefCell<HashSet<String>>,
  task_names: Vec<String>,
}

impl<'a> TaskRunner<'a> {
  async fn run_tasks_in_parallel(
    &mut self,
    task_names: Vec<String>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    self.task_names = task_names;
    let mut queue = futures_unordered::FuturesUnordered::new();

    while self.has_remaining_tasks() {
      while queue.len() < self.concurrency {
        if let Some(task) = self.get_next_task() {
          queue.push(task);
        } else {
          break;
        }
      }

      // If queue is empty at this point, then there are no more tasks in the queue.
      let Some(result) = queue.next().await else {
        break;
      };

      let (exit_code, name) = result?;
      if exit_code > 0 {
        return Ok(exit_code);
      }

      self.running.borrow_mut().remove(&name);
      self.completed.borrow_mut().insert(name);
    }

    Ok(0)
  }

  fn get_task(
    &self,
    task_name: &str,
  ) -> Result<(&Url, TaskOrScript), TaskError> {
    let Some(result) = self.tasks_config.task(task_name) else {
      return Err(TaskError::NotFound(task_name.to_string()));
    };

    Ok(result)
  }

  fn has_remaining_tasks(&self) -> bool {
    self.completed.borrow().len() < self.task_names.len()
  }

  fn get_next_task(
    &'a self,
  ) -> Option<LocalBoxFuture<'a, Result<(i32, String), AnyError>>> {
    for name in &self.task_names {
      if self.completed.borrow().contains(name)
        || self.running.borrow().contains(name)
      {
        continue;
      }

      let should_run = if let Ok((_, def)) = self.get_task(name) {
        match def {
          TaskOrScript::Task(_, def) => def
            .dependencies
            .iter()
            .all(|dep| self.completed.borrow().contains(dep)),
          TaskOrScript::Script(_, _) => true,
        }
      } else {
        false
      };

      // TODO(bartlomieju): is this correct? Shouldn't it be `return None;`?
      if !should_run {
        continue;
      }

      self.running.borrow_mut().insert(name.clone());
      let name = name.clone();
      return Some(
        async move {
          self
            .run_task(&name)
            .await
            .map(|exit_code| (exit_code, name.clone()))
        }
        .boxed_local(),
      );
    }
    None
  }

  async fn run_task(
    &self,
    task_name: &String,
  ) -> Result<i32, deno_core::anyhow::Error> {
    let (dir_url, task_or_script) = self.get_task(task_name.as_str()).unwrap();

    match task_or_script {
      TaskOrScript::Task(_tasks, definition) => {
        self.run_deno_task(dir_url, task_name, definition).await
      }
      TaskOrScript::Script(scripts, _script) => {
        self.run_npm_script(dir_url, task_name, scripts).await
      }
    }
  }

  async fn run_deno_task(
    &self,
    dir_url: &Url,
    task_name: &String,
    definition: &TaskDefinition,
  ) -> Result<i32, deno_core::anyhow::Error> {
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

  async fn run_npm_script(
    &self,
    dir_url: &Url,
    task_name: &String,
    scripts: &IndexMap<String, String>,
  ) -> Result<i32, deno_core::anyhow::Error> {
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

  fn sort_tasks_topo(&self, name: &str) -> Result<Vec<String>, TaskError> {
    let mut marked: Vec<String> = vec![];
    let mut sorted: Vec<String> = vec![];

    self.sort_visit(name, &mut marked, &mut sorted)?;

    Ok(sorted)
  }

  fn sort_visit(
    &self,
    name: &str,
    marked: &mut Vec<String>,
    sorted: &mut Vec<String>,
  ) -> Result<(), TaskError> {
    // Already sorted
    if sorted.contains(&name.to_string()) {
      return Ok(());
    }

    // Graph has a cycle
    if marked.contains(&name.to_string()) {
      return Err(TaskError::TaskDepCycle(name.to_string()));
    }

    marked.push(name.to_string());

    let Some(def) = self.tasks_config.task(name) else {
      return Err(TaskError::NotFound(name.to_string()));
    };

    if let TaskOrScript::Task(_, actual_def) = def.1 {
      for dep in &actual_def.dependencies {
        self.sort_visit(dep, marked, sorted)?
      }
    }

    sorted.push(name.to_string());

    Ok(())
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
    return Ok(());
  }

  struct AvailableTaskDescription {
    is_root: bool,
    is_deno: bool,
    name: String,
    task: TaskDefinition,
  }
  let mut seen_task_names = HashSet::with_capacity(tasks_config.tasks_count());
  let mut task_descriptions = Vec::with_capacity(tasks_config.tasks_count());

  for maybe_config in [&tasks_config.member, &tasks_config.root] {
    let Some(config) = maybe_config else {
      continue;
    };

    if let Some(config) = config.deno_json.as_ref() {
      let is_root = !is_cwd_root_dir
        && config.folder_url == *workspace_dir.workspace.root_dir().as_ref();

      for (name, definition) in &config.tasks {
        if !seen_task_names.insert(name) {
          continue; // already seen
        }
        task_descriptions.push(AvailableTaskDescription {
          is_root,
          is_deno: true,
          name: name.to_string(),
          task: definition.clone(),
        });
      }
    }

    if let Some(config) = config.package_json.as_ref() {
      let is_root = !is_cwd_root_dir
        && config.folder_url == *workspace_dir.workspace.root_dir().as_ref();
      for (name, script) in &config.tasks {
        if !seen_task_names.insert(name) {
          continue; // already seen
        }

        task_descriptions.push(AvailableTaskDescription {
          is_root,
          is_deno: false,
          name: name.to_string(),
          task: deno_config::deno_json::TaskDefinition {
            command: script.to_string(),
            dependencies: vec![],
            description: None,
          },
        });
      }
    }
  }

  for desc in task_descriptions {
    writeln!(
      writer,
      "- {}{}",
      colors::cyan(desc.name),
      if desc.is_root {
        if desc.is_deno {
          format!(" {}", colors::italic_gray("(workspace)"))
        } else {
          format!(" {}", colors::italic_gray("(workspace package.json)"))
        }
      } else if desc.is_deno {
        "".to_string()
      } else {
        format!(" {}", colors::italic_gray("(package.json)"))
      }
    )?;
    if let Some(description) = &desc.task.description {
      let slash_slash = colors::italic_gray("//");
      writeln!(
        writer,
        "    {slash_slash} {}",
        colors::italic_gray(description)
      )?;
    }
    writeln!(writer, "    {}", desc.task.command)?;
    if !desc.task.dependencies.is_empty() {
      writeln!(
        writer,
        "    {} {}",
        colors::gray("depends on:"),
        colors::cyan(desc.task.dependencies.join(", "))
      )?;
    }
  }

  Ok(())
}
