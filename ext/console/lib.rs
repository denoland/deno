// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::include_js_files;
use deno_core::Extension;

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/console",
      "01_colors.js",
      "02_console.js",
    ))
    .build()
}
