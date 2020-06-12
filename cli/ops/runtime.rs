// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{JsonOp, Value};
use crate::colors;
use crate::op_error::OpError;
use crate::state::State;
use crate::version;
use crate::DenoSubcommand;
use deno_core::CoreIsolate;
use deno_core::ModuleSpecifier;
use deno_core::ZeroCopyBuf;
use std::env;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_start", s.stateful_json_op(op_start));
  i.register_op("op_main_module", s.stateful_json_op(op_main_module));
  i.register_op("op_metrics", s.stateful_json_op(op_metrics));
}

fn op_start(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let state = state.borrow();
  let gs = &state.global_state;

  Ok(JsonOp::Sync(json!({
    // TODO(bartlomieju): `cwd` field is not used in JS, remove?
    "args": gs.flags.argv.clone(),
    "cwd": &env::current_dir().unwrap(),
    "debugFlag": gs.flags.log_level.map_or(false, |l| l == log::Level::Debug),
    "denoVersion": version::DENO,
    "noColor": !colors::use_color(),
    "pid": std::process::id(),
    "repl": gs.flags.subcommand == DenoSubcommand::Repl,
    "target": env!("TARGET"),
    "tsVersion": version::TYPESCRIPT,
    "unstableFlag": gs.flags.unstable,
    "v8Version": version::v8(),
    "versionFlag": gs.flags.version,
  })))
}

fn op_main_module(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let main = &state.borrow().main_module.to_string();
  let main_url = ModuleSpecifier::resolve_url_or_path(&main)?;
  if main_url.as_url().scheme() == "file" {
    let main_path = std::env::current_dir().unwrap().join(main_url.to_string());
    state.check_read_blind(&main_path, "main_module")?;
  }
  state.check_unstable("Deno.mainModule");
  Ok(JsonOp::Sync(json!(&main)))
}

fn op_metrics(
  state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let state = state.borrow();
  let m = &state.metrics;

  Ok(JsonOp::Sync(json!({
    "opsDispatched": m.ops_dispatched,
    "opsDispatchedSync": m.ops_dispatched_sync,
    "opsDispatchedAsync": m.ops_dispatched_async,
    "opsDispatchedAsyncUnref": m.ops_dispatched_async_unref,
    "opsCompleted": m.ops_completed,
    "opsCompletedSync": m.ops_completed_sync,
    "opsCompletedAsync": m.ops_completed_async,
    "opsCompletedAsyncUnref": m.ops_completed_async_unref,
    "bytesSentControl": m.bytes_sent_control,
    "bytesSentData": m.bytes_sent_data,
    "bytesReceived": m.bytes_received
  })))
}
