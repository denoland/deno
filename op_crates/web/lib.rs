// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::crate_modules;
use std::path::PathBuf;

crate_modules!();

pub struct WebScripts {
  pub dom_exception: String,
  pub event: String,
  pub base64: String,
  pub text_encoding: String,
}

fn get_str_path(file_name: &str) -> String {
  PathBuf::from(DENO_CRATE_PATH)
    .join(file_name)
    .to_string_lossy()
    .to_string()
}

pub fn get_scripts() -> WebScripts {
  WebScripts {
    dom_exception: get_str_path("00_dom_exception.js"),
    event: get_str_path("01_event.js"),
    base64: get_str_path("07_base64.js"),
    text_encoding: get_str_path("08_text_encoding.js"),
  }
}
