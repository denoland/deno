// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_fs::FileSystemRc;
use pathdiff::diff_paths;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use crate::resolution;
use crate::NodePermissions;
use crate::NodeResolver;
use crate::NpmResolverRc;

fn ensure_read_permission<P>(
  state: &mut OpState,
  file_path: &Path,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  let resolver = state.borrow::<NpmResolverRc>();
  let permissions = state.borrow::<P>();
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
    ensure_read_permission::<P>(state, &path)?;
    let fs = state.borrow::<FileSystemRc>();
    let canonicalized_path =
      deno_core::strip_unc_prefix(fs.realpath_sync(&path)?);
    Url::from_file_path(canonicalized_path)
      .map_err(|e| generic_error(format!("URL from Path-String: {:#?}", e)))?
  };
  let url_path = Path::new(url.path());
  ensure_read_permission::<P>(state, url_path)?;
  let fs = state.borrow::<FileSystemRc>();
  if !fs.exists_sync(url_path) {
    return Err(generic_error("File not found"));
  }
  let node_resolver = state.borrow::<Rc<NodeResolver>>();
  match node_resolver.url_to_node_resolution(url)? {
    resolution::NodeResolution::Esm(u) => Ok(u.to_string()),
    resolution::NodeResolution::CommonJs(u) => wrap_cjs(state, u),
    _ => Err(generic_error("Neither ESM nor CJS")),
  }
}

///
/// Wrap a CJS file-URL and the required setup in a stringified `data:`-URL
///
fn wrap_cjs(state: &mut OpState, url: Url) -> Result<String, AnyError> {
  let fs = state.borrow::<FileSystemRc>();
  let cwd = fs.cwd()?;
  let cwd_url = Url::from_directory_path(&cwd)
    .map_err(|e| generic_error(format!("Create CWD Url: {:#?}", e)))?;
  let rel_path = match diff_paths(url.path(), &cwd) {
    Some(p) => Path::new(".").join(p),
    None => url.path().into(),
  };
  Ok(format!(
    "data:text/javascript,import {{ createRequire }} from \"node:module\";\
    const require = createRequire(\"{}\"); require(\"{}\");",
    cwd_url,
    rel_path.to_string_lossy()
  ))
}
