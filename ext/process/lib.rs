// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::prelude::ExitStatusExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::rc::Rc;

use deno_core::op2;
use deno_core::serde_json;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToJsBuffer;
use deno_error::JsErrorBox;
use deno_io::fs::FileResource;
use deno_io::ChildStderrResource;
use deno_io::ChildStdinResource;
use deno_io::ChildStdoutResource;
use deno_io::IntoRawIoHandle;
use deno_os::SignalError;
use deno_permissions::PermissionsContainer;
use deno_permissions::RunQueryDescriptor;
use serde::Deserialize;
use serde::Serialize;
use tokio::process::Command;

pub mod ipc;
use ipc::IpcJsonStreamResource;
use ipc::IpcRefTracker;

pub const UNSTABLE_FEATURE_NAME: &str = "process";

#[derive(Copy, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stdio {
  Inherit,
  Piped,
  Null,
  IpcForInternalUse,
}

impl Stdio {
  pub fn as_stdio(&self) -> std::process::Stdio {
    match &self {
      Stdio::Inherit => std::process::Stdio::inherit(),
      Stdio::Piped => std::process::Stdio::piped(),
      Stdio::Null => std::process::Stdio::null(),
      _ => unreachable!(),
    }
  }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum StdioOrRid {
  Stdio(Stdio),
  Rid(ResourceId),
}

impl<'de> Deserialize<'de> for StdioOrRid {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use serde_json::Value;
    let value = Value::deserialize(deserializer)?;
    match value {
      Value::String(val) => match val.as_str() {
        "inherit" => Ok(StdioOrRid::Stdio(Stdio::Inherit)),
        "piped" => Ok(StdioOrRid::Stdio(Stdio::Piped)),
        "null" => Ok(StdioOrRid::Stdio(Stdio::Null)),
        "ipc_for_internal_use" => {
          Ok(StdioOrRid::Stdio(Stdio::IpcForInternalUse))
        }
        val => Err(serde::de::Error::unknown_variant(
          val,
          &["inherit", "piped", "null"],
        )),
      },
      Value::Number(val) => match val.as_u64() {
        Some(val) if val <= ResourceId::MAX as u64 => {
          Ok(StdioOrRid::Rid(val as ResourceId))
        }
        _ => Err(serde::de::Error::custom("Expected a positive integer")),
      },
      _ => Err(serde::de::Error::custom(
        r#"Expected a resource id, "inherit", "piped", or "null""#,
      )),
    }
  }
}

impl StdioOrRid {
  pub fn as_stdio(
    &self,
    state: &mut OpState,
  ) -> Result<std::process::Stdio, ProcessError> {
    match &self {
      StdioOrRid::Stdio(val) => Ok(val.as_stdio()),
      StdioOrRid::Rid(rid) => {
        Ok(FileResource::with_file(state, *rid, |file| {
          file.as_stdio().map_err(deno_error::JsErrorBox::from_err)
        })?)
      }
    }
  }

  pub fn is_ipc(&self) -> bool {
    matches!(self, StdioOrRid::Stdio(Stdio::IpcForInternalUse))
  }
}

#[allow(clippy::disallowed_types)]
pub type NpmProcessStateProviderRc =
  deno_fs::sync::MaybeArc<dyn NpmProcessStateProvider>;

pub trait NpmProcessStateProvider:
  std::fmt::Debug + deno_fs::sync::MaybeSend + deno_fs::sync::MaybeSync
{
  /// Gets a string containing the serialized npm state of the process.
  ///
  /// This will be set on the `DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE` environment
  /// variable when doing a `child_process.fork`. The implementor can then check this environment
  /// variable on startup to repopulate the internal npm state.
  fn get_npm_process_state(&self) -> String {
    // This method is only used in the CLI.
    String::new()
  }
}

#[derive(Debug)]
pub struct EmptyNpmProcessStateProvider;

impl NpmProcessStateProvider for EmptyNpmProcessStateProvider {}

deno_core::extension!(
  deno_process,
  ops = [
    op_spawn_child,
    op_spawn_wait,
    op_spawn_sync,
    op_spawn_kill,
    deprecated::op_run,
    deprecated::op_run_status,
    deprecated::op_kill,
  ],
  esm = ["40_process.js"],
  options = { get_npm_process_state: Option<NpmProcessStateProviderRc>  },
  state = |state, options| {
    state.put::<NpmProcessStateProviderRc>(options.get_npm_process_state.unwrap_or(deno_fs::sync::MaybeArc::new(EmptyNpmProcessStateProvider)));
  },
);

/// Second member stores the pid separately from the RefCell. It's needed for
/// `op_spawn_kill`, where the RefCell is borrowed mutably by `op_spawn_wait`.
struct ChildResource(RefCell<tokio::process::Child>, u32);

impl Resource for ChildResource {
  fn name(&self) -> Cow<str> {
    "child".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnArgs {
  cmd: String,
  args: Vec<String>,
  cwd: Option<String>,
  clear_env: bool,
  env: Vec<(String, String)>,
  #[cfg(unix)]
  gid: Option<u32>,
  #[cfg(unix)]
  uid: Option<u32>,
  #[cfg(windows)]
  windows_raw_arguments: bool,
  ipc: Option<i32>,

  #[serde(flatten)]
  stdio: ChildStdio,

  input: Option<JsBuffer>,

  extra_stdio: Vec<Stdio>,
  detached: bool,
  needs_npm_process_state: bool,
}

#[cfg(unix)]
deno_error::js_error_wrapper!(nix::Error, JsNixError, |err| {
  match err {
    nix::Error::ECHILD => "NotFound",
    nix::Error::EINVAL => "TypeError",
    nix::Error::ENOENT => "NotFound",
    nix::Error::ENOTTY => "BadResource",
    nix::Error::EPERM => "PermissionDenied",
    nix::Error::ESRCH => "NotFound",
    nix::Error::ELOOP => "FilesystemLoop",
    nix::Error::ENOTDIR => "NotADirectory",
    nix::Error::ENETUNREACH => "NetworkUnreachable",
    nix::Error::EISDIR => "IsADirectory",
    nix::Error::UnknownErrno => "Error",
    &nix::Error::ENOTSUP => unreachable!(),
    _ => "Error",
  }
});

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ProcessError {
  #[class(inherit)]
  #[error("Failed to spawn '{command}': {error}")]
  SpawnFailed {
    command: String,
    #[source]
    #[inherit]
    error: Box<ProcessError>,
  },
  #[class(inherit)]
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[cfg(unix)]
  #[class(inherit)]
  #[error(transparent)]
  Nix(JsNixError),
  #[class(inherit)]
  #[error("failed resolving cwd: {0}")]
  FailedResolvingCwd(#[source] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  RunPermission(#[from] CheckRunPermissionError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(deno_core::error::ResourceError),
  #[class(generic)]
  #[error(transparent)]
  BorrowMut(std::cell::BorrowMutError),
  #[class(generic)]
  #[error(transparent)]
  Which(which::Error),
  #[class(type)]
  #[error("Child process has already terminated.")]
  ChildProcessAlreadyTerminated,
  #[class(type)]
  #[error("Invalid pid")]
  InvalidPid,
  #[class(inherit)]
  #[error(transparent)]
  Signal(#[from] SignalError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
  #[class(type)]
  #[error("Missing cmd")]
  MissingCmd, // only for Deno.run
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildStdio {
  stdin: StdioOrRid,
  stdout: StdioOrRid,
  stderr: StdioOrRid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildStatus {
  success: bool,
  code: i32,
  signal: Option<String>,
}

impl TryFrom<ExitStatus> for ChildStatus {
  type Error = SignalError;

  fn try_from(status: ExitStatus) -> Result<Self, Self::Error> {
    let code = status.code();
    #[cfg(unix)]
    let signal = status.signal();
    #[cfg(not(unix))]
    let signal: Option<i32> = None;

    let status = if let Some(signal) = signal {
      ChildStatus {
        success: false,
        code: 128 + signal,
        #[cfg(unix)]
        signal: Some(deno_os::signal::signal_int_to_str(signal)?.to_string()),
        #[cfg(not(unix))]
        signal: None,
      }
    } else {
      let code = code.expect("Should have either an exit code or a signal.");

      ChildStatus {
        success: code == 0,
        code,
        signal: None,
      }
    };

    Ok(status)
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnOutput {
  status: ChildStatus,
  stdout: Option<ToJsBuffer>,
  stderr: Option<ToJsBuffer>,
}

type CreateCommand = (
  std::process::Command,
  Option<ResourceId>,
  Vec<Option<ResourceId>>,
  Vec<deno_io::RawBiPipeHandle>,
);

pub fn npm_process_state_tempfile(
  contents: &[u8],
) -> Result<deno_io::RawIoHandle, std::io::Error> {
  let mut temp_file = tempfile::tempfile()?;
  temp_file.write_all(contents)?;
  let handle = temp_file.into_raw_io_handle();
  #[cfg(windows)]
  {
    use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
    // make the handle inheritable
    // SAFETY: winapi call, handle is valid
    unsafe {
      windows_sys::Win32::Foundation::SetHandleInformation(
        handle as _,
        HANDLE_FLAG_INHERIT,
        HANDLE_FLAG_INHERIT,
      );
    }
    Ok(handle)
  }
  #[cfg(unix)]
  {
    // SAFETY: libc call, fd is valid
    let inheritable = unsafe {
      // duplicate the FD to get a new one that doesn't have the CLOEXEC flag set
      // so it can be inherited by the child process
      libc::dup(handle)
    };
    // SAFETY: libc call, fd is valid
    unsafe {
      // close the old one
      libc::close(handle);
    }
    Ok(inheritable)
  }
}

pub const NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME: &str =
  "DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE_FD";

fn create_command(
  state: &mut OpState,
  mut args: SpawnArgs,
  api_name: &str,
) -> Result<CreateCommand, ProcessError> {
  let maybe_npm_process_state = if args.needs_npm_process_state {
    let provider = state.borrow::<NpmProcessStateProviderRc>();
    let process_state = provider.get_npm_process_state();
    let fd = npm_process_state_tempfile(process_state.as_bytes())?;
    args.env.push((
      NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME.to_string(),
      (fd as usize).to_string(),
    ));
    Some(fd)
  } else {
    None
  };

  let (cmd, run_env) = compute_run_cmd_and_check_permissions(
    &args.cmd,
    args.cwd.as_deref(),
    &args.env,
    args.clear_env,
    state,
    api_name,
  )?;
  let mut command = std::process::Command::new(cmd);

  #[cfg(windows)]
  {
    if args.detached {
      // TODO(nathanwhit): Currently this causes the process to hang
      // until the detached process exits (so never). It repros with just the
      // rust std library, so it's either a bug or requires more control than we have.
      // To be resolved at the same time as additional stdio support.
      log::warn!("detached processes are not currently supported on Windows");
    }
    if args.windows_raw_arguments {
      for arg in args.args.iter() {
        command.raw_arg(arg);
      }
    } else {
      command.args(args.args);
    }
  }

  #[cfg(not(windows))]
  command.args(args.args);

  command.current_dir(run_env.cwd);
  command.env_clear();
  command.envs(run_env.envs.into_iter().map(|(k, v)| (k.into_inner(), v)));

  #[cfg(unix)]
  if let Some(gid) = args.gid {
    command.gid(gid);
  }
  #[cfg(unix)]
  if let Some(uid) = args.uid {
    command.uid(uid);
  }

  if args.stdio.stdin.is_ipc() {
    args.ipc = Some(0);
  } else if args.input.is_some() {
    command.stdin(std::process::Stdio::piped());
  } else {
    command.stdin(args.stdio.stdin.as_stdio(state)?);
  }

  command.stdout(match args.stdio.stdout {
    StdioOrRid::Stdio(Stdio::Inherit) => StdioOrRid::Rid(1).as_stdio(state)?,
    value => value.as_stdio(state)?,
  });
  command.stderr(match args.stdio.stderr {
    StdioOrRid::Stdio(Stdio::Inherit) => StdioOrRid::Rid(2).as_stdio(state)?,
    value => value.as_stdio(state)?,
  });

  #[cfg(unix)]
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    let mut extra_pipe_rids = Vec::new();
    let mut fds_to_dup = Vec::new();
    let mut fds_to_close = Vec::new();
    let mut ipc_rid = None;
    if let Some(fd) = maybe_npm_process_state {
      fds_to_close.push(fd);
    }
    if let Some(ipc) = args.ipc {
      if ipc >= 0 {
        let (ipc_fd1, ipc_fd2) = deno_io::bi_pipe_pair_raw()?;
        fds_to_dup.push((ipc_fd2, ipc));
        fds_to_close.push(ipc_fd2);
        /* One end returned to parent process (this) */
        let pipe_rid = state.resource_table.add(IpcJsonStreamResource::new(
          ipc_fd1 as _,
          IpcRefTracker::new(state.external_ops_tracker.clone()),
        )?);
        /* The other end passed to child process via NODE_CHANNEL_FD */
        command.env("NODE_CHANNEL_FD", format!("{}", ipc));
        ipc_rid = Some(pipe_rid);
      }
    }

    for (i, stdio) in args.extra_stdio.into_iter().enumerate() {
      // index 0 in `extra_stdio` actually refers to fd 3
      // because we handle stdin,stdout,stderr specially
      let fd = (i + 3) as i32;
      // TODO(nathanwhit): handle inherited, but this relies on the parent process having
      // fds open already. since we don't generally support dealing with raw fds,
      // we can't properly support this
      if matches!(stdio, Stdio::Piped) {
        let (fd1, fd2) = deno_io::bi_pipe_pair_raw()?;
        fds_to_dup.push((fd2, fd));
        fds_to_close.push(fd2);
        let rid = state.resource_table.add(
          match deno_io::BiPipeResource::from_raw_handle(fd1) {
            Ok(v) => v,
            Err(e) => {
              log::warn!("Failed to open bidirectional pipe for fd {fd}: {e}");
              extra_pipe_rids.push(None);
              continue;
            }
          },
        );
        extra_pipe_rids.push(Some(rid));
      } else {
        extra_pipe_rids.push(None);
      }
    }

    let detached = args.detached;
    command.pre_exec(move || {
      if detached {
        libc::setsid();
      }
      for &(src, dst) in &fds_to_dup {
        if src >= 0 && dst >= 0 {
          let _fd = libc::dup2(src, dst);
          libc::close(src);
        }
      }
      libc::setgroups(0, std::ptr::null());
      Ok(())
    });

    Ok((command, ipc_rid, extra_pipe_rids, fds_to_close))
  }

  #[cfg(windows)]
  {
    let mut ipc_rid = None;
    let mut handles_to_close = Vec::with_capacity(1);
    if let Some(handle) = maybe_npm_process_state {
      handles_to_close.push(handle);
    }
    if let Some(ipc) = args.ipc {
      if ipc >= 0 {
        let (hd1, hd2) = deno_io::bi_pipe_pair_raw()?;

        /* One end returned to parent process (this) */
        let pipe_rid =
          Some(state.resource_table.add(IpcJsonStreamResource::new(
            hd1 as i64,
            IpcRefTracker::new(state.external_ops_tracker.clone()),
          )?));

        /* The other end passed to child process via NODE_CHANNEL_FD */
        command.env("NODE_CHANNEL_FD", format!("{}", hd2 as i64));

        handles_to_close.push(hd2);

        ipc_rid = pipe_rid;
      }
    }

    if args.extra_stdio.iter().any(|s| matches!(s, Stdio::Piped)) {
      log::warn!(
        "Additional stdio pipes beyond stdin/stdout/stderr are not currently supported on windows"
      );
    }

    Ok((command, ipc_rid, vec![], handles_to_close))
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Child {
  rid: ResourceId,
  pid: u32,
  stdin_rid: Option<ResourceId>,
  stdout_rid: Option<ResourceId>,
  stderr_rid: Option<ResourceId>,
  ipc_pipe_rid: Option<ResourceId>,
  extra_pipe_rids: Vec<Option<ResourceId>>,
}

fn spawn_child(
  state: &mut OpState,
  command: std::process::Command,
  ipc_pipe_rid: Option<ResourceId>,
  extra_pipe_rids: Vec<Option<ResourceId>>,
  detached: bool,
) -> Result<Child, ProcessError> {
  let mut command = tokio::process::Command::from(command);
  // TODO(@crowlkats): allow detaching processes.
  //  currently deno will orphan a process when exiting with an error or Deno.exit()
  // We want to kill child when it's closed
  if !detached {
    command.kill_on_drop(true);
  }

  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(err) => {
      let command = command.as_std();
      let command_name = command.get_program().to_string_lossy();

      if let Some(cwd) = command.get_current_dir() {
        // launching a sub process always depends on the real
        // file system so using these methods directly is ok
        #[allow(clippy::disallowed_methods)]
        if !cwd.exists() {
          return Err(
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              format!(
                "Failed to spawn '{}': No such cwd '{}'",
                command_name,
                cwd.to_string_lossy()
              ),
            )
            .into(),
          );
        }

        #[allow(clippy::disallowed_methods)]
        if !cwd.is_dir() {
          return Err(
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              format!(
                "Failed to spawn '{}': cwd is not a directory '{}'",
                command_name,
                cwd.to_string_lossy()
              ),
            )
            .into(),
          );
        }
      }

      return Err(ProcessError::SpawnFailed {
        command: command.get_program().to_string_lossy().to_string(),
        error: Box::new(err.into()),
      });
    }
  };

  let pid = child.id().expect("Process ID should be set.");

  let stdin_rid = child
    .stdin
    .take()
    .map(|stdin| state.resource_table.add(ChildStdinResource::from(stdin)));

  let stdout_rid = child
    .stdout
    .take()
    .map(|stdout| state.resource_table.add(ChildStdoutResource::from(stdout)));

  let stderr_rid = child
    .stderr
    .take()
    .map(|stderr| state.resource_table.add(ChildStderrResource::from(stderr)));

  let child_rid = state
    .resource_table
    .add(ChildResource(RefCell::new(child), pid));

  Ok(Child {
    rid: child_rid,
    pid,
    stdin_rid,
    stdout_rid,
    stderr_rid,
    ipc_pipe_rid,
    extra_pipe_rids,
  })
}

fn compute_run_cmd_and_check_permissions(
  arg_cmd: &str,
  arg_cwd: Option<&str>,
  arg_envs: &[(String, String)],
  arg_clear_env: bool,
  state: &mut OpState,
  api_name: &str,
) -> Result<(PathBuf, RunEnv), ProcessError> {
  let run_env =
    compute_run_env(arg_cwd, arg_envs, arg_clear_env).map_err(|e| {
      ProcessError::SpawnFailed {
        command: arg_cmd.to_string(),
        error: Box::new(e),
      }
    })?;
  let cmd =
    resolve_cmd(arg_cmd, &run_env).map_err(|e| ProcessError::SpawnFailed {
      command: arg_cmd.to_string(),
      error: Box::new(e),
    })?;
  check_run_permission(
    state,
    &RunQueryDescriptor::Path {
      requested: arg_cmd.to_string(),
      resolved: cmd.clone(),
    },
    &run_env,
    api_name,
  )?;
  Ok((cmd, run_env))
}

#[derive(Debug)]
struct EnvVarKey {
  inner: OsString,
  // Windows treats env vars as case insensitive, so use
  // a normalized value for comparisons instead of the raw
  // case sensitive value
  #[cfg(windows)]
  normalized: OsString,
}

impl EnvVarKey {
  pub fn new(value: OsString) -> Self {
    Self {
      #[cfg(windows)]
      normalized: value.to_ascii_uppercase(),
      inner: value,
    }
  }

  pub fn from_str(value: &str) -> Self {
    Self::new(OsString::from(value))
  }

  pub fn into_inner(self) -> OsString {
    self.inner
  }

  pub fn comparison_value(&self) -> &OsString {
    #[cfg(windows)]
    {
      &self.normalized
    }
    #[cfg(not(windows))]
    {
      &self.inner
    }
  }
}

impl std::hash::Hash for EnvVarKey {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.comparison_value().hash(state);
  }
}

impl std::cmp::Eq for EnvVarKey {}
impl std::cmp::PartialEq for EnvVarKey {
  fn eq(&self, other: &Self) -> bool {
    self.comparison_value() == other.comparison_value()
  }
}

struct RunEnv {
  envs: HashMap<EnvVarKey, OsString>,
  cwd: PathBuf,
}

/// Computes the current environment, which will then be used to inform
/// permissions and finally spawning. This is very important to compute
/// ahead of time so that the environment used to verify permissions is
/// the same environment used to spawn the sub command. This protects against
/// someone doing timing attacks by changing the environment on a worker.
fn compute_run_env(
  arg_cwd: Option<&str>,
  arg_envs: &[(String, String)],
  arg_clear_env: bool,
) -> Result<RunEnv, ProcessError> {
  #[allow(clippy::disallowed_methods)]
  let cwd =
    std::env::current_dir().map_err(ProcessError::FailedResolvingCwd)?;
  let cwd = arg_cwd
    .map(|cwd_arg| resolve_path(cwd_arg, &cwd))
    .unwrap_or(cwd);
  let envs = if arg_clear_env {
    arg_envs
      .iter()
      .map(|(k, v)| (EnvVarKey::from_str(k), OsString::from(v)))
      .collect()
  } else {
    let mut envs = std::env::vars_os()
      .map(|(k, v)| (EnvVarKey::new(k), v))
      .collect::<HashMap<_, _>>();
    for (key, value) in arg_envs {
      envs.insert(EnvVarKey::from_str(key), OsString::from(value));
    }
    envs
  };
  Ok(RunEnv { envs, cwd })
}

fn resolve_cmd(cmd: &str, env: &RunEnv) -> Result<PathBuf, ProcessError> {
  let is_path = cmd.contains('/');
  #[cfg(windows)]
  let is_path = is_path || cmd.contains('\\') || Path::new(&cmd).is_absolute();
  if is_path {
    Ok(resolve_path(cmd, &env.cwd))
  } else {
    let path = env.envs.get(&EnvVarKey::new(OsString::from("PATH")));
    match which::which_in(cmd, path, &env.cwd) {
      Ok(cmd) => Ok(cmd),
      Err(which::Error::CannotFindBinaryPath) => {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
      }
      Err(err) => Err(ProcessError::Which(err)),
    }
  }
}

fn resolve_path(path: &str, cwd: &Path) -> PathBuf {
  deno_path_util::normalize_path(cwd.join(path))
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CheckRunPermissionError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error("{0}")]
  Other(JsErrorBox),
}

fn check_run_permission(
  state: &mut OpState,
  cmd: &RunQueryDescriptor,
  run_env: &RunEnv,
  api_name: &str,
) -> Result<(), CheckRunPermissionError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();
  if !permissions.query_run_all(api_name) {
    // error the same on all platforms
    let env_var_names = get_requires_allow_all_env_vars(run_env);
    if !env_var_names.is_empty() {
      // we don't allow users to launch subprocesses with any LD_ or DYLD_*
      // env vars set because this allows executing code (ex. LD_PRELOAD)
      return Err(CheckRunPermissionError::Other(
        JsErrorBox::new(
          "NotCapable",
          format!(
            "Requires --allow-run permissions to spawn subprocess with {0} environment variable{1}. Alternatively, spawn with {2} environment variable{1} unset.",
            env_var_names.join(", "),
            if env_var_names.len() != 1 { "s" } else { "" },
            if env_var_names.len() != 1 { "these" } else { "the" }
          ),
        ),
      ));
    }
    permissions.check_run(cmd, api_name)?;
  }
  Ok(())
}

fn get_requires_allow_all_env_vars(env: &RunEnv) -> Vec<&str> {
  fn requires_allow_all(key: &str) -> bool {
    fn starts_with_ignore_case(key: &str, search_value: &str) -> bool {
      if let Some((key, _)) = key.split_at_checked(search_value.len()) {
        search_value.eq_ignore_ascii_case(key)
      } else {
        false
      }
    }

    let key = key.trim();
    // we could be more targted here, but there are quite a lot of
    // LD_* and DYLD_* env variables
    starts_with_ignore_case(key, "LD_") || starts_with_ignore_case(key, "DYLD_")
  }

  fn is_empty(value: &OsString) -> bool {
    value.is_empty()
      || value.to_str().map(|v| v.trim().is_empty()).unwrap_or(false)
  }

  let mut found_envs = env
    .envs
    .iter()
    .filter_map(|(k, v)| {
      let key = k.comparison_value().to_str()?;
      if requires_allow_all(key) && !is_empty(v) {
        Some(key)
      } else {
        None
      }
    })
    .collect::<Vec<_>>();
  found_envs.sort();
  found_envs
}

#[op2(stack_trace)]
#[serde]
fn op_spawn_child(
  state: &mut OpState,
  #[serde] args: SpawnArgs,
  #[string] api_name: String,
) -> Result<Child, ProcessError> {
  let detached = args.detached;
  let (command, pipe_rid, extra_pipe_rids, handles_to_close) =
    create_command(state, args, &api_name)?;
  let child = spawn_child(state, command, pipe_rid, extra_pipe_rids, detached);
  for handle in handles_to_close {
    deno_io::close_raw_handle(handle);
  }
  child
}

#[op2(async)]
#[allow(clippy::await_holding_refcell_ref)]
#[serde]
async fn op_spawn_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<ChildStatus, ProcessError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ChildResource>(rid)
    .map_err(ProcessError::Resource)?;
  let result = resource
    .0
    .try_borrow_mut()
    .map_err(ProcessError::BorrowMut)?
    .wait()
    .await?
    .try_into()?;
  if let Ok(resource) = state.borrow_mut().resource_table.take_any(rid) {
    resource.close();
  }
  Ok(result)
}

#[op2(stack_trace)]
#[serde]
fn op_spawn_sync(
  state: &mut OpState,
  #[serde] args: SpawnArgs,
) -> Result<SpawnOutput, ProcessError> {
  let stdout = matches!(args.stdio.stdout, StdioOrRid::Stdio(Stdio::Piped));
  let stderr = matches!(args.stdio.stderr, StdioOrRid::Stdio(Stdio::Piped));
  let input = args.input.clone();
  let (mut command, _, _, _) =
    create_command(state, args, "Deno.Command().outputSync()")?;

  let mut child = command.spawn().map_err(|e| ProcessError::SpawnFailed {
    command: command.get_program().to_string_lossy().to_string(),
    error: Box::new(e.into()),
  })?;
  if let Some(input) = input {
    let mut stdin = child.stdin.take().ok_or_else(|| {
      ProcessError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        "stdin is not available",
      ))
    })?;
    stdin.write_all(&input)?;
    stdin.flush()?;
  }
  let output =
    child
      .wait_with_output()
      .map_err(|e| ProcessError::SpawnFailed {
        command: command.get_program().to_string_lossy().to_string(),
        error: Box::new(e.into()),
      })?;
  Ok(SpawnOutput {
    status: output.status.try_into()?,
    stdout: if stdout {
      Some(output.stdout.into())
    } else {
      None
    },
    stderr: if stderr {
      Some(output.stderr.into())
    } else {
      None
    },
  })
}

#[op2(fast)]
fn op_spawn_kill(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] signal: String,
) -> Result<(), ProcessError> {
  if let Ok(child_resource) = state.resource_table.get::<ChildResource>(rid) {
    deprecated::kill(child_resource.1 as i32, &signal)?;
    return Ok(());
  }
  Err(ProcessError::ChildProcessAlreadyTerminated)
}

mod deprecated {
  use super::*;

  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct RunArgs {
    cmd: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    stdin: StdioOrRid,
    stdout: StdioOrRid,
    stderr: StdioOrRid,
  }

  struct ChildResource {
    child: AsyncRefCell<tokio::process::Child>,
  }

  impl Resource for ChildResource {
    fn name(&self) -> Cow<str> {
      "child".into()
    }
  }

  impl ChildResource {
    fn borrow_mut(self: Rc<Self>) -> AsyncMutFuture<tokio::process::Child> {
      RcRef::map(self, |r| &r.child).borrow_mut()
    }
  }

  #[derive(Serialize)]
  #[serde(rename_all = "camelCase")]
  // TODO(@AaronO): maybe find a more descriptive name or a convention for return structs
  pub struct RunInfo {
    rid: ResourceId,
    pid: Option<u32>,
    stdin_rid: Option<ResourceId>,
    stdout_rid: Option<ResourceId>,
    stderr_rid: Option<ResourceId>,
  }

  #[op2(stack_trace)]
  #[serde]
  pub fn op_run(
    state: &mut OpState,
    #[serde] run_args: RunArgs,
  ) -> Result<RunInfo, ProcessError> {
    let args = run_args.cmd;
    let cmd = args.first().ok_or(ProcessError::MissingCmd)?;
    let (cmd, run_env) = compute_run_cmd_and_check_permissions(
      cmd,
      run_args.cwd.as_deref(),
      &run_args.env,
      /* clear env */ false,
      state,
      "Deno.run()",
    )?;

    let mut c = Command::new(cmd);
    for arg in args.iter().skip(1) {
      c.arg(arg);
    }
    c.current_dir(run_env.cwd);

    c.env_clear();
    for (key, value) in run_env.envs {
      c.env(key.inner, value);
    }

    #[cfg(unix)]
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      c.pre_exec(|| {
        libc::setgroups(0, std::ptr::null());
        Ok(())
      });
    }

    // TODO: make this work with other resources, eg. sockets
    c.stdin(run_args.stdin.as_stdio(state)?);
    c.stdout(
      match run_args.stdout {
        StdioOrRid::Stdio(Stdio::Inherit) => StdioOrRid::Rid(1),
        value => value,
      }
      .as_stdio(state)?,
    );
    c.stderr(
      match run_args.stderr {
        StdioOrRid::Stdio(Stdio::Inherit) => StdioOrRid::Rid(2),
        value => value,
      }
      .as_stdio(state)?,
    );

    // We want to kill child when it's closed
    c.kill_on_drop(true);

    // Spawn the command.
    let mut child = c.spawn()?;
    let pid = child.id();

    let stdin_rid = match child.stdin.take() {
      Some(child_stdin) => {
        let rid = state
          .resource_table
          .add(ChildStdinResource::from(child_stdin));
        Some(rid)
      }
      None => None,
    };

    let stdout_rid = match child.stdout.take() {
      Some(child_stdout) => {
        let rid = state
          .resource_table
          .add(ChildStdoutResource::from(child_stdout));
        Some(rid)
      }
      None => None,
    };

    let stderr_rid = match child.stderr.take() {
      Some(child_stderr) => {
        let rid = state
          .resource_table
          .add(ChildStderrResource::from(child_stderr));
        Some(rid)
      }
      None => None,
    };

    let child_resource = ChildResource {
      child: AsyncRefCell::new(child),
    };
    let child_rid = state.resource_table.add(child_resource);

    Ok(RunInfo {
      rid: child_rid,
      pid,
      stdin_rid,
      stdout_rid,
      stderr_rid,
    })
  }

  #[derive(Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct ProcessStatus {
    got_signal: bool,
    exit_code: i32,
    exit_signal: i32,
  }

  #[op2(async)]
  #[serde]
  pub async fn op_run_status(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
  ) -> Result<ProcessStatus, ProcessError> {
    let resource = state
      .borrow_mut()
      .resource_table
      .get::<ChildResource>(rid)
      .map_err(ProcessError::Resource)?;
    let mut child = resource.borrow_mut().await;
    let run_status = child.wait().await?;
    let code = run_status.code();

    #[cfg(unix)]
    let signal = run_status.signal();
    #[cfg(not(unix))]
    let signal = Default::default();

    code
      .or(signal)
      .expect("Should have either an exit code or a signal.");
    let got_signal = signal.is_some();

    Ok(ProcessStatus {
      got_signal,
      exit_code: code.unwrap_or(-1),
      exit_signal: signal.unwrap_or(-1),
    })
  }

  #[cfg(unix)]
  pub fn kill(pid: i32, signal: &str) -> Result<(), ProcessError> {
    let signo = deno_os::signal::signal_str_to_int(signal)
      .map_err(SignalError::InvalidSignalStr)?;
    use nix::sys::signal::kill as unix_kill;
    use nix::sys::signal::Signal;
    use nix::unistd::Pid;
    let sig =
      Signal::try_from(signo).map_err(|e| ProcessError::Nix(JsNixError(e)))?;
    unix_kill(Pid::from_raw(pid), Some(sig))
      .map_err(|e| ProcessError::Nix(JsNixError(e)))
  }

  #[cfg(not(unix))]
  pub fn kill(pid: i32, signal: &str) -> Result<(), ProcessError> {
    use std::io::Error;
    use std::io::ErrorKind::NotFound;

    use winapi::shared::minwindef::DWORD;
    use winapi::shared::minwindef::FALSE;
    use winapi::shared::minwindef::TRUE;
    use winapi::shared::winerror::ERROR_INVALID_PARAMETER;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::processthreadsapi::TerminateProcess;
    use winapi::um::winnt::PROCESS_TERMINATE;

    if !matches!(signal, "SIGKILL" | "SIGTERM") {
      Err(
        SignalError::InvalidSignalStr(deno_os::signal::InvalidSignalStrError(
          signal.to_string(),
        ))
        .into(),
      )
    } else if pid <= 0 {
      Err(ProcessError::InvalidPid)
    } else {
      let handle =
        // SAFETY: winapi call
        unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, pid as DWORD) };

      if handle.is_null() {
        // SAFETY: winapi call
        let err = match unsafe { GetLastError() } {
          ERROR_INVALID_PARAMETER => Error::from(NotFound), // Invalid `pid`.
          errno => Error::from_raw_os_error(errno as i32),
        };
        Err(err.into())
      } else {
        // SAFETY: winapi calls
        unsafe {
          let is_terminated = TerminateProcess(handle, 1);
          CloseHandle(handle);
          match is_terminated {
            FALSE => Err(Error::last_os_error().into()),
            TRUE => Ok(()),
            _ => unreachable!(),
          }
        }
      }
    }
  }

  #[op2(fast, stack_trace)]
  pub fn op_kill(
    state: &mut OpState,
    #[smi] pid: i32,
    #[string] signal: String,
    #[string] api_name: String,
  ) -> Result<(), ProcessError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_run_all(&api_name)?;
    kill(pid, &signal)
  }
}
