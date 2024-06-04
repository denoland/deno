// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use base64::prelude::BASE64_STANDARD;
use base64::Engine;

static SOURCE_MAP_PREFIX: &[u8] =
  b"//# sourceMappingURL=data:application/json;base64,";

pub fn source_map_from_code(code: &[u8]) -> Option<Vec<u8>> {
  let last_line = code.rsplit(|u| *u == b'\n').next()?;
  if last_line.starts_with(SOURCE_MAP_PREFIX) {
    let input = last_line.split_at(SOURCE_MAP_PREFIX.len()).1;
    let decoded_map = BASE64_STANDARD
      .decode(input)
      .expect("Unable to decode source map from emitted file.");
    Some(decoded_map)
  } else {
    None
  }
}
