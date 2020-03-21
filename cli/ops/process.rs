// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::{StreamResource, StreamResourceHolder};
use crate::op_error::OpError;
use crate::signal::kill;
use crate::state::State;
use deno_core::*;
use futures;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::TryFutureExt;
use std;
use std::convert::From;
use tokio::process::Command;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_run", s.stateful_json_op(op_run));
  i.register_op("op_run_status", s.stateful_json_op(op_run_status));
  i.register_op("op_kill", s.stateful_json_op(op_kill));
}

fn clone_file(rid: u32, state: &State) -> Result<std::fs::File, OpError> {
  let mut state = state.borrow_mut();
  let repr_holder = state
    .resource_table
    .get_mut::<StreamResourceHolder>(rid)
    .ok_or_else(OpError::bad_resource_id)?;
  let file = match repr_holder.resource {
    StreamResource::FsFile(ref mut file, _) => file,
    _ => return Err(OpError::bad_resource_id()),
  };
  let tokio_file = futures::executor::block_on(file.try_clone())?;
  let std_file = futures::executor::block_on(tokio_file.into_std());
  Ok(std_file)
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
  cmd: Vec<String>,
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
  child: tokio::process::Child,
}

fn op_run(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let run_args: RunArgs = serde_json::from_value(args)?;

  state.check_run()?;
  let state_ = state.clone();

  let args = run_args.cmd;
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
    let file = clone_file(stdin_rid, &state_)?;
    c.stdin(file);
  } else {
    c.stdin(subprocess_stdio_map(run_args.stdin.as_ref()));
  }

  let stdout_rid = run_args.stdout_rid;
  if stdout_rid > 0 {
    let file = clone_file(stdout_rid, &state_)?;
    c.stdout(file);
  } else {
    c.stdout(subprocess_stdio_map(run_args.stdout.as_ref()));
  }

  let stderr_rid = run_args.stderr_rid;
  if stderr_rid > 0 {
    let file = clone_file(stderr_rid, &state_)?;
    c.stderr(file);
  } else {
    c.stderr(subprocess_stdio_map(run_args.stderr.as_ref()));
  }

  // We want to kill child when it's closed
  c.kill_on_drop(true);

  // Spawn the command.
  let mut child = c.spawn()?;
  let pid = child.id();

  let mut state = state_.borrow_mut();
  let table = &mut state.resource_table;

  let stdin_rid = match child.stdin.take() {
    Some(child_stdin) => {
      let rid = table.add(
        "childStdin",
        Box::new(StreamResourceHolder::new(StreamResource::ChildStdin(
          child_stdin,
        ))),
      );
      Some(rid)
    }
    None => None,
  };

  let stdout_rid = match child.stdout.take() {
    Some(child_stdout) => {
      let rid = table.add(
        "childStdout",
        Box::new(StreamResourceHolder::new(StreamResource::ChildStdout(
          child_stdout,
        ))),
      );
      Some(rid)
    }
    None => None,
  };

  let stderr_rid = match child.stderr.take() {
    Some(child_stderr) => {
      let rid = table.add(
        "childStderr",
        Box::new(StreamResourceHolder::new(StreamResource::ChildStderr(
          child_stderr,
        ))),
      );
      Some(rid)
    }
    None => None,
  };

  let child_resource = ChildResource { child };
  let child_rid = table.add("child", Box::new(child_resource));

  Ok(JsonOp::Sync(json!({
    "rid": child_rid,
    "pid": pid,
    "stdinRid": stdin_rid,
    "stdoutRid": stdout_rid,
    "stderrRid": stderr_rid,
  })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunStatusArgs {
  rid: i32,
}

fn op_run_status(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: RunStatusArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  state.check_run()?;
  let state = state.clone();

  let future = async move {
    let run_status = poll_fn(|cx| {
      let resource_table = &mut state.borrow_mut().resource_table;
      let child_resource = resource_table
        .get_mut::<ChildResource>(rid)
        .ok_or_else(OpError::bad_resource_id)?;
      let child = &mut child_resource.child;
      child.map_err(OpError::from).poll_unpin(cx)
    })
    .await?;

    let code = run_status.code();

    #[cfg(unix)]
    let signal = run_status.signal();
    #[cfg(not(unix))]
    let signal = None;

    code
      .or(signal)
      .expect("Should have either an exit code or a signal.");
    let got_signal = signal.is_some();

    Ok(json!({
       "gotSignal": got_signal,
       "exitCode": code.unwrap_or(-1),
       "exitSignal": signal.unwrap_or(-1),
    }))
  };

  Ok(JsonOp::Async(future.boxed_local()))
}

#[derive(Deserialize)]
struct KillArgs {
  pid: i32,
  signo: i32,
}

fn op_kill(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  state.check_run()?;

  let args: KillArgs = serde_json::from_value(args)?;
  kill(args.pid, args.signo)?;
  Ok(JsonOp::Sync(json!({})))
}
