// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::path::PathBuf;

deno_core::extension!(deno_console, esm = ["01_colors.js", "02_console.js"],);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}
