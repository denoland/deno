// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::cache::HttpCache;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::sync::Arc;

pub const DOCUMENT_SCHEMES: [&str; 8] = [
  "data",
  "blob",
  "file",
  "http",
  "https",
  "untitled",
  "deno-notebook-cell",
  "vscode-notebook-cell",
];

#[derive(Debug)]
pub struct SpecifierResolver {
  cache: Arc<dyn HttpCache>,
  redirects: Mutex<HashMap<ModuleSpecifier, ModuleSpecifier>>,
}

impl SpecifierResolver {
  pub fn new(cache: Arc<dyn HttpCache>) -> Self {
    Self {
      cache,
      redirects: Mutex::new(HashMap::new()),
    }
  }

  pub fn resolve(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let scheme = specifier.scheme();
    if !DOCUMENT_SCHEMES.contains(&scheme) {
      return None;
    }

    if scheme == "http" || scheme == "https" {
      let mut redirects = self.redirects.lock();
      if let Some(specifier) = redirects.get(specifier) {
        Some(specifier.clone())
      } else {
        let redirect = self.resolve_remote(specifier, 10)?;
        redirects.insert(specifier.clone(), redirect.clone());
        Some(redirect)
      }
    } else {
      Some(specifier.clone())
    }
  }

  fn resolve_remote(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: usize,
  ) -> Option<ModuleSpecifier> {
    if redirect_limit > 0 {
      let cache_key = self.cache.cache_item_key(specifier).ok()?;
      let headers = self
        .cache
        .read_metadata(&cache_key)
        .ok()
        .flatten()
        .map(|m| m.headers)?;
      if let Some(location) = headers.get("location") {
        let redirect =
          deno_core::resolve_import(location, specifier.as_str()).ok()?;
        self.resolve_remote(&redirect, redirect_limit - 1)
      } else {
        Some(specifier.clone())
      }
    } else {
      None
    }
  }
}
