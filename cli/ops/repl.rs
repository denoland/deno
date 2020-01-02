// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::repl;
use crate::repl::Repl;
use crate::state::ThreadSafeState;
use deno::Resource;
use deno::*;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "repl_start",
    s.core_op(json_op(s.stateful_op(op_repl_start))),
  );
  i.register_op(
    "repl_readline",
    s.core_op(json_op(s.stateful_op(op_repl_readline))),
  );
}

struct ReplResource(Arc<Mutex<Repl>>);

impl Resource for ReplResource {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

fn op_repl_start(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReplStartArgs = serde_json::from_value(args)?;

  debug!("op_repl_start {}", args.history_file);
  let history_path =
    repl::history_path(&state.global_state.dir, &args.history_file);
  let repl = repl::Repl::new(history_path);
  let resource = ReplResource(Arc::new(Mutex::new(repl)));
  let mut table = state.lock_resource_table();
  let rid = table.add("repl", Box::new(resource));
  Ok(JsonOp::Sync(json!(rid)))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

fn op_repl_readline(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);
  let state = state.clone();

  blocking_json(false, move || {
    let table = state.lock_resource_table();
    let resource = table.get::<ReplResource>(rid).ok_or_else(bad_resource)?;
    let repl = resource.0.clone();
    let line = repl.lock().unwrap().readline(&prompt)?;
    Ok(json!(line))
  })
}
