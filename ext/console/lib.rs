// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::Extension;
use deno_core::ExtensionBuilder;
use std::path::PathBuf;

fn ext() -> ExtensionBuilder {
  Extension::builder(env!("CARGO_PKG_NAME"))
}
pub fn init() -> Extension {
  ext().build()
}

pub fn init_esm() -> Extension {
  ext()
    .esm(include_js_files!("01_colors.js", "02_console.js",))
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}
