// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::npm::CliNpmResolver;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;

pub mod bench;
pub mod testing;

pub fn cli_exts(npm_resolver: Arc<CliNpmResolver>) -> Vec<Extension> {
  vec![deno_cli::init_ops(npm_resolver)]
}

deno_core::extension!(deno_cli,
  ops = [op_npm_process_state],
  options = {
    npm_resolver: Arc<CliNpmResolver>,
  },
  state = |state, options| {
    state.put(options.npm_resolver);
  },
  customizer = |ext: &mut deno_core::ExtensionBuilder| {
    ext.force_op_registration();
  },
);

#[op]
fn op_npm_process_state(state: &mut OpState) -> Result<String, AnyError> {
  let npm_resolver = state.borrow_mut::<Arc<CliNpmResolver>>();
  Ok(npm_resolver.get_npm_process_state())
}
