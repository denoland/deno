// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;

use super::CacheType;
use super::Cacher;

/// Emit cache for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecifierEmitCacheData {
  pub source_hash: String,
  pub text: String,
  pub map: Option<String>,
}

pub trait EmitCache {
  /// Gets the emit data from the cache.
  fn get_emit_data(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<SpecifierEmitCacheData>;
  /// Sets the emit data in the cache.
  fn set_emit_data(
    &self,
    specifier: ModuleSpecifier,
    data: SpecifierEmitCacheData,
  ) -> Result<(), AnyError>;
  /// Gets the stored hash of the source of the provider specifier
  /// to tell if the emit is out of sync with the source.
  /// TODO(13302): this is actually not reliable and should be removed
  /// once switching to an sqlite db
  fn get_source_hash(&self, specifier: &ModuleSpecifier) -> Option<String>;
  /// Gets the emitted JavaScript of the TypeScript source.
  /// TODO(13302): remove this once switching to an sqlite db
  fn get_emit_text(&self, specifier: &ModuleSpecifier) -> Option<String>;
}

impl<T: Cacher> EmitCache for T {
  fn get_emit_data(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<SpecifierEmitCacheData> {
    Some(SpecifierEmitCacheData {
      source_hash: self.get_source_hash(specifier)?,
      text: self.get_emit_text(specifier)?,
      map: self.get(CacheType::SourceMap, specifier),
    })
  }

  fn get_source_hash(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.get(CacheType::Version, specifier)
  }

  fn get_emit_text(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.get(CacheType::Emit, specifier)
  }

  fn set_emit_data(
    &self,
    specifier: ModuleSpecifier,
    data: SpecifierEmitCacheData,
  ) -> Result<(), AnyError> {
    self.set(CacheType::Version, &specifier, data.source_hash)?;
    self.set(CacheType::Emit, &specifier, data.text)?;
    if let Some(map) = data.map {
      self.set(CacheType::SourceMap, &specifier, map)?;
    }
    Ok(())
  }
}
