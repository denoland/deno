// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::Extension;

/// Load and execute the javascript code.
pub fn init() -> Extension {
  Extension::pure_js(vec![(
    "deno:op_crates/webidl/00_webidl.js",
    include_str!("00_webidl.js"),
  )])
}
