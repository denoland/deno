// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::ModuleSpecifier;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(any(test, debug_assertions), derive(Debug))]
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
}
