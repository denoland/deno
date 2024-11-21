// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_config::workspace::FolderConfigs;
use deno_config::workspace::TaskDefinition;
use deno_config::workspace::TaskOrScript;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceMemberTasksConfig;
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
use regex::Regex;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::util::fs::canonicalize_path;

#[derive(Debug)]
struct PackageTaskInfo {
  matched_tasks: Vec<String>,
  tasks_config: WorkspaceTasksConfig,
}

pub async fn execute_script(
  flags: Arc<Flags>,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let start_dir = &cli_options.start_dir;
  if !start_dir.has_deno_or_pkg_json() && !task_flags.eval {
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

  fn arg_to_regex(input: &str) -> Result<regex::Regex, regex::Error> {
    let mut regex_str = regex::escape(input);
    regex_str = regex_str.replace("\\*", ".*");

    Regex::new(&regex_str)
  }

  let packages_task_configs: Vec<PackageTaskInfo> = if let Some(filter) =
    &task_flags.filter
  {
    let task_name = task_flags.task.as_ref().unwrap();

    // Filter based on package name
    let package_regex = arg_to_regex(filter)?;
    let task_regex = arg_to_regex(task_name)?;

    let mut packages_task_info: Vec<PackageTaskInfo> = vec![];

    fn matches_package(
      config: &FolderConfigs,
      force_use_pkg_json: bool,
      regex: &Regex,
    ) -> bool {
      if !force_use_pkg_json {
        if let Some(deno_json) = &config.deno_json {
          if let Some(name) = &deno_json.json.name {
            if regex.is_match(name) {
              return true;
            }
          }
        }
      }

      if let Some(package_json) = &config.pkg_json {
        if let Some(name) = &package_json.name {
          if regex.is_match(name) {
            return true;
          }
        }
      }

      false
    }

    let workspace = cli_options.workspace();
    for folder in workspace.config_folders() {
      if !matches_package(folder.1, force_use_pkg_json, &package_regex) {
        continue;
      }

      let member_dir = workspace.resolve_member_dir(folder.0);
      let mut tasks_config = member_dir.to_tasks_config()?;
      if force_use_pkg_json {
        tasks_config = tasks_config.with_only_pkg_json();
      }

      // Any of the matched tasks could be a child task of another matched
      // one. Therefore we need to filter these out to ensure that every
      // task is only run once.
      let mut matched: HashSet<String> = HashSet::new();
      let mut visited: HashSet<String> = HashSet::new();

      fn visit_task(
        tasks_config: &WorkspaceTasksConfig,
        visited: &mut HashSet<String>,
        name: &str,
      ) {
        if visited.contains(name) {
          return;
        }

        visited.insert(name.to_string());

        if let Some((_, TaskOrScript::Task(_, task))) = &tasks_config.task(name)
        {
          for dep in &task.dependencies {
            visit_task(tasks_config, visited, dep);
          }
        }
      }

      // Match tasks in deno.json
      for name in tasks_config.task_names() {
        if task_regex.is_match(name) && !visited.contains(name) {
          matched.insert(name.to_string());
          visit_task(&tasks_config, &mut visited, name);
        }
      }

      packages_task_info.push(PackageTaskInfo {
        matched_tasks: matched
          .iter()
          .map(|s| s.to_string())
          .collect::<Vec<_>>(),
        tasks_config,
      });
    }

    // Logging every task definition would be too spammy. Pnpm only
    // logs a simple message too.
    if packages_task_info
      .iter()
      .all(|config| config.matched_tasks.is_empty())
    {
      log::warn!(
        "{}",
        colors::red(format!(
          "No matching task or script '{}' found in selected packages.",
          task_name
        ))
      );
      return Ok(0);
    }

    // FIXME: Sort packages topologically
    //

    packages_task_info
  } else {
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

    vec![PackageTaskInfo {
      tasks_config,
      matched_tasks: vec![task_name.to_string()],
    }]
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

  let task_runner = TaskRunner {
    task_flags: &task_flags,
    npm_resolver: npm_resolver.as_ref(),
    node_resolver: node_resolver.as_ref(),
    env_vars,
    cli_options,
    concurrency: no_of_concurrent_tasks.into(),
  };

  if task_flags.eval {
    return task_runner
      .run_deno_task(
        &Url::from_directory_path(cli_options.initial_cwd()).unwrap(),
        "",
        &TaskDefinition {
          command: task_flags.task.as_ref().unwrap().to_string(),
          dependencies: vec![],
          description: None,
        },
      )
      .await;
  }

  for task_config in &packages_task_configs {
    let exit_code = task_runner.run_tasks(task_config).await?;
    if exit_code > 0 {
      return Ok(exit_code);
    }
  }

  Ok(0)
}

struct RunSingleOptions<'a> {
  task_name: &'a str,
  script: &'a str,
  cwd: &'a Path,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
}

struct TaskRunner<'a> {
  task_flags: &'a TaskFlags,
  npm_resolver: &'a dyn CliNpmResolver,
  node_resolver: &'a NodeResolver,
  env_vars: HashMap<String, String>,
  cli_options: &'a CliOptions,
  concurrency: usize,
}

impl<'a> TaskRunner<'a> {
  pub async fn run_tasks(
    &self,
    pkg_tasks_config: &PackageTaskInfo,
  ) -> Result<i32, deno_core::anyhow::Error> {
    match sort_tasks_topo(pkg_tasks_config) {
      Ok(sorted) => self.run_tasks_in_parallel(sorted).await,
      Err(err) => match err {
        TaskError::NotFound(name) => {
          if self.task_flags.is_run {
            return Err(anyhow!("Task not found: {}", name));
          }

          log::error!("Task not found: {}", name);
          if log::log_enabled!(log::Level::Error) {
            self.print_available_tasks(&pkg_tasks_config.tasks_config)?;
          }
          Ok(1)
        }
        TaskError::TaskDepCycle { path } => {
          log::error!("Task cycle detected: {}", path.join(" -> "));
          Ok(1)
        }
      },
    }
  }

  pub fn print_available_tasks(
    &self,
    tasks_config: &WorkspaceTasksConfig,
  ) -> Result<(), std::io::Error> {
    print_available_tasks(
      &mut std::io::stderr(),
      &self.cli_options.start_dir,
      tasks_config,
    )
  }

  async fn run_tasks_in_parallel(
    &self,
    tasks: Vec<ResolvedTask<'a>>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    struct PendingTasksContext<'a> {
      completed: HashSet<usize>,
      running: HashSet<usize>,
      tasks: &'a [ResolvedTask<'a>],
    }

    impl<'a> PendingTasksContext<'a> {
      fn has_remaining_tasks(&self) -> bool {
        self.completed.len() < self.tasks.len()
      }

      fn mark_complete(&mut self, task: &ResolvedTask) {
        self.running.remove(&task.id);
        self.completed.insert(task.id);
      }

      fn get_next_task<'b>(
        &mut self,
        runner: &'b TaskRunner<'b>,
      ) -> Option<
        LocalBoxFuture<'b, Result<(i32, &'a ResolvedTask<'a>), AnyError>>,
      >
      where
        'a: 'b,
      {
        for task in self.tasks.iter() {
          if self.completed.contains(&task.id)
            || self.running.contains(&task.id)
          {
            continue;
          }

          let should_run = task
            .dependencies
            .iter()
            .all(|dep_id| self.completed.contains(dep_id));
          if !should_run {
            continue;
          }

          self.running.insert(task.id);
          return Some(
            async move {
              match task.task_or_script {
                TaskOrScript::Task(_, def) => {
                  runner.run_deno_task(task.folder_url, task.name, def).await
                }
                TaskOrScript::Script(scripts, _) => {
                  runner
                    .run_npm_script(task.folder_url, task.name, scripts)
                    .await
                }
              }
              .map(|exit_code| (exit_code, task))
            }
            .boxed_local(),
          );
        }
        None
      }
    }

    let mut context = PendingTasksContext {
      completed: HashSet::with_capacity(tasks.len()),
      running: HashSet::with_capacity(self.concurrency),
      tasks: &tasks,
    };

    let mut queue = futures_unordered::FuturesUnordered::new();

    while context.has_remaining_tasks() {
      while queue.len() < self.concurrency {
        if let Some(task) = context.get_next_task(self) {
          queue.push(task);
        } else {
          break;
        }
      }

      // If queue is empty at this point, then there are no more tasks in the queue.
      let Some(result) = queue.next().await else {
        debug_assert_eq!(context.tasks.len(), 0);
        break;
      };

      let (exit_code, name) = result?;
      if exit_code > 0 {
        return Ok(exit_code);
      }

      context.mark_complete(name);
    }

    Ok(0)
  }

  pub async fn run_deno_task(
    &self,
    dir_url: &Url,
    task_name: &str,
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

  pub async fn run_npm_script(
    &self,
    dir_url: &Url,
    task_name: &str,
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
      task_name.to_string(),
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
}

#[derive(Debug)]
enum TaskError {
  NotFound(String),
  TaskDepCycle { path: Vec<String> },
}

struct ResolvedTask<'a> {
  id: usize,
  name: &'a str,
  folder_url: &'a Url,
  task_or_script: TaskOrScript<'a>,
  dependencies: Vec<usize>,
}

fn sort_tasks_topo<'a>(
  pkg_task_config: &'a PackageTaskInfo,
) -> Result<Vec<ResolvedTask<'a>>, TaskError> {
  trait TasksConfig {
    fn task(
      &self,
      name: &str,
    ) -> Option<(&Url, TaskOrScript, &dyn TasksConfig)>;
  }

  impl TasksConfig for WorkspaceTasksConfig {
    fn task(
      &self,
      name: &str,
    ) -> Option<(&Url, TaskOrScript, &dyn TasksConfig)> {
      if let Some(member) = &self.member {
        if let Some((dir_url, task_or_script)) = member.task(name) {
          return Some((dir_url, task_or_script, self as &dyn TasksConfig));
        }
      }
      if let Some(root) = &self.root {
        if let Some((dir_url, task_or_script)) = root.task(name) {
          // switch to only using the root tasks for the dependencies
          return Some((dir_url, task_or_script, root as &dyn TasksConfig));
        }
      }
      None
    }
  }

  impl TasksConfig for WorkspaceMemberTasksConfig {
    fn task(
      &self,
      name: &str,
    ) -> Option<(&Url, TaskOrScript, &dyn TasksConfig)> {
      self.task(name).map(|(dir_url, task_or_script)| {
        (dir_url, task_or_script, self as &dyn TasksConfig)
      })
    }
  }

  fn sort_visit<'a>(
    name: &'a str,
    sorted: &mut Vec<ResolvedTask<'a>>,
    mut path: Vec<(&'a Url, &'a str)>,
    tasks_config: &'a dyn TasksConfig,
  ) -> Result<usize, TaskError> {
    let Some((folder_url, task_or_script, tasks_config)) =
      tasks_config.task(name)
    else {
      return Err(TaskError::NotFound(name.to_string()));
    };

    if let Some(existing_task) = sorted
      .iter()
      .find(|task| task.name == name && task.folder_url == folder_url)
    {
      // already exists
      return Ok(existing_task.id);
    }

    if path.contains(&(folder_url, name)) {
      path.push((folder_url, name));
      return Err(TaskError::TaskDepCycle {
        path: path.iter().map(|(_, s)| s.to_string()).collect(),
      });
    }

    let mut dependencies: Vec<usize> = Vec::new();
    if let TaskOrScript::Task(_, task) = task_or_script {
      dependencies.reserve(task.dependencies.len());
      for dep in &task.dependencies {
        let mut path = path.clone();
        path.push((folder_url, name));
        dependencies.push(sort_visit(dep, sorted, path, tasks_config)?);
      }
    }

    let id = sorted.len();
    sorted.push(ResolvedTask {
      id,
      name,
      folder_url,
      task_or_script,
      dependencies,
    });

    Ok(id)
  }

  let mut sorted: Vec<ResolvedTask<'a>> = vec![];

  for name in &pkg_task_config.matched_tasks {
    sort_visit(name, &mut sorted, Vec::new(), &pkg_task_config.tasks_config)?;
  }

  Ok(sorted)
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
