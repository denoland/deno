// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_fs::FileSystemRc;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use deno_permissions::PermissionCheckError;
use crate::NodePermissions;
use crate::NodeRequireLoaderRc;

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a, P>(
  state: &mut OpState,
  file_path: &'a Path,
) -> Result<Cow<'a, Path>, PermissionCheckError>
where
  P: NodePermissions + 'static,
{
  let loader = state.borrow::<NodeRequireLoaderRc>().clone();
  let permissions = state.borrow_mut::<P>();
  loader.ensure_read_permission(permissions, file_path)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WorkerThreadsFilenameError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] #[inherit] PermissionCheckError),
  #[class(inherit)]
  #[error("{0}")]
  UrlParse(#[from] #[inherit] url::ParseError),
  #[class(generic)]
  #[error("Relative path entries must start with '.' or '..'")]
  InvalidRelativeUrl,
  #[class(generic)]
  #[error("URL from Path-String")]
  UrlFromPathString,
  #[class(generic)]
  #[error("URL to Path-String")]
  UrlToPathString,
  #[class(generic)]
  #[error("URL to Path")]
  UrlToPath,
  #[class(generic)]
  #[error("File not found [{0:?}]")]
  FileNotFound(PathBuf),
  #[class(inherit)]
  #[error(transparent)]
  Fs(#[from] #[inherit] deno_io::fs::FsError),
}

// todo(dsherret): we should remove this and do all this work inside op_create_worker
#[op2(stack_trace)]
#[string]
pub fn op_worker_threads_filename<P>(
  state: &mut OpState,
  #[string] specifier: String,
) -> Result<String, WorkerThreadsFilenameError>
where
  P: NodePermissions + 'static,
{
  if specifier.starts_with("data:") {
    return Ok(specifier);
  }
  let url: Url = if specifier.starts_with("file:") {
    Url::parse(&specifier)?
  } else {
    let path = PathBuf::from(&specifier);
    if path.is_relative() && !specifier.starts_with('.') {
      return Err(WorkerThreadsFilenameError::InvalidRelativeUrl);
    }
    let path = ensure_read_permission::<P>(state, &path)
      ?;
    let fs = state.borrow::<FileSystemRc>();
    let canonicalized_path =
      deno_path_util::strip_unc_prefix(fs.realpath_sync(&path)?);
    Url::from_file_path(canonicalized_path)
      .map_err(|_| WorkerThreadsFilenameError::UrlFromPathString)?
  };
  let url_path = url
    .to_file_path()
    .map_err(|_| WorkerThreadsFilenameError::UrlToPathString)?;
  let url_path = ensure_read_permission::<P>(state, &url_path)
    ?;
  let fs = state.borrow::<FileSystemRc>();
  if !fs.exists_sync(&url_path) {
    return Err(WorkerThreadsFilenameError::FileNotFound(
      url_path.to_path_buf(),
    ));
  }
  Ok(url.to_string())
}
