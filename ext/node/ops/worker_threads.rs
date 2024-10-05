// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_fs::FileSystemRc;
use node_resolver::NodeResolution;
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use crate::NodePermissions;
use crate::NodeRequireResolverRc;
use crate::NodeResolverRc;

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a, P>(
  state: &mut OpState,
  file_path: &'a Path,
) -> Result<Cow<'a, Path>, AnyError>
where
  P: NodePermissions + 'static,
{
  let resolver = state.borrow::<NodeRequireResolverRc>().clone();
  let permissions = state.borrow_mut::<P>();
  resolver.ensure_read_permission(permissions, file_path)
}

#[op2]
#[string]
pub fn op_worker_threads_filename<P>(
  state: &mut OpState,
  #[string] specifier: String,
) -> Result<String, AnyError>
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
      return Err(generic_error(
        "Relative path entries must start with '.' or '..'",
      ));
    }
    let path = ensure_read_permission::<P>(state, &path)?;
    let fs = state.borrow::<FileSystemRc>();
    let canonicalized_path =
      deno_core::strip_unc_prefix(fs.realpath_sync(&path)?);
    Url::from_file_path(canonicalized_path)
      .map_err(|e| generic_error(format!("URL from Path-String: {:#?}", e)))?
  };
  let url_path = url
    .to_file_path()
    .map_err(|e| generic_error(format!("URL to Path-String: {:#?}", e)))?;
  let url_path = ensure_read_permission::<P>(state, &url_path)?;
  let fs = state.borrow::<FileSystemRc>();
  if !fs.exists_sync(&url_path) {
    return Err(generic_error(format!("File not found [{:?}]", url_path)));
  }
  let node_resolver = state.borrow::<NodeResolverRc>();
  match node_resolver.url_to_node_resolution(url)? {
    NodeResolution::Esm(u) => Ok(u.to_string()),
    NodeResolution::CommonJs(u) => wrap_cjs(u),
    NodeResolution::BuiltIn(_) => Err(generic_error("Neither ESM nor CJS")),
  }
}

///
/// Wrap a CJS file-URL and the required setup in a stringified `data:`-URL
///
fn wrap_cjs(url: Url) -> Result<String, AnyError> {
  let path = url
    .to_file_path()
    .map_err(|e| generic_error(format!("URL to Path: {:#?}", e)))?;
  let filename = path.file_name().unwrap().to_string_lossy();
  Ok(format!(
    "data:text/javascript,import {{ createRequire }} from \"node:module\";\
    const require = createRequire(\"{}\"); require(\"./{}\");",
    url, filename,
  ))
}
