// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use console_static_text::ansi::strip_ansi_codes;
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
use deno_task_shell::KillSignal;
use deno_task_shell::ShellCommand;
use indexmap::IndexMap;
use indexmap::IndexSet;
use regex::Regex;

use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::colors;
use crate::factory::CliFactory;
use crate::node::CliNodeResolver;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::PackageCaching;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::task_runner::run_future_forwarding_signals;
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

  // TODO(bartlomieju): this whole huge if statement should be a separate function, preferably with unit tests
  let (packages_task_configs, name) = if let Some(filter) = &task_flags.filter {
    // Filter based on package name
    let package_regex = package_filter_to_regex(filter)?;

    let Some(task_name) = &task_flags.task else {
      print_available_tasks_workspace(
        cli_options,
        &package_regex,
        filter,
        force_use_pkg_json,
        task_flags.recursive,
      )?;

      return Ok(0);
    };
    let task_regex = arg_to_task_name_filter(task_name)?;

    let mut packages_task_info: Vec<PackageTaskInfo> = vec![];

    let workspace = cli_options.workspace();
    for folder in workspace.config_folders() {
      if !task_flags.recursive
        && !matches_package(folder.1, force_use_pkg_json, &package_regex)
      {
        continue;
      }

      let member_dir = workspace.resolve_member_dir(folder.0);
      let mut tasks_config = member_dir.to_tasks_config()?;
      if force_use_pkg_json {
        tasks_config = tasks_config.with_only_pkg_json();
      }

      let matched_tasks = match_tasks(&tasks_config, &task_regex);

      if matched_tasks.is_empty() {
        continue;
      }

      packages_task_info.push(PackageTaskInfo {
        matched_tasks,
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

    (packages_task_info, task_name)
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
        None,
      )?;
      return Ok(0);
    };

    let task_regex = arg_to_task_name_filter(task_name)?;
    let matched_tasks = match_tasks(&tasks_config, &task_regex);

    (
      vec![PackageTaskInfo {
        tasks_config,
        matched_tasks,
      }],
      task_name,
    )
  };

  let maybe_lockfile = factory.maybe_lockfile().await?.cloned();
  let npm_installer = factory.npm_installer_if_managed().await?;
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
    npm_installer: npm_installer.map(|n| n.as_ref()),
    npm_resolver,
    node_resolver: node_resolver.as_ref(),
    env_vars,
    cli_options,
    maybe_lockfile,
    concurrency: no_of_concurrent_tasks.into(),
  };

  let kill_signal = KillSignal::default();
  run_future_forwarding_signals(kill_signal.clone(), async {
    if task_flags.eval {
      return task_runner
        .run_deno_task(
          &Url::from_directory_path(cli_options.initial_cwd()).unwrap(),
          "",
          &TaskDefinition {
            command: Some(task_flags.task.as_ref().unwrap().to_string()),
            dependencies: vec![],
            description: None,
          },
          kill_signal,
          cli_options.argv(),
        )
        .await;
    }

    for task_config in &packages_task_configs {
      let exit_code = task_runner
        .run_tasks(task_config, name, &kill_signal, cli_options.argv())
        .await?;
      if exit_code > 0 {
        return Ok(exit_code);
      }
    }

    Ok(0)
  })
  .await
}

struct RunSingleOptions<'a> {
  task_name: &'a str,
  script: &'a str,
  cwd: PathBuf,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  kill_signal: KillSignal,
  argv: &'a [String],
}

struct TaskRunner<'a> {
  task_flags: &'a TaskFlags,
  npm_installer: Option<&'a NpmInstaller>,
  npm_resolver: &'a CliNpmResolver,
  node_resolver: &'a CliNodeResolver,
  env_vars: HashMap<OsString, OsString>,
  cli_options: &'a CliOptions,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  concurrency: usize,
}

impl<'a> TaskRunner<'a> {
  pub async fn run_tasks(
    &self,
    pkg_tasks_config: &PackageTaskInfo,
    task_name: &str,
    kill_signal: &KillSignal,
    argv: &[String],
  ) -> Result<i32, deno_core::anyhow::Error> {
    match sort_tasks_topo(pkg_tasks_config, task_name) {
      Ok(sorted) => self.run_tasks_in_parallel(sorted, kill_signal, argv).await,
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
      None,
    )
  }

  async fn run_tasks_in_parallel(
    &self,
    tasks: Vec<ResolvedTask<'a>>,
    kill_signal: &KillSignal,
    args: &[String],
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
        kill_signal: &KillSignal,
        argv: &'a [String],
      ) -> Option<
        LocalBoxFuture<'b, Result<(i32, &'a ResolvedTask<'a>), AnyError>>,
      >
      where
        'a: 'b,
      {
        let mut tasks_iter = self.tasks.iter().peekable();
        while let Some(task) = tasks_iter.next() {
          let args = if tasks_iter.peek().is_none() {
            argv
          } else {
            &[]
          };

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
          let kill_signal = kill_signal.clone();
          return Some(
            async move {
              match task.task_or_script {
                TaskOrScript::Task(_, def) => {
                  runner
                    .run_deno_task(
                      task.folder_url,
                      task.name,
                      def,
                      kill_signal,
                      args,
                    )
                    .await
                }
                TaskOrScript::Script(scripts, _) => {
                  runner
                    .run_npm_script(
                      task.folder_url,
                      task.name,
                      scripts,
                      kill_signal,
                      args,
                    )
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
        if let Some(task) = context.get_next_task(self, kill_signal, args) {
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
    kill_signal: KillSignal,
    argv: &'a [String],
  ) -> Result<i32, deno_core::anyhow::Error> {
    let Some(command) = &definition.command else {
      log::info!(
        "{} {} {}",
        colors::green("Task"),
        colors::cyan(task_name),
        colors::gray("(no command)")
      );
      return Ok(0);
    };

    self.maybe_npm_install().await?;

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
        script: command,
        cwd,
        custom_commands,
        kill_signal,
        argv,
      })
      .await
  }

  pub async fn run_npm_script(
    &self,
    dir_url: &Url,
    task_name: &str,
    scripts: &IndexMap<String, String>,
    kill_signal: KillSignal,
    argv: &[String],
  ) -> Result<i32, deno_core::anyhow::Error> {
    // ensure the npm packages are installed if using a managed resolver
    self.maybe_npm_install().await?;

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
            cwd: cwd.clone(),
            custom_commands: custom_commands.clone(),
            kill_signal: kill_signal.clone(),
            argv,
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
      kill_signal,
      argv,
    } = opts;

    output_task(
      opts.task_name,
      &task_runner::get_script_with_args(script, argv),
    );

    Ok(
      task_runner::run_task(task_runner::RunTaskOptions {
        task_name,
        script,
        cwd,
        env_vars: self.env_vars.clone(),
        custom_commands,
        init_cwd: self.cli_options.initial_cwd(),
        argv,
        root_node_modules_dir: self.npm_resolver.root_node_modules_path(),
        stdio: None,
        kill_signal,
      })
      .await?
      .exit_code,
    )
  }

  async fn maybe_npm_install(&self) -> Result<(), AnyError> {
    if let Some(npm_installer) = self.npm_installer {
      npm_installer
        .ensure_top_level_package_json_install()
        .await?;
      npm_installer.cache_packages(PackageCaching::All).await?;
      if let Some(lockfile) = &self.maybe_lockfile {
        lockfile.write_if_changed()?;
      }
    }
    Ok(())
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
  task_name: &str,
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

  if sorted.is_empty() {
    return Err(TaskError::NotFound(task_name.to_string()));
  }

  Ok(sorted)
}

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

fn output_task(task_name: &str, script: &str) {
  log::info!(
    "{} {} {}",
    colors::green("Task"),
    colors::cyan(task_name),
    script,
  );
}

fn print_available_tasks_workspace(
  cli_options: &Arc<CliOptions>,
  package_regex: &Regex,
  filter: &str,
  force_use_pkg_json: bool,
  recursive: bool,
) -> Result<(), AnyError> {
  let workspace = cli_options.workspace();

  let mut matched = false;
  for folder in workspace.config_folders() {
    if !recursive
      && !matches_package(folder.1, force_use_pkg_json, package_regex)
    {
      continue;
    }
    matched = true;

    let member_dir = workspace.resolve_member_dir(folder.0);
    let mut tasks_config = member_dir.to_tasks_config()?;

    let mut pkg_name = folder
      .1
      .deno_json
      .as_ref()
      .and_then(|deno| deno.json.name.clone())
      .or(folder.1.pkg_json.as_ref().and_then(|pkg| pkg.name.clone()));

    if force_use_pkg_json {
      tasks_config = tasks_config.with_only_pkg_json();
      pkg_name = folder.1.pkg_json.as_ref().and_then(|pkg| pkg.name.clone());
    }

    print_available_tasks(
      &mut std::io::stdout(),
      &cli_options.start_dir,
      &tasks_config,
      pkg_name,
    )?;
  }

  if !matched {
    log::warn!(
      "{}",
      colors::red(format!("No package name matched the filter '{}' in available 'deno.json' or 'package.json' files.", filter))
    );
  }

  Ok(())
}

fn print_available_tasks(
  writer: &mut dyn std::io::Write,
  workspace_dir: &Arc<WorkspaceDirectory>,
  tasks_config: &WorkspaceTasksConfig,
  pkg_name: Option<String>,
) -> Result<(), std::io::Error> {
  let heading = if let Some(s) = pkg_name {
    format!("Available tasks ({}):", colors::cyan(s))
  } else {
    "Available tasks:".to_string()
  };

  writeln!(writer, "{}", colors::green(heading))?;
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
            command: Some(script.to_string()),
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
      for line in description.lines() {
        writeln!(
          writer,
          "    {slash_slash} {}",
          colors::italic_gray(strip_ansi_codes_and_escape_control_chars(line))
        )?;
      }
    }
    if let Some(command) = &desc.task.command {
      writeln!(
        writer,
        "    {}",
        strip_ansi_codes_and_escape_control_chars(command)
      )?;
    };
    if !desc.task.dependencies.is_empty() {
      let dependencies = desc
        .task
        .dependencies
        .into_iter()
        .map(|d| strip_ansi_codes_and_escape_control_chars(&d))
        .collect::<Vec<_>>()
        .join(", ");
      writeln!(
        writer,
        "    {} {}",
        colors::gray("depends on:"),
        colors::cyan(dependencies)
      )?;
    }
  }

  Ok(())
}

fn strip_ansi_codes_and_escape_control_chars(s: &str) -> String {
  strip_ansi_codes(s)
    .chars()
    .map(|c| match c {
      '\n' => "\\n".to_string(),
      '\r' => "\\r".to_string(),
      '\t' => "\\t".to_string(),
      c if c.is_control() => format!("\\x{:02x}", c as u8),
      c => c.to_string(),
    })
    .collect()
}

fn visit_task_and_dependencies(
  tasks_config: &WorkspaceTasksConfig,
  visited: &mut HashSet<String>,
  name: &str,
) {
  if visited.contains(name) {
    return;
  }

  visited.insert(name.to_string());

  if let Some((_, TaskOrScript::Task(_, task))) = &tasks_config.task(name) {
    for dep in &task.dependencies {
      visit_task_and_dependencies(tasks_config, visited, dep);
    }
  }
}

// Any of the matched tasks could be a child task of another matched
// one. Therefore we need to filter these out to ensure that every
// task is only run once.
fn match_tasks(
  tasks_config: &WorkspaceTasksConfig,
  task_name_filter: &TaskNameFilter,
) -> Vec<String> {
  let mut matched: IndexSet<String> = IndexSet::new();
  let mut visited: HashSet<String> = HashSet::new();

  // Match tasks in deno.json
  for name in tasks_config.task_names() {
    let matches_filter = match &task_name_filter {
      TaskNameFilter::Exact(n) => *n == name,
      TaskNameFilter::Regex(re) => re.is_match(name),
    };

    if matches_filter && !visited.contains(name) {
      matched.insert(name.to_string());
      visit_task_and_dependencies(tasks_config, &mut visited, name);
    }
  }

  matched.iter().map(|s| s.to_string()).collect::<Vec<_>>()
}

fn package_filter_to_regex(input: &str) -> Result<regex::Regex, regex::Error> {
  let mut regex_str = regex::escape(input);
  regex_str = regex_str.replace("\\*", ".*");

  Regex::new(&regex_str)
}

fn arg_to_task_name_filter(input: &str) -> Result<TaskNameFilter, AnyError> {
  if !input.contains("*") {
    return Ok(TaskNameFilter::Exact(input));
  }

  let mut regex_str = regex::escape(input);
  regex_str = regex_str.replace("\\*", ".*");
  let re = Regex::new(&regex_str)?;
  Ok(TaskNameFilter::Regex(re))
}

#[derive(Debug)]
enum TaskNameFilter<'s> {
  Exact(&'s str),
  Regex(regex::Regex),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_arg_to_task_name_filter() {
    assert!(matches!(
      arg_to_task_name_filter("test").unwrap(),
      TaskNameFilter::Exact("test")
    ));
    assert!(matches!(
      arg_to_task_name_filter("test-").unwrap(),
      TaskNameFilter::Exact("test-")
    ));
    assert!(matches!(
      arg_to_task_name_filter("test*").unwrap(),
      TaskNameFilter::Regex(_)
    ));
  }
}
