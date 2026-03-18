// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_permissions::PermissionsContainer;

use crate::ExtNodeSys;
use crate::NodeRequireLoaderRc;

/// Default thread stack size in MB, matching Node.js default.
pub const DEFAULT_STACK_SIZE_MB: usize = 4;

/// Resolved resource limits with V8 defaults filled in for unspecified values.
/// Stored in the worker's OpState so the JS polyfill can read actual values.
#[derive(deno_core::serde::Serialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedResourceLimits {
  pub max_young_generation_size_mb: usize,
  pub max_old_generation_size_mb: usize,
  pub code_range_size_mb: usize,
  pub stack_size_mb: usize,
}

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a>(
  state: &mut OpState,
  file_path: Cow<'a, Path>,
) -> Result<Cow<'a, Path>, JsErrorBox> {
  let loader = state.borrow::<NodeRequireLoaderRc>().clone();
  let permissions = state.borrow_mut::<PermissionsContainer>();
  loader.ensure_read_permission(permissions, file_path)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WorkerThreadsFilenameError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(JsErrorBox),
  #[class(inherit)]
  #[error("{0}")]
  UrlParse(
    #[from]
    #[inherit]
    url::ParseError,
  ),
  #[class(generic)]
  #[error("Relative path entries must start with '.' or '..'")]
  InvalidRelativeUrl,
  #[class(generic)]
  #[error(transparent)]
  UrlFromPathString(#[from] deno_path_util::PathToUrlError),
  #[class(generic)]
  #[error(transparent)]
  UrlToPathString(#[from] deno_path_util::UrlToFilePathError),
  #[class(generic)]
  #[error("URL to Path")]
  UrlToPath,
  #[class(generic)]
  #[error("File not found [{0:?}]")]
  FileNotFound(PathBuf),
  #[class(inherit)]
  #[error(transparent)]
  Fs(
    #[from]
    #[inherit]
    deno_io::fs::FsError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
}

// todo(dsherret): we should remove this and do all this work inside op_create_worker
#[op2(stack_trace)]
#[string]
pub fn op_worker_threads_filename<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] specifier: &str,
) -> Result<Option<String>, WorkerThreadsFilenameError> {
  if specifier.starts_with("data:") {
    return Ok(None); // use input
  }
  let url: Url = if specifier.starts_with("file:") {
    Url::parse(specifier)?
  } else {
    let path = Path::new(specifier);
    if path.is_relative() && !specifier.starts_with('.') {
      return Err(WorkerThreadsFilenameError::InvalidRelativeUrl);
    }
    let path = ensure_read_permission(state, Cow::Borrowed(path))
      .map_err(WorkerThreadsFilenameError::Permission)?;
    let sys = state.borrow::<TSys>();
    let canonicalized_path = match sys.fs_canonicalize(&path) {
      Ok(p) => Cow::Owned(deno_path_util::strip_unc_prefix(p)),
      Err(_) => path,
    };
    deno_path_util::url_from_file_path(&canonicalized_path)?
  };
  let url_path = deno_path_util::url_to_file_path(&url)?;
  let _url_path = ensure_read_permission(state, Cow::Owned(url_path))
    .map_err(WorkerThreadsFilenameError::Permission)?;
  Ok(Some(url.into()))
}

/// Returns the resolved resource limits for this worker, or None if
/// no resource limits were configured. Called from worker_threads
/// polyfill during init to get actual V8 values (with defaults filled in).
#[op2]
#[serde]
pub fn op_worker_get_resource_limits(
  state: &mut OpState,
) -> Option<ResolvedResourceLimits> {
  state.try_borrow::<ResolvedResourceLimits>().cloned()
}
