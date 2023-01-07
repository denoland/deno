use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::error::uri_error;
use deno_core::error::AnyError;

/// Checks if the path has extension Deno supports.
pub fn is_supported_ext(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(
      ext.as_str(),
      "ts" | "tsx" | "js" | "jsx" | "mjs" | "mts" | "cjs" | "cts"
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

/// Attempts to convert a specifier to a file path. By default, uses the Url
/// crate's `to_file_path()` method, but falls back to try and resolve unix-style
/// paths on Windows.
pub fn specifier_to_file_path(
  specifier: &ModuleSpecifier,
) -> Result<PathBuf, AnyError> {
  let result = if cfg!(windows) {
    match specifier.to_file_path() {
      Ok(path) => Ok(path),
      Err(()) => {
        // This might be a unix-style path which is used in the tests even on Windows.
        // Attempt to see if we can convert it to a `PathBuf`. This code should be removed
        // once/if https://github.com/servo/rust-url/issues/730 is implemented.
        if specifier.scheme() == "file"
          && specifier.host().is_none()
          && specifier.port().is_none()
          && specifier.path_segments().is_some()
        {
          let path_str = specifier.path();
          match String::from_utf8(
            percent_encoding::percent_decode(path_str.as_bytes()).collect(),
          ) {
            Ok(path_str) => Ok(PathBuf::from(path_str)),
            Err(_) => Err(()),
          }
        } else {
          Err(())
        }
      }
    }
  } else {
    specifier.to_file_path()
  };
  match result {
    Ok(path) => Ok(path),
    Err(()) => Err(uri_error(format!(
      "Invalid file path.\n  Specifier: {}",
      specifier
    ))),
  }
}

/// Ensures a specifier that will definitely be a directory has a trailing slash.
pub fn ensure_directory_specifier(
  mut specifier: ModuleSpecifier,
) -> ModuleSpecifier {
  let path = specifier.path();
  if !path.ends_with('/') {
    let new_path = format!("{}/", path);
    specifier.set_path(&new_path);
  }
  specifier
}

/// Gets the parent of this module specifier.
pub fn specifier_parent(specifier: &ModuleSpecifier) -> ModuleSpecifier {
  let mut specifier = specifier.clone();
  // don't use specifier.segments() because it will strip the leading slash
  let mut segments = specifier.path().split('/').collect::<Vec<_>>();
  if segments.iter().all(|s| s.is_empty()) {
    return specifier;
  }
  if let Some(last) = segments.last() {
    if last.is_empty() {
      segments.pop();
    }
    segments.pop();
    let new_path = format!("{}/", segments.join("/"));
    specifier.set_path(&new_path);
  }
  specifier
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

  Some(if text.starts_with("../") || text.starts_with("./") {
    text
  } else {
    format!("./{}", text)
  })
}

/// This function checks if input path has trailing slash or not. If input path
/// has trailing slash it will return true else it will return false.
pub fn path_has_trailing_slash(path: &Path) -> bool {
  if let Some(path_str) = path.to_str() {
    if cfg!(windows) {
      path_str.ends_with('\\')
    } else {
      path_str.ends_with('/')
    }
  } else {
    false
  }
}

/// Gets a path with the specified file stem suffix.
///
/// Ex. `file.ts` with suffix `_2` returns `file_2.ts`
pub fn path_with_stem_suffix(path: &Path, suffix: &str) -> PathBuf {
  if let Some(file_name) = path.file_name().map(|f| f.to_string_lossy()) {
    if let Some(file_stem) = path.file_stem().map(|f| f.to_string_lossy()) {
      if let Some(ext) = path.extension().map(|f| f.to_string_lossy()) {
        return if file_stem.to_lowercase().ends_with(".d") {
          path.with_file_name(format!(
            "{}{}.{}.{}",
            &file_stem[..file_stem.len() - ".d".len()],
            suffix,
            // maintain casing
            &file_stem[file_stem.len() - "d".len()..],
            ext
          ))
        } else {
          path.with_file_name(format!("{}{}.{}", file_stem, suffix, ext))
        };
      }
    }

    path.with_file_name(format!("{}{}", file_name, suffix))
  } else {
    path.with_file_name(suffix)
  }
}

/// Gets if the provided character is not supported on all
/// kinds of file systems.
pub fn is_banned_path_char(c: char) -> bool {
  matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
}

/// Gets a safe local directory name for the provided url.
///
/// For example:
/// https://deno.land:8080/path -> deno.land_8080/path
pub fn root_url_to_safe_local_dirname(root: &ModuleSpecifier) -> PathBuf {
  fn sanitize_segment(text: &str) -> String {
    text
      .chars()
      .map(|c| if is_banned_segment_char(c) { '_' } else { c })
      .collect()
  }

  fn is_banned_segment_char(c: char) -> bool {
    matches!(c, '/' | '\\') || is_banned_path_char(c)
  }

  let mut result = String::new();
  if let Some(domain) = root.domain() {
    result.push_str(&sanitize_segment(domain));
  }
  if let Some(port) = root.port() {
    if !result.is_empty() {
      result.push('_');
    }
    result.push_str(&port.to_string());
  }
  let mut result = PathBuf::from(result);
  if let Some(segments) = root.path_segments() {
    for segment in segments.filter(|s| !s.is_empty()) {
      result = result.join(sanitize_segment(segment));
    }
  }

  result
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_is_supported_ext() {
    assert!(!is_supported_ext(Path::new("tests/subdir/redirects")));
    assert!(!is_supported_ext(Path::new("README.md")));
    assert!(is_supported_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_ext(Path::new("testdata/run/001_hello.js")));
    assert!(is_supported_ext(Path::new("testdata/run/002_hello.ts")));
    assert!(is_supported_ext(Path::new("foo.jsx")));
    assert!(is_supported_ext(Path::new("foo.tsx")));
    assert!(is_supported_ext(Path::new("foo.TS")));
    assert!(is_supported_ext(Path::new("foo.TSX")));
    assert!(is_supported_ext(Path::new("foo.JS")));
    assert!(is_supported_ext(Path::new("foo.JSX")));
    assert!(is_supported_ext(Path::new("foo.mjs")));
    assert!(is_supported_ext(Path::new("foo.mts")));
    assert!(is_supported_ext(Path::new("foo.cjs")));
    assert!(is_supported_ext(Path::new("foo.cts")));
    assert!(!is_supported_ext(Path::new("foo.mjsx")));
  }

  #[test]
  fn test_specifier_to_file_path() {
    run_success_test("file:///", "/");
    run_success_test("file:///test", "/test");
    run_success_test("file:///dir/test/test.txt", "/dir/test/test.txt");
    run_success_test(
      "file:///dir/test%20test/test.txt",
      "/dir/test test/test.txt",
    );

    fn run_success_test(specifier: &str, expected_path: &str) {
      let result =
        specifier_to_file_path(&ModuleSpecifier::parse(specifier).unwrap())
          .unwrap();
      assert_eq!(result, PathBuf::from(expected_path));
    }
  }

  #[test]
  fn test_ensure_directory_specifier() {
    run_test("file:///", "file:///");
    run_test("file:///test", "file:///test/");
    run_test("file:///test/", "file:///test/");
    run_test("file:///test/other", "file:///test/other/");
    run_test("file:///test/other/", "file:///test/other/");

    fn run_test(specifier: &str, expected: &str) {
      let result =
        ensure_directory_specifier(ModuleSpecifier::parse(specifier).unwrap());
      assert_eq!(result.to_string(), expected);
    }
  }

  #[test]
  fn test_specifier_parent() {
    run_test("file:///", "file:///");
    run_test("file:///test", "file:///");
    run_test("file:///test/", "file:///");
    run_test("file:///test/other", "file:///test/");
    run_test("file:///test/other.txt", "file:///test/");
    run_test("file:///test/other/", "file:///test/");

    fn run_test(specifier: &str, expected: &str) {
      let result =
        specifier_parent(&ModuleSpecifier::parse(specifier).unwrap());
      assert_eq!(result.to_string(), expected);
    }
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
        "from: \"{}\" to: \"{}\"",
        from_str,
        to_str
      );
    }
  }

  #[test]
  fn test_path_has_trailing_slash() {
    #[cfg(not(windows))]
    {
      run_test("/Users/johndoe/Desktop/deno-project/target/", true);
      run_test(r"/Users/johndoe/deno-project/target//", true);
      run_test("/Users/johndoe/Desktop/deno-project", false);
      run_test(r"/Users/johndoe/deno-project\", false);
    }

    #[cfg(windows)]
    {
      run_test(r"C:\test\deno-project\", true);
      run_test(r"C:\test\deno-project\\", true);
      run_test(r"C:\test\file.txt", false);
      run_test(r"C:\test\file.txt/", false);
    }

    fn run_test(path_str: &str, expected: bool) {
      let path = Path::new(path_str);
      let result = path_has_trailing_slash(path);
      assert_eq!(result, expected);
    }
  }

  #[test]
  fn test_path_with_stem_suffix() {
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/"), "_2"),
      PathBuf::from("/_2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test"), "_2"),
      PathBuf::from("/test_2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.txt"), "_2"),
      PathBuf::from("/test_2.txt")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test/subdir"), "_2"),
      PathBuf::from("/test/subdir_2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test/subdir.other.txt"), "_2"),
      PathBuf::from("/test/subdir.other_2.txt")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.ts"), "_2"),
      PathBuf::from("/test_2.d.ts")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.D.TS"), "_2"),
      PathBuf::from("/test_2.D.TS")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.mts"), "_2"),
      PathBuf::from("/test_2.d.mts")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.cts"), "_2"),
      PathBuf::from("/test_2.d.cts")
    );
  }
}
