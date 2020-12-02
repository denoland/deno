// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::memory_cache::FileId;

use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
#[allow(unused)]
pub enum CacheStatus {
  Memory(FileId),
  Local(PathBuf),
  Pending,
  Remote(PathBuf),
}

#[derive(Debug, Default)]
pub struct Sources {
  sources: HashMap<ModuleSpecifier, CacheStatus>,
}

// impl Sources {
//   pub fn resolve(_specifier: &str, _containing: &ModuleSpecifier) -> Option<(ModuleSpecifier, CacheStatus)> {
//     None
//   }
// }
