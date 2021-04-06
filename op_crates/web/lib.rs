// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::JsRuntime;
use std::path::PathBuf;

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    (
      "deno:op_crates/web/01_dom_exception.js",
      include_str!("01_dom_exception.js"),
    ),
    (
      "deno:op_crates/web/02_event.js",
      include_str!("02_event.js"),
    ),
    (
      "deno:op_crates/web/03_abort_signal.js",
      include_str!("03_abort_signal.js"),
    ),
    (
      "deno:op_crates/web/04_global_interfaces.js",
      include_str!("04_global_interfaces.js"),
    ),
    (
      "deno:op_crates/web/08_text_encoding.js",
      include_str!("08_text_encoding.js"),
    ),
    (
      "deno:op_crates/web/12_location.js",
      include_str!("12_location.js"),
    ),
  ];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_web.d.ts")
}
