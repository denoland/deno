// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files_from_crate;
use deno_core::Extension;

/// Load and execute the javascript code.
pub fn init() -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .js(include_js_files_from_crate!(
      prefix "internal:ext/webidl",
      "00_webidl.js",
    ))
    .build()
}
