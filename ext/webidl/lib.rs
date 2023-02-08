// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::Extension;

/// Load and execute the javascript code.
pub fn init() -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .esm(include_js_files!("00_webidl.js",))
    .build()
}
