// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::PureJsModule;
use std::path::PathBuf;

/// Load and execute the javascript code.
pub fn init() -> PureJsModule {
  PureJsModule::new(include_js_files!(
    root "deno:op_crates/web",
    "01_dom_exception.js",
    "02_event.js",
    "03_abort_signal.js",
    "04_global_interfaces.js",
    "08_text_encoding.js",
    "12_location.js",
    "21_filereader.js",
  ))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_web.d.ts")
}
