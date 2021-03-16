// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::PureJsModule;

/// Load and execute the javascript code.
pub fn init() -> PureJsModule {
  PureJsModule::new(vec![(
    "deno:op_crates/webidl/00_webidl.js",
    include_str!("00_webidl.js"),
  )])
}
