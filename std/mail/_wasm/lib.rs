// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use mailparse::dateparse;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> Result<i64, JsValue> {
    let date = std::str::from_utf8(data).unwrap();
    return dateparse(date).map_err(|e| JsValue::from_str(&e.to_string()))
}

