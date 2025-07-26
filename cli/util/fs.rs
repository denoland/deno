// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Error;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_config::glob::FileCollector;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPattern;
use deno_config::glob::PathOrPatternSet;
use deno_config::glob::WalkEntry;
use deno_core::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;

use crate::sys::CliSys;

/// Creates a std::fs::File handling if the parent does not exist.
pub fn create_file(file_path: &Path) -> std::io::Result<std::fs::File> {
  match std::fs::File::create(file_path) {
    Ok(file) => Ok(file),
    Err(err) => {
      if err.kind() == ErrorKind::NotFound {
        let parent_dir_path = file_path.parent().unwrap();
        match std::fs::create_dir_all(parent_dir_path) {
          Ok(()) => {
            return std::fs::File::create(file_path)
              .map_err(|err| add_file_context_to_err(file_path, err));
          }
          Err(create_err) => {
            if !parent_dir_path.exists() {
              return Err(Error::new(
                create_err.kind(),
                format!(
                  "{:#} (for '{}')\nCheck the permission of the directory.",
                  create_err,
                  parent_dir_path.display()
                ),
              ));
            }
          }
        }
      }
      Err(add_file_context_to_err(file_path, err))
    }
  }
}

fn add_file_context_to_err(file_path: &Path, err: Error) -> Error {
  Error::new(
    err.kind(),
    format!("{:#} (for '{}')", err, file_path.display()),
  )
}

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  Ok(deno_path_util::strip_unc_prefix(path.canonicalize()?))
}

/// Canonicalizes a path which might be non-existent by going up the
/// ancestors until it finds a directory that exists, canonicalizes
/// that path, then adds back the remaining path components.
///
/// Note: When using this, you should be aware that a symlink may
/// subsequently be created along this path by some other code.
pub fn canonicalize_path_maybe_not_exists(
  path: &Path,
) -> Result<PathBuf, Error> {
  deno_path_util::fs::canonicalize_path_maybe_not_exists(
    &CliSys::default(),
    path,
  )
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
/// Note: This ignores all .git and node_modules folders.
pub fn collect_specifiers(
  mut files: FilePatterns,
  vendor_folder: Option<PathBuf>,
  predicate: impl Fn(WalkEntry) -> bool,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let mut prepared = vec![];

  // break out the remote specifiers
  if let Some(include_mut) = &mut files.include {
    let includes = std::mem::take(include_mut);
    let path_or_patterns = includes.into_path_or_patterns();
    let mut result = Vec::with_capacity(path_or_patterns.len());
    for path_or_pattern in path_or_patterns {
      match path_or_pattern {
        PathOrPattern::Path(path) => {
          if path.is_dir() {
            result.push(PathOrPattern::Path(path));
          } else if !files.exclude.matches_path(&path) {
            let url = specifier_from_file_path(&path)?;
            prepared.push(url);
          }
        }
        PathOrPattern::NegatedPath(path) => {
          // add it back
          result.push(PathOrPattern::NegatedPath(path));
        }
        PathOrPattern::RemoteUrl(remote_url) => {
          prepared.push(remote_url);
        }
        PathOrPattern::Pattern(pattern) => {
          // add it back
          result.push(PathOrPattern::Pattern(pattern));
        }
      }
    }
    *include_mut = PathOrPatternSet::new(result);
  }

  let collected_files = FileCollector::new(predicate)
    .ignore_git_folder()
    .ignore_node_modules()
    .set_vendor_folder(vendor_folder)
    .collect_file_patterns(&CliSys::default(), files);
  let mut collected_files_as_urls = collected_files
    .iter()
    .map(|f| specifier_from_file_path(f).unwrap())
    .collect::<Vec<ModuleSpecifier>>();

  collected_files_as_urls.sort();
  prepared.extend(collected_files_as_urls);

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

/// Gets the total size (in bytes) of a directory.
pub fn dir_size(path: &Path) -> std::io::Result<u64> {
  let entries = std::fs::read_dir(path)?;
  let mut total = 0;
  for entry in entries {
    let entry = entry?;
    total += match entry.metadata()? {
      data if data.is_dir() => dir_size(&entry.path())?,
      data => data.len(),
    };
  }
  Ok(total)
}

pub fn specifier_from_file_path(
  path: &Path,
) -> Result<ModuleSpecifier, AnyError> {
  ModuleSpecifier::from_file_path(path)
    .map_err(|_| anyhow!("Invalid file path '{}'", path.display()))
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use test_util::PathRef;
  use test_util::TempDir;

  use super::*;

  #[test]
  fn test_collect_specifiers() {
    fn create_files(dir_path: &PathRef, files: &[&str]) {
      dir_path.create_dir_all();
      for f in files {
        dir_path.join(f).write("");
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

    let predicate = |e: WalkEntry| {
      // exclude dotfiles
      e.path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|f| !f.starts_with('.'))
        .unwrap_or(false)
    };

    let result = collect_specifiers(
      FilePatterns {
        base: root_dir_path.to_path_buf(),
        include: Some(
          PathOrPatternSet::from_include_relative_path_or_patterns(
            root_dir_path.as_path(),
            &[
              "http://localhost:8080".to_string(),
              "./".to_string(),
              "https://localhost:8080".to_string(),
            ],
          )
          .unwrap(),
        ),
        exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
          ignore_dir_path.to_path_buf(),
        )]),
      },
      None,
      predicate,
    )
    .unwrap();

    let root_dir_url = ModuleSpecifier::from_file_path(&root_dir_path)
      .unwrap()
      .to_string();
    let expected = vec![
      "http://localhost:8080/".to_string(),
      "https://localhost:8080/".to_string(),
      format!("{root_dir_url}/a.ts"),
      format!("{root_dir_url}/b.js"),
      format!("{root_dir_url}/c.tsx"),
      format!("{root_dir_url}/child/README.md"),
      format!("{root_dir_url}/child/e.mjs"),
      format!("{root_dir_url}/child/f.mjsx"),
      format!("{root_dir_url}/d.jsx"),
    ];

    assert_eq!(
      result
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>(),
      expected
    );

    let scheme = if cfg!(target_os = "windows") {
      "file:///"
    } else {
      "file://"
    };
    let result = collect_specifiers(
      FilePatterns {
        base: root_dir_path.to_path_buf(),
        include: Some(PathOrPatternSet::new(vec![
          PathOrPattern::new(&format!(
            "{}{}",
            scheme,
            root_dir_path.join("child").to_string().replace('\\', "/")
          ))
          .unwrap(),
        ])),
        exclude: Default::default(),
      },
      None,
      predicate,
    )
    .unwrap();

    let expected = vec![
      format!("{root_dir_url}/child/README.md"),
      format!("{root_dir_url}/child/e.mjs"),
      format!("{root_dir_url}/child/f.mjsx"),
    ];

    assert_eq!(
      result
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>(),
      expected
    );
  }
}
