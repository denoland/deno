// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_core::parking_lot::Mutex;
use deno_error::JsErrorBox;
use deno_runtime::deno_node::NodePermissions;
use sys_traits::FsCanonicalize;

use crate::sys::CliSys;

#[derive(Debug)]
pub enum NpmRegistryReadPermissionCheckerMode {
  Byonm,
  Global(PathBuf),
  Local(PathBuf),
}

#[derive(Debug)]
pub struct NpmRegistryReadPermissionChecker {
  sys: CliSys,
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

impl NpmRegistryReadPermissionChecker {
  pub fn new(sys: CliSys, mode: NpmRegistryReadPermissionCheckerMode) -> Self {
    Self {
      sys,
      cache: Default::default(),
      mode,
    }
  }

  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  pub fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &'a Path,
  ) -> Result<Cow<'a, Path>, JsErrorBox> {
    if permissions.query_read_all() {
      return Ok(Cow::Borrowed(path)); // skip permissions checks below
    }

    match &self.mode {
      NpmRegistryReadPermissionCheckerMode::Byonm => {
        if path.components().any(|c| c.as_os_str() == "node_modules") {
          Ok(Cow::Borrowed(path))
        } else {
          permissions
            .check_read_path(path)
            .map_err(JsErrorBox::from_err)
        }
      }
      NpmRegistryReadPermissionCheckerMode::Global(registry_path)
      | NpmRegistryReadPermissionCheckerMode::Local(registry_path) => {
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
            if let Some(path_canon) = canonicalize(path)? {
              if path_canon.starts_with(registry_path_canon) {
                return Ok(Cow::Owned(path_canon));
              }
            } else if path.starts_with(registry_path_canon)
              || path.starts_with(registry_path)
            {
              return Ok(Cow::Borrowed(path));
            }
          }
        }

        permissions
          .check_read_path(path)
          .map_err(JsErrorBox::from_err)
      }
    }
  }
}
