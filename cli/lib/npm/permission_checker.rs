// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
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

/// Canonicalizes the nearest existing entry, then appends any unresolved path
/// components. Using symlink metadata to find that entry is important because
/// it stops at a dangling symlink or junction instead of treating the link's
/// name as an ordinary missing component.
fn canonicalize_path_maybe_not_exists<TSys>(
  sys: &TSys,
  mut path: &Path,
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
            let target = sys.fs_read_link(path)?;
            let target = if target.is_absolute() {
              target
            } else {
              path.parent().unwrap_or_else(|| Path::new(".")).join(target)
            };
            let target =
              deno_path_util::normalize_path(Cow::Owned(target)).into_owned();
            canonicalize_path_maybe_not_exists(sys, &target)?
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
  ) -> Result<Cow<'a, Path>, JsErrorBox> {
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
        // allow reading if it's in the node_modules
        let is_path_in_node_modules = path.starts_with(registry_path)
          && path
            .components()
            .all(|c| !matches!(c, std::path::Component::ParentDir));

        if is_path_in_node_modules {
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
            let path_canon = if let Some(path_canon) = canonicalize(&path)? {
              path_canon
            } else {
              canonicalize_path_maybe_not_exists(&self.sys, &path).map_err(
                |source| {
                  JsErrorBox::from_err(EnsureRegistryReadPermissionError {
                    path: path.to_path_buf(),
                    source,
                  })
                },
              )?
            };
            if path_canon.starts_with(&registry_path_canon) {
              return Ok(Cow::Owned(path_canon));
            }
            path = Cow::Owned(path_canon);
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
