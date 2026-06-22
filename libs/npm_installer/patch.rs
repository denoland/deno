// Copyright 2018-2026 the Deno authors. MIT license.

//! Applies unified-diff (`.patch`) files to an extracted npm package, mirroring
//! the `patchedDependencies` workflow of pnpm/bun and the `patch-package` tool.
//!
//! Patches are applied to the per-project `node_modules` copy of a package
//! (never to the shared global cache), after the package has been cloned out of
//! the cache. Because that clone is made of hardlinks back into the global
//! cache, each patched file is rewritten by removing the link and writing a
//! fresh file so the global cache is left untouched.

use std::path::Path;
use std::path::PathBuf;

use sys_traits::FsRead;
use sys_traits::FsRemoveFile;
use sys_traits::FsWrite;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ApplyPatchError {
  #[class(inherit)]
  #[error("Failed to read patch file '{}'.", path.display())]
  ReadPatch {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(type)]
  #[error("Failed to parse patch file '{}'.", path.display())]
  ParsePatch {
    path: PathBuf,
    #[source]
    source: diffy::ParsePatchError,
  },
  #[class(type)]
  #[error("Patch '{}' does not name a target file.", path.display())]
  MissingTarget { path: PathBuf },
  #[class(inherit)]
  #[error("Failed to read '{}' while applying patch '{}'.", target.display(), patch.display())]
  ReadTarget {
    target: PathBuf,
    patch: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(type)]
  #[error("Failed to apply patch '{}' to '{}'.", patch.display(), target.display())]
  Apply {
    target: PathBuf,
    patch: PathBuf,
    #[source]
    source: diffy::ApplyError,
  },
  #[class(inherit)]
  #[error("Failed to write '{}' while applying patch '{}'.", target.display(), patch.display())]
  WriteTarget {
    target: PathBuf,
    patch: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
}

/// Applies the unified diff at `patch_path` to the package extracted at
/// `package_dir`. The patch may touch multiple files.
pub fn apply_patch<TSys: FsRead + FsWrite + FsRemoveFile>(
  sys: &TSys,
  package_dir: &Path,
  patch_path: &Path,
) -> Result<(), ApplyPatchError> {
  let contents = sys.fs_read_to_string(patch_path).map_err(|source| {
    ApplyPatchError::ReadPatch {
      path: patch_path.to_path_buf(),
      source,
    }
  })?;

  for file_diff in split_file_diffs(&contents) {
    let patch = diffy::Patch::from_str(file_diff).map_err(|source| {
      ApplyPatchError::ParsePatch {
        path: patch_path.to_path_buf(),
        source,
      }
    })?;

    let target_rel = patch
      .modified()
      .or_else(|| patch.original())
      .map(strip_diff_prefix)
      .ok_or_else(|| ApplyPatchError::MissingTarget {
        path: patch_path.to_path_buf(),
      })?;
    let target = package_dir.join(target_rel);

    let original = sys.fs_read_to_string(&target).map_err(|source| {
      ApplyPatchError::ReadTarget {
        target: target.clone(),
        patch: patch_path.to_path_buf(),
        source,
      }
    })?;
    let patched = diffy::apply(&original, &patch).map_err(|source| {
      ApplyPatchError::Apply {
        target: target.clone(),
        patch: patch_path.to_path_buf(),
        source,
      }
    })?;

    // Break the hardlink back into the global cache before writing: remove the
    // existing name, then write a fresh file so the cache stays pristine.
    let _ = sys.fs_remove_file(&target);
    sys
      .fs_write(&target, patched.as_bytes())
      .map_err(|source| ApplyPatchError::WriteTarget {
        target: target.clone(),
        patch: patch_path.to_path_buf(),
        source,
      })?;
  }

  Ok(())
}

/// Splits a (possibly multi-file) git/unified diff into one slice per file.
///
/// pnpm, bun and patch-package all emit git-style diffs where each file begins
/// with a `diff --git` line; we slice on that and hand the `--- `/`+++ `/`@@`
/// portion to the diff applier. A plain `diff -u` patch (no `diff --git`
/// header) is treated as a single-file diff.
fn split_file_diffs(contents: &str) -> Vec<&str> {
  let mut sections = Vec::new();
  let mut start: Option<usize> = None;
  let mut search_from = 0;
  while let Some(rel) = contents[search_from..].find("diff --git ") {
    let idx = search_from + rel;
    // only treat it as a header when at the start of a line
    if idx == 0 || contents.as_bytes()[idx - 1] == b'\n' {
      if let Some(prev) = start.take() {
        sections.push(&contents[prev..idx]);
      }
      start = Some(idx);
    }
    search_from = idx + "diff --git ".len();
  }
  match start {
    Some(prev) => sections.push(&contents[prev..]),
    None if !contents.trim().is_empty() => sections.push(contents),
    None => {}
  }
  sections
    .into_iter()
    .filter_map(|section| {
      // diffy parses starting at the `--- ` header, so drop any leading
      // `diff --git`/`index`/`new file mode` git metadata lines.
      section.find("--- ").map(|pos| &section[pos..])
    })
    .collect()
}

/// Strips a leading `a/` or `b/` path prefix that git-style diffs add to file
/// names, and trims any trailing tab-separated timestamp.
fn strip_diff_prefix(name: &str) -> &str {
  let name = name.split('\t').next().unwrap_or(name);
  name
    .strip_prefix("a/")
    .or_else(|| name.strip_prefix("b/"))
    .unwrap_or(name)
}
