// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::crate_modules;
use std::path::PathBuf;
use std::collections::HashMap;

crate_modules!();

pub fn get_scripts() -> HashMap<String, PathBuf> {
    let crate_path = PathBuf::from(DENO_CRATE_PATH);
    let mut m = HashMap::new();
    m.insert("dom_exception".to_string(), crate_path.join("00_dom_exception.js"));
    m.insert("event".to_string(), crate_path.join("01_event.js"));
    m.insert("base64".to_string(), crate_path.join("07_base64.js"));
    m.insert("text_encoding".to_string(), crate_path.join("08_text_encoding.js"));
    m
}