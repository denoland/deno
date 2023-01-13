// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::ParsedSourceCache;

use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use std::sync::Arc;

/// A hashing function that takes the source code and emit options
/// hash then generates a string hash which can be stored to
/// determine if the cached emit is valid or not.
pub fn get_source_hash(source_text: &str, emit_options_hash: u64) -> u64 {
  FastInsecureHasher::new()
    .write_str(source_text)
    .write_u64(emit_options_hash)
    .finish()
}

pub fn emit_parsed_source(
  emit_cache: &EmitCache,
  parsed_source_cache: &ParsedSourceCache,
  specifier: &ModuleSpecifier,
  media_type: MediaType,
  source: &Arc<str>,
  emit_options: &deno_ast::EmitOptions,
  emit_config_hash: u64,
) -> Result<String, AnyError> {
  let source_hash = get_source_hash(source, emit_config_hash);

  if let Some(emit_code) = emit_cache.get_emit_code(specifier, source_hash) {
    Ok(emit_code)
  } else {
    // this will use a cached version if it exists
    let parsed_source = parsed_source_cache.get_or_parse_module(
      specifier,
      source.clone(),
      media_type,
    )?;
    let transpiled_source = parsed_source.transpile(emit_options)?;
    debug_assert!(transpiled_source.source_map.is_none());
    emit_cache.set_emit_code(specifier, source_hash, &transpiled_source.text);
    Ok(transpiled_source.text)
  }
}
