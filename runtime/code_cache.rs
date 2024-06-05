// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;

pub enum CodeCacheType {
  EsModule,
  Script,
}

impl CodeCacheType {
  pub fn as_str(&self) -> &str {
    match self {
      Self::EsModule => "esmodule",
      Self::Script => "script",
    }
  }
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
