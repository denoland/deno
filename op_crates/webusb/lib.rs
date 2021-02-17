// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;

pub use rusb; // Re-export rusb


/// Execute this crates' JS source files.
pub fn init(isolate: &mut JsRuntime) {
    let files = vec![(
      "deno:op_crates/webusb/01_webusb.js",
      include_str!("01_webusb.js"),
    )];
    for (url, source_code) in files {
      isolate.execute(url, source_code).unwrap();
    }
}


pub fn op_webusb_get_devices(
    state: &mut OpState,
    _args: Value,
    zero_copy: &mut [ZeroCopyBuf],
  ) -> Result<Value, AnyError> {
    rusb::devices()
  
    Ok(json!({}))
}
  