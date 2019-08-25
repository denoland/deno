// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::resources;
use crate::signal::kill;
use crate::state::ThreadSafeState;
use deno::*;
use futures;
use futures::Future;
use std;
use std::convert::From;
use std::process::Command;
use tokio_process::CommandExt;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

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

pub fn op_run(
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
  let child = c.spawn_async().map_err(ErrBox::from)?;

  let pid = child.id();
  let resources = resources::add_child(child);

  Ok(JsonOp::Sync(json!({
    "rid": resources.child_rid,
    "pid": pid,
    "stdinRid": resources.stdin_rid,
    "stdoutRid": resources.stdout_rid,
    "stderrRid": resources.stderr_rid,
  })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunStatusArgs {
  rid: i32,
}

pub fn op_run_status(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: RunStatusArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;

  state.check_run()?;

  let future = resources::child_status(rid)?;

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

pub fn op_kill(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  state.check_run()?;

  let args: KillArgs = serde_json::from_value(args)?;
  kill(args.pid, args.signo)?;
  Ok(JsonOp::Sync(json!({})))
}
