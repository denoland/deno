// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::io::FromRawFd;
#[cfg(unix)]
use std::os::unix::prelude::ExitStatusExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;
use std::process::ExitStatus;
#[cfg(unix)]
use std::process::Stdio as StdStdio;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_io::ChildProcessStdio;
use deno_io::ChildStderrResource;
use deno_io::ChildStdinResource;
use deno_io::ChildStdoutResource;
use deno_io::IntoRawIoHandle;
use deno_os::SignalError;
use deno_permissions::PathQueryDescriptor;
use deno_permissions::PermissionsContainer;
use deno_permissions::RunQueryDescriptor;
#[cfg(windows)]
use deno_subprocess_windows::Child as AsyncChild;
#[cfg(windows)]
use deno_subprocess_windows::Command;
#[cfg(windows)]
use deno_subprocess_windows::Stdio as StdStdio;
use serde::Deserialize;
#[cfg(unix)]
use tokio::process::Child as AsyncChild;

pub mod ipc;
use ipc::IpcAdvancedStreamResource;
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
  pub fn as_stdio(&self) -> StdStdio {
    match &self {
      Stdio::Inherit => StdStdio::inherit(),
      Stdio::Piped => StdStdio::piped(),
      Stdio::Null => StdStdio::null(),
      // IPC uses a pipe internally
      Stdio::IpcForInternalUse => StdStdio::piped(),
    }
  }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum StdioOrFd {
  Stdio(Stdio),
  Fd(i32),
}

impl<'de> Deserialize<'de> for StdioOrFd {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use serde_json::Value;
    let value = Value::deserialize(deserializer)?;
    match value {
      Value::String(val) => match val.as_str() {
        "inherit" => Ok(StdioOrFd::Stdio(Stdio::Inherit)),
        "piped" => Ok(StdioOrFd::Stdio(Stdio::Piped)),
        "null" => Ok(StdioOrFd::Stdio(Stdio::Null)),
        "ipc_for_internal_use" => {
          Ok(StdioOrFd::Stdio(Stdio::IpcForInternalUse))
        }
        val => Err(serde::de::Error::unknown_variant(
          val,
          &["inherit", "piped", "null"],
        )),
      },
      Value::Number(val) => match val.as_i64() {
        Some(val) if val >= 0 && val <= i32::MAX as i64 => {
          Ok(StdioOrFd::Fd(val as i32))
        }
        _ => Err(serde::de::Error::custom(
          "Expected a non-negative integer file descriptor",
        )),
      },
      _ => Err(serde::de::Error::custom(
        r#"Expected a file descriptor, "inherit", "piped", or "null""#,
      )),
    }
  }
}

impl StdioOrFd {
  pub fn as_stdio(&self) -> Result<StdStdio, ProcessError> {
    match &self {
      StdioOrFd::Stdio(val) => Ok(val.as_stdio()),
      StdioOrFd::Fd(fd) => {
        #[cfg(unix)]
        {
          // Safety: we dup the fd so the original remains open for the caller
          let new_fd = unsafe { libc::dup(*fd) };
          if new_fd < 0 {
            return Err(ProcessError::Io(std::io::Error::last_os_error()));
          }
          // Safety: new_fd is a valid, freshly duplicated file descriptor
          Ok(unsafe {
            StdStdio::from(std::os::unix::io::OwnedFd::from_raw_fd(new_fd))
          })
        }
        #[cfg(windows)]
        {
          // SAFETY: *fd is a valid CRT file descriptor obtained from fs.openSync
          let handle = unsafe { libc::get_osfhandle(*fd as _) };
          if handle == -1 {
            return Err(ProcessError::Io(std::io::Error::last_os_error()));
          }
          // SAFETY: handle is a valid OS handle returned by get_osfhandle (checked above)
          let borrowed = unsafe {
            std::os::windows::io::BorrowedHandle::borrow_raw(
              handle as std::os::windows::io::RawHandle,
            )
          };
          let owned =
            borrowed.try_clone_to_owned().map_err(ProcessError::Io)?;
          Ok(StdStdio::from(owned))
        }
      }
    }
  }

  pub fn is_ipc(&self) -> bool {
    matches!(self, StdioOrFd::Stdio(Stdio::IpcForInternalUse))
  }
}

#[allow(clippy::disallowed_types, reason = "definition")]
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
    op_spawn_child_ref,
    op_spawn_child_unref,
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

/// Wraps an async child process handle.
///
/// `pid` is stored separately from the `RefCell` because it's needed for
/// `op_spawn_kill`, where the `RefCell` is borrowed mutably by `op_spawn_wait`.
///
/// `kill_on_drop` controls whether the child process is killed when this
/// resource is dropped (e.g. when the parent process exits). It defaults to
/// `true` for non-detached processes. Calling `unref()` sets it to `false`,
/// allowing the child to outlive the parent — matching Node.js semantics.
struct ChildResource {
  child: RefCell<AsyncChild>,
  pid: u32,
  kill_on_drop: Cell<bool>,
}

impl Resource for ChildResource {
  fn name(&self) -> Cow<'_, str> {
    "child".into()
  }
}

impl Drop for ChildResource {
  fn drop(&mut self) {
    if self.kill_on_drop.get() {
      #[cfg(unix)]
      {
        // Send SIGKILL to the child process. Best-effort; ignore errors
        // (e.g. the process may have already exited).
        // SAFETY: libc::kill is safe to call with any pid/signal combination;
        // it simply returns an error for invalid inputs.
        unsafe {
          libc::kill(self.pid as i32, libc::SIGKILL);
        }
      }
      #[cfg(windows)]
      {
        let _ = deno_subprocess_windows::process_kill(
          self.pid as i32,
          /* SIGTERM */ 15,
        );
      }
    }
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

  serialization: Option<ChildIpcSerialization>,

  #[serde(flatten)]
  stdio: ChildStdio,

  input: Option<JsBuffer>,

  extra_stdio: Vec<StdioOrFd>,
  detached: bool,
  needs_npm_process_state: bool,
  #[cfg(unix)]
  argv0: Option<String>,

  #[serde(default)]
  timeout: Option<u64>,
  #[cfg(unix)]
  #[serde(default)]
  #[cfg_attr(
    windows,
    allow(dead_code, reason = "deserialized from JS but only used on Unix")
  )]
  kill_signal: Option<KillSignal>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[cfg_attr(
  windows,
  allow(dead_code, reason = "deserialized from JS but only used on Unix")
)]
enum KillSignal {
  String(String),
  Number(i32),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChildIpcSerialization {
  Json,
  Advanced,
}

impl std::fmt::Display for ChildIpcSerialization {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        ChildIpcSerialization::Json => "json",
        ChildIpcSerialization::Advanced => "advanced",
      }
    )
  }
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
  Which(deno_permissions::which::Error),
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
  stdin: StdioOrFd,
  stdout: StdioOrFd,
  stderr: StdioOrFd,
}

#[derive(ToV8)]
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
        signal: Some(deno_signals::signal_int_to_str(signal)?.to_string()),
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

#[derive(ToV8)]
pub struct SpawnOutput {
  pid: u32,
  status: ChildStatus,
  stdout: Option<Uint8Array>,
  stderr: Option<Uint8Array>,
  killed_by_timeout: bool,
}

type CreateCommand = (
  Command,
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
  let mut command = Command::new(cmd);

  #[cfg(windows)]
  {
    if args.detached {
      command.detached();
    }

    if args.windows_raw_arguments {
      command.verbatim_arguments(true);
    }
    command.args(args.args);
  }

  #[cfg(not(windows))]
  {
    if let Some(ref argv0) = args.argv0 {
      command.arg0(argv0);
    }
    command.args(args.args);
  }

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
    command.stdin(StdStdio::piped());
  } else {
    command.stdin(args.stdio.stdin.as_stdio()?);
  }

  command.stdout(match args.stdio.stdout {
    StdioOrFd::Stdio(Stdio::Inherit) => {
      let cs = state.borrow::<ChildProcessStdio>();
      StdStdio::from(cs.stdout.try_clone().map_err(ProcessError::Io)?)
    }
    value => value.as_stdio()?,
  });
  command.stderr(match args.stdio.stderr {
    StdioOrFd::Stdio(Stdio::Inherit) => {
      let cs = state.borrow::<ChildProcessStdio>();
      StdStdio::from(cs.stderr.try_clone().map_err(ProcessError::Io)?)
    }
    value => value.as_stdio()?,
  });

  #[cfg(unix)]
  // TODO(bartlomieju):
  #[allow(
    clippy::undocumented_unsafe_blocks,
    reason = "TODO: add safety comment"
  )]
  unsafe {
    let mut extra_pipe_rids = Vec::new();
    let mut fds_to_dup = Vec::new();
    let mut fds_to_close = Vec::new();
    let mut ipc_rid = None;
    if let Some(fd) = maybe_npm_process_state {
      fds_to_close.push(fd);
    }
    if let Some(ipc) = args.ipc
      && ipc >= 0
    {
      let (ipc_fd1, ipc_fd2) = deno_io::bi_pipe_pair_raw()?;
      fds_to_dup.push((ipc_fd2, ipc));
      fds_to_close.push(ipc_fd2);
      /* One end returned to parent process (this) */
      let pipe_rid = match args.serialization {
        Some(ChildIpcSerialization::Json) | None => {
          state.resource_table.add(IpcJsonStreamResource::new(
            ipc_fd1 as _,
            IpcRefTracker::new(state.external_ops_tracker.clone()),
          )?)
        }
        Some(ChildIpcSerialization::Advanced) => {
          state.resource_table.add(IpcAdvancedStreamResource::new(
            ipc_fd1 as _,
            IpcRefTracker::new(state.external_ops_tracker.clone()),
          )?)
        }
      };

      /* The other end passed to child process via NODE_CHANNEL_FD */
      command.env("NODE_CHANNEL_FD", format!("{}", ipc));
      command.env(
        "NODE_CHANNEL_SERIALIZATION_MODE",
        args
          .serialization
          .unwrap_or(ChildIpcSerialization::Json)
          .to_string(),
      );
      ipc_rid = Some(pipe_rid);
    }

    for (i, stdio) in args.extra_stdio.into_iter().enumerate() {
      // index 0 in `extra_stdio` actually refers to fd 3
      // because we handle stdin,stdout,stderr specially
      let target_fd = (i + 3) as i32;
      match stdio {
        StdioOrFd::Stdio(Stdio::Piped) => {
          let (fd1, fd2) = deno_io::bi_pipe_pair_raw()?;
          fds_to_dup.push((fd2, target_fd));
          fds_to_close.push(fd2);
          let rid = state.resource_table.add(
            match deno_io::BiPipeResource::from_raw_handle(fd1) {
              Ok(v) => v,
              Err(e) => {
                log::warn!(
                  "Failed to open bidirectional pipe for fd {target_fd}: {e}"
                );
                extra_pipe_rids.push(None);
                continue;
              }
            },
          );
          extra_pipe_rids.push(Some(rid));
        }
        StdioOrFd::Fd(fd) => {
          // Dup the caller's fd onto the target fd slot in the child
          fds_to_dup.push((fd, target_fd));
          extra_pipe_rids.push(None);
        }
        _ => {
          extra_pipe_rids.push(None);
        }
      }
    }

    let detached = args.detached;
    if detached || !fds_to_dup.is_empty() || args.gid.is_some() {
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
    }

    Ok((command, ipc_rid, extra_pipe_rids, fds_to_close))
  }

  #[cfg(windows)]
  {
    let mut extra_pipe_rids = Vec::with_capacity(args.extra_stdio.len());

    let mut ipc_rid = None;
    let mut handles_to_close = Vec::with_capacity(1);
    if let Some(handle) = maybe_npm_process_state {
      handles_to_close.push(handle);
    }
    if let Some(ipc) = args.ipc
      && ipc >= 0
    {
      let (hd1, hd2) = deno_io::bi_pipe_pair_raw()?;

      /* One end returned to parent process (this) */
      let pipe_rid = match args.serialization {
        Some(ChildIpcSerialization::Json) | None => {
          state.resource_table.add(IpcJsonStreamResource::new(
            hd1 as _,
            IpcRefTracker::new(state.external_ops_tracker.clone()),
          )?)
        }
        Some(ChildIpcSerialization::Advanced) => {
          state.resource_table.add(IpcAdvancedStreamResource::new(
            hd1 as _,
            IpcRefTracker::new(state.external_ops_tracker.clone()),
          )?)
        }
      };

      /* The other end passed to child process via NODE_CHANNEL_FD */
      command.env("NODE_CHANNEL_FD", format!("{}", hd2 as i64));
      command.env(
        "NODE_CHANNEL_SERIALIZATION_MODE",
        args
          .serialization
          .unwrap_or(ChildIpcSerialization::Json)
          .to_string(),
      );

      handles_to_close.push(hd2);

      ipc_rid = Some(pipe_rid);
    }

    for (i, stdio) in args.extra_stdio.into_iter().enumerate() {
      // index 0 in `extra_stdio` actually refers to fd 3
      // because we handle stdin,stdout,stderr specially
      let target_fd = (i + 3) as i32;
      match stdio {
        StdioOrFd::Stdio(Stdio::Piped) => {
          let (fd1, fd2) = deno_io::bi_pipe_pair_raw()?;
          handles_to_close.push(fd2);
          let rid = state.resource_table.add(
            match deno_io::BiPipeResource::from_raw_handle(fd1) {
              Ok(v) => v,
              Err(e) => {
                log::warn!(
                  "Failed to open bidirectional pipe for fd {target_fd}: {e}"
                );
                extra_pipe_rids.push(None);
                continue;
              }
            },
          );
          command.extra_handle(Some(fd2));
          extra_pipe_rids.push(Some(rid));
        }
        StdioOrFd::Fd(fd) => {
          // SAFETY: fd is a valid CRT file descriptor passed from the JS stdio array
          let handle = unsafe { libc::get_osfhandle(fd as _) };
          if handle == -1 {
            return Err(ProcessError::Io(std::io::Error::last_os_error()));
          }
          command.extra_handle(Some(handle as _));
          extra_pipe_rids.push(None);
        }
        _ => {
          // no handle, push an empty handle so we get the right fds for following handles
          command.extra_handle(None);
          extra_pipe_rids.push(None);
        }
      }
    }

    Ok((command, ipc_rid, extra_pipe_rids, handles_to_close))
  }
}

#[derive(ToV8)]
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
  command: Command,
  ipc_pipe_rid: Option<ResourceId>,
  extra_pipe_rids: Vec<Option<ResourceId>>,
  detached: bool,
) -> Result<Child, ProcessError> {
  #[cfg(windows)]
  let mut command = command;
  #[cfg(not(windows))]
  let mut command = tokio::process::Command::from(command);
  // Note: we do NOT set `command.kill_on_drop(true)` here. Instead,
  // `ChildResource` implements its own `Drop` that kills the child process
  // by PID when `kill_on_drop` is true. This allows `unref()` to disable
  // kill-on-drop, so the child can outlive the parent (matching Node.js
  // semantics for `child_process.unref()`).

  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(err) => {
      #[cfg(not(windows))]
      let command = command.as_std();
      let command_name = command.get_program().to_string_lossy();

      if let Some(cwd) = command.get_current_dir() {
        // launching a sub process always depends on the real
        // file system so using these methods directly is ok
        #[allow(clippy::disallowed_methods, reason = "requires real fs")]
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

        #[allow(clippy::disallowed_methods, reason = "requires real fs")]
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
        command: command.get_program().to_string_lossy().into_owned(),
        error: Box::new(err.into()),
      });
    }
  };

  let pid = child.id().expect("Process ID should be set.");

  #[cfg(not(windows))]
  let stdin_rid = child
    .stdin
    .take()
    .map(|stdin| state.resource_table.add(ChildStdinResource::from(stdin)));

  #[cfg(windows)]
  let stdin_rid = child
    .stdin
    .take()
    .map(tokio::process::ChildStdin::from_std)
    .transpose()?
    .map(|stdin| state.resource_table.add(ChildStdinResource::from(stdin)));

  #[cfg(not(windows))]
  let stdout_rid = child
    .stdout
    .take()
    .map(|stdout| state.resource_table.add(ChildStdoutResource::from(stdout)));

  #[cfg(windows)]
  let stdout_rid = child
    .stdout
    .take()
    .map(tokio::process::ChildStdout::from_std)
    .transpose()?
    .map(|stdout| state.resource_table.add(ChildStdoutResource::from(stdout)));

  #[cfg(not(windows))]
  let stderr_rid = child
    .stderr
    .take()
    .map(|stderr| state.resource_table.add(ChildStderrResource::from(stderr)));

  #[cfg(windows)]
  let stderr_rid = child
    .stderr
    .take()
    .map(tokio::process::ChildStderr::from_std)
    .transpose()?
    .map(|stderr| state.resource_table.add(ChildStderrResource::from(stderr)));

  let child_rid = state.resource_table.add(ChildResource {
    child: RefCell::new(child),
    pid,
    kill_on_drop: Cell::new(!detached),
  });

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
    &RunQueryDescriptor::Path(
      PathQueryDescriptor::new_known_absolute(Cow::Borrowed(&cmd))
        .with_requested(arg_cmd.to_string()),
    ),
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
  #[allow(
    clippy::disallowed_methods,
    reason = "ok for now because launching a sub process requires the real fs"
  )]
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
    match deno_permissions::which::which_in(
      sys_traits::impls::RealSys,
      cmd,
      path.cloned(),
      env.cwd.clone(),
    ) {
      Ok(cmd) => Ok(cmd),
      Err(deno_permissions::which::Error::CannotFindBinaryPath) => {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
      }
      Err(err) => Err(ProcessError::Which(err)),
    }
  }
}

fn resolve_path(path: &str, cwd: &Path) -> PathBuf {
  deno_path_util::normalize_path(Cow::Owned(cwd.join(path))).into_owned()
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
      return Err(CheckRunPermissionError::Other(JsErrorBox::new(
        "NotCapable",
        format!(
          "Requires --allow-run permissions to spawn subprocess with {0} environment variable{1}. Alternatively, spawn with {2} environment variable{1} unset.",
          env_var_names.join(", "),
          if env_var_names.len() != 1 { "s" } else { "" },
          if env_var_names.len() != 1 {
            "these"
          } else {
            "the"
          }
        ),
      )));
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

#[op2]
#[allow(
  clippy::await_holding_refcell_ref,
  reason = "ref is dropped before await points"
)]
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
    .child
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
fn op_spawn_sync(
  state: &mut OpState,
  #[serde] args: SpawnArgs,
) -> Result<SpawnOutput, ProcessError> {
  let stdout = matches!(args.stdio.stdout, StdioOrFd::Stdio(Stdio::Piped));
  let stderr = matches!(args.stdio.stderr, StdioOrFd::Stdio(Stdio::Piped));
  let input = args.input.clone();
  let timeout = args.timeout;
  #[cfg(unix)]
  let kill_signal = args.kill_signal.clone();
  let (mut command, _, _, _) =
    create_command(state, args, "Deno.Command().outputSync()")?;

  // When timeout is specified on Unix, create a new process group so we can
  // kill the entire tree (shell + children) on timeout, not just the shell.
  #[cfg(unix)]
  if timeout.is_some_and(|t| t > 0) {
    command.process_group(0);
  }

  let mut child = command.spawn().map_err(|e| ProcessError::SpawnFailed {
    command: command.get_program().to_string_lossy().into_owned(),
    error: Box::new(e.into()),
  })?;
  #[cfg(unix)]
  let pid = child.id();
  #[cfg(windows)]
  let pid = child.id().expect("Process ID should be set.");
  if let Some(input) = input {
    let mut stdin = child.stdin.take().ok_or_else(|| {
      ProcessError::Io(std::io::Error::other("stdin is not available"))
    })?;
    stdin.write_all(&input)?;
    stdin.flush()?;
  }

  // Take stdout/stderr pipes from child so we can read them in background
  // threads. This lets us drop the pipes on timeout to unblock the readers
  // (matching libuv's behavior of stopping pipe reads after process kill).
  let child_stdout = child.stdout.take();
  let child_stderr = child.stderr.take();

  let stdout_handle = child_stdout.map(|pipe| {
    std::thread::spawn(move || {
      let mut buf = Vec::new();
      let mut pipe = pipe;
      let _ = std::io::Read::read_to_end(&mut pipe, &mut buf);
      buf
    })
  });
  let stderr_handle = child_stderr.map(|pipe| {
    std::thread::spawn(move || {
      let mut buf = Vec::new();
      let mut pipe = pipe;
      let _ = std::io::Read::read_to_end(&mut pipe, &mut buf);
      buf
    })
  });

  // If timeout is specified, spawn a thread that will kill the child
  // after the timeout expires. Uses a condvar so the timer thread can be
  // cancelled promptly when the child exits before the deadline.
  let killed_by_timeout = Arc::new(AtomicBool::new(false));
  let cancel = Arc::new((Mutex::new(false), Condvar::new()));
  if let Some(timeout_ms) = timeout
    && timeout_ms > 0
  {
    #[cfg(unix)]
    let child_id = child.id();
    #[cfg(windows)]
    let child_id = child.id().expect("Process ID should be set.");
    let killed = killed_by_timeout.clone();
    let cancel2 = cancel.clone();
    #[cfg(unix)]
    let signal: i32 = match &kill_signal {
      Some(KillSignal::Number(n)) => *n,
      Some(KillSignal::String(s)) => {
        deno_signals::signal_str_to_int(s).unwrap_or(libc::SIGTERM)
      }
      None => libc::SIGTERM,
    };
    std::thread::spawn(move || {
      let (lock, cvar) = &*cancel2;
      let guard = lock.lock().unwrap();
      let timeout = std::time::Duration::from_millis(timeout_ms);
      let (guard, wait_result) = cvar
        .wait_timeout_while(guard, timeout, |cancelled| !*cancelled)
        .unwrap();
      // If cancelled or woken before the timeout, the child already exited.
      if *guard || !wait_result.timed_out() {
        return;
      }
      killed.store(true, Ordering::SeqCst);
      // NOTE: There is a minor race window where the child exits and its
      // PID gets recycled before we send the kill signal. The condvar
      // cancel above prevents this in practice (the main thread cancels
      // the timer immediately after wait() returns), but if the OS
      // recycles the PID in that narrow window we could signal the wrong
      // process. This matches libuv's behavior.
      #[cfg(unix)]
      // SAFETY: child_id is a valid PID from the spawned child process.
      // We use negative PID to kill the entire process group (created via
      // process_group(0) above), ensuring shell children are also killed.
      // NOTE: There is a minor theoretical race window where the child
      // could exit and its PID get recycled between wait() returning and
      // the condvar cancel reaching this thread. In practice the condvar
      // cancellation is near-instant so this window is negligible.
      unsafe {
        libc::kill(-(child_id as i32), signal);
      }
      #[cfg(windows)]
      // SAFETY: child_id is a valid PID from the spawned child process.
      // OpenProcess/TerminateProcess/CloseHandle are safe to call with
      // valid arguments.
      unsafe {
        let handle = windows_sys::Win32::System::Threading::OpenProcess(
          windows_sys::Win32::System::Threading::PROCESS_TERMINATE,
          false.into(),
          child_id,
        );
        if !handle.is_null() {
          windows_sys::Win32::System::Threading::TerminateProcess(handle, 1);
          windows_sys::Win32::Foundation::CloseHandle(handle);
        }
      }
    });
  }

  #[cfg(unix)]
  let status = child.wait().map_err(|e| ProcessError::SpawnFailed {
    command: command.get_program().to_string_lossy().into_owned(),
    error: Box::new(e.into()),
  })?;
  #[cfg(windows)]
  let status =
    child
      .wait_blocking()
      .map_err(|e| ProcessError::SpawnFailed {
        command: command.get_program().to_string_lossy().into_owned(),
        error: Box::new(e.into()),
      })?;

  // Cancel the timeout thread if it's still waiting.
  {
    let (lock, cvar) = &*cancel;
    let mut cancelled = lock.lock().unwrap();
    *cancelled = true;
    cvar.notify_one();
  }

  let timed_out = killed_by_timeout.load(Ordering::SeqCst);

  // Collect stdout/stderr from background reader threads.
  // On Unix, the process group kill ensures all children are dead and
  // pipes reach EOF, so join() completes immediately.
  // On Windows, TerminateProcess closes the child's pipe ends, so
  // join() also completes promptly for the direct child. In the rare
  // case of orphaned grandchildren holding pipes, we use a short
  // join timeout to avoid blocking indefinitely.
  let collect_pipe = |handle: Option<std::thread::JoinHandle<Vec<u8>>>| {
    let h = handle?;
    #[cfg(unix)]
    {
      h.join().ok()
    }
    #[cfg(windows)]
    {
      if timed_out {
        // Brief timeout to avoid blocking on orphaned grandchildren.
        let start = std::time::Instant::now();
        loop {
          if h.is_finished() {
            return h.join().ok();
          }
          if start.elapsed() > std::time::Duration::from_millis(200) {
            return None;
          }
          std::thread::sleep(std::time::Duration::from_millis(10));
        }
      } else {
        h.join().ok()
      }
    }
  };
  let stdout_bytes = collect_pipe(stdout_handle).unwrap_or_default();
  let stderr_bytes = collect_pipe(stderr_handle).unwrap_or_default();

  Ok(SpawnOutput {
    pid,
    status: status.try_into()?,
    stdout: if stdout {
      Some(stdout_bytes.into())
    } else {
      None
    },
    stderr: if stderr {
      Some(stderr_bytes.into())
    } else {
      None
    },
    killed_by_timeout: timed_out,
  })
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum SignalArg {
  String(String),
  Int(i32),
}

#[op2(stack_trace)]
fn op_spawn_kill(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[serde] signal: SignalArg,
) -> Result<(), ProcessError> {
  if let Ok(child_resource) = state.resource_table.get::<ChildResource>(rid) {
    deprecated::kill(child_resource.pid as i32, &signal)?;
    return Ok(());
  }
  Err(ProcessError::ChildProcessAlreadyTerminated)
}

/// Disable kill-on-drop for a child process, allowing it to outlive the parent.
/// Called from JS `ChildProcess.unref()`.
#[op2(fast)]
fn op_spawn_child_unref(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), deno_core::error::ResourceError> {
  let resource = state.resource_table.get::<ChildResource>(rid)?;
  resource.kill_on_drop.set(false);
  Ok(())
}

/// Re-enable kill-on-drop for a child process.
/// Called from JS `ChildProcess.ref()`.
#[op2(fast)]
fn op_spawn_child_ref(
  state: &mut OpState,
  #[smi] rid: ResourceId,
) -> Result<(), deno_core::error::ResourceError> {
  let resource = state.resource_table.get::<ChildResource>(rid)?;
  resource.kill_on_drop.set(true);
  Ok(())
}

mod deprecated {
  use deno_core::FromV8;
  #[cfg(windows)]
  use deno_subprocess_windows::Child;
  #[cfg(not(windows))]
  use tokio::process::Child;

  use super::*;

  #[derive(FromV8)]
  pub struct RunArgs {
    cmd: Vec<String>,
    cwd: Option<String>,
    env: Vec<(String, String)>,
    #[from_v8(serde)]
    stdin: StdioOrFd,
    #[from_v8(serde)]
    stdout: StdioOrFd,
    #[from_v8(serde)]
    stderr: StdioOrFd,
  }

  struct ChildResource {
    child: AsyncRefCell<Child>,
  }

  impl Resource for ChildResource {
    fn name(&self) -> Cow<'_, str> {
      "child".into()
    }
  }

  impl ChildResource {
    fn borrow_mut(self: Rc<Self>) -> AsyncMutFuture<Child> {
      RcRef::map(self, |r| &r.child).borrow_mut()
    }
  }

  #[derive(ToV8)]
  // TODO(@AaronO): maybe find a more descriptive name or a convention for return structs
  pub struct RunInfo {
    rid: ResourceId,
    pid: Option<u32>,
    stdin_rid: Option<ResourceId>,
    stdout_rid: Option<ResourceId>,
    stderr_rid: Option<ResourceId>,
  }

  #[op2(stack_trace)]
  pub fn op_run(
    state: &mut OpState,
    #[scoped] run_args: RunArgs,
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

    #[cfg(windows)]
    let mut c = Command::new(cmd);
    #[cfg(not(windows))]
    let mut c = tokio::process::Command::new(cmd);
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
    #[allow(
      clippy::undocumented_unsafe_blocks,
      reason = "TODO: add safety comment"
    )]
    unsafe {
      c.pre_exec(|| {
        libc::setgroups(0, std::ptr::null());
        Ok(())
      });
    }

    // TODO: make this work with other resources, eg. sockets
    c.stdin(run_args.stdin.as_stdio()?);
    c.stdout(match run_args.stdout {
      StdioOrFd::Stdio(Stdio::Inherit) => {
        let cs = state.borrow::<ChildProcessStdio>();
        StdStdio::from(cs.stdout.try_clone().map_err(ProcessError::Io)?)
      }
      value => value.as_stdio()?,
    });
    c.stderr(match run_args.stderr {
      StdioOrFd::Stdio(Stdio::Inherit) => {
        let cs = state.borrow::<ChildProcessStdio>();
        StdStdio::from(cs.stderr.try_clone().map_err(ProcessError::Io)?)
      }
      value => value.as_stdio()?,
    });

    // We want to kill child when it's closed
    c.kill_on_drop(true);

    // Spawn the command.
    let mut child = c.spawn()?;
    let pid = child.id();

    let stdin_rid = match child.stdin.take() {
      Some(child_stdin) => {
        #[cfg(windows)]
        let child_stdin = tokio::process::ChildStdin::from_std(child_stdin)?;
        let rid = state
          .resource_table
          .add(ChildStdinResource::from(child_stdin));
        Some(rid)
      }
      None => None,
    };

    let stdout_rid = match child.stdout.take() {
      Some(child_stdout) => {
        #[cfg(windows)]
        let child_stdout = tokio::process::ChildStdout::from_std(child_stdout)?;
        let rid = state
          .resource_table
          .add(ChildStdoutResource::from(child_stdout));
        Some(rid)
      }
      None => None,
    };

    let stderr_rid = match child.stderr.take() {
      Some(child_stderr) => {
        #[cfg(windows)]
        let child_stderr = tokio::process::ChildStderr::from_std(child_stderr)?;
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

  #[derive(ToV8)]
  pub struct ProcessStatus {
    got_signal: bool,
    exit_code: i32,
    exit_signal: i32,
  }

  #[op2]
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
  pub fn kill(pid: i32, signal: &SignalArg) -> Result<(), ProcessError> {
    let signo = match signal {
      SignalArg::Int(n) => *n,
      SignalArg::String(s) => deno_signals::signal_str_to_int(s)
        .map_err(SignalError::InvalidSignalStr)?,
    };
    use nix::sys::signal::Signal;
    use nix::sys::signal::kill as unix_kill;
    use nix::unistd::Pid;

    // Signal 0 is special, it checks if the process exists without sending a signal
    let sig = if signo == 0 {
      None
    } else {
      Some(
        Signal::try_from(signo)
          .map_err(|e| ProcessError::Nix(JsNixError(e)))?,
      )
    };

    unix_kill(Pid::from_raw(pid), sig)
      .map_err(|e| ProcessError::Nix(JsNixError(e)))
  }

  #[cfg(not(unix))]
  pub fn kill(pid: i32, signal: &SignalArg) -> Result<(), ProcessError> {
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

    let signo = match signal {
      SignalArg::Int(n) => *n,
      SignalArg::String(s) => deno_signals::signal_str_to_int(s)
        .map_err(SignalError::InvalidSignalStr)?,
    };

    if signo == 0 {
      // Signal 0 is a health check: verify the process is still alive.
      // SAFETY: winapi call
      let handle = unsafe {
        OpenProcess(
          winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
          FALSE,
          pid as DWORD,
        )
      };
      if handle.is_null() {
        return Err(Error::from(NotFound).into());
      }
      let mut status: DWORD = 0;
      // SAFETY: winapi call
      let alive = unsafe {
        winapi::um::processthreadsapi::GetExitCodeProcess(handle, &mut status)
          != FALSE
          && status == 259 // STILL_ACTIVE
      };
      // SAFETY: winapi call
      unsafe {
        CloseHandle(handle);
      }
      if alive {
        return Ok(());
      } else {
        return Err(Error::from(NotFound).into());
      }
    }

    // On Windows, SIGINT/SIGTERM/SIGKILL/SIGQUIT/SIGABRT all result in
    // process termination via TerminateProcess, matching libuv behavior.
    // SIGABRT is 22 on Windows (CRT), unlike 6 on Unix.
    if !matches!(signo, 2 | 3 | 9 | 15 | 22) {
      return Err(
        SignalError::InvalidSignalStr(deno_signals::InvalidSignalStrError(
          format!("{signo}"),
        ))
        .into(),
      );
    }

    if pid <= 0 {
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

  #[op2(stack_trace)]
  pub fn op_kill(
    state: &mut OpState,
    #[smi] pid: i32,
    #[serde] signal: SignalArg,
    #[string] api_name: String,
  ) -> Result<(), ProcessError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_run_all(&api_name)?;
    kill(pid, &signal)
  }
}
