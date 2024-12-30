// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;

use crate::ExtNodeSys;
use crate::NodePermissions;
use crate::NodeRequireLoaderRc;

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a, P>(
  state: &mut OpState,
  file_path: &'a Path,
) -> Result<Cow<'a, Path>, deno_core::error::AnyError>
where
  P: NodePermissions + 'static,
{
  let loader = state.borrow::<NodeRequireLoaderRc>().clone();
  let permissions = state.borrow_mut::<P>();
  loader.ensure_read_permission(permissions, file_path)
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerThreadsFilenameError {
  #[error(transparent)]
  Permission(deno_core::error::AnyError),
  #[error("{0}")]
  UrlParse(#[from] url::ParseError),
  #[error("Relative path entries must start with '.' or '..'")]
  InvalidRelativeUrl,
  #[error("URL from Path-String")]
  UrlFromPathString,
  #[error("URL to Path-String")]
  UrlToPathString,
  #[error("URL to Path")]
  UrlToPath,
  #[error("File not found [{0:?}]")]
  FileNotFound(PathBuf),
  #[error(transparent)]
  Fs(#[from] std::io::Error),
}

// todo(dsherret): we should remove this and do all this work inside op_create_worker
#[op2(stack_trace)]
#[string]
pub fn op_worker_threads_filename<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] specifier: String,
) -> Result<String, WorkerThreadsFilenameError> {
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
      .map_err(WorkerThreadsFilenameError::Permission)?;
    let sys = state.borrow::<TSys>();
    let canonicalized_path =
      deno_path_util::strip_unc_prefix(sys.fs_canonicalize(&path)?);
    Url::from_file_path(canonicalized_path)
      .map_err(|_| WorkerThreadsFilenameError::UrlFromPathString)?
  };
  let url_path = url
    .to_file_path()
    .map_err(|_| WorkerThreadsFilenameError::UrlToPathString)?;
  let url_path = ensure_read_permission::<P>(state, &url_path)
    .map_err(WorkerThreadsFilenameError::Permission)?;
  let sys = state.borrow::<TSys>();
  if !sys.fs_exists_no_err(&url_path) {
    return Err(WorkerThreadsFilenameError::FileNotFound(
      url_path.to_path_buf(),
    ));
  }
  Ok(url.to_string())
}
