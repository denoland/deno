// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::ParsedSourceCache;

use deno_core::error::AnyError;
use deno_core::ModuleCodeString;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use std::sync::Arc;

pub struct Emitter {
  emit_cache: EmitCache,
  parsed_source_cache: Arc<ParsedSourceCache>,
  emit_options: deno_ast::EmitOptions,
  // cached hash of the emit options
  emit_options_hash: u64,
}

impl Emitter {
  pub fn new(
    emit_cache: EmitCache,
    parsed_source_cache: Arc<ParsedSourceCache>,
    emit_options: deno_ast::EmitOptions,
  ) -> Self {
    let emit_options_hash = FastInsecureHasher::hash(&emit_options);
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
      if let Module::Js(module) = module {
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

  /// Gets a cached emit if the source matches the hash found in the cache.
  pub fn maybe_cached_emit(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
  ) -> Option<String> {
    let source_hash = self.get_source_hash(source);
    self.emit_cache.get_emit_code(specifier, source_hash)
  }

  pub fn emit_parsed_source(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source: &Arc<str>,
  ) -> Result<ModuleCodeString, AnyError> {
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

  /// Expects a file URL, panics otherwise.
  pub async fn load_and_emit_for_hmr(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<String, AnyError> {
    let media_type = MediaType::from_specifier(specifier);
    let source_code = tokio::fs::read_to_string(
      ModuleSpecifier::to_file_path(specifier).unwrap(),
    )
    .await?;
    let source_arc: Arc<str> = source_code.into();
    let parsed_source = self
      .parsed_source_cache
      .get_or_parse_module(specifier, source_arc, media_type)?;
    let mut options = self.emit_options.clone();
    options.inline_source_map = false;
    let transpiled_source = parsed_source.transpile(&options)?;
    Ok(transpiled_source.text)
  }

  /// A hashing function that takes the source code and uses the global emit
  /// options then generates a string hash which can be stored to
  /// determine if the cached emit is valid or not.
  fn get_source_hash(&self, source_text: &str) -> u64 {
    FastInsecureHasher::new()
      .write_str(source_text)
      .write_u64(self.emit_options_hash)
      .finish()
  }
}
