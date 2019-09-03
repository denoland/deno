// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{blocking_json, wrap_json_op, Deserialize, JsonOp};
use crate::repl;
use crate::resources;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;

// Repl Start

pub struct OpReplStart;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplStartArgs {
  history_file: String,
}

impl DenoOpDispatcher for OpReplStart {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ReplStartArgs = serde_json::from_value(args)?;

        debug!("op_repl_start {}", args.history_file);
        let history_path = repl::history_path(&state.dir, &args.history_file);
        let repl = repl::Repl::new(history_path);
        let resource = resources::add_repl(repl);

        Ok(JsonOp::Sync(json!(resource.rid)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "replStart";
}

// Repl Readline

pub struct OpReplReadline;

#[derive(Deserialize)]
struct ReplReadlineArgs {
  rid: i32,
  prompt: String,
}

impl DenoOpDispatcher for OpReplReadline {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ReplReadlineArgs = serde_json::from_value(args)?;
        let rid = args.rid;
        let prompt = args.prompt;
        debug!("op_repl_readline {} {}", rid, prompt);

        blocking_json(false, move || {
          let repl = resources::get_repl(rid as u32)?;
          let line = repl.lock().unwrap().readline(&prompt)?;
          Ok(json!(line))
        })
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "replReadline";
}
