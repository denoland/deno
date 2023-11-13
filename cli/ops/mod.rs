// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::Extension;

pub mod bench;
pub mod jupyter;
pub mod testing;

pub fn cli_exts() -> Vec<Extension> {
  vec![
    #[cfg(not(feature = "__runtime_js_sources"))]
    cli::init_ops(),
    #[cfg(feature = "__runtime_js_sources")]
    cli::init_ops_and_esm(),
  ]
}

// ESM parts duplicated in `../build.rs`. Keep in sync!
deno_core::extension!(cli,
  deps = [runtime],
  esm_entry_point = "ext:cli/99_main.js",
  esm = [
    dir "js",
    "40_testing.js",
    "40_jupyter.js",
    "99_main.js"
  ],
  customizer = |ext: &mut deno_core::Extension| {
    ext.esm_files.to_mut().push(deno_core::ExtensionFileSource {
      specifier: "ext:cli/runtime/js/99_main.js",
      code: deno_core::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
        deno_runtime::js::PATH_FOR_99_MAIN_JS,
      ),
    });
  },
);
