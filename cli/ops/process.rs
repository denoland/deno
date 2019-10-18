// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::resources;
use crate::resources::DenoResource;
use crate::resources::ResourceId;
use crate::signal::kill;
use crate::state::ThreadSafeState;
use deno::*;
use futures;
use futures::Future;
use futures::Poll;
use std;
use std::convert::From;
use std::process::Command;
use std::process::ExitStatus;
use tokio_process::CommandExt;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("run", s.core_op(json_op(s.stateful_op(op_run))));
  i.register_op(
    "run_status",
    s.core_op(json_op(s.stateful_op(op_run_status))),
  );
  i.register_op("kill", s.core_op(json_op(s.stateful_op(op_kill))));
}

fn subprocess_stdio_map(s: &str) -> std::process::Stdio {
  match s {
    "inherit" => std::process::Stdio::inherit(),
    "piped" => std::process::Stdio::piped(),
    "null" => std::process::Stdio::null(),
    _ => unreachable!(),
  }
}

struct ResourceChild(tokio_process::Child);

impl DenoResource for ResourceChild {
  fn inspect_repr(&self) -> &str {
    "child"
  }
}

struct ResourceChildStdin(tokio_process::ChildStdin);

impl DenoResource for ResourceChildStdin {
  fn inspect_repr(&self) -> &str {
    "childStdin"
  }
}

struct ResourceChildStdout(tokio_process::ChildStdout);

impl DenoResource for ResourceChildStdout {
  fn inspect_repr(&self) -> &str {
    "childStdout"
  }
}

struct ResourceChildStderr(tokio_process::ChildStderr);

impl DenoResource for ResourceChildStderr {
  fn inspect_repr(&self) -> &str {
    "childStderr"
  }
}

pub struct ChildResources {
  pub child_rid: Option<ResourceId>,
  pub stdin_rid: Option<ResourceId>,
  pub stdout_rid: Option<ResourceId>,
  pub stderr_rid: Option<ResourceId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunArgs {
  args: Vec<String>,
  cwd: Option<String>,
  env: Vec<(String, String)>,
  stdin: String,
  stdout: String,
  stderr: String,
  stdin_rid: u32,
  stdout_rid: u32,
  stderr_rid: u32,
}

fn op_run(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let run_args: RunArgs = serde_json::from_value(args)?;

  state.check_run()?;

  let args = run_args.args;
  let env = run_args.env;
  let cwd = run_args.cwd;

  let mut c = Command::new(args.get(0).unwrap());
  (1..args.len()).for_each(|i| {
    let arg = args.get(i).unwrap();
    c.arg(arg);
  });
  cwd.map(|d| c.current_dir(d));
  for (key, value) in &env {
    c.env(key, value);
  }

  // TODO: make this work with other resources, eg. sockets
  let stdin_rid = run_args.stdin_rid;
  if stdin_rid > 0 {
    c.stdin(resources::get_file(stdin_rid)?);
  } else {
    c.stdin(subprocess_stdio_map(run_args.stdin.as_ref()));
  }

  let stdout_rid = run_args.stdout_rid;
  if stdout_rid > 0 {
    c.stdout(resources::get_file(stdout_rid)?);
  } else {
    c.stdout(subprocess_stdio_map(run_args.stdout.as_ref()));
  }

  let stderr_rid = run_args.stderr_rid;
  if stderr_rid > 0 {
    c.stderr(resources::get_file(stderr_rid)?);
  } else {
    c.stderr(subprocess_stdio_map(run_args.stderr.as_ref()));
  }

  // Spawn the command.
  let mut child = c.spawn_async().map_err(ErrBox::from)?;
  let pid = child.id();

  let mut resources = ChildResources {
    child_rid: None,
    stdin_rid: None,
    stdout_rid: None,
    stderr_rid: None,
  };

  if child.stdin().is_some() {
    let stdin = child.stdin().take().unwrap();
    let res = resources::add_resource(Box::new(ResourceChildStdin(stdin)));
    resources.stdin_rid = Some(res.rid);
  }
  if child.stdout().is_some() {
    let stdout = child.stdout().take().unwrap();
    let res = resources::add_resource(Box::new(ResourceChildStdout(stdout)));
    resources.stdout_rid = Some(res.rid);
  }
  if child.stderr().is_some() {
    let stderr = child.stderr().take().unwrap();
    let res = resources::add_resource(Box::new(ResourceChildStderr(stderr)));
    resources.stderr_rid = Some(res.rid);
  }

  let res = resources::add_resource(Box::new(ResourceChild(child)));
  resources.child_rid = Some(res.rid);

  Ok(JsonOp::Sync(json!({
    "rid": resources.child_rid.unwrap(),
    "pid": pid,
    "stdinRid": resources.stdin_rid,
    "stdoutRid": resources.stdout_rid,
    "stderrRid": resources.stderr_rid,
  })))
}

pub struct ChildStatus {
  rid: ResourceId,
}

// Invert the dumbness that tokio_process causes by making Child itself a future.
impl Future for ChildStatus {
  type Item = ExitStatus;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<ExitStatus, ErrBox> {
    let mut resource_table = resources::get_table();
    let resource = resource_table.get_mut::<ResourceChild>(&self.rid)?;
    resource.0.poll().map_err(ErrBox::from)
  }
}

pub fn child_status(rid: ResourceId) -> Result<ChildStatus, ErrBox> {
  let resource_table = resources::get_table();
  let _resource = resource_table.get::<ResourceChild>(&rid)?;
  Ok(ChildStatus { rid })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunStatusArgs {
  rid: i32,
}

fn op_run_status(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RunStatusArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  state.check_run()?;

  let future = child_status(rid)?;

  let future = future.and_then(move |run_status| {
    let code = run_status.code();

    #[cfg(unix)]
    let signal = run_status.signal();
    #[cfg(not(unix))]
    let signal = None;

    code
      .or(signal)
      .expect("Should have either an exit code or a signal.");
    let got_signal = signal.is_some();

    futures::future::ok(json!({
      "gotSignal": got_signal,
       "exitCode": code.unwrap_or(-1),
       "exitSignal": signal.unwrap_or(-1),
    }))
  });

  Ok(JsonOp::Async(Box::new(future)))
}

#[derive(Deserialize)]
struct KillArgs {
  pid: i32,
  signo: i32,
}

fn op_kill(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_run()?;

  let args: KillArgs = serde_json::from_value(args)?;
  kill(args.pid, args.signo)?;
  Ok(JsonOp::Sync(json!({})))
}
