// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_task_shell::ExecutableCommand;
use deno_task_shell::ExecuteResult;
use deno_task_shell::KillSignal;
use deno_task_shell::ShellCommand;
use deno_task_shell::ShellCommandContext;
use deno_task_shell::ShellPipeReader;
use deno_task_shell::ShellPipeWriter;
use tokio::task::JoinHandle;
use tokio::task::LocalSet;
use tokio_util::sync::CancellationToken;

use crate::node::CliNodeResolver;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliNpmResolver;
use crate::util::fs::canonicalize_path;

pub fn get_script_with_args(script: &str, argv: &[String]) -> String {
  let additional_args = argv
    .iter()
    // Wrap each argument in single quotes so the shell preserves the
    // content literally (including backslashes, $, `, etc.). For an
    // argument containing a single quote, splice in a double-quoted `'`
    // using the POSIX idiom `'foo'"'"'bar'`.
    .map(|a| format!("'{}'", a.replace('\'', "'\"'\"'")))
    .collect::<Vec<_>>()
    .join(" ");

  let script = format!("{script} {additional_args}");
  script.trim().to_owned()
}

pub struct TaskStdio(Option<ShellPipeReader>, ShellPipeWriter);

pub struct PrefixedWriter<W: std::io::Write> {
  prefix: Vec<u8>,
  inner: W,
  line_buf: Vec<u8>,
  at_line_start: bool,
}

impl<W: std::io::Write> PrefixedWriter<W> {
  pub fn new(prefix: String, inner: W) -> Self {
    Self {
      prefix: prefix.into_bytes(),
      inner,
      line_buf: Vec::new(),
      at_line_start: true,
    }
  }
}

impl<W: std::io::Write> std::io::Write for PrefixedWriter<W> {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let mut rest = buf;
    while !rest.is_empty() {
      if self.at_line_start {
        self.line_buf.extend_from_slice(&self.prefix);
        self.at_line_start = false;
      }
      match rest.iter().position(|&b| b == b'\n') {
        Some(pos) => {
          self.line_buf.extend_from_slice(&rest[..pos + 1]);
          self.inner.write_all(&self.line_buf)?;
          self.line_buf.clear();
          self.at_line_start = true;
          rest = &rest[pos + 1..];
        }
        None => {
          self.line_buf.extend_from_slice(rest);
          break;
        }
      }
    }
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    if !self.line_buf.is_empty() {
      self.inner.write_all(&self.line_buf)?;
      self.line_buf.clear();
      self.at_line_start = true;
    }
    self.inner.flush()
  }
}

pub fn make_prefixed_task_io(prefix: String) -> (TaskIo, Vec<JoinHandle<()>>) {
  let (out_r, out_w) = deno_task_shell::pipe();
  let (err_r, err_w) = deno_task_shell::pipe();

  let out_prefix = prefix.clone();
  let out_handle = tokio::task::spawn_blocking(move || {
    let mut writer = PrefixedWriter::new(out_prefix, std::io::stdout());
    let _ = out_r.pipe_to(&mut writer);
    let _ = writer.flush();
  });

  let err_handle = tokio::task::spawn_blocking(move || {
    let mut writer = PrefixedWriter::new(prefix, std::io::stderr());
    let _ = err_r.pipe_to(&mut writer);
    let _ = writer.flush();
  });

  (
    TaskIo {
      stdout: TaskStdio(None, out_w),
      stderr: TaskStdio(None, err_w),
    },
    vec![out_handle, err_handle],
  )
}

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
  /// Directories to prepend to `PATH` for the duration of the task,
  /// ordered closest-first (closest entry ends up first in `PATH`).
  pub node_modules_bin_dirs: &'a [PathBuf],
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
  mut opts: RunTaskOptions<'_>,
) -> Result<TaskResult, AnyError> {
  let script = get_script_with_args(opts.script, opts.argv);
  let seq_list = deno_task_shell::parser::parse(&script)
    .with_context(|| format!("Error parsing script '{}'.", opts.task_name))?;
  let env_vars =
    prepare_env_vars(opts.env_vars, opts.init_cwd, opts.node_modules_bin_dirs);
  if !opts.custom_commands.contains_key("deno") {
    opts
      .custom_commands
      .insert("deno".to_string(), Rc::new(DenoCommand::default()));
  }
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
  node_modules_bin_dirs: &[PathBuf],
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
  // Prepend in reverse so the closest dir ends up first in `PATH`.
  for bin_dir in node_modules_bin_dirs.iter().rev() {
    prepend_to_path(&mut env_vars, bin_dir.as_os_str().to_os_string());
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
        std::env::current_exe()
          .and_then(|p| canonicalize_path(&p))
          .unwrap(),
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

pub struct DenoCommand(ExecutableCommand);

impl Default for DenoCommand {
  fn default() -> Self {
    Self(ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
    ))
  }
}

impl ShellCommand for DenoCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    self.0.execute(context)
  }
}

pub struct NodeCommand;

impl ShellCommand for NodeCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    // Parse the `node ...` invocation with the shared `node_shim` crate — the
    // same parser/translation Deno uses when `child_process` spawns `node` — so
    // npm lifecycle scripts (and `deno task`) can run `node -e`/`-p`,
    // `node --require`, plain `node <script>`, etc. without a real `node` binary
    // on `PATH`.
    //
    // See https://github.com/denoland/deno/issues/24916.
    let node_args = context
      .args
      .iter()
      .map(|arg| arg.to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    let Ok(parsed) = node_shim::parse_args(node_args) else {
      // Unsupported flag: defer to the real `node` binary.
      return ExecutableCommand::new("node".to_string(), PathBuf::from("node"))
        .execute(context);
    };

    let translated = node_shim::translate_to_deno_args(
      parsed,
      &node_shim::TranslateOptions::for_task_command(),
    );
    // node_shim returns empty args for the REPL (`node` with no script); defer
    // to the real `node` binary there too, preserving the previous behavior.
    if translated.deno_args.is_empty() {
      return ExecutableCommand::new("node".to_string(), PathBuf::from("node"))
        .execute(context);
    }

    let mut state = context.state;
    state.apply_env_var(
      OsStr::new(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME),
      OsStr::new("1"),
    );
    // Forward any Node options node_shim couldn't express as Deno flags (e.g.
    // `--no-warnings`) through `NODE_OPTIONS`, appending to any existing value
    // just like `child_process` does.
    if !translated.node_options.is_empty() {
      let node_options_name = OsStr::new("NODE_OPTIONS");
      let mut node_options = state
        .get_var(node_options_name)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
      for opt in &translated.node_options {
        if !node_options.is_empty() {
          node_options.push(' ');
        }
        node_options.push_str(opt);
      }
      state.apply_env_var(node_options_name, OsStr::new(&node_options));
    }

    let new_args = translated
      .deno_args
      .into_iter()
      .map(OsString::from)
      .collect::<Vec<_>>();
    ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
    )
    .execute(ShellCommandContext {
      args: new_args,
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
      log::warn!(
        "{} node-gyp was used in a script, but was not listed as a dependency. Either add it as a dependency or install it globally (e.g. `npm install -g node-gyp`)",
        crate::colors::yellow("Warning")
      );
      Box::pin(std::future::ready(ExecuteResult::from_exit_code(0)))
    } else {
      ExecutableCommand::new(
        "node-gyp".to_string(),
        "node-gyp".to_string().into(),
      )
      .execute(context)
    }
  }
}

pub struct NpxCommand;

impl ShellCommand for NpxCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if let Some(first_arg) = context.args.first().cloned() {
      match context.state.resolve_custom_command(&first_arg) {
        Some(command) => {
          let context = ShellCommandContext {
            args: context.args.into_iter().skip(1).collect::<Vec<_>>(),
            ..context
          };
          command.execute(context)
        }
        _ => {
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
      }
    } else {
      let _ = context.stderr.write_line("npx: missing command");
      Box::pin(std::future::ready(ExecuteResult::from_exit_code(1)))
    }
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
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
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
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliNpmResolver,
  bin_dirs: &[PathBuf],
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut commands = match npm_resolver {
    CliNpmResolver::Byonm(_) => {
      // Walk the bin dirs in order (closest first) and merge; closest wins.
      let mut commands: HashMap<String, Rc<dyn ShellCommand>> = HashMap::new();
      for bin_dir in bin_dirs {
        for (name, cmd) in
          resolve_npm_commands_from_bin_dir(bin_dir, node_resolver)
        {
          commands.entry(name).or_insert(cmd);
        }
      }
      commands
    }
    CliNpmResolver::Managed(npm_resolver) => {
      resolve_managed_npm_commands(node_resolver, npm_resolver)?
    }
  };
  commands.insert("npm".to_string(), Rc::new(NpmCommand));
  Ok(commands)
}

/// Builds the list of `node_modules/.bin` directories to consult for a task.
///
/// For BYONM this walks up the filesystem from `cwd` collecting every
/// `<ancestor>/node_modules/.bin` directory (matching how Node, npm, and pnpm
/// resolve bin commands). For the managed npm resolver there is only ever a
/// single `node_modules/.bin`.
pub fn resolve_task_node_modules_bin_dirs(
  npm_resolver: &CliNpmResolver,
  cwd: &Path,
) -> Vec<PathBuf> {
  match npm_resolver {
    CliNpmResolver::Byonm(_) => cwd
      .ancestors()
      .map(|dir| dir.join("node_modules").join(".bin"))
      .collect(),
    CliNpmResolver::Managed(npm_resolver) => npm_resolver
      .root_node_modules_path()
      .map(|p| vec![p.join(".bin")])
      .unwrap_or_default(),
  }
}

pub fn resolve_npm_commands_from_bin_dir(
  bin_dir: &Path,
  node_resolver: &CliNodeResolver,
) -> HashMap<String, Rc<dyn ShellCommand>> {
  let bin_commands = node_resolver.resolve_npm_commands_from_bin_dir(bin_dir);
  bin_commands
    .into_iter()
    .map(|(command_name, path)| {
      (
        command_name.clone(),
        Rc::new(NodeModulesFileRunCommand {
          command_name,
          path: path.path().to_path_buf(),
        }) as Rc<dyn ShellCommand>,
      )
    })
    .collect()
}

fn resolve_managed_npm_commands(
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliManagedNpmResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut result = HashMap::new();
  for id in npm_resolver.resolution().top_level_packages() {
    let package_folder = npm_resolver.resolve_pkg_folder_from_pkg_id(&id)?;
    let bins =
      node_resolver.resolve_npm_binary_commands_for_package(&package_folder)?;
    result.extend(bins.into_iter().map(|(command_name, path)| {
      (
        command_name.clone(),
        Rc::new(NodeModulesFileRunCommand {
          command_name,
          path: path.path().to_path_buf(),
        }) as Rc<dyn ShellCommand>,
      )
    }));
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

  // Save the terminal mode up front so it can be restored once the task (and
  // all of its child processes) have finished. A child process such as a dev
  // server may switch the terminal into raw mode and, when terminated via
  // Ctrl+C, leave it in a broken state (no echo, no line editing). Declared
  // first so it is dropped last, i.e. after the children have been torn down by
  // the kill-signal drop guards below.
  let terminal_mode_guard = crate::util::console::TerminalModeGuard::acquire();
  let saved_terminal_mode = terminal_mode_guard.saved();

  let token = CancellationToken::new();
  let _token_drop_guard = token.clone().drop_guard();
  let _drop_guard = kill_signal.clone().drop_guard();

  spawn_future_with_cancellation(
    listen_ctrl_c(kill_signal.clone(), saved_terminal_mode),
    token.clone(),
  );
  #[cfg(unix)]
  spawn_future_with_cancellation(
    listen_and_forward_all_signals(kill_signal),
    token,
  );

  future.await
}

async fn listen_ctrl_c(
  kill_signal: KillSignal,
  saved_terminal_mode: crate::util::console::SavedTerminalMode,
) {
  while let Ok(()) = deno_signals::ctrl_c().await {
    // On windows, ctrl+c is sent to the process group, so the signal would
    // have already been sent to the child process. We still want to listen
    // for ctrl+c here to keep the process alive when receiving it, but no
    // need to forward the signal because it's already been sent.
    if cfg!(windows) {
      // A child (e.g. a dev server) may have put the terminal into raw mode.
      // Restore it now so that input echoes again — for instance at cmd.exe's
      // "Terminate batch job (Y/N)?" prompt that appears for `.cmd` scripts.
      // The TerminalModeGuard still restores once more when the task ends, in
      // case the child re-enters raw mode before exiting.
      saved_terminal_mode.restore();
    } else {
      kill_signal.send(deno_task_shell::SignalKind::SIGINT)
    }
  }
}

#[cfg(unix)]
async fn listen_and_forward_all_signals(kill_signal: KillSignal) {
  use deno_core::futures::FutureExt;
  use deno_signals::SIGNAL_NUMS;

  // listen and forward every signal we support
  let mut futures = Vec::with_capacity(SIGNAL_NUMS.len());
  for signo in SIGNAL_NUMS.iter().copied() {
    if signo == libc::SIGKILL || signo == libc::SIGSTOP {
      continue; // skip, can't listen to these
    }

    let kill_signal = kill_signal.clone();
    futures.push(
      async move {
        let Ok(mut stream) = deno_signals::signal_stream(signo) else {
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
  fn test_get_script_with_args() {
    let cases: &[(&[&str], &str)] = &[
      (&[], "echo"),
      (&["hello"], "echo 'hello'"),
      (&["hello", "world"], "echo 'hello' 'world'"),
      // Windows path with trailing backslash (issue #31453).
      (&[".\\dist\\"], "echo '.\\dist\\'"),
      // Dollar sign and backtick must not be expanded.
      (&["$HOME"], "echo '$HOME'"),
      (&["`cmd`"], "echo '`cmd`'"),
      // Double quotes pass through literally.
      (&["foo\"bar"], "echo 'foo\"bar'"),
      // Single quote uses the POSIX `'"'"'` idiom.
      (&["it's"], "echo 'it'\"'\"'s'"),
    ];
    for (argv, expected) in cases {
      let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
      assert_eq!(get_script_with_args("echo", &argv), *expected);
    }
  }
}
