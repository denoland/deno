// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::resources;
use crate::signal::kill;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures;
use futures::Future;
use std;
use std::convert::From;
use std::process::Command;
use tokio_process::CommandExt;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

fn subprocess_stdio_map(v: msg::ProcessStdio) -> std::process::Stdio {
  match v {
    msg::ProcessStdio::Inherit => std::process::Stdio::inherit(),
    msg::ProcessStdio::Piped => std::process::Stdio::piped(),
    msg::ProcessStdio::Null => std::process::Stdio::null(),
  }
}

pub fn op_run(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if !base.sync() {
    return Err(deno_error::no_async_support());
  }
  let cmd_id = base.cmd_id();

  state.check_run()?;

  assert!(data.is_none());
  let inner = base.inner_as_run().unwrap();
  let args = inner.args().unwrap();
  let env = inner.env().unwrap();
  let cwd = inner.cwd();

  let mut c = Command::new(args.get(0));
  (1..args.len()).for_each(|i| {
    let arg = args.get(i);
    c.arg(arg);
  });
  cwd.map(|d| c.current_dir(d));
  (0..env.len()).for_each(|i| {
    let entry = env.get(i);
    c.env(entry.key().unwrap(), entry.value().unwrap());
  });

  // TODO: make this work with other resources, eg. sockets
  let stdin_rid = inner.stdin_rid();
  if stdin_rid > 0 {
    c.stdin(resources::get_file(stdin_rid)?);
  } else {
    c.stdin(subprocess_stdio_map(inner.stdin()));
  }

  let stdout_rid = inner.stdout_rid();
  if stdout_rid > 0 {
    c.stdout(resources::get_file(stdout_rid)?);
  } else {
    c.stdout(subprocess_stdio_map(inner.stdout()));
  }

  let stderr_rid = inner.stderr_rid();
  if stderr_rid > 0 {
    c.stderr(resources::get_file(stderr_rid)?);
  } else {
    c.stderr(subprocess_stdio_map(inner.stderr()));
  }

  // Spawn the command.
  let child = c.spawn_async().map_err(ErrBox::from)?;

  let pid = child.id();
  let resources = resources::add_child(child);

  let mut res_args = msg::RunResArgs {
    rid: resources.child_rid,
    pid,
    ..Default::default()
  };

  if let Some(stdin_rid) = resources.stdin_rid {
    res_args.stdin_rid = stdin_rid;
  }
  if let Some(stdout_rid) = resources.stdout_rid {
    res_args.stdout_rid = stdout_rid;
  }
  if let Some(stderr_rid) = resources.stderr_rid {
    res_args.stderr_rid = stderr_rid;
  }

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::RunRes::create(builder, &res_args);
  Ok(Op::Sync(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::RunRes,
      ..Default::default()
    },
  )))
}

pub fn op_run_status(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_run_status().unwrap();
  let rid = inner.rid();

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

    let builder = &mut FlatBufferBuilder::new();
    let inner = msg::RunStatusRes::create(
      builder,
      &msg::RunStatusResArgs {
        got_signal,
        exit_code: code.unwrap_or(-1),
        exit_signal: signal.unwrap_or(-1),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::RunStatusRes,
        ..Default::default()
      },
    ))
  });
  if base.sync() {
    let buf = future.wait()?;
    Ok(Op::Sync(buf))
  } else {
    Ok(Op::Async(Box::new(future)))
  }
}

pub fn op_kill(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  state.check_run()?;

  assert!(data.is_none());
  let inner = base.inner_as_kill().unwrap();
  let pid = inner.pid();
  let signo = inner.signo();
  kill(pid, signo)?;
  ok_buf(empty_buf())
}
