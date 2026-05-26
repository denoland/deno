// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::glob::PathGlobMatch;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;

/// Checks if the path has an extension Deno supports for script execution.
pub fn is_script_ext(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(
      ext.as_str(),
      "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" | "cjs" | "cts"
    )
  } else {
    false
  }
}

/// Checks if the path has an extension Deno supports for importing.
pub fn is_importable_ext(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(
      ext.as_str(),
      "ts"
        | "tsx"
        | "js"
        | "jsx"
        | "mjs"
        | "mts"
        | "cjs"
        | "cts"
        | "json"
        | "wasm"
    )
  } else {
    false
  }
}

/// Get the extension of a file in lowercase.
pub fn get_extension(file_path: &Path) -> Option<String> {
  file_path
    .extension()
    .and_then(|e| e.to_str())
    .map(|e| e.to_lowercase())
}

/// TypeScript figures out the type of file based on the extension, but we take
/// other factors into account like the file headers. The hack here is to map the
/// specifier passed to TypeScript to a new specifier with the file extension.
pub fn mapped_specifier_for_tsc(
  specifier: &ModuleSpecifier,
  media_type: MediaType,
) -> Option<String> {
  let ext_media_type = MediaType::from_specifier(specifier);
  if media_type != ext_media_type {
    // we can't just add on the extension because typescript considers
    // all .d.*.ts files as declaration files in TS 5.0+
    if media_type != MediaType::Dts
      && media_type == MediaType::TypeScript
      && specifier
        .path()
        .split('/')
        .next_back()
        .map(|last| last.contains(".d."))
        .unwrap_or(false)
    {
      let mut path_parts = specifier
        .path()
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
      let last_part = path_parts.last_mut().unwrap();
      *last_part = last_part.replace(".d.", "$d$");
      let mut specifier = specifier.clone();
      specifier.set_path(&path_parts.join("/"));
      Some(format!("{}{}", specifier, media_type.as_ts_extension()))
    } else {
      Some(format!("{}{}", specifier, media_type.as_ts_extension()))
    }
  } else {
    None
  }
}

/// `from.make_relative(to)` but with fixes.
pub fn relative_specifier(
  from: &ModuleSpecifier,
  to: &ModuleSpecifier,
) -> Option<String> {
  let is_dir = to.path().ends_with('/');

  if is_dir && from == to {
    return Some("./".to_string());
  }

  // workaround for url crate not adding a trailing slash for a directory
  // it seems to be fixed once a version greater than 2.2.2 is released
  let text = from.make_relative(to)?;

  let text = if text.starts_with("../") || text.starts_with("./") {
    text
  } else {
    format!("./{text}")
  };
  Some(to_percent_decoded_str(&text))
}

pub fn relative_specifier_path_for_display(
  from: &ModuleSpecifier,
  to: &ModuleSpecifier,
) -> String {
  if to.scheme() == "file" && from.scheme() == "file" {
    let relative_specifier = relative_specifier(from, to)
      .map(Cow::Owned)
      .unwrap_or_else(|| Cow::Borrowed(to.as_str()));
    let relative_specifier = if relative_specifier.starts_with("../../../") {
      to.as_str()
    } else {
      relative_specifier.trim_start_matches("./")
    };
    to_percent_decoded_str(relative_specifier)
  } else {
    to_percent_decoded_str(to.as_str())
  }
}

pub fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
  if path.is_absolute() != base.is_absolute() {
    return if path.is_absolute() {
      Some(path.to_path_buf())
    } else {
      None
    };
  }

  let mut path_components = path.components();
  let mut base_components = base.components();
  let mut components = Vec::new();
  loop {
    match (path_components.next(), base_components.next()) {
      (None, None) => break,
      (Some(component), None) => {
        components.push(component);
        components.extend(path_components);
        break;
      }
      (None, Some(_)) => components.push(Component::ParentDir),
      (Some(path_component), Some(base_component))
        if components.is_empty() && path_component == base_component => {}
      (Some(path_component), Some(Component::CurDir)) => {
        components.push(path_component)
      }
      (Some(_), Some(Component::ParentDir)) => return None,
      (Some(path_component), Some(_)) => {
        components.push(Component::ParentDir);
        components.extend(base_components.map(|_| Component::ParentDir));
        components.push(path_component);
        components.extend(path_components);
        break;
      }
    }
  }

  Some(components.iter().map(|c| c.as_os_str()).collect())
}

/// Slightly different behaviour than the default matching
/// where an exact path needs to be matched to be opted-in
/// rather than just a partial directory match.
///
/// This is used by the test and bench filtering.
pub fn matches_pattern_or_exact_path(
  path_or_pattern_set: &PathOrPatternSet,
  path: &Path,
) -> bool {
  for p in path_or_pattern_set.inner().iter().rev() {
    match p {
      PathOrPattern::Path(p) => {
        if p == path {
          return true;
        }
      }
      PathOrPattern::NegatedPath(p) => {
        if path.starts_with(p) {
          return false;
        }
      }
      PathOrPattern::RemoteUrl(_) => {}
      PathOrPattern::Pattern(p) => match p.matches_path(path) {
        PathGlobMatch::Matched => return true,
        PathGlobMatch::MatchedNegated => return false,
        PathGlobMatch::NotMatched => {}
      },
    }
  }
  false
}

/// For decoding percent-encodeing string
/// could be used for module specifier string literal of local modules,
/// or local file path to display `non-ASCII` characters correctly
/// # Examples
/// ```
/// use crate::util::path::to_percent_decoded_str;
///
/// let str = to_percent_decoded_str("file:///Users/path/to/%F0%9F%A6%95.ts");
/// assert_eq!(str, "file:///Users/path/to/🦕.ts");
/// ```
pub fn to_percent_decoded_str(s: &str) -> String {
  match percent_encoding::percent_decode_str(s).decode_utf8() {
    Ok(s) => s.to_string(),
    // when failed to decode, return the original string
    Err(_) => s.to_string(),
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_is_script_ext() {
    assert!(!is_script_ext(Path::new("tests/subdir/redirects")));
    assert!(!is_script_ext(Path::new("README.md")));
    assert!(is_script_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_script_ext(Path::new("testdata/run/001_hello.js")));
    assert!(is_script_ext(Path::new("testdata/run/002_hello.ts")));
    assert!(is_script_ext(Path::new("foo.jsx")));
    assert!(is_script_ext(Path::new("foo.tsx")));
    assert!(is_script_ext(Path::new("foo.TS")));
    assert!(is_script_ext(Path::new("foo.TSX")));
    assert!(is_script_ext(Path::new("foo.JS")));
    assert!(is_script_ext(Path::new("foo.JSX")));
    assert!(is_script_ext(Path::new("foo.mjs")));
    assert!(is_script_ext(Path::new("foo.mts")));
    assert!(is_script_ext(Path::new("foo.cjs")));
    assert!(is_script_ext(Path::new("foo.cts")));
    assert!(!is_script_ext(Path::new("foo.json")));
    assert!(!is_script_ext(Path::new("foo.wasm")));
    assert!(!is_script_ext(Path::new("foo.mjsx")));
  }

  #[test]
  fn test_relative_path() {
    assert_eq!(
      relative_path(Path::new("/foo/bar/baz"), Path::new("/foo/bar")),
      Some(PathBuf::from("baz"))
    );
    assert_eq!(
      relative_path(Path::new("/foo/bar"), Path::new("/foo/bar/baz")),
      Some(PathBuf::from(".."))
    );
    assert_eq!(
      relative_path(Path::new("foo/bar/baz"), Path::new("foo")),
      Some(PathBuf::from("bar/baz"))
    );
    assert_eq!(relative_path(Path::new("foo"), Path::new("/bar")), None);
    assert_eq!(
      relative_path(Path::new("/foo"), Path::new("bar")),
      Some(PathBuf::from("/foo"))
    );
    assert_eq!(
      relative_path(Path::new("../foo"), Path::new("bar")),
      Some(PathBuf::from("../../foo"))
    );
  }

  #[test]
  fn test_is_importable_ext() {
    assert!(!is_importable_ext(Path::new("tests/subdir/redirects")));
    assert!(!is_importable_ext(Path::new("README.md")));
    assert!(is_importable_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_importable_ext(Path::new("testdata/run/001_hello.js")));
    assert!(is_importable_ext(Path::new("testdata/run/002_hello.ts")));
    assert!(is_importable_ext(Path::new("foo.jsx")));
    assert!(is_importable_ext(Path::new("foo.tsx")));
    assert!(is_importable_ext(Path::new("foo.TS")));
    assert!(is_importable_ext(Path::new("foo.TSX")));
    assert!(is_importable_ext(Path::new("foo.JS")));
    assert!(is_importable_ext(Path::new("foo.JSX")));
    assert!(is_importable_ext(Path::new("foo.mjs")));
    assert!(is_importable_ext(Path::new("foo.mts")));
    assert!(is_importable_ext(Path::new("foo.cjs")));
    assert!(is_importable_ext(Path::new("foo.cts")));
    assert!(is_importable_ext(Path::new("foo.json")));
    assert!(is_importable_ext(Path::new("foo.wasm")));
    assert!(!is_importable_ext(Path::new("foo.mjsx")));
  }

  #[test]
  fn test_relative_specifier() {
    let fixtures: Vec<(&str, &str, Option<&str>)> = vec![
      ("file:///from", "file:///to", Some("./to")),
      ("file:///from", "file:///from/other", Some("./from/other")),
      ("file:///from", "file:///from/other/", Some("./from/other/")),
      ("file:///from", "file:///other/from", Some("./other/from")),
      ("file:///from/", "file:///other/from", Some("../other/from")),
      ("file:///from", "file:///other/from/", Some("./other/from/")),
      (
        "file:///from",
        "file:///to/other.txt",
        Some("./to/other.txt"),
      ),
      (
        "file:///from/test",
        "file:///to/other.txt",
        Some("../to/other.txt"),
      ),
      (
        "file:///from/other.txt",
        "file:///to/other.txt",
        Some("../to/other.txt"),
      ),
      (
        "https://deno.land/x/a/b/d.ts",
        "https://deno.land/x/a/b/c.ts",
        Some("./c.ts"),
      ),
      (
        "https://deno.land/x/a/b/d.ts",
        "https://deno.land/x/a/c.ts",
        Some("../c.ts"),
      ),
      (
        "https://deno.land/x/a/b/d.ts",
        "https://deno.land/x/a/b/c/d.ts",
        Some("./c/d.ts"),
      ),
      (
        "https://deno.land/x/a/b/c/",
        "https://deno.land/x/a/b/c/d.ts",
        Some("./d.ts"),
      ),
      (
        "https://deno.land/x/a/b/c/",
        "https://deno.land/x/a/b/c/d/e.ts",
        Some("./d/e.ts"),
      ),
      (
        "https://deno.land/x/a/b/c/f.ts",
        "https://deno.land/x/a/b/c/d/e.ts",
        Some("./d/e.ts"),
      ),
      (
        "https://deno.land/x/a/b/d.ts",
        "https://deno.land/x/a/c.ts?foo=bar",
        Some("../c.ts?foo=bar"),
      ),
      (
        "https://deno.land/x/a/b/d.ts?foo=bar",
        "https://deno.land/x/a/b/c.ts",
        Some("./c.ts"),
      ),
      ("file:///a/b/d.ts", "file:///a/b/c.ts", Some("./c.ts")),
      ("https://deno.land/x/a/b/c.ts", "file:///a/b/c.ts", None),
      (
        "https://deno.land/",
        "https://deno.land/x/a/b/c.ts",
        Some("./x/a/b/c.ts"),
      ),
      (
        "https://deno.land/x/d/e/f.ts",
        "https://deno.land/x/a/b/c.ts",
        Some("../../a/b/c.ts"),
      ),
    ];
    for (from_str, to_str, expected) in fixtures {
      let from = ModuleSpecifier::parse(from_str).unwrap();
      let to = ModuleSpecifier::parse(to_str).unwrap();
      let actual = relative_specifier(&from, &to);
      assert_eq!(
        actual.as_deref(),
        expected,
        "from: \"{from_str}\" to: \"{to_str}\""
      );
    }
  }

  #[test]
  fn test_to_percent_decoded_str() {
    let str = to_percent_decoded_str("%F0%9F%A6%95");
    assert_eq!(str, "🦕");
  }
}
