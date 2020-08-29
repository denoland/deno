// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, Value};
use crate::repl;
use crate::repl::Repl;
use crate::state::State;
use deno_core::BufVec;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::ResourceTable;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  let t = &CoreIsolate::state(i).borrow().resource_table.clone();

  i.register_op("op_repl_start", s.stateful_json_op_sync(t, op_repl_start));
  i.register_op(
    "op_repl_readline",
    s.stateful_json_op_async(t, op_repl_readline),
  );
}

struct ReplResource(Arc<Mutex<Repl>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

fn op_repl_start(
  state: &State,
  resource_table: &mut ResourceTable,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: ReplStartArgs = serde_json::from_value(args)?;
  debug!("op_repl_start {}", args.history_file);
  let history_path =
    repl::history_path(&state.global_state.dir, &args.history_file);
  let repl = repl::Repl::new(history_path);
  let resource = ReplResource(Arc::new(Mutex::new(repl)));
  let rid = resource_table.add("repl", Box::new(resource));
  Ok(json!(rid))
}

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

async fn op_repl_readline(
  _state: Rc<State>,
  resource_table: Rc<RefCell<ResourceTable>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, ErrBox> {
  let args: ReplReadlineArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let prompt = args.prompt;
  debug!("op_repl_readline {} {}", rid, prompt);
  let resource_table = resource_table.borrow();
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
