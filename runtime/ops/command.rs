// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::io::ChildStderrResource;
use super::io::ChildStdinResource;
use super::io::ChildStdoutResource;
use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::process::ExitStatus;
use std::rc::Rc;

#[cfg(unix)]
use std::os::unix::prelude::ExitStatusExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_command_spawn::decl(),
      op_command_status::decl(),
      op_command_wait::decl(),
      op_command_output::decl(),
      op_command_sync::decl(),
    ])
    .build()
}

struct ChildResource(AsyncRefCell<tokio::process::Child>);

impl Resource for ChildResource {
  fn name(&self) -> Cow<str> {
    "child".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stdio {
  Inherit,
  Piped,
  Null,
}

fn subprocess_stdio_map(s: &Stdio) -> Result<std::process::Stdio, AnyError> {
  match s {
    Stdio::Inherit => Ok(std::process::Stdio::inherit()),
    Stdio::Piped => Ok(std::process::Stdio::piped()),
    Stdio::Null => Ok(std::process::Stdio::null()),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandArgs {
  cmd: String,
  args: Vec<String>,
  cwd: Option<String>,
  clear_env: bool,
  env: Vec<(String, String)>,
  #[cfg(unix)]
  gid: Option<u32>,
  #[cfg(unix)]
  uid: Option<u32>,

  #[serde(flatten)]
  stdio: CommandStdio,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandStdio {
  stdin: Stdio,
  stdout: Stdio,
  stderr: Stdio,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandStatus {
  success: bool,
  code: i32,
  signal: Option<i32>,
}

impl From<std::process::ExitStatus> for CommandStatus {
  fn from(status: ExitStatus) -> Self {
    let code = status.code();
    #[cfg(unix)]
    let signal = status.signal();
    #[cfg(not(unix))]
    let signal = None;

    if let Some(signal) = signal {
      CommandStatus {
        success: false,
        code: 128 + signal,
        signal: Some(signal),
      }
    } else {
      let code = code.expect("Should have either an exit code or a signal.");

      CommandStatus {
        success: code == 0,
        code,
        signal: None,
      }
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutput {
  status: CommandStatus,
  stdout: Option<ZeroCopyBuf>,
  stderr: Option<ZeroCopyBuf>,
}

fn create_command(
  state: &mut OpState,
  args: CommandArgs,
) -> Result<std::process::Command, AnyError> {
  state.borrow_mut::<Permissions>().run.check(&args.cmd)?;

  let mut command = std::process::Command::new(args.cmd);
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
    super::check_unstable(state, "Deno.Command.gid");
    command.gid(gid);
  }
  #[cfg(unix)]
  if let Some(uid) = args.uid {
    super::check_unstable(state, "Deno.Command.uid");
    command.uid(uid);
  }
  #[cfg(unix)]
  unsafe {
    command.pre_exec(|| {
      libc::setgroups(0, std::ptr::null());
      Ok(())
    });
  }

  command.stdin(subprocess_stdio_map(&args.stdio.stdin)?);
  command.stdout(subprocess_stdio_map(&args.stdio.stdout)?);
  command.stderr(subprocess_stdio_map(&args.stdio.stderr)?);

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
fn op_command_spawn(
  state: &mut OpState,
  args: CommandArgs,
) -> Result<Child, AnyError> {
  let mut command = tokio::process::Command::from(create_command(state, args)?);
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

  let child_rid = state
    .resource_table
    .add(ChildResource(AsyncRefCell::new(child)));

  Ok(Child {
    rid: child_rid,
    pid,
    stdin_rid,
    stdout_rid,
    stderr_rid,
  })
}

#[op]
fn op_command_status(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<Option<CommandStatus>, AnyError> {
  let resource = state.resource_table.get::<ChildResource>(rid)?;
  let mut child = RcRef::map(resource, |r| &r.0).try_borrow_mut().unwrap();
  Ok(child.try_wait()?.map(|status| status.into()))
}

#[op]
async fn op_command_wait(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<CommandStatus, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .take::<ChildResource>(rid)?;
  let mut child = RcRef::map(resource, |r| &r.0).borrow_mut().await;
  Ok(child.wait().await?.into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChildStdio {}

#[op]
async fn op_command_output(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  stdout_rid: Option<ResourceId>,
  stderr_rid: Option<ResourceId>,
) -> Result<CommandOutput, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .take::<ChildResource>(rid)?;
  let resource = Rc::try_unwrap(resource).ok().unwrap();
  let mut child = resource.0.into_inner();

  if let Some(stdout_rid) = stdout_rid {
    let stdout = state
      .borrow_mut()
      .resource_table
      .take::<ChildStdoutResource>(stdout_rid)?;
    child.stdout = Some(Rc::try_unwrap(stdout).unwrap().into_inner());
  }
  if let Some(stderr_rid) = stderr_rid {
    let stderr = state
      .borrow_mut()
      .resource_table
      .take::<ChildStderrResource>(stderr_rid)?;
    child.stderr = Some(Rc::try_unwrap(stderr).unwrap().into_inner());
  }

  let output = child.wait_with_output().await?;

  Ok(CommandOutput {
    status: output.status.into(),
    stdout: stdout_rid.map(|_| output.stdout.into()),
    stderr: stderr_rid.map(|_| output.stderr.into()),
  })
}

#[op]
fn op_command_sync(
  state: &mut OpState,
  args: CommandArgs,
) -> Result<CommandOutput, AnyError> {
  let stdout = matches!(args.stdio.stdout, Stdio::Piped);
  let stderr = matches!(args.stdio.stderr, Stdio::Piped);
  let output = create_command(state, args)?.output()?;

  Ok(CommandOutput {
    status: output.status.into(),
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
