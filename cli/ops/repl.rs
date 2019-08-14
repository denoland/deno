// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::msg;
use crate::ops::blocking;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::repl;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;

pub fn op_repl_start(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_repl_start().unwrap();
  let cmd_id = base.cmd_id();
  let history_file = String::from(inner.history_file().unwrap());

  debug!("op_repl_start {}", history_file);
  let history_path = repl::history_path(&state.dir, &history_file);
  let repl = repl::Repl::new(history_path);
  let resource = resources::add_repl(repl);

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::ReplStartRes::create(
    builder,
    &msg::ReplStartResArgs { rid: resource.rid },
  );
  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ReplStartRes,
      ..Default::default()
    },
  ))
}

pub fn op_repl_readline(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_repl_readline().unwrap();
  let cmd_id = base.cmd_id();
  let rid = inner.rid();
  let prompt = inner.prompt().unwrap().to_owned();
  debug!("op_repl_readline {} {}", rid, prompt);

  blocking(base.sync(), move || {
    let repl = resources::get_repl(rid)?;
    let line = repl.lock().unwrap().readline(&prompt)?;

    let builder = &mut FlatBufferBuilder::new();
    let line_off = builder.create_string(&line);
    let inner = msg::ReplReadlineRes::create(
      builder,
      &msg::ReplReadlineResArgs {
        line: Some(line_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReplReadlineRes,
        ..Default::default()
      },
    ))
  })
}
