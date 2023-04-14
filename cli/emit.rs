// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::ParsedSourceCache;

use deno_core::error::AnyError;
use deno_core::ModuleCode;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use std::sync::Arc;

#[derive(Clone)]
pub struct Emitter {
  emit_cache: EmitCache,
  parsed_source_cache: ParsedSourceCache,
  emit_options: deno_ast::EmitOptions,
  // cached hash of the emit options
  emit_options_hash: u64,
}

impl Emitter {
  pub fn new(
    emit_cache: EmitCache,
    parsed_source_cache: ParsedSourceCache,
    emit_options: deno_ast::EmitOptions,
  ) -> Self {
    let emit_options_hash = FastInsecureHasher::new()
      .write_hashable(&emit_options)
      .finish();
    Self {
      emit_cache,
      parsed_source_cache,
      emit_options,
      emit_options_hash,
    }
  }

  pub fn cache_module_emits(
    &self,
    graph: &ModuleGraph,
  ) -> Result<(), AnyError> {
    for module in graph.modules() {
      if let Module::Esm(module) = module {
        let is_emittable = matches!(
          module.media_type,
          MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Jsx
            | MediaType::Tsx
        );
        if is_emittable {
          self.emit_parsed_source(
            &module.specifier,
            module.media_type,
            &module.source,
          )?;
        }
      }
    }
    Ok(())
  }

  pub fn emit_parsed_source(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source: &Arc<str>,
  ) -> Result<ModuleCode, AnyError> {
    let source_hash = self.get_source_hash(source);

    if let Some(emit_code) =
      self.emit_cache.get_emit_code(specifier, source_hash)
    {
      Ok(emit_code.into())
    } else {
      // this will use a cached version if it exists
      let parsed_source = self.parsed_source_cache.get_or_parse_module(
        specifier,
        source.clone(),
        media_type,
      )?;
      let transpiled_source = parsed_source.transpile(&self.emit_options)?;
      debug_assert!(transpiled_source.source_map.is_none());
      self.emit_cache.set_emit_code(
        specifier,
        source_hash,
        &transpiled_source.text,
      );
      Ok(transpiled_source.text.into())
    }
  }

  /// A hashing function that takes the source code and uses the global emit
  /// options then generates a string hash which can be stored to
  /// determine if the cached emit is valid or not.
  pub fn get_source_hash(&self, source_text: &str) -> u64 {
    FastInsecureHasher::new()
      .write_str(source_text)
      .write_u64(self.emit_options_hash)
      .finish()
  }
}
