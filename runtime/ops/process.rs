// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::check_unstable;
use crate::permissions::PermissionsContainer;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_json;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_io::ChildStderrResource;
use deno_io::ChildStdinResource;
use deno_io::ChildStdoutResource;
use deno_io::StdFileResource;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::process::ExitStatus;
use std::rc::Rc;
use tokio::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(unix)]
use std::os::unix::prelude::ExitStatusExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Copy, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stdio {
  Inherit,
  Piped,
  Null,
}

impl Stdio {
  pub fn as_stdio(&self) -> std::process::Stdio {
    match &self {
      Stdio::Inherit => std::process::Stdio::inherit(),
      Stdio::Piped => std::process::Stdio::piped(),
      Stdio::Null => std::process::Stdio::null(),
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
  ) -> Result<std::process::Stdio, AnyError> {
    match &self {
      StdioOrRid::Stdio(val) => Ok(val.as_stdio()),
      StdioOrRid::Rid(rid) => StdFileResource::as_stdio(state, *rid),
    }
  }
}

deno_core::extension!(
  deno_process,
  ops = [
    op_spawn_child,
    op_spawn_wait,
    op_spawn_sync,
    deprecated::op_run,
    deprecated::op_run_status,
    deprecated::op_kill,
  ],
  customizer = |ext: &mut deno_core::ExtensionBuilder| {
    ext.force_op_registration();
  },
);

struct ChildResource(tokio::process::Child);

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

  #[serde(flatten)]
  stdio: ChildStdio,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildStdio {
  stdin: Stdio,
  stdout: Stdio,
  stderr: Stdio,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildStatus {
  success: bool,
  code: i32,
  signal: Option<String>,
}

impl TryFrom<ExitStatus> for ChildStatus {
  type Error = AnyError;

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
        signal: Some(
          crate::ops::signal::signal_int_to_str(signal)?.to_string(),
        ),
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
  stdout: Option<ZeroCopyBuf>,
  stderr: Option<ZeroCopyBuf>,
}

fn create_command(
  state: &mut OpState,
  args: SpawnArgs,
  api_name: &str,
) -> Result<std::process::Command, AnyError> {
  state
    .borrow_mut::<PermissionsContainer>()
    .check_run(&args.cmd, api_name)?;

  let mut command = std::process::Command::new(args.cmd);

  #[cfg(windows)]
  if args.windows_raw_arguments {
    for arg in args.args.iter() {
      command.raw_arg(arg);
    }
  } else {
    command.args(args.args);
  }

  #[cfg(not(windows))]
  command.args(args.args);

  if let Some(cwd) = args.cwd {
    command.current_dir(cwd);
  }

  if args.clear_env {
    command.env_clear();
  }
  command.envs(args.env);

  #[cfg(unix)]
  if let Some(gid) = args.gid {
    command.gid(gid);
  }
  #[cfg(unix)]
  if let Some(uid) = args.uid {
    command.uid(uid);
  }
  #[cfg(unix)]
  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    command.pre_exec(|| {
      libc::setgroups(0, std::ptr::null());
      Ok(())
    });
  }

  command.stdin(args.stdio.stdin.as_stdio());
  command.stdout(match args.stdio.stdout {
    Stdio::Inherit => StdioOrRid::Rid(1).as_stdio(state)?,
    value => value.as_stdio(),
  });
  command.stderr(match args.stdio.stderr {
    Stdio::Inherit => StdioOrRid::Rid(2).as_stdio(state)?,
    value => value.as_stdio(),
  });

  Ok(command)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Child {
  rid: ResourceId,
  pid: u32,
  stdin_rid: Option<ResourceId>,
  stdout_rid: Option<ResourceId>,
  stderr_rid: Option<ResourceId>,
}

fn spawn_child(
  state: &mut OpState,
  command: std::process::Command,
) -> Result<Child, AnyError> {
  let mut command = tokio::process::Command::from(command);
  // TODO(@crowlkats): allow detaching processes.
  //  currently deno will orphan a process when exiting with an error or Deno.exit()
  // We want to kill child when it's closed
  command.kill_on_drop(true);

  let mut child = command.spawn()?;
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

  let child_rid = state.resource_table.add(ChildResource(child));

  Ok(Child {
    rid: child_rid,
    pid,
    stdin_rid,
    stdout_rid,
    stderr_rid,
  })
}

#[op]
fn op_spawn_child(
  state: &mut OpState,
  args: SpawnArgs,
  api_name: String,
) -> Result<Child, AnyError> {
  let command = create_command(state, args, &api_name)?;
  spawn_child(state, command)
}

#[op]
async fn op_spawn_wait(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<ChildStatus, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .take::<ChildResource>(rid)?;
  Rc::try_unwrap(resource)
    .ok()
    .unwrap()
    .0
    .wait()
    .await?
    .try_into()
}

#[op]
fn op_spawn_sync(
  state: &mut OpState,
  args: SpawnArgs,
) -> Result<SpawnOutput, AnyError> {
  let stdout = matches!(args.stdio.stdout, Stdio::Piped);
  let stderr = matches!(args.stdio.stderr, Stdio::Piped);
  let output =
    create_command(state, args, "Deno.Command().outputSync()")?.output()?;

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

mod deprecated {
  use super::*;

  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct RunArgs {
    cmd: Vec<String>,
    cwd: Option<String>,
    clear_env: bool,
    env: Vec<(String, String)>,
    #[cfg(unix)]
    gid: Option<u32>,
    #[cfg(unix)]
    uid: Option<u32>,
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
  struct RunInfo {
    rid: ResourceId,
    pid: Option<u32>,
    stdin_rid: Option<ResourceId>,
    stdout_rid: Option<ResourceId>,
    stderr_rid: Option<ResourceId>,
  }

  #[op]
  fn op_run(
    state: &mut OpState,
    run_args: RunArgs,
  ) -> Result<RunInfo, AnyError> {
    let args = run_args.cmd;
    state
      .borrow_mut::<PermissionsContainer>()
      .check_run(&args[0], "Deno.run()")?;
    let env = run_args.env;
    let cwd = run_args.cwd;

    let mut c = Command::new(args.get(0).unwrap());
    (1..args.len()).for_each(|i| {
      let arg = args.get(i).unwrap();
      c.arg(arg);
    });
    cwd.map(|d| c.current_dir(d));

    if run_args.clear_env {
      super::check_unstable(state, "Deno.run.clearEnv");
      c.env_clear();
    }
    for (key, value) in &env {
      c.env(key, value);
    }

    #[cfg(unix)]
    if let Some(gid) = run_args.gid {
      super::check_unstable(state, "Deno.run.gid");
      c.gid(gid);
    }
    #[cfg(unix)]
    if let Some(uid) = run_args.uid {
      super::check_unstable(state, "Deno.run.uid");
      c.uid(uid);
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
  struct ProcessStatus {
    got_signal: bool,
    exit_code: i32,
    exit_signal: i32,
  }

  #[op]
  async fn op_run_status(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
  ) -> Result<ProcessStatus, AnyError> {
    let resource = state
      .borrow_mut()
      .resource_table
      .get::<ChildResource>(rid)?;
    let mut child = resource.borrow_mut().await;
    let run_status = child.wait().await?;
    let code = run_status.code();

    #[cfg(unix)]
    let signal = run_status.signal();
    #[cfg(not(unix))]
    let signal = None;

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
  pub fn kill(pid: i32, signal: &str) -> Result<(), AnyError> {
    let signo = super::super::signal::signal_str_to_int(signal)?;
    use nix::sys::signal::kill as unix_kill;
    use nix::sys::signal::Signal;
    use nix::unistd::Pid;
    let sig = Signal::try_from(signo)?;
    unix_kill(Pid::from_raw(pid), Option::Some(sig)).map_err(AnyError::from)
  }

  #[cfg(not(unix))]
  pub fn kill(pid: i32, signal: &str) -> Result<(), AnyError> {
    use deno_core::error::type_error;
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
      Err(type_error(format!("Invalid signal: {signal}")))
    } else if pid <= 0 {
      Err(type_error("Invalid pid"))
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

  #[op]
  fn op_kill(
    state: &mut OpState,
    pid: i32,
    signal: String,
    api_name: String,
  ) -> Result<(), AnyError> {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_run_all(&api_name)?;
    kill(pid, &signal)?;
    Ok(())
  }
}
