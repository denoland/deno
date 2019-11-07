// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::resources;
use crate::resources::CloneFileFuture;
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

struct ChildResource {
  child: tokio_process::Child,
}

impl Resource for ChildResource {}

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
    let file = (CloneFileFuture { rid: stdin_rid }).wait()?.into_std();
    c.stdin(file);
  } else {
    c.stdin(subprocess_stdio_map(run_args.stdin.as_ref()));
  }

  let stdout_rid = run_args.stdout_rid;
  if stdout_rid > 0 {
    let file = (CloneFileFuture { rid: stdout_rid }).wait()?.into_std();
    c.stdout(file);
  } else {
    c.stdout(subprocess_stdio_map(run_args.stdout.as_ref()));
  }

  let stderr_rid = run_args.stderr_rid;
  if stderr_rid > 0 {
    let file = (CloneFileFuture { rid: stderr_rid }).wait()?.into_std();
    c.stderr(file);
  } else {
    c.stderr(subprocess_stdio_map(run_args.stderr.as_ref()));
  }

  // Spawn the command.
  let mut child = c.spawn_async().map_err(ErrBox::from)?;
  let pid = child.id();

  let stdin_rid = if child.stdin().is_some() {
    let rid = resources::add_child_stdin(child.stdin().take().unwrap());
    Some(rid)
  } else {
    None
  };

  let stdout_rid = if child.stdout().is_some() {
    let rid = resources::add_child_stdout(child.stdout().take().unwrap());
    Some(rid)
  } else {
    None
  };

  let stderr_rid = if child.stderr().is_some() {
    let rid = resources::add_child_stderr(child.stderr().take().unwrap());
    Some(rid)
  } else {
    None
  };

  let child_resource = ChildResource { child };
  let mut table = resources::lock_resource_table();
  let child_rid = table.add("child", Box::new(child_resource));

  Ok(JsonOp::Sync(json!({
    "rid": child_rid,
    "pid": pid,
    "stdinRid": stdin_rid,
    "stdoutRid": stdout_rid,
    "stderrRid": stderr_rid,
  })))
}

pub struct ChildStatus {
  rid: ResourceId,
}

impl Future for ChildStatus {
  type Item = ExitStatus;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<ExitStatus, ErrBox> {
    let mut table = resources::lock_resource_table();
    let child_resource = table
      .get_mut::<ChildResource>(self.rid)
      .ok_or_else(bad_resource)?;
    let child = &mut child_resource.child;
    child.poll().map_err(ErrBox::from)
  }
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

  let future = ChildStatus { rid };

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
