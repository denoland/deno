// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;

pub enum CodeCacheType {
  EsModule,
  Script,
}

pub trait CodeCache: Send + Sync {
  fn get_sync(
    &self,
    specifier: &ModuleSpecifier,
    code_cache_type: CodeCacheType,
    source_hash: u64,
  ) -> Option<Vec<u8>>;
  fn set_sync(
    &self,
    specifier: ModuleSpecifier,
    code_cache_type: CodeCacheType,
    source_hash: u64,
    data: &[u8],
  );

  /// Gets if the code cache is still enabled.
  fn enabled(&self) -> bool {
    true
  }
}
