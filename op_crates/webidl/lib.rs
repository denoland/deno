// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::JsRuntime;

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![(
    "deno:op_crates/webidl/00_webidl.js",
    include_str!("00_webidl.js"),
  )];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}
