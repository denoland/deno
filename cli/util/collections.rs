// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::sync::Arc;

use deno_core::url::Url;

/// A map that stores values scoped to a specific directory
/// on the file system.
///
/// The root directory is considered "unscoped" so values that
/// fall outside the other directories land here (ex. remote modules).
pub struct FolderScopedMap<TValue> {
  unscoped: TValue,
  scoped: BTreeMap<Arc<Url>, TValue>,
}

impl<TValue> std::fmt::Debug for FolderScopedMap<TValue>
where
  TValue: std::fmt::Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FolderScopedMap")
      .field("unscoped", &self.unscoped)
      .field("scoped", &self.scoped)
      .finish()
  }
}

impl<TValue> Default for FolderScopedMap<TValue>
where
  TValue: Default,
{
  fn default() -> Self {
    Self::new(Default::default())
  }
}

impl<TValue> FolderScopedMap<TValue> {
  pub fn new(unscoped: TValue) -> Self {
    Self {
      unscoped,
      scoped: Default::default(),
    }
  }

  pub fn count(&self) -> usize {
    // +1 for unscoped
    self.scoped.len() + 1
  }

  pub fn get_for_specifier(&self, specifier: &Url) -> &TValue {
    self.get_for_specifier_str(specifier.as_str())
  }

  pub fn get_for_specifier_str(&self, specifier: &str) -> &TValue {
    self
      .scoped
      .iter()
      .rfind(|(s, _)| specifier.starts_with(s.as_str()))
      .map(|(_, v)| v)
      .unwrap_or(&self.unscoped)
  }

  pub fn insert(&mut self, dir_url: Arc<Url>, value: TValue) {
    debug_assert!(dir_url.path().ends_with("/")); // must be a dir url
    debug_assert_eq!(dir_url.scheme(), "file");
    self.scoped.insert(dir_url, value);
  }
}
