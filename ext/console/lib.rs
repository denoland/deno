// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::Extension;
use std::path::PathBuf;

pub fn init() -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .esm(include_js_files!("01_colors.js", "02_console.js",))
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}
