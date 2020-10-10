// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use mailparse::dateparse;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> i64 {
    let date = std::str::from_utf8(data).unwrap();
    return dateparse(date).unwrap()
}

