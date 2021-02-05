// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use super::text::LineIndex;

/// An lsp representation of an asset in memory, that has either been retrieved
/// from static assets built into Rust, or static assets built into tsc.
#[derive(Debug, Clone)]
pub struct AssetDocument {
  pub text: String,
  pub line_index: LineIndex,
}

#[derive(Debug, Clone, Default)]
pub struct Assets(Arc<Mutex<HashMap<ModuleSpecifier, Option<AssetDocument>>>>);

impl Assets {
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Option<AssetDocument>> {
    self.0.lock().unwrap().get(specifier).cloned()
  }

  pub fn insert(
    &self,
    specifier: ModuleSpecifier,
    maybe_asset: Option<AssetDocument>,
  ) -> Option<Option<AssetDocument>> {
    self.0.lock().unwrap().insert(specifier, maybe_asset)
  }
}
