// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_semver::package::PackageNv;
use deno_task_shell::ExecutableCommand;
use deno_task_shell::ExecuteResult;
use deno_task_shell::KillSignal;
use deno_task_shell::ShellCommand;
use deno_task_shell::ShellCommandContext;
use deno_task_shell::ShellPipeReader;
use deno_task_shell::ShellPipeWriter;
use lazy_regex::Lazy;
use regex::Regex;
use tokio::task::JoinHandle;
use tokio::task::LocalSet;
use tokio_util::sync::CancellationToken;

use crate::node::CliNodeResolver;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliNpmResolver;

pub fn get_script_with_args(script: &str, argv: &[String]) -> String {
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

pub struct TaskStdio(Option<ShellPipeReader>, ShellPipeWriter);

impl TaskStdio {
  pub fn stdout() -> Self {
    Self(None, ShellPipeWriter::stdout())
  }

  pub fn stderr() -> Self {
    Self(None, ShellPipeWriter::stderr())
  }

  pub fn piped() -> Self {
    let (r, w) = deno_task_shell::pipe();
    Self(Some(r), w)
  }
}

pub struct TaskIo {
  pub stdout: TaskStdio,
  pub stderr: TaskStdio,
}

impl Default for TaskIo {
  fn default() -> Self {
    Self {
      stdout: TaskStdio::stdout(),
      stderr: TaskStdio::stderr(),
    }
  }
}

pub struct RunTaskOptions<'a> {
  pub task_name: &'a str,
  pub script: &'a str,
  pub cwd: PathBuf,
  pub init_cwd: &'a Path,
  pub env_vars: HashMap<OsString, OsString>,
  pub argv: &'a [String],
  pub custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  pub root_node_modules_dir: Option<&'a Path>,
  pub stdio: Option<TaskIo>,
  pub kill_signal: KillSignal,
}

pub type TaskCustomCommands = HashMap<String, Rc<dyn ShellCommand>>;

pub struct TaskResult {
  pub exit_code: i32,
  pub stdout: Option<Vec<u8>>,
  pub stderr: Option<Vec<u8>>,
}

pub async fn run_task(
  opts: RunTaskOptions<'_>,
) -> Result<TaskResult, AnyError> {
  let script = get_script_with_args(opts.script, opts.argv);
  let seq_list = deno_task_shell::parser::parse(&script)
    .with_context(|| format!("Error parsing script '{}'.", opts.task_name))?;
  let env_vars =
    prepare_env_vars(opts.env_vars, opts.init_cwd, opts.root_node_modules_dir);
  let state = deno_task_shell::ShellState::new(
    env_vars,
    opts.cwd,
    opts.custom_commands,
    opts.kill_signal,
  );
  let stdio = opts.stdio.unwrap_or_default();
  let (
    TaskStdio(stdout_read, stdout_write),
    TaskStdio(stderr_read, stderr_write),
  ) = (stdio.stdout, stdio.stderr);

  fn read(reader: ShellPipeReader) -> JoinHandle<Result<Vec<u8>, AnyError>> {
    tokio::task::spawn_blocking(move || {
      let mut buf = Vec::new();
      reader.pipe_to(&mut buf)?;
      Ok(buf)
    })
  }

  let stdout = stdout_read.map(read);
  let stderr = stderr_read.map(read);

  let local = LocalSet::new();
  let future = async move {
    let exit_code = deno_task_shell::execute_with_pipes(
      seq_list,
      state,
      ShellPipeReader::stdin(),
      stdout_write,
      stderr_write,
    )
    .await;
    Ok::<_, AnyError>(TaskResult {
      exit_code,
      stdout: if let Some(stdout) = stdout {
        Some(stdout.await??)
      } else {
        None
      },
      stderr: if let Some(stderr) = stderr {
        Some(stderr.await??)
      } else {
        None
      },
    })
  };
  local.run_until(future).await
}

fn prepare_env_vars(
  mut env_vars: HashMap<OsString, OsString>,
  initial_cwd: &Path,
  node_modules_dir: Option<&Path>,
) -> HashMap<OsString, OsString> {
  const INIT_CWD_NAME: &str = "INIT_CWD";
  if !env_vars.contains_key(OsStr::new(INIT_CWD_NAME)) {
    // if not set, set an INIT_CWD env var that has the cwd
    env_vars.insert(
      INIT_CWD_NAME.into(),
      initial_cwd.to_path_buf().into_os_string(),
    );
  }
  if !env_vars
    .contains_key(OsStr::new(crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR))
  {
    env_vars.insert(
      crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR.into(),
      crate::npm::get_npm_config_user_agent().into(),
    );
  }
  if let Some(node_modules_dir) = node_modules_dir {
    prepend_to_path(
      &mut env_vars,
      node_modules_dir.join(".bin").into_os_string(),
    );
  }
  env_vars
}

fn prepend_to_path(
  env_vars: &mut HashMap<OsString, OsString>,
  value: OsString,
) {
  match env_vars.get_mut(OsStr::new("PATH")) {
    Some(path) => {
      if path.is_empty() {
        *path = value;
      } else {
        let mut new_path = value;
        new_path.push(if cfg!(windows) { ";" } else { ":" });
        new_path.push(&path);
        *path = new_path;
      }
    }
    None => {
      env_vars.insert("PATH".into(), value);
    }
  }
}

pub fn real_env_vars() -> HashMap<OsString, OsString> {
  std::env::vars_os()
    .map(|(k, v)| {
      if cfg!(windows) {
        (k.to_ascii_uppercase(), v)
      } else {
        (k, v)
      }
    })
    .collect()
}

// WARNING: Do not depend on this env var in user code. It's not stable API.
pub(crate) static USE_PKG_JSON_HIDDEN_ENV_VAR_NAME: &str =
  "DENO_INTERNAL_TASK_USE_PKG_JSON";

pub struct NpmCommand;

impl ShellCommand for NpmCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if context.args.first().and_then(|s| s.to_str()) == Some("run")
      && context.args.len() >= 2
      // for now, don't run any npm scripts that have a flag because
      // we don't handle stuff like `--workspaces` properly
      && !context.args.iter().any(|s| s.to_string_lossy().starts_with('-'))
    {
      // run with deno task instead
      let mut args: Vec<OsString> = Vec::with_capacity(context.args.len());
      args.push("task".into());
      args.extend(context.args.into_iter().skip(1));

      let mut state = context.state;
      state.apply_env_var(
        OsStr::new(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME),
        OsStr::new("1"),
      );
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
    let npm_path = match context.state.resolve_command_path(OsStr::new("npm")) {
      Ok(path) => path,
      Err(err) => {
        let _ = context.stderr.write_line(&format!("{}", err));
        return Box::pin(std::future::ready(ExecuteResult::from_exit_code(
          err.exit_code(),
        )));
      }
    };
    ExecutableCommand::new("npm".to_string(), npm_path).execute(context)
  }
}

pub struct NodeCommand;

impl ShellCommand for NodeCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    // continue to use Node if the first argument is a flag
    // or there are no arguments provided for some reason
    if context.args.is_empty()
      || ({
        let first_arg = context.args[0].to_string_lossy();
        first_arg.starts_with('-') // has a flag
      })
    {
      return ExecutableCommand::new("node".to_string(), PathBuf::from("node"))
        .execute(context);
    }

    let mut args: Vec<OsString> = Vec::with_capacity(7 + context.args.len());
    args.extend([
      "run".into(),
      "-A".into(),
      "--unstable-bare-node-builtins".into(),
      "--unstable-detect-cjs".into(),
      "--unstable-node-globals".into(),
      "--unstable-sloppy-imports".into(),
      "--unstable-unsafe-proto".into(),
    ]);
    args.extend(context.args);

    let mut state = context.state;
    state.apply_env_var(
      OsStr::new(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME),
      OsStr::new("1"),
    );
    ExecutableCommand::new("deno".to_string(), std::env::current_exe().unwrap())
      .execute(ShellCommandContext {
        args,
        state,
        ..context
      })
  }
}

pub struct NodeGypCommand;

impl ShellCommand for NodeGypCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    // at the moment this shell command is just to give a warning if node-gyp is not found
    // in the future, we could try to run/install node-gyp for the user with deno
    if context
      .state
      .resolve_command_path(OsStr::new("node-gyp"))
      .is_err()
    {
      log::warn!("{} node-gyp was used in a script, but was not listed as a dependency. Either add it as a dependency or install it globally (e.g. `npm install -g node-gyp`)", crate::colors::yellow("Warning"));
    }
    ExecutableCommand::new(
      "node-gyp".to_string(),
      "node-gyp".to_string().into(),
    )
    .execute(context)
  }
}

pub struct NpxCommand;

impl ShellCommand for NpxCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if let Some(first_arg) = context.args.first().cloned() {
      if let Some(command) = context.state.resolve_custom_command(&first_arg) {
        let context = ShellCommandContext {
          args: context.args.into_iter().skip(1).collect::<Vec<_>>(),
          ..context
        };
        command.execute(context)
      } else {
        // can't find the command, so fallback to running the real npx command
        let npx_path =
          match context.state.resolve_command_path(OsStr::new("npx")) {
            Ok(npx) => npx,
            Err(err) => {
              let _ = context.stderr.write_line(&format!("{}", err));
              return Box::pin(std::future::ready(
                ExecuteResult::from_exit_code(err.exit_code()),
              ));
            }
          };
        ExecutableCommand::new("npx".to_string(), npx_path).execute(context)
      }
    } else {
      let _ = context.stderr.write_line("npx: missing command");
      Box::pin(std::future::ready(ExecuteResult::from_exit_code(1)))
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
    let mut args: Vec<OsString> = vec![
      "run".into(),
      "-A".into(),
      if self.npm_package.name == self.name {
        format!("npm:{}", self.npm_package)
      } else {
        format!("npm:{}/{}", self.npm_package, self.name)
      }
      .into(),
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
pub struct NodeModulesFileRunCommand {
  pub command_name: String,
  pub path: PathBuf,
}

impl ShellCommand for NodeModulesFileRunCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    let mut args: Vec<OsString> = vec![
      "run".into(),
      "--ext=js".into(),
      "-A".into(),
      self.path.clone().into_os_string(),
    ];
    args.extend(context.args);
    let executable_command = deno_task_shell::ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe().unwrap(),
    );
    // set this environment variable so that the launched process knows the npm command name
    context.state.apply_env_var(
      OsStr::new("DENO_INTERNAL_NPM_CMD_NAME"),
      OsStr::new(&self.command_name),
    );
    executable_command.execute(ShellCommandContext { args, ..context })
  }
}

pub fn resolve_custom_commands(
  npm_resolver: &CliNpmResolver,
  node_resolver: &CliNodeResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut commands = match npm_resolver {
    CliNpmResolver::Byonm(npm_resolver) => {
      let node_modules_dir = npm_resolver.root_node_modules_path().unwrap();
      resolve_npm_commands_from_bin_dir(node_modules_dir)
    }
    CliNpmResolver::Managed(npm_resolver) => {
      resolve_managed_npm_commands(npm_resolver, node_resolver)?
    }
  };
  commands.insert("npm".to_string(), Rc::new(NpmCommand));
  Ok(commands)
}

pub fn resolve_npm_commands_from_bin_dir(
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

  let maybe_first_line = {
    let index = text.find("\n")?;
    Some(&text[0..index])
  };

  if let Some(first_line) = maybe_first_line {
    // NOTE(bartlomieju): this is not perfect, but handle two most common scenarios
    // where Node is run without any args. If there are args then we use `NodeCommand`
    // struct.
    if first_line == "#!/usr/bin/env node"
      || first_line == "#!/usr/bin/env -S node"
    {
      // launch this file itself because it's a JS file
      return Some(file_path);
    }
  }

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

fn resolve_managed_npm_commands(
  npm_resolver: &CliManagedNpmResolver,
  node_resolver: &CliNodeResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut result = HashMap::new();
  for id in npm_resolver.resolution().top_level_packages() {
    let package_folder = npm_resolver.resolve_pkg_folder_from_pkg_id(&id)?;
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

/// Runs a deno task future forwarding any signals received
/// to the process.
///
/// Signal listeners and ctrl+c listening will be setup.
pub async fn run_future_forwarding_signals<TOutput>(
  kill_signal: KillSignal,
  future: impl std::future::Future<Output = TOutput>,
) -> TOutput {
  fn spawn_future_with_cancellation(
    future: impl std::future::Future<Output = ()> + 'static,
    token: CancellationToken,
  ) {
    deno_core::unsync::spawn(async move {
      tokio::select! {
        _ = future => {}
        _ = token.cancelled() => {}
      }
    });
  }

  let token = CancellationToken::new();
  let _token_drop_guard = token.clone().drop_guard();
  let _drop_guard = kill_signal.clone().drop_guard();

  spawn_future_with_cancellation(
    listen_ctrl_c(kill_signal.clone()),
    token.clone(),
  );
  #[cfg(unix)]
  spawn_future_with_cancellation(
    listen_and_forward_all_signals(kill_signal),
    token,
  );

  future.await
}

async fn listen_ctrl_c(kill_signal: KillSignal) {
  while let Ok(()) = tokio::signal::ctrl_c().await {
    // On windows, ctrl+c is sent to the process group, so the signal would
    // have already been sent to the child process. We still want to listen
    // for ctrl+c here to keep the process alive when receiving it, but no
    // need to forward the signal because it's already been sent.
    if !cfg!(windows) {
      kill_signal.send(deno_task_shell::SignalKind::SIGINT)
    }
  }
}

#[cfg(unix)]
async fn listen_and_forward_all_signals(kill_signal: KillSignal) {
  use deno_core::futures::FutureExt;
  use deno_runtime::deno_os::signal::SIGNAL_NUMS;

  // listen and forward every signal we support
  let mut futures = Vec::with_capacity(SIGNAL_NUMS.len());
  for signo in SIGNAL_NUMS.iter().copied() {
    if signo == libc::SIGKILL || signo == libc::SIGSTOP {
      continue; // skip, can't listen to these
    }

    let kill_signal = kill_signal.clone();
    futures.push(
      async move {
        let Ok(mut stream) = tokio::signal::unix::signal(
          tokio::signal::unix::SignalKind::from_raw(signo),
        ) else {
          return;
        };
        let signal_kind: deno_task_shell::SignalKind = signo.into();
        while let Some(()) = stream.recv().await {
          kill_signal.send(signal_kind);
        }
      }
      .boxed_local(),
    )
  }
  deno_core::futures::future::join_all(futures).await;
}

#[cfg(test)]
mod test {

  use super::*;

  #[test]
  fn test_prepend_to_path() {
    let mut env_vars = HashMap::new();

    prepend_to_path(&mut env_vars, "/example".into());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".into(), "/example".into())])
    );

    prepend_to_path(&mut env_vars, "/example2".into());
    let separator = if cfg!(windows) { ";" } else { ":" };
    assert_eq!(
      env_vars,
      HashMap::from([(
        "PATH".into(),
        format!("/example2{}/example", separator).into()
      )])
    );

    env_vars.get_mut(OsStr::new("PATH")).unwrap().clear();
    prepend_to_path(&mut env_vars, "/example".into());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".into(), "/example".into())])
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
    // example shim on unix
    let unix_shim = r#"#!/usr/bin/env -S node
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
