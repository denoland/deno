// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::normalize_path;
use deno_core::op;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::OpState;
use once_cell::sync::Lazy;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub mod errors;
mod package_json;
mod resolution;

pub use package_json::PackageJson;
pub use resolution::get_closest_package_json;
pub use resolution::get_package_scope_config;
pub use resolution::legacy_main_resolve;
pub use resolution::package_exports_resolve;
pub use resolution::package_imports_resolve;
pub use resolution::package_resolve;
pub use resolution::NodeModuleKind;
pub use resolution::DEFAULT_CONDITIONS;

pub trait NodePermissions {
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError>;
}

pub trait DenoDirNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &Path,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError>;

  fn in_npm_package(&self, path: &Path) -> bool;

  fn ensure_read_permission(&self, path: &Path) -> Result<(), AnyError>;
}

pub const MODULE_ES_SHIM: &str = include_str!("./module_es_shim.js");

pub static NODE_GLOBAL_THIS_NAME: Lazy<String> = Lazy::new(|| {
  let now = std::time::SystemTime::now();
  let seconds = now
    .duration_since(std::time::SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  // use a changing variable name to make it hard to depend on this
  format!("__DENO_NODE_GLOBAL_THIS_{}__", seconds)
});

struct Unstable(pub bool);

pub fn init<P: NodePermissions + 'static>(
  unstable: bool,
  maybe_npm_resolver: Option<Rc<dyn DenoDirNpmResolver>>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/node",
      "01_node.js",
      "02_require.js",
    ))
    .ops(vec![
      op_require_init_paths::decl(),
      op_require_node_module_paths::decl::<P>(),
      op_require_proxy_path::decl(),
      op_require_is_deno_dir_package::decl(),
      op_require_resolve_deno_dir::decl(),
      op_require_is_request_relative::decl(),
      op_require_resolve_lookup_paths::decl(),
      op_require_try_self_parent_path::decl::<P>(),
      op_require_try_self::decl(),
      op_require_real_path::decl::<P>(),
      op_require_path_is_absolute::decl(),
      op_require_path_dirname::decl(),
      op_require_stat::decl::<P>(),
      op_require_path_resolve::decl(),
      op_require_path_basename::decl(),
      op_require_read_file::decl::<P>(),
      op_require_as_file_path::decl(),
      op_require_resolve_exports::decl(),
      op_require_read_closest_package_json::decl::<P>(),
      op_require_read_package_scope::decl(),
      op_require_package_imports_resolve::decl::<P>(),
    ])
    .state(move |state| {
      state.put(Unstable(unstable));
      if let Some(npm_resolver) = maybe_npm_resolver.clone() {
        state.put(npm_resolver);
      }
      Ok(())
    })
    .build()
}

fn check_unstable(state: &OpState) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!("Unstable API 'require'. The --unstable flag must be provided.",);
    std::process::exit(70);
  }
}

fn ensure_read_permission<P>(
  state: &mut OpState,
  file_path: &Path,
) -> Result<(), AnyError>
where
  P: NodePermissions + 'static,
{
  let resolver = {
    let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>();
    resolver.clone()
  };
  if resolver.ensure_read_permission(file_path).is_ok() {
    return Ok(());
  }

  state.borrow_mut::<P>().check_read(file_path)
}

#[op]
pub fn op_require_init_paths(state: &mut OpState) -> Vec<String> {
  check_unstable(state);

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
pub fn op_require_node_module_paths<P>(
  state: &mut OpState,
  from: String,
) -> Result<Vec<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  // Guarantee that "from" is absolute.
  let from = deno_core::resolve_path(&from)
    .unwrap()
    .to_file_path()
    .unwrap();

  ensure_read_permission::<P>(state, &from)?;

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
fn op_require_proxy_path(state: &mut OpState, filename: String) -> String {
  check_unstable(state);
  // Allow a directory to be passed as the filename
  let trailing_slash = if cfg!(windows) {
    filename.ends_with('\\')
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
fn op_require_is_request_relative(
  state: &mut OpState,
  request: String,
) -> bool {
  check_unstable(state);
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
  check_unstable(state);
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>();
  resolver
    .resolve_package_folder_from_package(
      &request,
      &PathBuf::from(parent_filename),
    )
    .ok()
    .map(|p| p.to_string_lossy().to_string())
}

#[op]
fn op_require_is_deno_dir_package(state: &mut OpState, path: String) -> bool {
  check_unstable(state);
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>();
  resolver.in_npm_package(&PathBuf::from(path))
}

#[op]
fn op_require_resolve_lookup_paths(
  state: &mut OpState,
  request: String,
  maybe_parent_paths: Option<Vec<String>>,
  parent_filename: String,
) -> Option<Vec<String>> {
  check_unstable(state);
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
fn op_require_path_is_absolute(state: &mut OpState, p: String) -> bool {
  check_unstable(state);
  PathBuf::from(p).is_absolute()
}

#[op]
fn op_require_stat<P>(
  state: &mut OpState,
  path: String,
) -> Result<i32, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  let path = PathBuf::from(path);
  ensure_read_permission::<P>(state, &path)?;
  if let Ok(metadata) = std::fs::metadata(&path) {
    if metadata.is_file() {
      return Ok(0);
    } else {
      return Ok(1);
    }
  }

  Ok(-1)
}

#[op]
fn op_require_real_path<P>(
  state: &mut OpState,
  request: String,
) -> Result<String, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  let path = PathBuf::from(request);
  ensure_read_permission::<P>(state, &path)?;
  let mut canonicalized_path = path.canonicalize()?;
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
fn op_require_path_resolve(state: &mut OpState, parts: Vec<String>) -> String {
  check_unstable(state);
  path_resolve(parts)
}

#[op]
fn op_require_path_dirname(state: &mut OpState, request: String) -> String {
  check_unstable(state);
  let p = PathBuf::from(request);
  p.parent().unwrap().to_string_lossy().to_string()
}

#[op]
fn op_require_path_basename(state: &mut OpState, request: String) -> String {
  check_unstable(state);
  let p = PathBuf::from(request);
  p.file_name().unwrap().to_string_lossy().to_string()
}

#[op]
fn op_require_try_self_parent_path<P>(
  state: &mut OpState,
  has_parent: bool,
  maybe_parent_filename: Option<String>,
  maybe_parent_id: Option<String>,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  if !has_parent {
    return Ok(None);
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Ok(Some(parent_filename));
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      if let Ok(cwd) = std::env::current_dir() {
        ensure_read_permission::<P>(state, &cwd)?;
        return Ok(Some(cwd.to_string_lossy().to_string()));
      }
    }
  }
  Ok(None)
}

#[op]
fn op_require_try_self(
  state: &mut OpState,
  parent_path: Option<String>,
  request: String,
) -> Result<Option<String>, AnyError> {
  check_unstable(state);
  if parent_path.is_none() {
    return Ok(None);
  }

  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>().clone();
  let pkg = resolution::get_package_scope_config(
    &Url::from_file_path(parent_path.unwrap()).unwrap(),
    &*resolver,
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
  } else if request.starts_with(&format!("{}/", pkg_name)) {
    expansion += &request[pkg_name.len()..];
  } else {
    return Ok(None);
  }

  let referrer = deno_core::url::Url::from_file_path(&pkg.path).unwrap();
  if let Some(exports) = &pkg.exports {
    resolution::package_exports_resolve(
      &pkg.path,
      expansion,
      exports,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      &*resolver,
    )
    .map(|r| Some(r.to_string_lossy().to_string()))
  } else {
    Ok(None)
  }
}

#[op]
fn op_require_read_file<P>(
  state: &mut OpState,
  file_path: String,
) -> Result<String, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  let file_path = PathBuf::from(file_path);
  ensure_read_permission::<P>(state, &file_path)?;
  Ok(std::fs::read_to_string(file_path)?)
}

#[op]
pub fn op_require_as_file_path(
  state: &mut OpState,
  file_or_url: String,
) -> String {
  check_unstable(state);
  match Url::parse(&file_or_url) {
    Ok(url) => url.to_file_path().unwrap().to_string_lossy().to_string(),
    Err(_) => file_or_url,
  }
}

#[op]
fn op_require_resolve_exports(
  state: &mut OpState,
  modules_path: String,
  _request: String,
  name: String,
  expansion: String,
  parent_path: String,
) -> Result<Option<String>, AnyError> {
  check_unstable(state);
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>().clone();

  let pkg_path = if resolver.in_npm_package(&PathBuf::from(&modules_path)) {
    modules_path
  } else {
    path_resolve(vec![modules_path, name])
  };
  let pkg = PackageJson::load(
    &*resolver,
    PathBuf::from(&pkg_path).join("package.json"),
  )?;

  if let Some(exports) = &pkg.exports {
    let referrer = Url::from_file_path(parent_path).unwrap();
    resolution::package_exports_resolve(
      &pkg.path,
      format!(".{}", expansion),
      exports,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      &*resolver,
    )
    .map(|r| Some(r.to_string_lossy().to_string()))
  } else {
    Ok(None)
  }
}

#[op]
fn op_require_read_closest_package_json<P>(
  state: &mut OpState,
  filename: String,
) -> Result<PackageJson, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  ensure_read_permission::<P>(
    state,
    PathBuf::from(&filename).parent().unwrap(),
  )?;
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>().clone();
  resolution::get_closest_package_json(
    &Url::from_file_path(filename).unwrap(),
    &*resolver,
  )
}

#[op]
fn op_require_read_package_scope(
  state: &mut OpState,
  filename: String,
) -> Option<PackageJson> {
  check_unstable(state);
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>().clone();
  resolution::get_package_scope_config(
    &Url::from_file_path(filename).unwrap(),
    &*resolver,
  )
  .ok()
}

#[op]
fn op_require_package_imports_resolve<P>(
  state: &mut OpState,
  parent_filename: String,
  request: String,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  check_unstable(state);
  let parent_path = PathBuf::from(&parent_filename);
  ensure_read_permission::<P>(state, &parent_path)?;
  let resolver = state.borrow::<Rc<dyn DenoDirNpmResolver>>().clone();
  let pkg = PackageJson::load(&*resolver, parent_path.join("package.json"))?;

  if pkg.imports.is_some() {
    let referrer =
      deno_core::url::Url::from_file_path(&parent_filename).unwrap();
    let r = resolution::package_imports_resolve(
      &request,
      &referrer,
      NodeModuleKind::Cjs,
      resolution::REQUIRE_CONDITIONS,
      &*resolver,
    )
    .map(|r| Some(Url::from_file_path(r).unwrap().to_string()));
    state.put(resolver);
    r
  } else {
    Ok(None)
  }
}
