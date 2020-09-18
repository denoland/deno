// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::repl;
use crate::repl::Repl;
use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_repl_start", op_repl_start);
  super::reg_json_async(rt, "op_repl_readline", op_repl_readline);
}

struct ReplResource(Arc<Mutex<Repl>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

fn op_repl_start(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ReplStartArgs = serde_json::from_value(args)?;
  debug!("op_repl_start {}", args.history_file);
  let history_path = {
    let cli_state = super::cli_state(state);
    repl::history_path(&cli_state.global_state.dir, &args.history_file)
  };
  let repl = repl::Repl::new(history_path);
  let resource = ReplResource(Arc::new(Mutex::new(repl)));
  let rid = state.resource_table.add("repl", Box::new(resource));
  Ok(json!(rid))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

async fn op_repl_readline(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);
  let repl = {
    let state = state.borrow();
    let resource = state
      .resource_table
      .get::<ReplResource>(rid)
      .ok_or_else(bad_resource_id)?;
    resource.0.clone()
  };
  tokio::task::spawn_blocking(move || {
    let line = repl.lock().unwrap().readline(&prompt)?;
    Ok(json!(line))
  })
  .await
  .unwrap()
}
