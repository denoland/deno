// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{blocking_json, Deserialize, JsonOp, Value};
use crate::repl;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

pub fn op_repl_start(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReplStartArgs = serde_json::from_value(args)?;

  debug!("op_repl_start {}", args.history_file);
  let history_path = repl::history_path(&state.dir, &args.history_file);
  let repl = repl::Repl::new(history_path);
  let resource = resources::add_repl(repl);

  Ok(JsonOp::Sync(json!(resource.rid)))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

pub fn op_repl_readline(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);

  blocking_json(false, move || {
    let repl = resources::get_repl(rid as u32)?;
    let line = repl.lock().unwrap().readline(&prompt)?;
    Ok(json!(line))
  })
}
