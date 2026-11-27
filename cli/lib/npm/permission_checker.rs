// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_error::JsErrorBox;
use deno_runtime::deno_permissions::OpenAccessKind;
use deno_runtime::deno_permissions::PermissionsContainer;
use parking_lot::Mutex;
use sys_traits::FsReadLink;

use crate::sys::DenoLibSys;

#[derive(Debug)]
pub enum NpmRegistryReadPermissionCheckerMode {
  Byonm,
  Global(PathBuf),
  Local(PathBuf),
}

#[derive(Debug)]
pub struct NpmRegistryReadPermissionChecker<TSys: DenoLibSys> {
  sys: TSys,
  cache: Mutex<HashMap<PathBuf, PathBuf>>,
  mode: NpmRegistryReadPermissionCheckerMode,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("failed canonicalizing '{path}'")]
struct EnsureRegistryReadPermissionError {
  path: PathBuf,
  #[source]
  #[inherit]
  source: std::io::Error,
}

const MAX_DANGLING_LINK_DEPTH: usize = 40;

/// Canonicalizes the nearest existing entry, then appends any unresolved path
/// components, explicitly following dangling symlinks and junctions. This
/// differs from `deno_path_util::fs::canonicalize_path_maybe_not_exists`, which
/// treats a dangling link's name as an ordinary unresolved component.
fn canonicalize_path_maybe_not_exists_following_dangling_links<TSys>(
  sys: &TSys,
  path: &Path,
) -> Result<PathBuf, std::io::Error>
where
  TSys: sys_traits::FsCanonicalize + sys_traits::FsMetadata + FsReadLink,
{
  canonicalize_path_maybe_not_exists_following_dangling_links_inner(
    sys,
    path,
    &mut HashSet::new(),
  )
}

fn canonicalize_path_maybe_not_exists_following_dangling_links_inner<TSys>(
  sys: &TSys,
  mut path: &Path,
  seen_links: &mut HashSet<PathBuf>,
) -> Result<PathBuf, std::io::Error>
where
  TSys: sys_traits::FsCanonicalize + sys_traits::FsMetadata + FsReadLink,
{
  let mut names_stack = Vec::new();
  loop {
    match sys.fs_symlink_metadata(path) {
      Ok(_) => {
        let mut canonicalized_path = match sys.fs_canonicalize(path) {
          Ok(path) => path,
          Err(err) if err.kind() == ErrorKind::NotFound => {
            let normalized_path =
              deno_path_util::normalize_path(Cow::Borrowed(path)).into_owned();
            if seen_links.len() >= MAX_DANGLING_LINK_DEPTH
              || !seen_links.insert(normalized_path)
            {
              return Err(std::io::Error::other(
                "too many levels of symbolic links",
              ));
            }
            let target = sys.fs_read_link(path)?;
            let target = if target.is_absolute() {
              target
            } else {
              path.parent().unwrap_or_else(|| Path::new(".")).join(target)
            };
            let target =
              deno_path_util::normalize_path(Cow::Owned(target)).into_owned();
            canonicalize_path_maybe_not_exists_following_dangling_links_inner(
              sys, &target, seen_links,
            )?
          }
          Err(err) => return Err(err),
        };
        for name in names_stack.into_iter().rev() {
          canonicalized_path = canonicalized_path.join(name);
        }
        return Ok(
          deno_path_util::normalize_path(Cow::Owned(canonicalized_path))
            .into_owned(),
        );
      }
      Err(err) if err.kind() == ErrorKind::NotFound => {
        names_stack.push(match path.file_name() {
          Some(name) => name.to_owned(),
          None => return Err(err),
        });
        path = match path.parent() {
          Some(parent) if parent.as_os_str().is_empty() => Path::new("."),
          Some(parent) => parent,
          None => return Err(err),
        };
      }
      Err(err) => return Err(err),
    }
  }
}

/// Returns whether the prefix of `path` at the configured registry depth
/// resolves to the canonical registry root. Windows may spell an existing
/// ancestor using an 8.3 short name or different casing, so a component-wise
/// lexical prefix check alone is insufficient there.
fn path_uses_canonical_registry_prefix(
  path: &Path,
  registry_path: &Path,
  registry_path_canon: &Path,
  canonicalize: impl FnOnce(&Path) -> Option<PathBuf>,
) -> bool {
  let Some(extra_component_count) = path
    .components()
    .count()
    .checked_sub(registry_path.components().count())
  else {
    return false;
  };
  let Some(candidate_registry_path) =
    path.ancestors().nth(extra_component_count)
  else {
    return false;
  };
  canonicalize(candidate_registry_path)
    .is_some_and(|path| path == registry_path_canon)
}

impl<TSys: DenoLibSys> NpmRegistryReadPermissionChecker<TSys> {
  pub fn new(sys: TSys, mode: NpmRegistryReadPermissionCheckerMode) -> Self {
    Self {
      sys,
      cache: Default::default(),
      mode,
    }
  }

  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  pub fn ensure_read_permission<'a>(
    &self,
    permissions: &mut PermissionsContainer,
    path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, JsErrorBox>
  where
    TSys: FsReadLink,
  {
    if permissions.query_read_all() {
      return Ok(path); // skip permissions checks below
    }

    match &self.mode {
      NpmRegistryReadPermissionCheckerMode::Byonm => {
        // Normalize the path to collapse `.` and `..` components before
        // checking for a `node_modules` ancestor. Otherwise a traversal path
        // such as `./node_modules/../../../etc/passwd` would slip through the
        // check and be read without `--allow-read`.
        let path = deno_path_util::normalize_path(path);
        if path.components().any(|c| c.as_os_str() == "node_modules") {
          Ok(path)
        } else {
          permissions
            .check_open(path, OpenAccessKind::Read, None)
            .map(|p| p.into_path())
            .map_err(JsErrorBox::from_err)
        }
      }
      NpmRegistryReadPermissionCheckerMode::Global(registry_path)
      | NpmRegistryReadPermissionCheckerMode::Local(registry_path) => {
        let mut path = path;
        let has_parent_dir = path
          .components()
          .any(|c| matches!(c, std::path::Component::ParentDir));
        // Allow reading only when the lexical path is in the registry, or on
        // Windows when an equivalent spelling of that registry prefix is.
        let is_path_in_registry =
          !has_parent_dir && path.starts_with(registry_path);
        let could_use_canonical_registry_prefix = cfg!(windows)
          && !has_parent_dir
          && path.components().count() >= registry_path.components().count();

        if is_path_in_registry || could_use_canonical_registry_prefix {
          let mut cache = self.cache.lock();
          let mut canonicalize =
            |path: &Path| -> Result<Option<PathBuf>, JsErrorBox> {
              match cache.get(path) {
                Some(canon) => Ok(Some(canon.clone())),
                None => match self.sys.fs_canonicalize(path) {
                  Ok(canon) => {
                    cache.insert(path.to_path_buf(), canon.clone());
                    Ok(Some(canon))
                  }
                  Err(e) => {
                    if e.kind() == ErrorKind::NotFound {
                      return Ok(None);
                    }
                    Err(JsErrorBox::from_err(
                      EnsureRegistryReadPermissionError {
                        path: path.to_path_buf(),
                        source: e,
                      },
                    ))
                  }
                },
              }
            };
          if let Some(registry_path_canon) = canonicalize(registry_path)? {
            let is_path_in_registry = is_path_in_registry
              || (could_use_canonical_registry_prefix
                && path_uses_canonical_registry_prefix(
                  &path,
                  registry_path,
                  &registry_path_canon,
                  |path| canonicalize(path).ok().flatten(),
                ));
            if is_path_in_registry {
              let path_canon = match canonicalize(&path)? {
                Some(path_canon) => Some(path_canon),
                None => {
                  // Resolution errors must not disclose host filesystem state
                  // before the ordinary read permission check below.
                  canonicalize_path_maybe_not_exists_following_dangling_links(
                    &self.sys, &path,
                  )
                  .ok()
                }
              };
              if let Some(path_canon) = path_canon {
                if path_canon.starts_with(&registry_path_canon) {
                  return Ok(Cow::Owned(path_canon));
                }
                path = Cow::Owned(path_canon);
              }
            }
          }
        }

        permissions
          .check_open(path, OpenAccessKind::Read, None)
          .map(|p| p.into_path())
          .map_err(JsErrorBox::from_err)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use sys_traits::FsCanonicalize;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsSymlinkDir;

  use super::*;

  fn create_dangling_link_chain(
    sys: &sys_traits::impls::InMemorySys,
    root: &str,
    length: usize,
  ) {
    sys.fs_create_dir_all(root).unwrap();
    for index in 0..length {
      let link = PathBuf::from(format!("{root}/{index}"));
      let target = if index + 1 == length {
        PathBuf::from(format!("{root}/missing"))
      } else {
        PathBuf::from(format!("{root}/{}", index + 1))
      };
      sys.fs_symlink_dir(&target, &link).unwrap();
    }
  }

  #[test]
  fn follows_bounded_dangling_link_chain() {
    let sys = sys_traits::impls::InMemorySys::default();
    create_dangling_link_chain(&sys, "/links", MAX_DANGLING_LINK_DEPTH);

    assert_eq!(
      canonicalize_path_maybe_not_exists_following_dangling_links(
        &sys,
        Path::new("/links/0"),
      )
      .unwrap(),
      PathBuf::from("/links/missing"),
    );
  }

  #[test]
  fn rejects_excessive_dangling_link_chain() {
    let sys = sys_traits::impls::InMemorySys::default();
    create_dangling_link_chain(&sys, "/links", MAX_DANGLING_LINK_DEPTH + 1);

    let err = canonicalize_path_maybe_not_exists_following_dangling_links(
      &sys,
      Path::new("/links/0"),
    )
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Other);
  }

  #[test]
  fn recognizes_canonical_registry_prefix_alias() {
    let sys = sys_traits::impls::InMemorySys::default();
    sys.fs_create_dir_all("/real/root/node_modules").unwrap();
    sys.fs_create_dir_all("/alias/root").unwrap();
    sys.fs_create_dir_all("/other/root/node_modules").unwrap();
    sys
      .fs_symlink_dir("/real/root/node_modules", "/alias/root/node_modules")
      .unwrap();

    let registry_path = Path::new("/real/root/node_modules");
    let canonicalize = |path: &Path| sys.fs_canonicalize(path).ok();
    assert!(path_uses_canonical_registry_prefix(
      Path::new("/alias/root/node_modules/pkg/missing"),
      registry_path,
      registry_path,
      canonicalize,
    ));
    assert!(!path_uses_canonical_registry_prefix(
      Path::new("/other/root/node_modules/pkg/missing"),
      registry_path,
      registry_path,
      canonicalize,
    ));
  }
}
