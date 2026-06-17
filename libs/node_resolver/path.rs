// Copyright 2018-2026 the Deno authors. MIT license.

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

  pub fn to_string_lossy(&self) -> Cow<'_, str> {
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
    if cfg!(windows) {
      // `path_clean::clean` is purely lexical and only splits on `/`, so on
      // Windows it mishandles paths that mix `\` separators with
      // `/`-separated `..` segments. This happens when a backslash base path
      // is joined with a forward-slash relative specifier coming from JS
      // source (e.g. `require("../../types")`), producing a path like
      // `C:\pkg\dist\cjs\api\max\../../types`. `path_clean` then treats the
      // whole `C:\...\max\..` prefix as a single segment and the following
      // `..` backtracks over all of it, collapsing the path down to just
      // `types`. Walk the components ourselves instead, which correctly
      // understands both separators and the path prefix.
      clean_via_components(self)
    } else {
      path_clean::PathClean::clean(self)
    }
  }
}

/// Lexically normalize a path by walking its components, eliminating `.`
/// elements and resolving `..` elements against the preceding component. This
/// mirrors `path_clean::clean`'s semantics but, unlike that crate, correctly
/// handles Windows paths that mix `\` and `/` separators (`Path::components`
/// treats both as separators on Windows). `..` segments that would escape the
/// root are dropped, and leading `..` segments on a relative path are kept.
#[cfg_attr(not(windows), allow(dead_code, reason = "only used on windows"))]
fn clean_via_components(path: &Path) -> PathBuf {
  let mut components: Vec<Component> = Vec::new();
  for component in path.components() {
    match component {
      Component::CurDir => {
        // skip `.`
      }
      Component::ParentDir => match components.last() {
        Some(Component::Normal(_)) => {
          components.pop();
        }
        // can't go above the root, so drop the `..`
        Some(Component::RootDir | Component::Prefix(_)) => {}
        // leading `..` on a relative path (or after another `..`) is kept
        _ => components.push(component),
      },
      Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
        components.push(component);
      }
    }
  }
  if components.is_empty() {
    return PathBuf::from(".");
  }
  components.into_iter().collect::<PathBuf>()
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
    // Backslash base path joined with a forward-slash relative specifier
    // containing two or more `..` segments. Regression test for
    // https://github.com/denoland/deno/issues/29910 where this collapsed
    // down to just `types` because `path_clean` only splits on `/`.
    run_test("C:\\a\\b\\c\\d\\../../types", "C:\\a\\b\\types");
    run_test("C:\\a\\b\\c\\d\\../../../types", "C:\\a\\types");
    run_test("C:\\a\\b\\c\\../../helpers/x", "C:\\a\\helpers\\x");
    // `..` at/above the root is dropped.
    run_test("C:\\a\\../../types", "C:\\types");
    run_test("C:\\..", "C:\\");

    fn run_test(input: &str, expected: &str) {
      assert_eq!(PathBuf::from(input).clean(), PathBuf::from(expected));
    }
  }

  #[test]
  fn test_clean_via_components() {
    use super::*;

    // Portable coverage of the component-walking normalizer used on Windows.
    // Uses `/` separators so it parses identically on every platform; the
    // `#[cfg(windows)]` test above covers backslash/prefix handling.
    let cases = [
      ("test/path/..", "test"),
      ("test/../path", "path"),
      ("test/path/../../another/path", "another/path"),
      ("./test/./path", "test/path"),
      ("/test/../path", "/path"),
      ("/test/path/../../../..", "/"),
      ("test/path/../../../..", "../.."),
      ("../test/path", "../test/path"),
      ("a/b/c/d/../../types", "a/b/types"),
      ("test/..", "."),
    ];
    for (input, expected) in cases {
      assert_eq!(
        clean_via_components(Path::new(input)),
        PathBuf::from(expected),
        "input: {input}"
      );
    }
  }
}
