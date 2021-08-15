// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::Context;
pub use deno_core::normalize_path;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_crypto::rand;
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
  let mut canonicalized_path = path.canonicalize()?;
  if cfg!(windows) {
    canonicalized_path = PathBuf::from(
      canonicalized_path
        .display()
        .to_string()
        .trim_start_matches("\\\\?\\"),
    );
  }
  Ok(canonicalized_path)
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
    matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx" | "mjs")
  } else {
    false
  }
}

/// This function is similar to is_supported_ext but adds additional extensions
/// supported by `deno fmt`.
pub fn is_supported_ext_fmt(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(
      ext.as_str(),
      "ts" | "tsx" | "js" | "jsx" | "mjs" | "md" | "json" | "jsonc"
    )
  } else {
    false
  }
}

/// Checks if the path has a basename and extension Deno supports for tests.
pub fn is_supported_test_path(path: &Path) -> bool {
  use std::path::Component;
  if let Some(Component::Normal(basename_os_str)) =
    path.components().next_back()
  {
    let basename = basename_os_str.to_string_lossy();
    basename.ends_with("_test.ts")
      || basename.ends_with("_test.tsx")
      || basename.ends_with("_test.js")
      || basename.ends_with("_test.mjs")
      || basename.ends_with("_test.jsx")
      || basename.ends_with(".test.ts")
      || basename.ends_with(".test.tsx")
      || basename.ends_with(".test.js")
      || basename.ends_with(".test.mjs")
      || basename.ends_with(".test.jsx")
      || basename == "test.ts"
      || basename == "test.tsx"
      || basename == "test.js"
      || basename == "test.mjs"
      || basename == "test.jsx"
  } else {
    false
  }
}

/// Checks if the path has an extension Deno supports for tests.
pub fn is_supported_test_ext(path: &Path) -> bool {
  if let Some(ext) = get_extension(path) {
    matches!(ext.as_str(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "md")
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
    .filter_map(|i| i.canonicalize().ok())
    .collect();

  let cur_dir = [std::env::current_dir()?];
  let files = if files.is_empty() { &cur_dir } else { files };

  for file in files {
    for entry in WalkDir::new(file)
      .into_iter()
      .filter_entry(|e| {
        e.path().canonicalize().map_or(false, |c| {
          !canonicalized_ignore.iter().any(|i| c.starts_with(i))
        })
      })
      .filter_map(|e| match e {
        Ok(e) if !e.file_type().is_dir() && predicate(e.path()) => Some(e),
        _ => None,
      })
    {
      target_files.push(entry.into_path().canonicalize()?)
    }
  }

  Ok(target_files)
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
pub fn collect_specifiers<P>(
  include: Vec<String>,
  predicate: P,
) -> Result<Vec<ModuleSpecifier>, AnyError>
where
  P: Fn(&Path) -> bool,
{
  let mut prepared = vec![];

  let root_path = std::env::current_dir()?;
  for path in include {
    let lowercase_path = path.to_lowercase();
    if lowercase_path.starts_with("http://")
      || lowercase_path.starts_with("https://")
    {
      let url = ModuleSpecifier::parse(&path)?;
      prepared.push(url);
      continue;
    }

    let p = normalize_path(&root_path.join(path));
    if p.is_dir() {
      let test_files = collect_files(&[p], &[], &predicate).unwrap();
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

// Asynchronously removes a directory and all its descendants, but does not error
// when the directory does not exist.
pub async fn remove_dir_all_if_exists(path: &Path) -> std::io::Result<()> {
  let result = tokio::fs::remove_dir_all(path).await;
  match result {
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    _ => result,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

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
    assert!(!is_supported_ext(Path::new("foo.mjsx")));
  }

  #[test]
  fn test_is_supported_ext_fmt() {
    assert!(!is_supported_ext_fmt(Path::new("tests/subdir/redirects")));
    assert!(is_supported_ext_fmt(Path::new("README.md")));
    assert!(is_supported_ext_fmt(Path::new("readme.MD")));
    assert!(is_supported_ext_fmt(Path::new("lib/typescript.d.ts")));
    assert!(is_supported_ext_fmt(Path::new("testdata/001_hello.js")));
    assert!(is_supported_ext_fmt(Path::new("testdata/002_hello.ts")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.tsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.TS")));
    assert!(is_supported_ext_fmt(Path::new("foo.TSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.JS")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSX")));
    assert!(is_supported_ext_fmt(Path::new("foo.mjs")));
    assert!(!is_supported_ext_fmt(Path::new("foo.mjsx")));
    assert!(is_supported_ext_fmt(Path::new("foo.jsonc")));
    assert!(is_supported_ext_fmt(Path::new("foo.JSONC")));
    assert!(is_supported_ext_fmt(Path::new("foo.json")));
    assert!(is_supported_ext_fmt(Path::new("foo.JsON")));
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

    let t = TempDir::new().expect("tempdir fail");

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

    let t = TempDir::new().expect("tempdir fail");

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let result = collect_specifiers(
      vec![
        "http://localhost:8080".to_string(),
        root_dir_path.to_str().unwrap().to_string(),
        "https://localhost:8080".to_string(),
      ],
      |path| {
        // exclude dotfiles
        path
          .file_name()
          .and_then(|f| f.to_str())
          .map_or(false, |f| !f.starts_with('.'))
      },
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
      &format!("{}/ignore/g.d.ts", root_dir_url),
      "https://localhost:8080",
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<ModuleSpecifier>>();

    assert_eq!(result, expected);
  }
}
