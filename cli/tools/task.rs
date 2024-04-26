// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
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
  flags: Flags,
  task_flags: TaskFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags)?;
  let cli_options = factory.cli_options();
  let tasks_config = cli_options.resolve_tasks_config()?;
  let maybe_package_json = cli_options.maybe_package_json();
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
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;

  if let Some(
    deno_config::Task::Definition(script)
    | deno_config::Task::Commented {
      definition: script, ..
    },
  ) = tasks_config.get(task_name)
  {
    let config_file_url = cli_options.maybe_config_file_specifier().unwrap();
    let config_file_path = if config_file_url.scheme() == "file" {
      config_file_url.to_file_path().unwrap()
    } else {
      bail!("Only local configuration files are supported")
    };
    let cwd = match task_flags.cwd {
      Some(path) => canonicalize_path(&PathBuf::from(path))?,
      None => config_file_path.parent().unwrap().to_owned(),
    };

    let npm_commands =
      task_runner::resolve_npm_commands(npm_resolver.as_ref(), node_resolver)?;
    run_task(
      task_name,
      script,
      &cwd,
      cli_options,
      npm_commands,
      npm_resolver.as_ref(),
    )
    .await
  } else if package_json_scripts.contains_key(task_name) {
    let package_json_deps_provider = factory.package_json_deps_provider();

    if let Some(package_deps) = package_json_deps_provider.deps() {
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

    // ensure the npm packages are installed if using a node_modules
    // directory and managed resolver
    if cli_options.has_node_modules_dir() {
      if let Some(npm_resolver) = npm_resolver.as_managed() {
        npm_resolver.ensure_top_level_package_json_install().await?;
        npm_resolver.resolve_pending().await?;
      }
    }

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

    // At this point we already checked if the task name exists in package.json.
    // We can therefore check for "pre" and "post" scripts too, since we're only
    // dealing with package.json here and not deno.json
    let task_names = vec![
      format!("pre{}", task_name),
      task_name.clone(),
      format!("post{}", task_name),
    ];
    let npm_commands =
      task_runner::resolve_npm_commands(npm_resolver.as_ref(), node_resolver)?;
    for task_name in task_names {
      if let Some(script) = package_json_scripts.get(&task_name) {
        let exit_code = run_task(
          &task_name,
          script,
          &cwd,
          cli_options,
          npm_commands.clone(),
          npm_resolver.as_ref(),
        )
        .await?;
        if exit_code > 0 {
          return Ok(exit_code);
        }
      }
    }

    Ok(0)
  } else {
    eprintln!("Task not found: {task_name}");
    print_available_tasks(&tasks_config, &package_json_scripts);
    Ok(1)
  }
}

async fn run_task(
  task_name: &str,
  script: &str,
  cwd: &Path,
  cli_options: &CliOptions,
  npm_commands: HashMap<String, Rc<dyn ShellCommand>>,
  npm_resolver: &dyn CliNpmResolver,
) -> Result<i32, AnyError> {
  let script = get_script_with_args(script, cli_options);
  output_task(task_name, &script);
  task_runner::run_task(
    task_name,
    &script,
    cwd,
    npm_commands,
    npm_resolver.root_node_modules_path().map(|p| p.as_path()),
  )
  .await
}

fn output_task(task_name: &str, script: &str) {
  log::info!(
    "{} {} {}",
    colors::green("Task"),
    colors::cyan(&task_name),
    script,
  );
}

fn print_available_tasks(
  // order can be important, so these use an index map
  tasks_config: &IndexMap<String, deno_config::Task>,
  package_json_scripts: &IndexMap<String, String>,
) {
  eprintln!("{}", colors::green("Available tasks:"));

  let mut had_task = false;
  for (is_deno, (key, task)) in tasks_config
    .iter()
    .map(|(k, t)| (true, (k, t.clone())))
    .chain(
      package_json_scripts
        .iter()
        .filter(|(key, _)| !tasks_config.contains_key(*key))
        .map(|(k, v)| (false, (k, deno_config::Task::Definition(v.clone())))),
    )
  {
    eprintln!(
      "- {}{}",
      colors::cyan(key),
      if is_deno {
        "".to_string()
      } else {
        format!(" {}", colors::italic_gray("(package.json)"))
      }
    );
    let definition = match &task {
      deno_config::Task::Definition(definition) => definition,
      deno_config::Task::Commented { definition, .. } => definition,
    };
    if let deno_config::Task::Commented { comments, .. } = &task {
      let slash_slash = colors::italic_gray("//");
      for comment in comments {
        eprintln!("    {slash_slash} {}", colors::italic_gray(comment));
      }
    }
    eprintln!("    {definition}");
    had_task = true;
  }
  if !had_task {
    eprintln!("  {}", colors::red("No tasks found in configuration file"));
  }
}

fn get_script_with_args(script: &str, options: &CliOptions) -> String {
  let additional_args = options
    .argv()
    .iter()
    // surround all the additional arguments in double quotes
    // and sanitize any command substitution
    .map(|a| format!("\"{}\"", a.replace('"', "\\\"").replace('$', "\\$")))
    .collect::<Vec<_>>()
    .join(" ");
  let script = format!("{script} {additional_args}");
  script.trim().to_owned()
}
