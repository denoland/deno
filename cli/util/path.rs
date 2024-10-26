// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
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
      "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" | "cjs" | "cts" | "json"
    )
  } else {
    false
  }
}

/// Get the extension of a file in lowercase.
pub fn get_extension(file_path: &Path) -> Option<String> {
  return file_path
    .extension()
    .and_then(|e| e.to_str())
    .map(|e| e.to_lowercase());
}

pub fn specifier_has_extension(
  specifier: &ModuleSpecifier,
  searching_ext: &str,
) -> bool {
  let Some((_, ext)) = specifier.path().rsplit_once('.') else {
    return false;
  };
  let searching_ext = searching_ext.strip_prefix('.').unwrap_or(searching_ext);
  debug_assert!(!searching_ext.contains('.')); // exts like .d.ts are not implemented here
  if ext.len() != searching_ext.len() {
    return false;
  }
  ext.eq_ignore_ascii_case(searching_ext)
}

pub fn get_atomic_dir_path(file_path: &Path) -> PathBuf {
  let rand = gen_rand_path_component();
  let new_file_name = format!(
    ".{}_{}",
    file_path
      .file_name()
      .map(|f| f.to_string_lossy())
      .unwrap_or(Cow::Borrowed("")),
    rand
  );
  file_path.with_file_name(new_file_name)
}

pub fn get_atomic_file_path(file_path: &Path) -> PathBuf {
  let rand = gen_rand_path_component();
  let extension = format!("{rand}.tmp");
  file_path.with_extension(extension)
}

fn gen_rand_path_component() -> String {
  (0..4).fold(String::new(), |mut output, _| {
    output.push_str(&format!("{:02x}", rand::random::<u8>()));
    output
  })
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
        .last()
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

  // workaround using parent directory until https://github.com/servo/rust-url/pull/754 is merged
  let from = if !from.path().ends_with('/') {
    if let Some(end_slash) = from.path().rfind('/') {
      let mut new_from = from.clone();
      new_from.set_path(&from.path()[..end_slash + 1]);
      Cow::Owned(new_from)
    } else {
      Cow::Borrowed(from)
    }
  } else {
    Cow::Borrowed(from)
  };

  // workaround for url crate not adding a trailing slash for a directory
  // it seems to be fixed once a version greater than 2.2.2 is released
  let mut text = from.make_relative(to)?;
  if is_dir && !text.ends_with('/') && to.query().is_none() {
    text.push('/');
  }

  let text = if text.starts_with("../") || text.starts_with("./") {
    text
  } else {
    format!("./{text}")
  };
  Some(to_percent_decoded_str(&text))
}

#[cfg_attr(windows, allow(dead_code))]
pub fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
  pathdiff::diff_paths(to, from)
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
    assert!(!is_script_ext(Path::new("foo.mjsx")));
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
  fn test_specifier_has_extension() {
    fn get(specifier: &str, ext: &str) -> bool {
      specifier_has_extension(&ModuleSpecifier::parse(specifier).unwrap(), ext)
    }

    assert!(get("file:///a/b/c.ts", "ts"));
    assert!(get("file:///a/b/c.ts", ".ts"));
    assert!(!get("file:///a/b/c.ts", ".cts"));
    assert!(get("file:///a/b/c.CtS", ".cts"));
  }

  #[test]
  fn test_to_percent_decoded_str() {
    let str = to_percent_decoded_str("%F0%9F%A6%95");
    assert_eq!(str, "🦕");
  }
}
