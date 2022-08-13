// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::{uri_error, AnyError};
pub use deno_core::normalize_path;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_crypto::rand;
use std::borrow::Cow;
use std::env::current_dir;
use std::fs::OpenOptions;
use std::io::{Error, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn atomic_write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  let rand: String = (0..4)
    .map(|_| format!("{:02x}", rand::random::<u8>()))
    .collect();
  let extension = format!("{}.tmp", rand);
  let tmp_file = filename.with_extension(extension);
  write_file(&tmp_file, data, mode)?;
  std::fs::rename(tmp_file, filename)?;
  Ok(())
}

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  write_file_2(filename, data, true, mode, true, false)
}

pub fn write_file_2<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  update_mode: bool,
  mode: u32,
  is_create: bool,
  is_append: bool,
) -> std::io::Result<()> {
  let mut file = OpenOptions::new()
    .read(false)
    .write(true)
    .append(is_append)
    .truncate(!is_append)
    .create(is_create)
    .open(filename)?;

  if update_mode {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = mode & 0o777;
      let permissions = PermissionsExt::from_mode(mode);
      file.set_permissions(permissions)?;
    }
    #[cfg(not(unix))]
    let _ = mode;
  }

  file.write_all(data.as_ref())
}

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  let path = path.canonicalize()?;
  #[cfg(windows)]
  return Ok(strip_unc_prefix(path));
  #[cfg(not(windows))]
  return Ok(path);
}

#[cfg(windows)]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  use std::path::Component;
  use std::path::Prefix;

  let mut components = path.components();
  match components.next() {
    Some(Component::Prefix(prefix)) => {
      match prefix.kind() {
        // \\?\device
        Prefix::Verbatim(device) => {
          let mut path = PathBuf::new();
          path.push(format!(r"\\{}\", device.to_string_lossy()));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        // \\?\c:\path
        Prefix::VerbatimDisk(_) => {
          let mut path = PathBuf::new();
          path.push(prefix.as_os_str().to_string_lossy().replace(r"\\?\", ""));
          path.extend(components);
          path
        }
        // \\?\UNC\hostname\share_name\path
        Prefix::VerbatimUNC(hostname, share_name) => {
          let mut path = PathBuf::new();
          path.push(format!(
            r"\\{}\{}\",
            hostname.to_string_lossy(),
            share_name.to_string_lossy()
          ));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        _ => path,
      }
    }
    _ => path,
  }
}

pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  let resolved_path = if path.is_absolute() {
    path.to_owned()
  } else {
    let cwd =
      current_dir().context("Failed to get current working directory")?;
    cwd.join(path)
  };

  Ok(normalize_path(&resolved_path))
}

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

/// Checks if the path has a basename and extension Deno supports for tests.
pub fn is_supported_test_path(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    (basename.ends_with("_test")
      || basename.ends_with(".test")
      || basename == "test")
      && is_supported_ext(path)
  } else {
    false
  }
}

/// Checks if the path has a basename and extension Deno supports for benches.
pub fn is_supported_bench_path(path: &Path) -> bool {
  if let Some(name) = path.file_stem() {
    let basename = name.to_string_lossy();
    (basename.ends_with("_bench")
      || basename.ends_with(".bench")
      || basename == "bench")
      && is_supported_ext(path)
  } else {
    false
  }
}

/// Checks if the path has an extension Deno supports for tests.
pub fn is_supported_test_ext(path: &Path) -> bool {
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
        | "md"
        | "mkd"
        | "mkdn"
        | "mdwn"
        | "mdown"
        | "markdown"
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

/// Collects file paths that satisfy the given predicate, by recursively walking `files`.
/// If the walker visits a path that is listed in `ignore`, it skips descending into the directory.
pub fn collect_files<P>(
  files: &[PathBuf],
  ignore: &[PathBuf],
  predicate: P,
) -> Result<Vec<PathBuf>, AnyError>
where
  P: Fn(&Path) -> bool,
{
  let mut target_files = Vec::new();

  // retain only the paths which exist and ignore the rest
  let canonicalized_ignore: Vec<PathBuf> = ignore
    .iter()
    .filter_map(|i| canonicalize_path(i).ok())
    .collect();

  for file in files {
    for entry in WalkDir::new(file)
      .into_iter()
      .filter_entry(|e| {
        canonicalize_path(e.path()).map_or(false, |c| {
          !canonicalized_ignore.iter().any(|i| c.starts_with(i))
        })
      })
      .filter_map(|e| match e {
        Ok(e) if !e.file_type().is_dir() && predicate(e.path()) => Some(e),
        _ => None,
      })
    {
      target_files.push(canonicalize_path(entry.path())?)
    }
  }

  Ok(target_files)
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
pub fn collect_specifiers<P>(
  include: Vec<String>,
  ignore: &[PathBuf],
  predicate: P,
) -> Result<Vec<ModuleSpecifier>, AnyError>
where
  P: Fn(&Path) -> bool,
{
  let mut prepared = vec![];

  let root_path = current_dir()?;
  for path in include {
    let lowercase_path = path.to_lowercase();
    if lowercase_path.starts_with("http://")
      || lowercase_path.starts_with("https://")
    {
      let url = ModuleSpecifier::parse(&path)?;
      prepared.push(url);
      continue;
    }

    let p = if lowercase_path.starts_with("file://") {
      specifier_to_file_path(&ModuleSpecifier::parse(&path)?)?
    } else {
      root_path.join(path)
    };
    let p = normalize_path(&p);
    if p.is_dir() {
      let test_files = collect_files(&[p], ignore, &predicate).unwrap();
      let mut test_files_as_urls = test_files
        .iter()
        .map(|f| ModuleSpecifier::from_file_path(f).unwrap())
        .collect::<Vec<ModuleSpecifier>>();

      test_files_as_urls.sort();
      prepared.extend(test_files_as_urls);
    } else {
      let url = ModuleSpecifier::from_file_path(p).unwrap();
      prepared.push(url);
    }
  }

  Ok(prepared)
}

/// Asynchronously removes a directory and all its descendants, but does not error
/// when the directory does not exist.
pub async fn remove_dir_all_if_exists(path: &Path) -> std::io::Result<()> {
  let result = tokio::fs::remove_dir_all(path).await;
  match result {
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    _ => result,
  }
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
mod tests {
  use super::*;
  use test_util::TempDir;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a/..")).unwrap(), cwd);
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

  // TODO: Get a good expected value here for Windows.
  #[cfg(not(windows))]
  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("/a");
    assert_eq!(resolve_from_cwd(expected).unwrap(), expected);
  }

  #[test]
  fn test_is_supported_ext() {
    assert!(!is_supported_ext(Path::new("tests/subdir/redirects")));
    assert!(!is_supported_ext(Path::new("README.md")));
    assert!(is_supported_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_ext(Path::new("testdata/001_hello.js")));
    assert!(is_supported_ext(Path::new("testdata/002_hello.ts")));
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
  fn test_is_supported_test_ext() {
    assert!(!is_supported_test_ext(Path::new("tests/subdir/redirects")));
    assert!(is_supported_test_ext(Path::new("README.md")));
    assert!(is_supported_test_ext(Path::new("readme.MD")));
    assert!(is_supported_test_ext(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_test_ext(Path::new("testdata/001_hello.js")));
    assert!(is_supported_test_ext(Path::new("testdata/002_hello.ts")));
    assert!(is_supported_test_ext(Path::new("foo.jsx")));
    assert!(is_supported_test_ext(Path::new("foo.tsx")));
    assert!(is_supported_test_ext(Path::new("foo.TS")));
    assert!(is_supported_test_ext(Path::new("foo.TSX")));
    assert!(is_supported_test_ext(Path::new("foo.JS")));
    assert!(is_supported_test_ext(Path::new("foo.JSX")));
    assert!(is_supported_test_ext(Path::new("foo.mjs")));
    assert!(is_supported_test_ext(Path::new("foo.mts")));
    assert!(is_supported_test_ext(Path::new("foo.cjs")));
    assert!(is_supported_test_ext(Path::new("foo.cts")));
    assert!(!is_supported_test_ext(Path::new("foo.mjsx")));
    assert!(!is_supported_test_ext(Path::new("foo.jsonc")));
    assert!(!is_supported_test_ext(Path::new("foo.JSONC")));
    assert!(!is_supported_test_ext(Path::new("foo.json")));
    assert!(!is_supported_test_ext(Path::new("foo.JsON")));
  }

  #[test]
  fn test_is_supported_test_path() {
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.ts"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.tsx"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.js"
    )));
    assert!(is_supported_test_path(Path::new(
      "tests/subdir/foo_test.jsx"
    )));
    assert!(is_supported_test_path(Path::new("bar/foo.test.ts")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.tsx")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.js")));
    assert!(is_supported_test_path(Path::new("bar/foo.test.jsx")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.js")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.jsx")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.ts")));
    assert!(is_supported_test_path(Path::new("foo/bar/test.tsx")));
    assert!(!is_supported_test_path(Path::new("README.md")));
    assert!(!is_supported_test_path(Path::new("lib/typescript.d.ts")));
    assert!(!is_supported_test_path(Path::new("notatest.js")));
    assert!(!is_supported_test_path(Path::new("NotAtest.ts")));
  }

  #[test]
  fn test_collect_files() {
    fn create_files(dir_path: &Path, files: &[&str]) {
      std::fs::create_dir(dir_path).expect("Failed to create directory");
      for f in files {
        let path = dir_path.join(f);
        std::fs::write(path, "").expect("Failed to create file");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let result = collect_files(&[root_dir_path], &[ignore_dir_path], |path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map_or(false, |f| !f.starts_with('.'))
    })
    .unwrap();
    let expected = [
      "a.ts",
      "b.js",
      "e.mjs",
      "f.mjsx",
      "README.md",
      "c.tsx",
      "d.jsx",
    ];
    for e in expected.iter() {
      assert!(result.iter().any(|r| r.ends_with(e)));
    }
    assert_eq!(result.len(), expected.len());
  }

  #[test]
  fn test_collect_specifiers() {
    fn create_files(dir_path: &Path, files: &[&str]) {
      std::fs::create_dir(dir_path).expect("Failed to create directory");
      for f in files {
        let path = dir_path.join(f);
        std::fs::write(path, "").expect("Failed to create file");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let predicate = |path: &Path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map_or(false, |f| !f.starts_with('.'))
    };

    let result = collect_specifiers(
      vec![
        "http://localhost:8080".to_string(),
        root_dir_path.to_str().unwrap().to_string(),
        "https://localhost:8080".to_string(),
      ],
      &[ignore_dir_path],
      predicate,
    )
    .unwrap();

    let root_dir_url = ModuleSpecifier::from_file_path(
      canonicalize_path(&root_dir_path).unwrap(),
    )
    .unwrap()
    .to_string();
    let expected: Vec<ModuleSpecifier> = [
      "http://localhost:8080",
      &format!("{}/a.ts", root_dir_url),
      &format!("{}/b.js", root_dir_url),
      &format!("{}/c.tsx", root_dir_url),
      &format!("{}/child/README.md", root_dir_url),
      &format!("{}/child/e.mjs", root_dir_url),
      &format!("{}/child/f.mjsx", root_dir_url),
      &format!("{}/d.jsx", root_dir_url),
      "https://localhost:8080",
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);

    let scheme = if cfg!(target_os = "windows") {
      "file:///"
    } else {
      "file://"
    };
    let result = collect_specifiers(
      vec![format!(
        "{}{}",
        scheme,
        root_dir_path
          .join("child")
          .to_str()
          .unwrap()
          .replace('\\', "/")
      )],
      &[],
      predicate,
    )
    .unwrap();

    let expected: Vec<ModuleSpecifier> = [
      &format!("{}/child/README.md", root_dir_url),
      &format!("{}/child/e.mjs", root_dir_url),
      &format!("{}/child/f.mjsx", root_dir_url),
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);
  }

  #[cfg(windows)]
  #[test]
  fn test_strip_unc_prefix() {
    run_test(r"C:\", r"C:\");
    run_test(r"C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\?\C:\", r"C:\");
    run_test(r"\\?\C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\.\C:\", r"\\.\C:\");
    run_test(r"\\.\C:\Test\file.txt", r"\\.\C:\Test\file.txt");

    run_test(r"\\?\UNC\localhost\", r"\\localhost");
    run_test(r"\\?\UNC\localhost\c$\", r"\\localhost\c$");
    run_test(
      r"\\?\UNC\localhost\c$\Windows\file.txt",
      r"\\localhost\c$\Windows\file.txt",
    );
    run_test(r"\\?\UNC\wsl$\deno.json", r"\\wsl$\deno.json");

    run_test(r"\\?\server1", r"\\server1");
    run_test(r"\\?\server1\e$\", r"\\server1\e$\");
    run_test(
      r"\\?\server1\e$\test\file.txt",
      r"\\server1\e$\test\file.txt",
    );

    fn run_test(input: &str, expected: &str) {
      assert_eq!(
        strip_unc_prefix(PathBuf::from(input)),
        PathBuf::from(expected)
      );
    }
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
    run_test("file:///from", "file:///to", Some("./to"));
    run_test("file:///from", "file:///from/other", Some("./from/other"));
    run_test("file:///from", "file:///from/other/", Some("./from/other/"));
    run_test("file:///from", "file:///other/from", Some("./other/from"));
    run_test("file:///from/", "file:///other/from", Some("../other/from"));
    run_test("file:///from", "file:///other/from/", Some("./other/from/"));
    run_test(
      "file:///from",
      "file:///to/other.txt",
      Some("./to/other.txt"),
    );
    run_test(
      "file:///from/test",
      "file:///to/other.txt",
      Some("../to/other.txt"),
    );
    run_test(
      "file:///from/other.txt",
      "file:///to/other.txt",
      Some("../to/other.txt"),
    );

    fn run_test(from: &str, to: &str, expected: Option<&str>) {
      let result = relative_specifier(
        &ModuleSpecifier::parse(from).unwrap(),
        &ModuleSpecifier::parse(to).unwrap(),
      );
      assert_eq!(result.as_deref(), expected);
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
