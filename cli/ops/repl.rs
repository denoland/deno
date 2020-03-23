// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::repl;
use crate::repl::Repl;
use crate::state::State;
use deno_core::*;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_repl_start", s.stateful_json_op(op_repl_start));
  i.register_op("op_repl_readline", s.stateful_json_op(op_repl_readline));
}

struct ReplResource(Arc<Mutex<Repl>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

fn op_repl_start(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ReplStartArgs = serde_json::from_value(args)?;

  debug!("op_repl_start {}", args.history_file);
  let history_path =
    repl::history_path(&state.borrow().global_state.dir, &args.history_file);
  let repl = repl::Repl::new(history_path);
  let mut state = state.borrow_mut();
  let resource = ReplResource(Arc::new(Mutex::new(repl)));
  let rid = state.resource_table.add("repl", Box::new(resource));
  Ok(JsonOp::Sync(json!(rid)))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

fn op_repl_readline(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<ReplResource>(rid)
    .ok_or_else(OpError::bad_resource_id)?;
  let repl = resource.0.clone();

  blocking_json(false, move || {
    let line = repl.lock().unwrap().readline(&prompt)?;
    Ok(json!(line))
  })
}
