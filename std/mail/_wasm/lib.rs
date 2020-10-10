// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use mailparse::dateparse;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_date(data: &[u8]) -> {
    let date = str::from_utf8(data).unwrap();
    dateparse(date)
}

