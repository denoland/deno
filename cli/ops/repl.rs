// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::repl;
use crate::repl::Repl;
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use serde_derive::Deserialize;
use serde_json::Value;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_repl_start", op_repl_start);
  s.register_op_json_async("op_repl_readline", op_repl_readline);
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
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: ReplStartArgs = serde_json::from_value(args)?;
  debug!("op_repl_start {}", args.history_file);
  let history_path =
    repl::history_path(&state.global_state.dir, &args.history_file);
  let repl = repl::Repl::new(history_path);
  let resource = ReplResource(Arc::new(Mutex::new(repl)));
  let rid = state
    .resource_table
    .borrow_mut()
    .add("repl", Box::new(resource));
  Ok(json!(rid))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

async fn op_repl_readline(
  state: Rc<State>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);
  let resource_table = state.resource_table.borrow();
  let resource = resource_table
    .get::<ReplResource>(rid)
    .ok_or_else(ErrBox::bad_resource_id)?;
  let repl = resource.0.clone();
  drop(resource_table);
  tokio::task::spawn_blocking(move || {
    let line = repl.lock().unwrap().readline(&prompt)?;
    Ok(json!(line))
  })
  .await
  .unwrap()
}
