// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use url::Url;

#[derive(Debug, Clone)]
pub enum UrlOrPath {
  Url(Url),
  Path(PathBuf),
}

impl UrlOrPath {
  pub fn is_file(&self) -> bool {
    match self {
      UrlOrPath::Url(url) => url.scheme() == "file",
      UrlOrPath::Path(_) => true,
    }
  }

  pub fn is_node_url(&self) -> bool {
    match self {
      UrlOrPath::Url(url) => url.scheme() == "node",
      UrlOrPath::Path(_) => false,
    }
  }

  pub fn into_path(
    self,
  ) -> Result<PathBuf, deno_path_util::UrlToFilePathError> {
    match self {
      UrlOrPath::Url(url) => deno_path_util::url_to_file_path(&url),
      UrlOrPath::Path(path) => Ok(path),
    }
  }

  pub fn into_url(self) -> Result<Url, deno_path_util::PathToUrlError> {
    match self {
      UrlOrPath::Url(url) => Ok(url),
      UrlOrPath::Path(path) => deno_path_util::url_from_file_path(&path),
    }
  }

  pub fn to_string_lossy(&self) -> Cow<str> {
    match self {
      UrlOrPath::Url(url) => Cow::Borrowed(url.as_str()),
      UrlOrPath::Path(path) => path.to_string_lossy(),
    }
  }
}

impl std::fmt::Display for UrlOrPath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      UrlOrPath::Url(url) => url.fmt(f),
      UrlOrPath::Path(path) => {
        // prefer displaying a url
        match deno_path_util::url_from_file_path(path) {
          Ok(url) => url.fmt(f),
          Err(_) => {
            write!(f, "{}", path.display())
          }
        }
      }
    }
  }
}

pub struct UrlOrPathRef<'a> {
  url: once_cell::unsync::OnceCell<Cow<'a, Url>>,
  path: once_cell::unsync::OnceCell<Cow<'a, Path>>,
}

impl<'a> UrlOrPathRef<'a> {
  pub fn from_path(path: &'a Path) -> Self {
    Self {
      url: Default::default(),
      path: once_cell::unsync::OnceCell::with_value(Cow::Borrowed(path)),
    }
  }

  pub fn from_url(url: &'a Url) -> Self {
    Self {
      path: Default::default(),
      url: once_cell::unsync::OnceCell::with_value(Cow::Borrowed(url)),
    }
  }

  pub fn url(&self) -> Result<&Url, deno_path_util::PathToUrlError> {
    self
      .url
      .get_or_try_init(|| {
        deno_path_util::url_from_file_path(self.path.get().unwrap())
          .map(Cow::Owned)
      })
      .map(|v| v.as_ref())
  }

  pub fn path(&self) -> Result<&Path, deno_path_util::UrlToFilePathError> {
    self
      .path
      .get_or_try_init(|| {
        deno_path_util::url_to_file_path(self.url.get().unwrap())
          .map(Cow::Owned)
      })
      .map(|v| v.as_ref())
  }

  pub fn display(&self) -> UrlOrPath {
    // prefer url
    if let Ok(url) = self.url() {
      UrlOrPath::Url(url.clone())
    } else {
      // this will always be set if url is None
      UrlOrPath::Path(self.path.get().unwrap().to_path_buf())
    }
  }
}

/// Extension to path_clean::PathClean
pub trait PathClean<T> {
  fn clean(&self) -> T;
}

impl PathClean<PathBuf> for PathBuf {
  fn clean(&self) -> PathBuf {
    fn is_clean_path(path: &Path) -> bool {
      let path = path.to_string_lossy();
      let mut current_index = 0;
      while let Some(index) = path[current_index..].find("\\.") {
        let trailing_index = index + current_index + 2;
        let mut trailing_chars = path[trailing_index..].chars();
        match trailing_chars.next() {
          Some('.') => match trailing_chars.next() {
            Some('/') | Some('\\') | None => {
              return false;
            }
            _ => {}
          },
          Some('/') | Some('\\') => {
            return false;
          }
          _ => {}
        }
        current_index = trailing_index;
      }
      true
    }

    let path = path_clean::PathClean::clean(self);
    if cfg!(windows) && !is_clean_path(&path) {
      // temporary workaround because path_clean::PathClean::clean is
      // not good enough on windows
      let mut components = Vec::new();

      for component in path.components() {
        match component {
          Component::CurDir => {
            // skip
          }
          Component::ParentDir => {
            let maybe_last_component = components.pop();
            if !matches!(maybe_last_component, Some(Component::Normal(_))) {
              panic!("Error normalizing: {}", path.display());
            }
          }
          Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
            components.push(component);
          }
        }
      }
      components.into_iter().collect::<PathBuf>()
    } else {
      path
    }
  }
}

#[cfg(test)]
mod test {
  #[cfg(windows)]
  #[test]
  fn test_path_clean() {
    use super::*;

    run_test("C:\\test\\./file.txt", "C:\\test\\file.txt");
    run_test("C:\\test\\../other/file.txt", "C:\\other\\file.txt");
    run_test("C:\\test\\../other\\file.txt", "C:\\other\\file.txt");

    fn run_test(input: &str, expected: &str) {
      assert_eq!(PathBuf::from(input).clean(), PathBuf::from(expected));
    }
  }
}
