// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::proc_state::ProcState;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;

pub mod bench;
pub mod testing;

pub fn cli_exts(ps: ProcState) -> Vec<Extension> {
  vec![deno_cli::init_ops(ps)]
}

deno_core::extension!(deno_cli,
  ops = [op_npm_process_state],
  options = {
    ps: ProcState,
  },
  state = |state, options| {
    state.put(options.ps);
  },
  customizer = |ext: &mut deno_core::ExtensionBuilder| {
    ext.force_op_registration();
  },
);

#[op]
fn op_npm_process_state(state: &mut OpState) -> Result<String, AnyError> {
  let proc_state = state.borrow_mut::<ProcState>();
  Ok(proc_state.npm_resolver.get_npm_process_state())
}
