// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::npm::CliNpmResolver;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::Extension;
use deno_core::OpState;

pub mod bench;
pub mod testing;

pub fn cli_exts(npm_resolver: Arc<CliNpmResolver>) -> Vec<Extension> {
  vec![
    #[cfg(not(feature = "__runtime_js_sources"))]
    cli::init_ops(npm_resolver),
    #[cfg(feature = "__runtime_js_sources")]
    cli::init_ops_and_esm(npm_resolver),
  ]
}

// ESM parts duplicated in `../build.rs`. Keep in sync!
deno_core::extension!(cli,
  deps = [runtime],
  ops = [op_npm_process_state],
  esm_entry_point = "ext:cli/99_main.js",
  esm = [
    dir "js",
    "40_testing.js",
    "99_main.js"
  ],
  options = {
    npm_resolver: Arc<CliNpmResolver>,
  },
  state = |state, options| {
    state.put(options.npm_resolver);
  },
  customizer = |ext: &mut deno_core::Extension| {
    ext.esm_files.to_mut().push(deno_core::ExtensionFileSource {
      specifier: "ext:cli/runtime/js/99_main.js",
      code: deno_core::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
        deno_runtime::js::PATH_FOR_99_MAIN_JS,
      ),
    });
  },
);

#[op2]
#[string]
fn op_npm_process_state(state: &mut OpState) -> Result<String, AnyError> {
  let npm_resolver = state.borrow_mut::<Arc<CliNpmResolver>>();
  Ok(npm_resolver.get_npm_process_state())
}
