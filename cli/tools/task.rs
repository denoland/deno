// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::path::Path;
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
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::stream::futures_unordered;
use deno_core::url::Url;
use deno_npm_installer::PackageCaching;
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
use crate::npm::CliNpmInstaller;
use crate::npm::CliNpmResolver;
use crate::task_runner;
use crate::task_runner::run_future_forwarding_signals;
use crate::util::fs::canonicalize_path;
use crate::util::progress_bar::ProgressBar;

#[derive(Debug)]
struct PackageTaskInfo {
  matched_tasks: Vec<String>,
  tasks_config: WorkspaceTasksConfig,
}

pub async fn execute_script(
  flags: Arc<Flags>,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let start_dir = &cli_options.start_dir;
  if !start_dir.has_deno_or_pkg_json() && !task_flags.eval {
    bail!(
      "deno task couldn't find deno.json(c) or package.json. See https://docs.deno.com/go/config"
    )
  }
  let force_use_pkg_json =
    std::env::var_os(crate::task_runner::USE_PKG_JSON_HIDDEN_ENV_VAR_NAME)
      .map(|v| {
        // always remove so sub processes don't inherit this env var

        // SAFETY: single-threaded at this point in startup
        unsafe {
          std::env::remove_var(
            crate::task_runner::USE_PKG_JSON_HIDDEN_ENV_VAR_NAME,
          )
        };
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
    let root_dir_url = workspace.root_dir_url();
    for (folder_url, folder) in
      workspace.config_folders_sorted_by_dependencies()
    {
      if !task_flags.recursive
        && !matches_package(
          folder,
          folder_url,
          force_use_pkg_json,
          &package_regex,
          folder_url == root_dir_url,
        )
      {
        continue;
      }

      let member_dir = workspace.resolve_member_dir(folder_url);
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
  let progress_bar = factory.text_only_progress_bar();
  let task_cache =
    crate::tools::task_cache::TaskCache::new(&factory.deno_dir()?.root);
  let mut env_vars = task_runner::real_env_vars();

  if flags.tunnel {
    env_vars.insert("DENO_CONNECTED".into(), "1".into());
  }

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
    progress_bar,
    env_vars,
    cli_options,
    maybe_lockfile,
    concurrency: no_of_concurrent_tasks.into(),
    task_cache: &task_cache,
  };

  let kill_signal = KillSignal::default();
  run_future_forwarding_signals(kill_signal.clone(), async {
    if task_flags.eval {
      return task_runner
        .run_deno_task(
          &Url::from_directory_path(cli_options.initial_cwd()).unwrap(),
          None,
          None,
          "",
          &TaskDefinition {
            command: Some(task_flags.task.as_ref().unwrap().to_string()),
            dependencies: vec![],
            description: None,
            files: Vec::new(),
            output: Vec::new(),
            env: Vec::new(),
          },
          kill_signal,
          cli_options.argv(),
          None,
        )
        .await;
    }

    task_runner
      .run_all_tasks(
        &packages_task_configs,
        name,
        &kill_signal,
        cli_options.argv(),
      )
      .await
  })
  .await
}

#[derive(Clone, Copy)]
struct ParallelInfo {
  color_index: usize,
  label_pad_width: usize,
  /// When true, the prefix label includes the package name (`[pkg#task]`) so
  /// identically-named tasks from sibling workspace members stay distinct.
  include_package: bool,
}

/// For each task that could run concurrently with at least one other task,
/// returns its sequential color index (0, 1, 2, …) so adjacent concurrent
/// tasks always get distinct colors regardless of how `task.id` is allocated.
///
/// A task is "potentially concurrent" when at least one other task is neither
/// a transitive dependency nor a transitive dependent — i.e. `run_tasks_in_parallel`
/// could have both in flight at the same time.
fn compute_concurrent_task_color_indices(
  tasks: &[ResolvedTask],
) -> HashMap<usize, usize> {
  if tasks.len() <= 1 {
    return HashMap::new();
  }

  // Build forward and reverse transitive dependency sets in a single pass.
  // Tasks arrive in topological order, so each task's direct dependencies
  // have already had their own trans_deps computed.
  let mut trans_deps: HashMap<usize, HashSet<usize>> = HashMap::new();
  let mut trans_dependents: HashMap<usize, HashSet<usize>> = HashMap::new();
  for task in tasks {
    let mut deps = HashSet::new();
    for &dep_id in &task.dependencies {
      deps.insert(dep_id);
      if let Some(d) = trans_deps.get(&dep_id) {
        deps.extend(d);
      }
    }
    for &dep_id in &deps {
      trans_dependents.entry(dep_id).or_default().insert(task.id);
    }
    trans_deps.insert(task.id, deps);
  }

  // Walk tasks in topological order (deterministic for a given graph) and
  // assign sequential color indices only to tasks that are concurrent with
  // at least one sibling. Stable across reruns for muscle memory.
  let mut color_indices = HashMap::new();
  let mut next_color = 0;
  for task in tasks {
    let dep_count = trans_deps[&task.id].len();
    let dependent_count = trans_dependents.get(&task.id).map_or(0, |s| s.len());
    if dep_count + dependent_count < tasks.len() - 1 {
      color_indices.insert(task.id, next_color);
      next_color += 1;
    }
  }
  color_indices
}

/// Returns the workspace member's display name for a task's prefix label —
/// its `name` from deno.json/package.json if set, otherwise the folder's
/// directory name as a fallback so unnamed sibling packages stay
/// distinguishable. Tasks from the workspace root get no fallback (their
/// root folder name is typically not a meaningful package name).
fn task_label_name<'a>(
  task: &ResolvedTask<'a>,
  workspace_root_url: &Url,
) -> Option<&'a str> {
  if let Some(name) = task.task_or_script.package_name() {
    return Some(name);
  }
  if task.task_or_script.folder_url() == workspace_root_url {
    return None;
  }
  workspace_folder_dir_name(task.task_or_script.folder_url())
}

/// Length of the inner part of the prefix label (without the surrounding
/// brackets) — used to right-align brackets across concurrent tasks.
fn label_inner_len(
  task: &ResolvedTask<'_>,
  include_package: bool,
  workspace_root_url: &Url,
) -> usize {
  if include_package
    && let Some(pkg) = task_label_name(task, workspace_root_url)
  {
    pkg.len() + 1 + task.name.len()
  } else {
    task.name.len()
  }
}

fn colorize_task_prefix(label: &str, color_index: usize) -> String {
  match color_index % 6 {
    0 => colors::cyan(label).to_string(),
    1 => colors::magenta(label).to_string(),
    2 => colors::yellow(label).to_string(),
    3 => colors::green(label).to_string(),
    4 => colors::intense_blue(label).to_string(),
    _ => colors::red(label).to_string(),
  }
}

struct RunSingleOptions<'a> {
  task_name: &'a str,
  package_name: Option<&'a str>,
  /// Display name used in the parallel-mode prefix label only. Falls back to
  /// the folder's directory name when the package itself is unnamed (and the
  /// folder isn't the workspace root). May be `None`.
  label_name: Option<&'a str>,
  script: &'a str,
  cwd: PathBuf,
  custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  node_modules_bin_dirs: Vec<PathBuf>,
  kill_signal: KillSignal,
  argv: &'a [String],
  parallel_info: Option<ParallelInfo>,
}

struct TaskRunner<'a> {
  task_flags: &'a TaskFlags,
  npm_installer: Option<&'a CliNpmInstaller>,
  npm_resolver: &'a CliNpmResolver,
  node_resolver: &'a CliNodeResolver,
  progress_bar: &'a ProgressBar,
  env_vars: HashMap<OsString, OsString>,
  cli_options: &'a CliOptions,
  maybe_lockfile: Option<Arc<CliLockfile>>,
  concurrency: usize,
  task_cache: &'a crate::tools::task_cache::TaskCache,
}

impl<'a> TaskRunner<'a> {
  /// Topologically sort all matched tasks across the given packages into a
  /// single flat list, then run them through `run_tasks_in_parallel` so tasks
  /// from sibling packages execute concurrently.
  pub async fn run_all_tasks(
    &self,
    packages: &'a [PackageTaskInfo],
    task_name: &str,
    kill_signal: &KillSignal,
    argv: &'a [String],
  ) -> Result<i32, deno_core::anyhow::Error> {
    let mut sorted: Vec<ResolvedTask<'a>> = Vec::new();
    for pkg in packages {
      if let Err(err) = sort_tasks_topo(pkg, &mut sorted) {
        return match err {
          TaskError::NotFound(name) => {
            if self.task_flags.is_run {
              return Err(anyhow!("Task not found: {}", name));
            }
            log::error!("Task not found: {}", name);
            if log::log_enabled!(log::Level::Error) {
              self.print_available_tasks(&pkg.tasks_config)?;
            }
            Ok(1)
          }
          TaskError::TaskDepCycle { path } => {
            log::error!("Task cycle detected: {}", path.join(" -> "));
            Ok(1)
          }
        };
      }
    }

    if sorted.is_empty() {
      if self.task_flags.is_run {
        return Err(anyhow!("Task not found: {}", task_name));
      }
      log::error!("Task not found: {}", task_name);
      if log::log_enabled!(log::Level::Error)
        && let Some(pkg) = packages.first()
      {
        self.print_available_tasks(&pkg.tasks_config)?;
      }
      return Ok(1);
    }

    self.run_tasks_in_parallel(sorted, kill_signal, argv).await
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
    let workspace_root_url: &Url =
      self.cli_options.workspace().root_dir_url().as_ref();
    let concurrent_task_color_indices = if self.task_flags.no_prefix {
      HashMap::new()
    } else {
      compute_concurrent_task_color_indices(&tasks)
    };
    // Include the package name in prefix labels when concurrent tasks span
    // more than one workspace member.
    let include_package = {
      let mut seen: HashSet<&Url> = HashSet::new();
      let mut multi = false;
      for t in &tasks {
        if !concurrent_task_color_indices.contains_key(&t.id) {
          continue;
        }
        if seen.insert(t.task_or_script.folder_url()) && seen.len() > 1 {
          multi = true;
          break;
        }
      }
      multi
    };
    let max_label_len = tasks
      .iter()
      .filter(|t| concurrent_task_color_indices.contains_key(&t.id))
      .map(|t| label_inner_len(t, include_package, workspace_root_url))
      .max()
      .unwrap_or(0);

    struct PendingTasksContext<'a> {
      completed: HashSet<usize>,
      running: HashSet<usize>,
      tasks: &'a [ResolvedTask<'a>],
      concurrent_task_color_indices: HashMap<usize, usize>,
      max_label_len: usize,
      include_package: bool,
      workspace_root_url: &'a Url,
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
          let parallel_info = self
            .concurrent_task_color_indices
            .get(&task.id)
            .map(|&color_index| ParallelInfo {
              color_index,
              label_pad_width: self.max_label_len,
              include_package: self.include_package,
            });
          let label_name = task_label_name(task, self.workspace_root_url);
          return Some(
            async move {
              match task.task_or_script {
                TaskOrScript::Task { task: def, .. } => {
                  runner
                    .run_deno_task(
                      task.task_or_script.folder_url(),
                      task.task_or_script.package_name(),
                      label_name,
                      task.name,
                      def,
                      kill_signal,
                      args,
                      parallel_info,
                    )
                    .await
                }
                TaskOrScript::Script { details, .. } => {
                  runner
                    .run_npm_script(
                      task.task_or_script.folder_url(),
                      task.task_or_script.package_name(),
                      label_name,
                      task.name,
                      &details.tasks,
                      kill_signal,
                      args,
                      parallel_info,
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
      concurrent_task_color_indices,
      max_label_len,
      include_package,
      workspace_root_url,
    };

    let mut queue = futures_unordered::FuturesUnordered::new();

    while context.has_remaining_tasks() {
      while queue.len() < self.concurrency {
        match context.get_next_task(self, kill_signal, args) {
          Some(task) => {
            queue.push(task);
          }
          _ => {
            break;
          }
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

  #[allow(
    clippy::too_many_arguments,
    reason = "parallel_info was added to an already-large signature; refactoring into a struct is deferred"
  )]
  pub async fn run_deno_task(
    &self,
    dir_url: &Url,
    package_name: Option<&str>,
    label_name: Option<&str>,
    task_name: &str,
    definition: &TaskDefinition,
    kill_signal: KillSignal,
    argv: &'a [String],
    parallel_info: Option<ParallelInfo>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    let Some(command) = &definition.command else {
      self.output_task(
        task_name,
        package_name,
        &colors::gray("(no command)").to_string(),
      );
      return Ok(0);
    };

    self.maybe_npm_install().await?;

    let cwd = match &self.task_flags.cwd {
      Some(path) => canonicalize_path(Path::new(path))
        .context("failed canonicalizing --cwd")?,
      None => {
        normalize_path(Cow::Owned(dir_url.to_file_path().unwrap())).into_owned()
      }
    };

    let node_modules_bin_dirs =
      task_runner::resolve_task_node_modules_bin_dirs(self.npm_resolver, &cwd);
    let custom_commands = task_runner::resolve_custom_commands(
      self.node_resolver,
      self.npm_resolver,
      &node_modules_bin_dirs,
    )?;

    // Input-based cache: if the task declares `files`, hash inputs +
    // command + listed env values and skip on match.
    let env_snapshot: std::collections::BTreeMap<String, String> = self
      .env_vars
      .iter()
      .filter_map(|(k, v)| {
        Some((k.to_str()?.to_string(), v.to_str()?.to_string()))
      })
      .collect();
    let cache_key = crate::tools::task_cache::TaskCacheKey {
      package_name,
      task_name,
      cwd: &cwd,
      command,
      argv,
      files: &definition.files,
      env_names: &definition.env,
      env: &env_snapshot,
    };
    let pending_fingerprint = match self.task_cache.lookup(&cache_key) {
      crate::tools::task_cache::CacheLookup::Hit => {
        self.output_task(
          task_name,
          package_name,
          &format!(
            "{} (cached, inputs unchanged)",
            colors::gray(task_runner::get_script_with_args(command, argv))
          ),
        );
        return Ok(0);
      }
      crate::tools::task_cache::CacheLookup::Miss(fp) => Some(fp),
      crate::tools::task_cache::CacheLookup::NotCacheable => None,
    };

    let exit_code = self
      .run_single(RunSingleOptions {
        task_name,
        package_name,
        label_name,
        script: command,
        cwd: cwd.clone(),
        custom_commands,
        node_modules_bin_dirs,
        kill_signal,
        argv,
        parallel_info,
      })
      .await?;

    if exit_code == 0
      && let Some(fp) = pending_fingerprint
    {
      self.task_cache.store(&cache_key, fp);
    }
    Ok(exit_code)
  }

  #[allow(
    clippy::too_many_arguments,
    reason = "parallel_info was added to an already-large signature; refactoring into a struct is deferred"
  )]
  pub async fn run_npm_script(
    &self,
    dir_url: &Url,
    package_name: Option<&str>,
    label_name: Option<&str>,
    task_name: &str,
    scripts: &IndexMap<String, String>,
    kill_signal: KillSignal,
    argv: &[String],
    parallel_info: Option<ParallelInfo>,
  ) -> Result<i32, deno_core::anyhow::Error> {
    // ensure the npm packages are installed if using a managed resolver
    self.maybe_npm_install().await?;

    let cwd = match &self.task_flags.cwd {
      Some(path) => Cow::Owned(canonicalize_path(Path::new(path))?),
      None => normalize_path(Cow::Owned(dir_url.to_file_path().unwrap())),
    };

    // At this point we already checked if the task name exists in package.json.
    // We can therefore check for "pre" and "post" scripts too, since we're only
    // dealing with package.json here and not deno.json
    let task_names = vec![
      format!("pre{}", task_name),
      task_name.to_string(),
      format!("post{}", task_name),
    ];
    let node_modules_bin_dirs =
      task_runner::resolve_task_node_modules_bin_dirs(self.npm_resolver, &cwd);
    let custom_commands = task_runner::resolve_custom_commands(
      self.node_resolver,
      self.npm_resolver,
      &node_modules_bin_dirs,
    )?;

    for task_name in &task_names {
      if let Some(script) = scripts.get(task_name) {
        let exit_code = self
          .run_single(RunSingleOptions {
            task_name,
            package_name,
            label_name,
            script,
            cwd: cwd.to_path_buf(),
            custom_commands: custom_commands.clone(),
            node_modules_bin_dirs: node_modules_bin_dirs.clone(),
            kill_signal: kill_signal.clone(),
            argv,
            parallel_info,
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
      package_name,
      label_name,
      script,
      cwd,
      custom_commands,
      node_modules_bin_dirs,
      kill_signal,
      argv,
      parallel_info,
    } = opts;

    self.output_task(
      task_name,
      package_name,
      &task_runner::get_script_with_args(script, argv),
    );

    let (stdio, prefix_handles) = if let Some(info) = parallel_info {
      // Right-align the entire `[name]` block so brackets line up across rows,
      // putting the leading padding spaces *outside* the brackets.
      let inner: Cow<str> = if info.include_package
        && let Some(pkg) = label_name
      {
        Cow::Owned(format!("{}#{}", pkg, task_name))
      } else {
        Cow::Borrowed(task_name)
      };
      let label = format!("[{}]", inner);
      let padded_label =
        format!("{:>width$}", label, width = info.label_pad_width + 2);
      let prefix =
        format!("{} ", colorize_task_prefix(&padded_label, info.color_index));
      let (io, handles) = task_runner::make_prefixed_task_io(prefix);
      (Some(io), handles)
    } else {
      (None, vec![])
    };

    let exit_code = task_runner::run_task(task_runner::RunTaskOptions {
      task_name,
      script,
      cwd,
      env_vars: self.env_vars.clone(),
      custom_commands,
      init_cwd: self.cli_options.initial_cwd(),
      argv,
      node_modules_bin_dirs: &node_modules_bin_dirs,
      stdio,
      kill_signal,
    })
    .await?
    .exit_code;

    for handle in prefix_handles {
      let _ = handle.await;
    }

    Ok(exit_code)
  }

  async fn maybe_npm_install(&self) -> Result<(), AnyError> {
    if let Some(npm_installer) = self.npm_installer {
      self.progress_bar.deferred_keep_initialize_alive();
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

  fn output_task(
    &self,
    task_name: &str,
    package_name: Option<&str>,
    script: &str,
  ) {
    log::info!(
      "{} {}{} {}",
      colors::green("Task"),
      colors::cyan(task_name),
      package_name
        .filter(
          |_| self.task_flags.recursive || self.task_flags.filter.is_some()
        )
        .map(|p| format!(" ({})", colors::gray(p)))
        .unwrap_or_default(),
      script,
    );
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
  task_or_script: TaskOrScript<'a>,
  dependencies: Vec<usize>,
}

fn sort_tasks_topo<'a>(
  pkg_task_config: &'a PackageTaskInfo,
  sorted: &mut Vec<ResolvedTask<'a>>,
) -> Result<(), TaskError> {
  trait TasksConfig {
    fn task(&self, name: &str) -> Option<(TaskOrScript<'_>, &dyn TasksConfig)>;
  }

  impl TasksConfig for WorkspaceTasksConfig {
    fn task(&self, name: &str) -> Option<(TaskOrScript<'_>, &dyn TasksConfig)> {
      if let Some(task_or_script) = self.member.task(name) {
        return Some((task_or_script, self as &dyn TasksConfig));
      }
      if let Some(task_or_script) = self.root.task(name) {
        // switch to only using the root tasks for the dependencies
        return Some((task_or_script, &self.root as &dyn TasksConfig));
      }
      None
    }
  }

  impl TasksConfig for WorkspaceMemberTasksConfig {
    fn task(&self, name: &str) -> Option<(TaskOrScript<'_>, &dyn TasksConfig)> {
      self
        .task(name)
        .map(|task_or_script| (task_or_script, self as &dyn TasksConfig))
    }
  }

  fn sort_visit<'a>(
    name: &'a str,
    sorted: &mut Vec<ResolvedTask<'a>>,
    mut path: Vec<(&'a Url, &'a str)>,
    tasks_config: &'a dyn TasksConfig,
  ) -> Result<usize, TaskError> {
    let Some((task_or_script, tasks_config)) = tasks_config.task(name) else {
      return Err(TaskError::NotFound(name.to_string()));
    };

    let folder_url = task_or_script.folder_url();
    if let Some(existing_task) = sorted.iter().find(|task| {
      task.name == name && task.task_or_script.folder_url() == folder_url
    }) {
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
    if let TaskOrScript::Task { task, .. } = task_or_script {
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
      task_or_script,
      dependencies,
    });

    Ok(id)
  }

  for name in &pkg_task_config.matched_tasks {
    sort_visit(name, sorted, Vec::new(), &pkg_task_config.tasks_config)?;
  }

  Ok(())
}

fn matches_package(
  config: &FolderConfigs,
  folder_url: &Url,
  force_use_pkg_json: bool,
  regex: &Regex,
  is_workspace_root: bool,
) -> bool {
  if !force_use_pkg_json
    && let Some(deno_json) = &config.deno_json
    && let Some(name) = &deno_json.json.name
    && regex.is_match(name)
  {
    return true;
  }

  if let Some(package_json) = &config.pkg_json
    && let Some(name) = &package_json.name
    && regex.is_match(name)
  {
    return true;
  }

  // Fall back to matching the workspace member's directory name so that
  // `deno task --filter <dir>` works when the package name diverges
  // from the directory (#28620). Skip the workspace root so it is never
  // accidentally selected by `--filter *`.
  if !is_workspace_root
    && let Some(dir_name) = workspace_folder_dir_name(folder_url)
    && regex.is_match(dir_name)
  {
    return true;
  }

  false
}

fn workspace_folder_dir_name(folder_url: &Url) -> Option<&str> {
  let path = folder_url.path();
  let trimmed = path.strip_suffix('/').unwrap_or(path);
  trimmed.rsplit('/').next().filter(|s| !s.is_empty())
}

fn print_available_tasks_workspace(
  cli_options: &Arc<CliOptions>,
  package_regex: &Regex,
  filter: &str,
  force_use_pkg_json: bool,
  recursive: bool,
) -> Result<(), AnyError> {
  let workspace = cli_options.workspace();
  let root_dir_url = workspace.root_dir_url();

  let mut matched = false;
  for (folder_url, folder) in workspace.config_folders() {
    if !recursive
      && !matches_package(
        folder,
        folder_url,
        force_use_pkg_json,
        package_regex,
        folder_url == root_dir_url,
      )
    {
      continue;
    }
    matched = true;

    let member_dir = workspace.resolve_member_dir(folder_url);
    let mut tasks_config = member_dir.to_tasks_config()?;

    let mut pkg_name = folder
      .deno_json
      .as_ref()
      .and_then(|deno| deno.json.name.clone())
      .or(folder.pkg_json.as_ref().and_then(|pkg| pkg.name.clone()));

    if force_use_pkg_json {
      tasks_config = tasks_config.with_only_pkg_json();
      pkg_name = folder.pkg_json.as_ref().and_then(|pkg| pkg.name.clone());
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
      colors::red(format!(
        "No package name matched the filter '{}' in available 'deno.json' or 'package.json' files.",
        filter
      ))
    );
  }

  Ok(())
}

pub struct AvailableTaskDescription {
  pub is_root: bool,
  pub is_deno: bool,
  pub name: String,
  pub task: TaskDefinition,
}

pub fn get_available_tasks_for_completion(
  flags: Arc<Flags>,
) -> Result<Vec<AvailableTaskDescription>, AnyError> {
  let recursive = match &flags.subcommand {
    crate::args::DenoSubcommand::Task(task_flags) => {
      task_flags.recursive || task_flags.filter.is_some()
    }
    _ => false,
  };

  let factory = crate::factory::CliFactory::from_flags(flags);
  let options = factory.cli_options()?;

  if recursive {
    let workspace = options.workspace();
    let mut all_tasks = Vec::new();
    let mut seen_task_names = HashSet::new();
    for folder_url in workspace.config_folders().keys() {
      let member_dir = workspace.resolve_member_dir(folder_url);
      let tasks_config = member_dir.to_tasks_config()?;
      let tasks = get_available_tasks(&member_dir, &tasks_config)?;
      for task in tasks {
        if seen_task_names.insert(task.name.clone()) {
          all_tasks.push(task);
        }
      }
    }
    Ok(all_tasks)
  } else {
    let member_dir = &options.start_dir;
    let tasks_config = member_dir.to_tasks_config()?;
    get_available_tasks(member_dir, &tasks_config).map_err(AnyError::from)
  }
}

fn get_available_tasks(
  workspace_dir: &Arc<WorkspaceDirectory>,
  tasks_config: &WorkspaceTasksConfig,
) -> Result<Vec<AvailableTaskDescription>, std::io::Error> {
  let is_cwd_root_dir = tasks_config.root.is_empty();

  let mut seen_task_names = HashSet::with_capacity(tasks_config.tasks_count());
  let mut task_descriptions = Vec::with_capacity(tasks_config.tasks_count());

  for config in [&tasks_config.member, &tasks_config.root] {
    if let Some(config) = config.deno_json.as_ref() {
      let is_root = !is_cwd_root_dir
        && config.folder_url
          == *workspace_dir.workspace.root_dir_url().as_ref();

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
        && config.folder_url
          == *workspace_dir.workspace.root_dir_url().as_ref();
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
            files: Vec::new(),
            output: Vec::new(),
            env: Vec::new(),
          },
        });
      }
    }
  }
  Ok(task_descriptions)
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

  if tasks_config.is_empty() {
    writeln!(
      writer,
      "  {}",
      colors::red("No tasks found in configuration file")
    )?;
    return Ok(());
  }

  let task_descriptions = get_available_tasks(workspace_dir, tasks_config)?;

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

  if let Some(TaskOrScript::Task { task, .. }) = &tasks_config.task(name) {
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
    if task_name_filter.matches(name) && !visited.contains(name) {
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

fn arg_to_task_name_filter(
  input: &str,
) -> Result<TaskNameFilter<'_>, AnyError> {
  // Parse an optional trailing exclusion group of the form `(!a|b|c)`.
  // Exclusion values are matched against what each `*` in the pattern
  // captures, e.g. `test:*(!e2e|interactive)` excludes `test:e2e` and
  // `test:interactive` but still matches `test:unit`.
  let (pattern, exclusions): (&str, Vec<&str>) = match input.rfind("(!") {
    Some(open) if input.ends_with(')') => {
      let inner = &input[open + 2..input.len() - 1];
      (&input[..open], inner.split('|').collect())
    }
    _ => (input, Vec::new()),
  };

  if !pattern.contains('*') {
    if !exclusions.is_empty() {
      return Err(anyhow!(
        "task name filter '{}' uses an exclusion group '(!...)' but has no wildcard '*' to exclude from",
        input
      ));
    }
    return Ok(TaskNameFilter::Exact(input));
  }

  let mut regex_str = regex::escape(pattern);
  regex_str = regex_str.replace("\\*", "(.*)");
  regex_str = format!("^{}", regex_str);
  let re = Regex::new(&regex_str)?;
  let exclusions = exclusions.into_iter().map(String::from).collect();
  Ok(TaskNameFilter::Regex { re, exclusions })
}

#[derive(Debug)]
enum TaskNameFilter<'s> {
  Exact(&'s str),
  Regex {
    re: regex::Regex,
    exclusions: Vec<String>,
  },
}

impl TaskNameFilter<'_> {
  fn matches(&self, name: &str) -> bool {
    match self {
      Self::Exact(n) => *n == name,
      Self::Regex { re, exclusions } => {
        let Some(caps) = re.captures(name) else {
          return false;
        };
        if exclusions.is_empty() {
          return true;
        }
        for i in 1..caps.len() {
          if let Some(m) = caps.get(i)
            && exclusions.iter().any(|e| e == m.as_str())
          {
            return false;
          }
        }
        true
      }
    }
  }
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
      TaskNameFilter::Regex { .. }
    ));

    let filter = arg_to_task_name_filter("test:*").unwrap();
    assert!(filter.matches("test:deno"));
    assert!(filter.matches("test:dprint"));
    assert!(!filter.matches("update:latest:deno"));
  }

  #[test]
  fn test_arg_to_task_name_filter_exclusion() {
    let filter = arg_to_task_name_filter("test:*(!e2e|interactive)").unwrap();
    assert!(filter.matches("test:unit"));
    assert!(filter.matches("test:integration"));
    assert!(!filter.matches("test:e2e"));
    assert!(!filter.matches("test:interactive"));
    assert!(!filter.matches("update:latest:deno"));

    let filter = arg_to_task_name_filter("*(!build)").unwrap();
    assert!(filter.matches("test"));
    assert!(filter.matches("lint"));
    assert!(!filter.matches("build"));

    let filter = arg_to_task_name_filter("test:*(!e2e)").unwrap();
    assert!(filter.matches("test:e2e:smoke"));
    assert!(!filter.matches("test:e2e"));
  }

  #[test]
  fn test_arg_to_task_name_filter_exclusion_without_wildcard_errors() {
    assert!(arg_to_task_name_filter("test(!e2e)").is_err());
  }
}
