// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::JsRuntime;
use std::path::PathBuf;

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    ("deno:op_crates/file/01_file.js", include_str!("01_file.js")),
    (
      "deno:op_crates/file/02_filereader.js",
      include_str!("02_filereader.js"),
    ),
  ];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_file.d.ts")
}
