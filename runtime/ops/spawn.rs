// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::io::ChildStderrResource;
use super::io::ChildStdinResource;
use super::io::ChildStdoutResource;
use super::process::Stdio;
use super::process::StdioOrRid;
use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::ExitStatus;
use std::rc::Rc;

#[cfg(unix)]
use std::os::unix::prelude::ExitStatusExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_spawn_child::decl(),
      op_spawn_wait::decl(),
      op_spawn_sync::decl(),
    ])
    .build()
}

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
  super::check_unstable(state, "Deno.spawn");
  state
    .borrow_mut::<Permissions>()
    .run
    .check(&args.cmd, Some(api_name))?;

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
    super::check_unstable(state, "Deno.spawn.gid");
    command.gid(gid);
  }
  #[cfg(unix)]
  if let Some(uid) = args.uid {
    super::check_unstable(state, "Deno.spawn.uid");
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

#[op]
fn op_spawn_child(
  state: &mut OpState,
  args: SpawnArgs,
  api_name: String,
) -> Result<Child, AnyError> {
  let mut command =
    tokio::process::Command::from(create_command(state, args, &api_name)?);
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
  let output = create_command(state, args, "Deno.spawnSync()")?.output()?;

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
