// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_config::workspace::FolderConfigs;
use deno_config::workspace::TaskDefinition;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceMemberTasksConfigFile;
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
use regex::Regex;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::util::fs::canonicalize_path;

#[derive(Debug, Clone)]
enum TaskKind {
  Deno,
  Npm,
}

#[derive(Debug, Clone)]
struct TaskInfo {
  kind: TaskKind,
  command: String,
  description: Option<String>,
  dependencies: Vec<String>,
}

#[derive(Debug)]
struct PackageTaskInfo<'a> {
  url: &'a Arc<Url>,
  matched_tasks: Vec<String>,
  tasks: HashMap<String, TaskInfo>,
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
    let mut regex_str = String::new();
    for ch in input.chars() {
      match ch {
        '*' => regex_str.push_str(".*"),
        '/' => regex_str.push_str("\\/"),
        _ => regex_str.push(ch),
      }
    }

    Regex::new(&regex_str)
  }

  let packages_task_configs: Vec<PackageTaskInfo> =
    if let Some(filter) = &task_flags.filter {
      let Some(task_name) = &task_flags.task else {
        writeln!(&mut std::io::stdout(), "Missing task argument")?;
        return Ok(0);
      };

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
              if regex.is_match(&name) {
                return true;
              }
            }
          }
        }

        if let Some(package_json) = &config.pkg_json {
          if let Some(name) = &package_json.name {
            if regex.is_match(&name) {
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

        let member_dir = workspace.resolve_member_dir(&folder.0);
        let mut tasks_config = member_dir.to_tasks_config()?;
        if force_use_pkg_json {
          tasks_config = tasks_config.with_only_pkg_json();
        }

        let mut config = PackageTaskInfo {
          tasks: HashMap::new(),
          matched_tasks: vec![],
          url: folder.0,
        };
        extract_tasks(&mut config, &tasks_config, force_use_pkg_json);

        // Any of the matched tasks could be a child task of another matched
        // one. Therefore we need to filter these out to ensure that every
        // task is only run once.
        let mut matched: HashSet<String> = HashSet::new();
        let mut visited: HashSet<String> = HashSet::new();
        fn visit_task(
          config: &PackageTaskInfo,
          visited: &mut HashSet<String>,
          name: &str,
        ) {
          if visited.contains(name) {
            return;
          }

          visited.insert(name.to_string());

          if let Some(task) = &config.tasks.get(name) {
            for dep in &task.dependencies {
              visit_task(config, visited, &dep);
            }
          }
        }

        // Match tasks in deno.json
        for name in config.tasks.keys() {
          if task_regex.is_match(name) {
            if !visited.contains(name) {
              matched.insert(name.to_string());
              visit_task(&config, &mut visited, name);
            }
          }
        }

        config.matched_tasks =
          matched.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        packages_task_info.push(config);
      }

      // Logging every task definition would be too spammy. Pnpm only
      // logs a simple message too.
      if packages_task_info
        .iter()
        .all(|config| config.matched_tasks.is_empty())
      {
        writeln!(
          &mut std::io::stdout(),
          "{}",
          colors::red(format!(
            "No matching task or script '{}' found in selected packages.",
            task_name
          ))
        )?;
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

      let mut config = PackageTaskInfo {
        url: start_dir.dir_url(),
        tasks: HashMap::new(),
        matched_tasks: vec![],
      };
      extract_tasks(&mut config, &tasks_config, force_use_pkg_json);

      let Some(task_name) = &task_flags.task else {
        print_available_tasks(
          &mut std::io::stdout(),
          &cli_options.start_dir,
          &config,
        )?;
        return Ok(0);
      };

      config.matched_tasks.push(task_name.to_string());
      vec![config]
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
        &"".to_string(),
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

fn extract_tasks(
  config: &mut PackageTaskInfo,
  tasks_config: &WorkspaceTasksConfig,
  force_use_pkg_json: bool,
) {
  fn extract_deno_json(
    config: &mut PackageTaskInfo,
    deno_json: &WorkspaceMemberTasksConfigFile<TaskDefinition>,
  ) {
    for (name, task) in &deno_json.tasks {
      if config.tasks.contains_key(name) {
        continue;
      }

      config.tasks.insert(
        name.to_string(),
        TaskInfo {
          kind: TaskKind::Deno,
          command: task.command.to_string(),
          dependencies: task.dependencies.clone(),
          description: task.description.clone(),
        },
      );
    }
  }

  fn extract_pkg_json(
    config: &mut PackageTaskInfo,
    pkg_json: &WorkspaceMemberTasksConfigFile<String>,
  ) {
    for (name, task) in &pkg_json.tasks {
      if config.tasks.contains_key(name) {
        continue;
      }

      config.tasks.insert(
        name.to_string(),
        TaskInfo {
          kind: TaskKind::Npm,
          command: task.to_string(),
          dependencies: vec![],
          description: None,
        },
      );
    }
  }

  if let Some(member) = &tasks_config.member {
    if !force_use_pkg_json {
      if let Some(deno_json) = &member.deno_json {
        extract_deno_json(config, deno_json);
      }
    }

    if let Some(package_json) = &member.package_json {
      extract_pkg_json(config, package_json)
    }
  }

  if let Some(root) = &tasks_config.root {
    if !force_use_pkg_json {
      if let Some(deno_json) = &root.deno_json {
        extract_deno_json(config, deno_json);
      }
    }

    if let Some(package_json) = &root.package_json {
      extract_pkg_json(config, package_json)
    }
  }
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
    tasks_config: &PackageTaskInfo<'a>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    match sort_tasks_topo(tasks_config) {
      Ok(sorted) => self.run_tasks_in_parallel(tasks_config, sorted).await,
      Err(err) => match err {
        TaskError::NotFound(name) => {
          if self.task_flags.is_run {
            return Err(anyhow!("Task not found: {}", name));
          }

          log::error!("Task not found: {}", name);
          if log::log_enabled!(log::Level::Error) {
            self.print_available_tasks(tasks_config)?;
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
    tasks_config: &PackageTaskInfo<'a>,
  ) -> Result<(), std::io::Error> {
    print_available_tasks(
      &mut std::io::stderr(),
      &self.cli_options.start_dir,
      tasks_config,
    )
  }

  async fn run_tasks_in_parallel(
    &self,
    tasks_config: &PackageTaskInfo<'a>,
    task_names: Vec<String>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    struct PendingTasksContext {
      completed: HashSet<String>,
      running: HashSet<String>,
      task_names: Vec<String>,
    }

    impl PendingTasksContext {
      fn has_remaining_tasks(&self) -> bool {
        self.completed.len() < self.task_names.len()
      }

      fn mark_complete(&mut self, task_name: String) {
        self.running.remove(&task_name);
        self.completed.insert(task_name);
      }

      fn get_next_task<'a>(
        &mut self,
        runner: &'a TaskRunner<'a>,
        tasks_config: &'a PackageTaskInfo<'a>,
      ) -> Option<LocalBoxFuture<'a, Result<(i32, String), AnyError>>> {
        for name in &self.task_names {
          if self.completed.contains(name) || self.running.contains(name) {
            continue;
          }

          let should_run = if let Some(task) = tasks_config.tasks.get(name) {
            task
              .dependencies
              .iter()
              .all(|dep| self.completed.contains(dep))
          } else {
            false
          };

          if !should_run {
            continue;
          }

          self.running.insert(name.clone());
          let name = name.clone();
          return Some(
            async move {
              runner
                .run_task_no_dependencies(tasks_config, &name)
                .await
                .map(|exit_code| (exit_code, name))
            }
            .boxed_local(),
          );
        }
        None
      }
    }

    let mut context = PendingTasksContext {
      completed: HashSet::with_capacity(task_names.len()),
      running: HashSet::with_capacity(self.concurrency),
      task_names,
    };

    let mut queue = futures_unordered::FuturesUnordered::new();

    while context.has_remaining_tasks() {
      while queue.len() < self.concurrency {
        if let Some(task) = context.get_next_task(self, tasks_config) {
          queue.push(task);
        } else {
          break;
        }
      }

      // If queue is empty at this point, then there are no more tasks in the queue.
      let Some(result) = queue.next().await else {
        debug_assert_eq!(context.task_names.len(), 0);
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

  async fn run_task_no_dependencies(
    &self,
    tasks_config: &'a PackageTaskInfo<'a>,
    task_name: &String,
  ) -> Result<i32, deno_core::anyhow::Error> {
    let dir_url = tasks_config.url;
    let task = tasks_config.tasks.get(task_name.as_str()).unwrap();

    match task.kind {
      TaskKind::Deno => self.run_deno_task(dir_url, task_name, task).await,
      TaskKind::Npm => {
        self.run_npm_script(dir_url, task_name, tasks_config).await
      }
    }
  }

  async fn run_deno_task(
    &self,
    dir_url: &Url,
    task_name: &String,
    definition: &TaskInfo,
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
    tasks_config: &PackageTaskInfo<'a>,
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
      if let Some(task) = tasks_config.tasks.get(task_name) {
        let exit_code = self
          .run_single(RunSingleOptions {
            task_name,
            script: &task.command,
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

fn sort_tasks_topo(
  task_config: &PackageTaskInfo,
) -> Result<Vec<String>, TaskError> {
  fn sort_visit<'a>(
    name: &'a str,
    sorted: &mut Vec<String>,
    mut path: Vec<&'a str>,
    tasks_config: &'a PackageTaskInfo,
  ) -> Result<(), TaskError> {
    // Already sorted
    if sorted.iter().any(|sorted_name| sorted_name == name) {
      return Ok(());
    }

    // Graph has a cycle
    if path.contains(&name) {
      path.push(name);
      return Err(TaskError::TaskDepCycle {
        path: path.iter().map(|s| s.to_string()).collect(),
      });
    }

    let Some(def) = tasks_config.tasks.get(name) else {
      return Err(TaskError::NotFound(name.to_string()));
    };

    for dep in &def.dependencies {
      let mut path = path.clone();
      path.push(name);
      sort_visit(dep, sorted, path, tasks_config)?
    }

    sorted.push(name.to_string());

    Ok(())
  }

  let mut sorted: Vec<String> = vec![];

  for name in &task_config.matched_tasks {
    sort_visit(name, &mut sorted, Vec::new(), task_config)?;
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
  tasks_config: &PackageTaskInfo,
) -> Result<(), std::io::Error> {
  writeln!(writer, "{}", colors::green("Available tasks:"))?;
  // let is_cwd_root_dir = tasks_config.root.is_none();
  let is_cwd_root_dir = false;

  if tasks_config.tasks.is_empty() {
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
    task: TaskInfo,
  }
  let mut task_descriptions = Vec::with_capacity(tasks_config.tasks.len());

  for (name, task) in &tasks_config.tasks {
    let is_root = *tasks_config.url.as_ref()
      == *workspace_dir.workspace.root_dir().as_ref();

    task_descriptions.push(AvailableTaskDescription {
      is_root,
      is_deno: if let TaskKind::Deno = task.kind {
        true
      } else {
        false
      },
      name: name.to_string(),
      task: task.clone(),
    });
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
