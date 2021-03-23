// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::BasicExtension;
use std::path::PathBuf;

pub fn init() -> BasicExtension {
  BasicExtension::pure_js(include_js_files!(
    prefix "deno:op_crates/console",
    "01_colors.js",
    "02_console.js",
  ))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}
