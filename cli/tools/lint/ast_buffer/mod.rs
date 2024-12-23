// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use swc::serialize_swc_to_buffer;

mod buffer;
mod selector;
mod swc;
mod ts_estree;

pub fn serialize_ast_to_buffer(parsed_source: &ParsedSource) -> Vec<u8> {
  // TODO: We could support multiple languages here
  serialize_swc_to_buffer(parsed_source)
}
