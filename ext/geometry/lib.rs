// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

deno_core::extension!(
  deno_geometry,
  deps = [deno_webidl, deno_web, deno_console],
  esm = ["01_geometry.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_geometry.d.ts")
}
