// Copyright 2018-2025 the Deno authors. MIT license.

use deno_ast::ParsedSource;
use swc::serialize_swc_to_buffer;

use crate::util::text_encoding::Utf16Map;

mod buffer;
mod swc;
mod ts_estree;

pub fn serialize_ast_to_buffer(
  parsed_source: &ParsedSource,
  utf16_map: &Utf16Map,
) -> Vec<u8> {
  // TODO: We could support multiple languages here
  serialize_swc_to_buffer(parsed_source, utf16_map)
}
