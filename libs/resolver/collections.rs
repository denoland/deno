// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;

use url::Url;

#[allow(clippy::disallowed_types)]
type UrlRc = crate::sync::MaybeArc<Url>;

/// A map that stores values scoped to a specific directory
/// on the file system.
///
/// The root directory is considered "unscoped" so values that
/// fall outside the other directories land here (ex. remote modules).
pub struct FolderScopedMap<TValue> {
  pub unscoped: TValue,
  scoped: BTreeMap<UrlRc, TValue>,
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
    let specifier_str = specifier.as_str();
    self
      .scoped
      .iter()
      .rfind(|(s, _)| specifier_str.starts_with(s.as_str()))
      .map(|(_, v)| v)
      .unwrap_or(&self.unscoped)
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> (Option<&UrlRc>, &TValue) {
    self
      .scoped
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .map(|(s, v)| (Some(s), v))
      .unwrap_or((None, &self.unscoped))
  }

  pub fn get_for_scope(&self, scope: Option<&Url>) -> Option<&TValue> {
    let Some(scope) = scope else {
      return Some(&self.unscoped);
    };
    self.scoped.get(scope)
  }

  pub fn entries(&self) -> impl Iterator<Item = (Option<&UrlRc>, &TValue)> {
    [(None, &self.unscoped)]
      .into_iter()
      .chain(self.scoped.iter().map(|(s, v)| (Some(s), v)))
  }

  pub fn insert(&mut self, dir_url: UrlRc, value: TValue) {
    debug_assert!(dir_url.path().ends_with("/")); // must be a dir url
    debug_assert_eq!(dir_url.scheme(), "file");
    self.scoped.insert(dir_url, value);
  }

  pub fn try_map<B, E>(
    &self,
    mut f: impl FnMut(&TValue) -> Result<B, E>,
  ) -> Result<FolderScopedMap<B>, E> {
    Ok(FolderScopedMap {
      unscoped: f(&self.unscoped)?,
      scoped: self
        .scoped
        .iter()
        .map(|(s, v)| Ok((s.clone(), f(v)?)))
        .collect::<Result<_, _>>()?,
    })
  }
}
