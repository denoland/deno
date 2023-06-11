// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::path::PathBuf;

deno_core::extension!(
  deno_console,
  esm = ["01_console.js"],
  exclude_js_sources_cfg = (all(
    feature = "exclude_js_sources",
    not(feature = "force_include_js_sources")
  )),
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}
