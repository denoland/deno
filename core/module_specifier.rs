// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::normalize_path;
use std::env::current_dir;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use url::ParseError;
use url::Url;

/// Error indicating the reason resolving a module specifier failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleResolutionError {
  InvalidUrl(ParseError),
  InvalidBaseUrl(ParseError),
  InvalidPath(PathBuf),
  ImportPrefixMissing(String, Option<String>),
}
use ModuleResolutionError::*;

impl Error for ModuleResolutionError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      InvalidUrl(ref err) | InvalidBaseUrl(ref err) => Some(err),
      _ => None,
    }
  }
}

impl fmt::Display for ModuleResolutionError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      InvalidUrl(ref err) => write!(f, "invalid URL: {}", err),
      InvalidBaseUrl(ref err) => {
        write!(f, "invalid base URL for relative import: {}", err)
      }
      InvalidPath(ref path) => write!(f, "invalid module path: {:?}", path),
      ImportPrefixMissing(ref specifier, ref maybe_referrer) => write!(
        f,
        "relative import path \"{}\" not prefixed with / or ./ or ../{}",
        specifier,
        match maybe_referrer {
          Some(referrer) => format!(" Imported from \"{}\"", referrer),
          None => format!(""),
        }
      ),
    }
  }
}

#[derive(
  Debug, Clone, Eq, Hash, PartialEq, serde::Serialize, Ord, PartialOrd,
)]
/// Resolved module specifier
pub struct ModuleSpecifier(Url);

impl ModuleSpecifier {
  fn is_dummy_specifier(specifier: &str) -> bool {
    specifier == "<unknown>"
  }

  pub fn as_url(&self) -> &Url {
    &self.0
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }

  /// Resolves module using this algorithm:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  pub fn resolve_import(
    specifier: &str,
    base: &str,
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    let url = match Url::parse(specifier) {
      // 1. Apply the URL parser to specifier.
      //    If the result is not failure, return he result.
      Ok(url) => url,

      // 2. If specifier does not start with the character U+002F SOLIDUS (/),
      //    the two-character sequence U+002E FULL STOP, U+002F SOLIDUS (./),
      //    or the three-character sequence U+002E FULL STOP, U+002E FULL STOP,
      //    U+002F SOLIDUS (../), return failure.
      Err(ParseError::RelativeUrlWithoutBase)
        if !(specifier.starts_with('/')
          || specifier.starts_with("./")
          || specifier.starts_with("../")) =>
      {
        let maybe_referrer = if base.is_empty() {
          None
        } else {
          Some(base.to_string())
        };
        return Err(ImportPrefixMissing(specifier.to_string(), maybe_referrer));
      }

      // 3. Return the result of applying the URL parser to specifier with base
      //    URL as the base URL.
      Err(ParseError::RelativeUrlWithoutBase) => {
        let base = if ModuleSpecifier::is_dummy_specifier(base) {
          // Handle <unknown> case, happening under e.g. repl.
          // Use CWD for such case.

          // Forcefully join base to current dir.
          // Otherwise, later joining in Url would be interpreted in
          // the parent directory (appending trailing slash does not work)
          let path = current_dir().unwrap().join(base);
          Url::from_file_path(path).unwrap()
        } else {
          Url::parse(base).map_err(InvalidBaseUrl)?
        };
        base.join(&specifier).map_err(InvalidUrl)?
      }

      // If parsing the specifier as a URL failed for a different reason than
      // it being relative, always return the original error. We don't want to
      // return `ImportPrefixMissing` or `InvalidBaseUrl` if the real
      // problem lies somewhere else.
      Err(err) => return Err(InvalidUrl(err)),
    };

    Ok(ModuleSpecifier(url))
  }

  /// Converts a string representing an absolute URL into a ModuleSpecifier.
  pub fn resolve_url(
    url_str: &str,
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    Url::parse(url_str)
      .map(ModuleSpecifier)
      .map_err(ModuleResolutionError::InvalidUrl)
  }

  /// Takes a string representing either an absolute URL or a file path,
  /// as it may be passed to deno as a command line argument.
  /// The string is interpreted as a URL if it starts with a valid URI scheme,
  /// e.g. 'http:' or 'file:' or 'git+ssh:'. If not, it's interpreted as a
  /// file path; if it is a relative path it's resolved relative to the current
  /// working directory.
  pub fn resolve_url_or_path(
    specifier: &str,
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    if Self::specifier_has_uri_scheme(specifier) {
      Self::resolve_url(specifier)
    } else {
      Self::resolve_path(specifier)
    }
  }

  /// Converts a string representing a relative or absolute path into a
  /// ModuleSpecifier. A relative path is considered relative to the current
  /// working directory.
  pub fn resolve_path(
    path_str: &str,
  ) -> Result<ModuleSpecifier, ModuleResolutionError> {
    let path = current_dir().unwrap().join(path_str);
    let path = normalize_path(&path);
    Url::from_file_path(path.clone())
      .map(ModuleSpecifier)
      .map_err(|()| ModuleResolutionError::InvalidPath(path))
  }

  /// Returns true if the input string starts with a sequence of characters
  /// that could be a valid URI scheme, like 'https:', 'git+ssh:' or 'data:'.
  ///
  /// According to RFC 3986 (https://tools.ietf.org/html/rfc3986#section-3.1),
  /// a valid scheme has the following format:
  ///   scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
  ///
  /// We additionally require the scheme to be at least 2 characters long,
  /// because otherwise a windows path like c:/foo would be treated as a URL,
  /// while no schemes with a one-letter name actually exist.
  fn specifier_has_uri_scheme(specifier: &str) -> bool {
    let mut chars = specifier.chars();
    let mut len = 0usize;
    // THe first character must be a letter.
    match chars.next() {
      Some(c) if c.is_ascii_alphabetic() => len += 1,
      _ => return false,
    }
    // Second and following characters must be either a letter, number,
    // plus sign, minus sign, or dot.
    loop {
      match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() || "+-.".contains(c) => len += 1,
        Some(':') if len >= 2 => return true,
        _ => return false,
      }
    }
  }
}

impl fmt::Display for ModuleSpecifier {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl From<Url> for ModuleSpecifier {
  fn from(url: Url) -> Self {
    ModuleSpecifier(url)
  }
}

impl PartialEq<String> for ModuleSpecifier {
  fn eq(&self, other: &String) -> bool {
    &self.to_string() == other
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  #[test]
  fn test_resolve_import() {
    fn get_path(specifier: &str) -> Url {
      let base_path = current_dir().unwrap().join("<unknown>");
      let base_url = Url::from_file_path(base_path).unwrap();
      base_url.join(specifier).unwrap()
    }
    let awesome = get_path("/awesome.ts");
    let awesome_srv = get_path("/service/awesome.ts");
    let tests = vec![
      ("/awesome.ts", "<unknown>", awesome.as_str()),
      ("/service/awesome.ts", "<unknown>", awesome_srv.as_str()),
      (
        "./005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/005_more_imports.ts",
      ),
      (
        "../005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/005_more_imports.ts",
      ),
      (
        "http://deno.land/core/tests/005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/005_more_imports.ts",
      ),
      (
        "data:text/javascript,export default 'grapes';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "data:text/javascript,export default 'grapes';",
      ),
      (
        "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
        "http://deno.land/core/tests/006_url_imports.ts",
        "blob:https://whatwg.org/d0360e2f-caee-469f-9a2f-87d5b0456f6f",
      ),
      (
        "javascript:export default 'artichokes';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "javascript:export default 'artichokes';",
      ),
      (
        "data:text/plain,export default 'kale';",
        "http://deno.land/core/tests/006_url_imports.ts",
        "data:text/plain,export default 'kale';",
      ),
      (
        "/dev/core/tests/005_more_imports.ts",
        "file:///home/yeti",
        "file:///dev/core/tests/005_more_imports.ts",
      ),
      (
        "//zombo.com/1999.ts",
        "https://cherry.dev/its/a/thing",
        "https://zombo.com/1999.ts",
      ),
      (
        "http://deno.land/this/url/is/valid",
        "base is clearly not a valid url",
        "http://deno.land/this/url/is/valid",
      ),
      (
        "//server/some/dir/file",
        "file:///home/yeti/deno",
        "file://server/some/dir/file",
      ),
      // This test is disabled because the url crate does not follow the spec,
      // dropping the server part from the final result.
      // (
      //   "/another/path/at/the/same/server",
      //   "file://server/some/dir/file",
      //   "file://server/another/path/at/the/same/server",
      // ),
    ];

    for (specifier, base, expected_url) in tests {
      let url = ModuleSpecifier::resolve_import(specifier, base)
        .unwrap()
        .to_string();
      assert_eq!(url, expected_url);
    }
  }

  #[test]
  fn test_resolve_import_error() {
    use url::ParseError::*;
    use ModuleResolutionError::*;

    let tests = vec![
      (
        "awesome.ts",
        "<unknown>",
        ImportPrefixMissing(
          "awesome.ts".to_string(),
          Some("<unknown>".to_string()),
        ),
      ),
      (
        "005_more_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing(
          "005_more_imports.ts".to_string(),
          Some("http://deno.land/core/tests/006_url_imports.ts".to_string()),
        ),
      ),
      (
        ".tomato",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing(
          ".tomato".to_string(),
          Some("http://deno.land/core/tests/006_url_imports.ts".to_string()),
        ),
      ),
      (
        "..zucchini.mjs",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing(
          "..zucchini.mjs".to_string(),
          Some("http://deno.land/core/tests/006_url_imports.ts".to_string()),
        ),
      ),
      (
        r".\yam.es",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing(
          r".\yam.es".to_string(),
          Some("http://deno.land/core/tests/006_url_imports.ts".to_string()),
        ),
      ),
      (
        r"..\yam.es",
        "http://deno.land/core/tests/006_url_imports.ts",
        ImportPrefixMissing(
          r"..\yam.es".to_string(),
          Some("http://deno.land/core/tests/006_url_imports.ts".to_string()),
        ),
      ),
      (
        "https://eggplant:b/c",
        "http://deno.land/core/tests/006_url_imports.ts",
        InvalidUrl(InvalidPort),
      ),
      (
        "https://eggplant@/c",
        "http://deno.land/core/tests/006_url_imports.ts",
        InvalidUrl(EmptyHost),
      ),
      (
        "./foo.ts",
        "/relative/base/url",
        InvalidBaseUrl(RelativeUrlWithoutBase),
      ),
    ];

    for (specifier, base, expected_err) in tests {
      let err = ModuleSpecifier::resolve_import(specifier, base).unwrap_err();
      assert_eq!(err, expected_err);
    }
  }

  #[test]
  fn test_resolve_url_or_path() {
    // Absolute URL.
    let mut tests: Vec<(&str, String)> = vec![
      (
        "http://deno.land/core/tests/006_url_imports.ts",
        "http://deno.land/core/tests/006_url_imports.ts".to_string(),
      ),
      (
        "https://deno.land/core/tests/006_url_imports.ts",
        "https://deno.land/core/tests/006_url_imports.ts".to_string(),
      ),
    ];

    // The local path tests assume that the cwd is the deno repo root.
    let cwd = current_dir().unwrap();
    let cwd_str = cwd.to_str().unwrap();

    if cfg!(target_os = "windows") {
      // Absolute local path.
      let expected_url = "file:///C:/deno/tests/006_url_imports.ts";
      tests.extend(vec![
        (
          r"C:/deno/tests/006_url_imports.ts",
          expected_url.to_string(),
        ),
        (
          r"C:\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        (
          r"\\?\C:\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        // Not supported: `Url::from_file_path()` fails.
        // (r"\\.\C:\deno\tests\006_url_imports.ts", expected_url.to_string()),
        // Not supported: `Url::from_file_path()` performs the wrong conversion.
        // (r"//./C:/deno/tests/006_url_imports.ts", expected_url.to_string()),
      ]);

      // Rooted local path without drive letter.
      let expected_url = format!(
        "file:///{}:/deno/tests/006_url_imports.ts",
        cwd_str.get(..1).unwrap(),
      );
      tests.extend(vec![
        (r"/deno/tests/006_url_imports.ts", expected_url.to_string()),
        (r"\deno\tests\006_url_imports.ts", expected_url.to_string()),
        (
          r"\deno\..\deno\tests\006_url_imports.ts",
          expected_url.to_string(),
        ),
        (r"\deno\.\tests\006_url_imports.ts", expected_url),
      ]);

      // Relative local path.
      let expected_url = format!(
        "file:///{}/tests/006_url_imports.ts",
        cwd_str.replace("\\", "/")
      );
      tests.extend(vec![
        (r"tests/006_url_imports.ts", expected_url.to_string()),
        (r"tests\006_url_imports.ts", expected_url.to_string()),
        (r"./tests/006_url_imports.ts", (*expected_url).to_string()),
        (r".\tests\006_url_imports.ts", (*expected_url).to_string()),
      ]);

      // UNC network path.
      let expected_url = "file://server/share/deno/cool";
      tests.extend(vec![
        (r"\\server\share\deno\cool", expected_url.to_string()),
        (r"\\server/share/deno/cool", expected_url.to_string()),
        // Not supported: `Url::from_file_path()` performs the wrong conversion.
        // (r"//server/share/deno/cool", expected_url.to_string()),
      ]);
    } else {
      // Absolute local path.
      let expected_url = "file:///deno/tests/006_url_imports.ts";
      tests.extend(vec![
        ("/deno/tests/006_url_imports.ts", expected_url.to_string()),
        ("//deno/tests/006_url_imports.ts", expected_url.to_string()),
      ]);

      // Relative local path.
      let expected_url = format!("file://{}/tests/006_url_imports.ts", cwd_str);
      tests.extend(vec![
        ("tests/006_url_imports.ts", expected_url.to_string()),
        ("./tests/006_url_imports.ts", expected_url.to_string()),
        (
          "tests/../tests/006_url_imports.ts",
          expected_url.to_string(),
        ),
        ("tests/./006_url_imports.ts", expected_url),
      ]);
    }

    for (specifier, expected_url) in tests {
      let url = ModuleSpecifier::resolve_url_or_path(specifier)
        .unwrap()
        .to_string();
      assert_eq!(url, expected_url);
    }
  }

  #[test]
  fn test_resolve_url_or_path_error() {
    use url::ParseError::*;
    use ModuleResolutionError::*;

    let mut tests = vec![
      ("https://eggplant:b/c", InvalidUrl(InvalidPort)),
      ("https://:8080/a/b/c", InvalidUrl(EmptyHost)),
    ];
    if cfg!(target_os = "windows") {
      let p = r"\\.\c:/stuff/deno/script.ts";
      tests.push((p, InvalidPath(PathBuf::from(p))));
    }

    for (specifier, expected_err) in tests {
      let err = ModuleSpecifier::resolve_url_or_path(specifier).unwrap_err();
      assert_eq!(err, expected_err);
    }
  }

  #[test]
  fn test_specifier_has_uri_scheme() {
    let tests = vec![
      ("http://foo.bar/etc", true),
      ("HTTP://foo.bar/etc", true),
      ("http:ftp:", true),
      ("http:", true),
      ("hTtP:", true),
      ("ftp:", true),
      ("mailto:spam@please.me", true),
      ("git+ssh://git@github.com/denoland/deno", true),
      ("blob:https://whatwg.org/mumbojumbo", true),
      ("abc.123+DEF-ghi:", true),
      ("abc.123+def-ghi:@", true),
      ("", false),
      (":not", false),
      ("http", false),
      ("c:dir", false),
      ("X:", false),
      ("./http://not", false),
      ("1abc://kinda/but/no", false),
      ("schluẞ://no/more", false),
    ];

    for (specifier, expected) in tests {
      let result = ModuleSpecifier::specifier_has_uri_scheme(specifier);
      assert_eq!(result, expected);
    }
  }

  #[test]
  fn test_normalize_path() {
    assert_eq!(normalize_path(Path::new("a/../b")), PathBuf::from("b"));
    assert_eq!(normalize_path(Path::new("a/./b/")), PathBuf::from("a/b/"));
    assert_eq!(
      normalize_path(Path::new("a/./b/../c")),
      PathBuf::from("a/c")
    );

    if cfg!(windows) {
      assert_eq!(
        normalize_path(Path::new("C:\\a\\.\\b\\..\\c")),
        PathBuf::from("C:\\a\\c")
      );
    }
  }
}
