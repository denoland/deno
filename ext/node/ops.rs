// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_core::op;
use deno_core::url::Url;
use deno_core::JsRuntimeInspector;
use deno_core::OpState;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use crate::NodeEnv;
use crate::NodeFs;

use super::resolution;
use super::NodeModuleKind;
use super::NodePermissions;
use super::NodeResolutionMode;
use super::PackageJson;
use super::RequireNpmResolver;

fn ensure_read_permission<P>(
  state: &mut OpState,
  file_path: &Path,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  let resolver = {
    let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>();
    resolver.clone()
  };
  let permissions = state.borrow_mut::<P>();
  resolver.ensure_read_permission(permissions, file_path)
}

#[op]
pub fn op_require_init_paths() -> Vec<String> {
  // todo(dsherret): this code is node compat mode specific and
  // we probably don't want it for small mammal, so ignore it for now

  // let (home_dir, node_path) = if cfg!(windows) {
  //   (
  //     std::env::var("USERPROFILE").unwrap_or_else(|_| "".into()),
  //     std::env::var("NODE_PATH").unwrap_or_else(|_| "".into()),
  //   )
  // } else {
  //   (
  //     std::env::var("HOME").unwrap_or_else(|_| "".into()),
  //     std::env::var("NODE_PATH").unwrap_or_else(|_| "".into()),
  //   )
  // };

  // let mut prefix_dir = std::env::current_exe().unwrap();
  // if cfg!(windows) {
  //   prefix_dir = prefix_dir.join("..").join("..")
  // } else {
  //   prefix_dir = prefix_dir.join("..")
  // }

  // let mut paths = vec![prefix_dir.join("lib").join("node")];

  // if !home_dir.is_empty() {
  //   paths.insert(0, PathBuf::from(&home_dir).join(".node_libraries"));
  //   paths.insert(0, PathBuf::from(&home_dir).join(".nod_modules"));
  // }

  // let mut paths = paths
  //   .into_iter()
  //   .map(|p| p.to_string_lossy().to_string())
  //   .collect();

  // if !node_path.is_empty() {
  //   let delimiter = if cfg!(windows) { ";" } else { ":" };
  //   let mut node_paths: Vec<String> = node_path
  //     .split(delimiter)
  //     .filter(|e| !e.is_empty())
  //     .map(|s| s.to_string())
  //     .collect();
  //   node_paths.append(&mut paths);
  //   paths = node_paths;
  // }

  vec![]
}

#[op]
pub fn op_require_node_module_paths<Env>(
  state: &mut OpState,
  from: String,
) -> Result<Vec<String>, AnyError>
where
  Env: NodeEnv + 'static,
{
  // Guarantee that "from" is absolute.
  let from = deno_core::resolve_path(
    &from,
    &(Env::Fs::current_dir()).context("Unable to get CWD")?,
  )
  .unwrap()
  .to_file_path()
  .unwrap();

  ensure_read_permission::<Env::P>(state, &from)?;

  if cfg!(windows) {
    // return root node_modules when path is 'D:\\'.
    let from_str = from.to_str().unwrap();
    if from_str.len() >= 3 {
      let bytes = from_str.as_bytes();
      if bytes[from_str.len() - 1] == b'\\' && bytes[from_str.len() - 2] == b':'
      {
        let p = from_str.to_owned() + "node_modules";
        return Ok(vec![p]);
      }
    }
  } else {
    // Return early not only to avoid unnecessary work, but to *avoid* returning
    // an array of two items for a root: [ '//node_modules', '/node_modules' ]
    if from.to_string_lossy() == "/" {
      return Ok(vec!["/node_modules".to_string()]);
    }
  }

  let mut paths = vec![];
  let mut current_path = from.as_path();
  let mut maybe_parent = Some(current_path);
  while let Some(parent) = maybe_parent {
    if !parent.ends_with("/node_modules") {
      paths.push(parent.join("node_modules").to_string_lossy().to_string());
      current_path = parent;
      maybe_parent = current_path.parent();
    }
  }

  if !cfg!(windows) {
    // Append /node_modules to handle root paths.
    paths.push("/node_modules".to_string());
  }

  Ok(paths)
}

#[op]
fn op_require_proxy_path(filename: String) -> String {
  // Allow a directory to be passed as the filename
  let trailing_slash = if cfg!(windows) {
    // Node also counts a trailing forward slash as a
    // directory for node on Windows, but not backslashes
    // on non-Windows platforms
    filename.ends_with('\\') || filename.ends_with('/')
  } else {
    filename.ends_with('/')
  };

  if trailing_slash {
    let p = PathBuf::from(filename);
    p.join("noop.js").to_string_lossy().to_string()
  } else {
    filename
  }
}

#[op]
fn op_require_is_request_relative(request: String) -> bool {
  if request.starts_with("./") || request.starts_with("../") || request == ".."
  {
    return true;
  }

  if cfg!(windows) {
    if request.starts_with(".\\") {
      return true;
    }

    if request.starts_with("..\\") {
      return true;
    }
  }

  false
}

#[op]
fn op_require_resolve_deno_dir(
  state: &mut OpState,
  request: String,
  parent_filename: String,
) -> Option<String> {
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>();
  resolver
    .resolve_package_folder_from_package(
      &request,
      &PathBuf::from(parent_filename),
      NodeResolutionMode::Execution,
    )
    .ok()
    .map(|p| p.to_string_lossy().to_string())
}

#[op]
fn op_require_is_deno_dir_package(state: &mut OpState, path: String) -> bool {
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>();
  resolver.in_npm_package(&PathBuf::from(path))
}

#[op]
fn op_require_resolve_lookup_paths(
  request: String,
  maybe_parent_paths: Option<Vec<String>>,
  parent_filename: String,
) -> Option<Vec<String>> {
  if !request.starts_with('.')
    || (request.len() > 1
      && !request.starts_with("..")
      && !request.starts_with("./")
      && (!cfg!(windows) || !request.starts_with(".\\")))
  {
    let module_paths = vec![];
    let mut paths = module_paths;
    if let Some(mut parent_paths) = maybe_parent_paths {
      if !parent_paths.is_empty() {
        paths.append(&mut parent_paths);
      }
    }

    if !paths.is_empty() {
      return Some(paths);
    } else {
      return None;
    }
  }

  // In REPL, parent.filename is null.
  // if (!parent || !parent.id || !parent.filename) {
  //   // Make require('./path/to/foo') work - normally the path is taken
  //   // from realpath(__filename) but in REPL there is no filename
  //   const mainPaths = ['.'];

  //   debug('looking for %j in %j', request, mainPaths);
  //   return mainPaths;
  // }

  let p = PathBuf::from(parent_filename);
  Some(vec![p.parent().unwrap().to_string_lossy().to_string()])
}

#[op]
fn op_require_path_is_absolute(p: String) -> bool {
  PathBuf::from(p).is_absolute()
}

#[op]
fn op_require_stat<Env>(
  state: &mut OpState,
  path: String,
) -> Result<i32, AnyError>
where
  Env: NodeEnv + 'static,
{
  let path = PathBuf::from(path);
  ensure_read_permission::<Env::P>(state, &path)?;
  if let Ok(metadata) = Env::Fs::metadata(&path) {
    if metadata.is_file {
      return Ok(0);
    } else {
      return Ok(1);
    }
  }

  Ok(-1)
}

#[op]
fn op_require_real_path<Env>(
  state: &mut OpState,
  request: String,
) -> Result<String, AnyError>
where
  Env: NodeEnv + 'static,
{
  let path = PathBuf::from(request);
  ensure_read_permission::<Env::P>(state, &path)?;
  let mut canonicalized_path = Env::Fs::canonicalize(&path)?;
  if cfg!(windows) {
    canonicalized_path = PathBuf::from(
      canonicalized_path
        .display()
        .to_string()
        .trim_start_matches("\\\\?\\"),
    );
  }
  Ok(canonicalized_path.to_string_lossy().to_string())
}

fn path_resolve(parts: Vec<String>) -> String {
  assert!(!parts.is_empty());
  let mut p = PathBuf::from(&parts[0]);
  if parts.len() > 1 {
    for part in &parts[1..] {
      p = p.join(part);
    }
  }
  normalize_path(p).to_string_lossy().to_string()
}

#[op]
fn op_require_path_resolve(parts: Vec<String>) -> String {
  path_resolve(parts)
}

#[op]
fn op_require_path_dirname(request: String) -> Result<String, AnyError> {
  let p = PathBuf::from(request);
  if let Some(parent) = p.parent() {
    Ok(parent.to_string_lossy().to_string())
  } else {
    Err(generic_error("Path doesn't have a parent"))
  }
}

#[op]
fn op_require_path_basename(request: String) -> Result<String, AnyError> {
  let p = PathBuf::from(request);
  if let Some(path) = p.file_name() {
    Ok(path.to_string_lossy().to_string())
  } else {
    Err(generic_error("Path doesn't have a file name"))
  }
}

#[op]
fn op_require_try_self_parent_path<Env>(
  state: &mut OpState,
  has_parent: bool,
  maybe_parent_filename: Option<String>,
  maybe_parent_id: Option<String>,
) -> Result<Option<String>, AnyError>
where
  Env: NodeEnv + 'static,
{
  if !has_parent {
    return Ok(None);
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Ok(Some(parent_filename));
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      if let Ok(cwd) = Env::Fs::current_dir() {
        ensure_read_permission::<Env::P>(state, &cwd)?;
        return Ok(Some(cwd.to_string_lossy().to_string()));
      }
    }
  }
  Ok(None)
}

#[op]
fn op_require_try_self<Env>(
  state: &mut OpState,
  parent_path: Option<String>,
  request: String,
) -> Result<Option<String>, AnyError>
where
  Env: NodeEnv + 'static,
{
  if parent_path.is_none() {
    return Ok(None);
  }

  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>().clone();
  let permissions = state.borrow_mut::<Env::P>();
  let pkg = resolution::get_package_scope_config::<Env::Fs>(
    &Url::from_file_path(parent_path.unwrap()).unwrap(),
    &*resolver,
    permissions,
  )
  .ok();
  if pkg.is_none() {
    return Ok(None);
  }

  let pkg = pkg.unwrap();
  if pkg.exports.is_none() {
    return Ok(None);
  }
  if pkg.name.is_none() {
    return Ok(None);
  }

  let pkg_name = pkg.name.as_ref().unwrap().to_string();
  let mut expansion = ".".to_string();

  if request == pkg_name {
    // pass
  } else if request.starts_with(&format!("{pkg_name}/")) {
    expansion += &request[pkg_name.len()..];
  } else {
    return Ok(None);
  }

  let referrer = deno_core::url::Url::from_file_path(&pkg.path).unwrap();
  if let Some(exports) = &pkg.exports {
    resolution::package_exports_resolve::<Env::Fs>(
      &pkg.path,
      expansion,
      exports,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      NodeResolutionMode::Execution,
      &*resolver,
      permissions,
    )
    .map(|r| Some(r.to_string_lossy().to_string()))
  } else {
    Ok(None)
  }
}

#[op]
fn op_require_read_file<Env>(
  state: &mut OpState,
  file_path: String,
) -> Result<String, AnyError>
where
  Env: NodeEnv + 'static,
{
  let file_path = PathBuf::from(file_path);
  ensure_read_permission::<Env::P>(state, &file_path)?;
  Ok(Env::Fs::read_to_string(file_path)?)
}

#[op]
pub fn op_require_as_file_path(file_or_url: String) -> String {
  if let Ok(url) = Url::parse(&file_or_url) {
    if let Ok(p) = url.to_file_path() {
      return p.to_string_lossy().to_string();
    }
  }

  file_or_url
}

#[op]
fn op_require_resolve_exports<Env>(
  state: &mut OpState,
  uses_local_node_modules_dir: bool,
  modules_path: String,
  _request: String,
  name: String,
  expansion: String,
  parent_path: String,
) -> Result<Option<String>, AnyError>
where
  Env: NodeEnv + 'static,
{
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>().clone();
  let permissions = state.borrow_mut::<Env::P>();

  let pkg_path = if resolver.in_npm_package(&PathBuf::from(&modules_path))
    && !uses_local_node_modules_dir
  {
    modules_path
  } else {
    let orignal = modules_path.clone();
    let mod_dir = path_resolve(vec![modules_path, name]);
    if Env::Fs::is_dir(&mod_dir) {
      mod_dir
    } else {
      orignal
    }
  };
  let pkg = PackageJson::load::<Env::Fs>(
    &*resolver,
    permissions,
    PathBuf::from(&pkg_path).join("package.json"),
  )?;

  if let Some(exports) = &pkg.exports {
    let referrer = Url::from_file_path(parent_path).unwrap();
    resolution::package_exports_resolve::<Env::Fs>(
      &pkg.path,
      format!(".{expansion}"),
      exports,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      NodeResolutionMode::Execution,
      &*resolver,
      permissions,
    )
    .map(|r| Some(r.to_string_lossy().to_string()))
  } else {
    Ok(None)
  }
}

#[op]
fn op_require_read_closest_package_json<Env>(
  state: &mut OpState,
  filename: String,
) -> Result<PackageJson, AnyError>
where
  Env: NodeEnv + 'static,
{
  ensure_read_permission::<Env::P>(
    state,
    PathBuf::from(&filename).parent().unwrap(),
  )?;
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>().clone();
  let permissions = state.borrow_mut::<Env::P>();
  resolution::get_closest_package_json::<Env::Fs>(
    &Url::from_file_path(filename).unwrap(),
    &*resolver,
    permissions,
  )
}

#[op]
fn op_require_read_package_scope<Env>(
  state: &mut OpState,
  package_json_path: String,
) -> Option<PackageJson>
where
  Env: NodeEnv + 'static,
{
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>().clone();
  let permissions = state.borrow_mut::<Env::P>();
  let package_json_path = PathBuf::from(package_json_path);
  PackageJson::load::<Env::Fs>(&*resolver, permissions, package_json_path).ok()
}

#[op]
fn op_require_package_imports_resolve<Env>(
  state: &mut OpState,
  parent_filename: String,
  request: String,
) -> Result<Option<String>, AnyError>
where
  Env: NodeEnv + 'static,
{
  let parent_path = PathBuf::from(&parent_filename);
  ensure_read_permission::<Env::P>(state, &parent_path)?;
  let resolver = state.borrow::<Rc<dyn RequireNpmResolver>>().clone();
  let permissions = state.borrow_mut::<Env::P>();
  let pkg = PackageJson::load::<Env::Fs>(
    &*resolver,
    permissions,
    parent_path.join("package.json"),
  )?;

  if pkg.imports.is_some() {
    let referrer =
      deno_core::url::Url::from_file_path(&parent_filename).unwrap();
    let r = resolution::package_imports_resolve::<Env::Fs>(
      &request,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      NodeResolutionMode::Execution,
      &*resolver,
      permissions,
    )
    .map(|r| Some(Url::from_file_path(r).unwrap().to_string()));
    state.put(resolver);
    r
  } else {
    Ok(None)
  }
}

#[op]
fn op_require_break_on_next_statement(state: &mut OpState) {
  let inspector = state.borrow::<Rc<RefCell<JsRuntimeInspector>>>();
  inspector
    .borrow_mut()
    .wait_for_session_and_break_on_next_statement()
}
