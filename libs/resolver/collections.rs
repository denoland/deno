// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;

use url::Url;

#[allow(clippy::disallowed_types)]
type UrlRc = deno_maybe_sync::MaybeArc<Url>;

/// A map that stores values scoped to a specific directory
/// on the file system.
pub struct FolderScopedMap<TValue> {
  scoped: BTreeMap<UrlRc, TValue>,
}

impl<TValue> std::fmt::Debug for FolderScopedMap<TValue>
where
  TValue: std::fmt::Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FolderScopedMap")
      .field("scoped", &self.scoped)
      .finish()
  }
}

impl<TValue> Default for FolderScopedMap<TValue> {
  fn default() -> Self {
    Self {
      scoped: Default::default(),
    }
  }
}

impl<TValue> FolderScopedMap<TValue> {
  pub fn from_map(map: BTreeMap<UrlRc, TValue>) -> Self {
    Self { scoped: map }
  }

  pub fn count(&self) -> usize {
    self.scoped.len()
  }

  pub fn get_for_specifier(&self, specifier: &Url) -> Option<&TValue> {
    let specifier_str = specifier.as_str();
    self
      .scoped
      .iter()
      .rfind(|(s, _)| specifier_str.starts_with(s.as_str()))
      .map(|(_, v)| v)
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> Option<(&UrlRc, &TValue)> {
    self
      .scoped
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
  }

  pub fn entries_for_specifier<'a>(
    &'a self,
    specifier: &Url,
  ) -> impl Iterator<Item = (&'a UrlRc, &'a TValue)> {
    struct ValueIter<
      'a,
      'b,
      TValue: 'a,
      Iter: Iterator<Item = (&'a UrlRc, &'a TValue)>,
    > {
      previously_found_dir: bool,
      iter: Iter,
      specifier: &'b Url,
    }

    impl<'a, TValue, Iter: Iterator<Item = (&'a UrlRc, &'a TValue)>> Iterator
      for ValueIter<'a, '_, TValue, Iter>
    {
      type Item = (&'a UrlRc, &'a TValue);

      fn next(&mut self) -> Option<Self::Item> {
        for (dir_url, value) in self.iter.by_ref() {
          if !self.specifier.as_str().starts_with(dir_url.as_str()) {
            if self.previously_found_dir {
              break;
            } else {
              continue;
            }
          }
          self.previously_found_dir = true;
          return Some((dir_url, value));
        }

        None
      }
    }

    ValueIter {
      previously_found_dir: false,
      iter: self.scoped.iter().rev(),
      specifier,
    }
  }

  pub fn get_for_scope(&self, scope: &Url) -> Option<&TValue> {
    self.scoped.get(scope)
  }

  pub fn entries(&self) -> impl Iterator<Item = (&UrlRc, &TValue)> {
    self.scoped.iter()
  }

  pub fn values(&self) -> impl Iterator<Item = &TValue> {
    self.scoped.values()
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
      scoped: self
        .scoped
        .iter()
        .map(|(s, v)| Ok((s.clone(), f(v)?)))
        .collect::<Result<_, _>>()?,
    })
  }
}

/// A map that stores values scoped to a specific directory
/// on the file system, but also having the concept of "unscoped"
/// for any folders that land outside.
///
/// The root directory is considered "unscoped" so values that
/// fall outside the other directories land here (ex. remote modules).
pub struct FolderScopedWithUnscopedMap<TValue> {
  pub unscoped: TValue,
  scoped: FolderScopedMap<TValue>,
}

impl<TValue> std::fmt::Debug for FolderScopedWithUnscopedMap<TValue>
where
  TValue: std::fmt::Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FolderScopedWithUnscopedMap")
      .field("unscoped", &self.unscoped)
      .field("scoped", &self.scoped)
      .finish()
  }
}

impl<TValue> Default for FolderScopedWithUnscopedMap<TValue>
where
  TValue: Default,
{
  fn default() -> Self {
    Self::new(Default::default())
  }
}

impl<TValue> FolderScopedWithUnscopedMap<TValue> {
  pub fn new(unscoped: TValue) -> Self {
    Self {
      unscoped,
      scoped: Default::default(),
    }
  }

  pub fn count(&self) -> usize {
    // +1 for unscoped
    self.scoped.count() + 1
  }

  pub fn get_for_specifier(&self, specifier: &Url) -> &TValue {
    self
      .scoped
      .get_for_specifier(specifier)
      .unwrap_or(&self.unscoped)
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> (Option<&UrlRc>, &TValue) {
    self
      .scoped
      .entry_for_specifier(specifier)
      .map(|(s, v)| (Some(s), v))
      .unwrap_or((None, &self.unscoped))
  }

  pub fn get_for_scope(&self, scope: Option<&Url>) -> Option<&TValue> {
    let Some(scope) = scope else {
      return Some(&self.unscoped);
    };
    self.scoped.get_for_scope(scope)
  }

  pub fn entries(&self) -> impl Iterator<Item = (Option<&UrlRc>, &TValue)> {
    [(None, &self.unscoped)]
      .into_iter()
      .chain(self.scoped.entries().map(|(s, v)| (Some(s), v)))
  }

  pub fn insert(&mut self, dir_url: UrlRc, value: TValue) {
    debug_assert!(dir_url.path().ends_with("/")); // must be a dir url
    debug_assert_eq!(dir_url.scheme(), "file");
    self.scoped.insert(dir_url, value);
  }

  pub fn try_map<B, E>(
    &self,
    mut f: impl FnMut(&TValue) -> Result<B, E>,
  ) -> Result<FolderScopedWithUnscopedMap<B>, E> {
    Ok(FolderScopedWithUnscopedMap {
      unscoped: f(&self.unscoped)?,
      scoped: self.scoped.try_map(f)?,
    })
  }
}
