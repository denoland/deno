// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
    specifier: &str,
    code_cache_type: CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
  ) -> Option<Vec<u8>>;
  fn set_sync(
    &self,
    specifier: &str,
    code_cache_type: CodeCacheType,
    source_hash: Option<&str>,
    source_timestamp: Option<u64>,
    data: &[u8],
  );
}
