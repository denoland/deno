// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::JsRuntimeInspector;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_fs::FileSystemRc;
use deno_package_json::PackageJsonRc;
use deno_path_util::normalize_path;
use node_resolver::NodeModuleKind;
use node_resolver::NodeResolutionMode;
use node_resolver::REQUIRE_CONDITIONS;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use crate::NodePermissions;
use crate::NodeRequireResolverRc;
use crate::NodeResolverRc;
use crate::NpmResolverRc;

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
#[serde]
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
  //   .map(|p| p.to_string_lossy().into_owned())
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

#[op2]
#[serde]
pub fn op_require_node_module_paths<P>(
  state: &mut OpState,
  #[string] from: String,
) -> Result<Vec<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  let fs = state.borrow::<FileSystemRc>();
  // Guarantee that "from" is absolute.
  let from = if from.starts_with("file:///") {
    url_to_file_path(&Url::parse(&from)?)?
  } else {
    let current_dir =
      &(fs.cwd().map_err(AnyError::from)).context("Unable to get CWD")?;
    deno_path_util::normalize_path(current_dir.join(from))
  };

  let from = ensure_read_permission::<P>(state, &from)?;

  if cfg!(windows) {
    // return root node_modules when path is 'D:\\'.
    let from_str = from.to_str().unwrap();
    if from_str.len() >= 3 {
      let bytes = from_str.as_bytes();
      if bytes[from_str.len() - 1] == b'\\' && bytes[from_str.len() - 2] == b':'
      {
        let p = format!("{}node_modules", from_str);
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

  let mut paths = Vec::with_capacity(from.components().count());
  let mut current_path = from.as_ref();
  let mut maybe_parent = Some(current_path);
  while let Some(parent) = maybe_parent {
    if !parent.ends_with("node_modules") {
      paths.push(parent.join("node_modules").to_string_lossy().into_owned());
    }
    current_path = parent;
    maybe_parent = current_path.parent();
  }

  Ok(paths)
}

#[op2]
#[string]
pub fn op_require_proxy_path(#[string] filename: String) -> String {
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
    p.join("noop.js").to_string_lossy().into_owned()
  } else {
    filename
  }
}

#[op2(fast)]
pub fn op_require_is_request_relative(#[string] request: String) -> bool {
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

#[op2]
#[string]
pub fn op_require_resolve_deno_dir(
  state: &mut OpState,
  #[string] request: String,
  #[string] parent_filename: String,
) -> Option<String> {
  let resolver = state.borrow::<NpmResolverRc>();
  resolver
    .resolve_package_folder_from_package(
      &request,
      &ModuleSpecifier::from_file_path(&parent_filename).unwrap_or_else(|_| {
        panic!("Url::from_file_path: [{:?}]", parent_filename)
      }),
    )
    .ok()
    .map(|p| p.to_string_lossy().into_owned())
}

#[op2(fast)]
pub fn op_require_is_deno_dir_package(
  state: &mut OpState,
  #[string] path: String,
) -> bool {
  let resolver = state.borrow::<NpmResolverRc>();
  resolver.in_npm_package_at_file_path(&PathBuf::from(path))
}

#[op2]
#[serde]
pub fn op_require_resolve_lookup_paths(
  #[string] request: String,
  #[serde] maybe_parent_paths: Option<Vec<String>>,
  #[string] parent_filename: String,
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
  Some(vec![p.parent().unwrap().to_string_lossy().into_owned()])
}

#[op2(fast)]
pub fn op_require_path_is_absolute(#[string] p: String) -> bool {
  PathBuf::from(p).is_absolute()
}

#[op2(fast)]
pub fn op_require_stat<P>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<i32, AnyError>
where
  P: NodePermissions + 'static,
{
  let path = PathBuf::from(path);
  let path = ensure_read_permission::<P>(state, &path)?;
  let fs = state.borrow::<FileSystemRc>();
  if let Ok(metadata) = fs.stat_sync(&path) {
    if metadata.is_file {
      return Ok(0);
    } else {
      return Ok(1);
    }
  }

  Ok(-1)
}

#[op2]
#[string]
pub fn op_require_real_path<P>(
  state: &mut OpState,
  #[string] request: String,
) -> Result<String, AnyError>
where
  P: NodePermissions + 'static,
{
  let path = PathBuf::from(request);
  let path = ensure_read_permission::<P>(state, &path)?;
  let fs = state.borrow::<FileSystemRc>();
  let canonicalized_path =
    deno_core::strip_unc_prefix(fs.realpath_sync(&path)?);
  Ok(canonicalized_path.to_string_lossy().into_owned())
}

fn path_resolve<'a>(mut parts: impl Iterator<Item = &'a str>) -> PathBuf {
  let mut p = PathBuf::from(parts.next().unwrap());
  for part in parts {
    p = p.join(part);
  }
  normalize_path(p)
}

#[op2]
#[string]
pub fn op_require_path_resolve(#[serde] parts: Vec<String>) -> String {
  path_resolve(parts.iter().map(|s| s.as_str()))
    .to_string_lossy()
    .into_owned()
}

#[op2]
#[string]
pub fn op_require_path_dirname(
  #[string] request: String,
) -> Result<String, AnyError> {
  let p = PathBuf::from(request);
  if let Some(parent) = p.parent() {
    Ok(parent.to_string_lossy().into_owned())
  } else {
    Err(generic_error("Path doesn't have a parent"))
  }
}

#[op2]
#[string]
pub fn op_require_path_basename(
  #[string] request: String,
) -> Result<String, AnyError> {
  let p = PathBuf::from(request);
  if let Some(path) = p.file_name() {
    Ok(path.to_string_lossy().into_owned())
  } else {
    Err(generic_error("Path doesn't have a file name"))
  }
}

#[op2]
#[string]
pub fn op_require_try_self_parent_path<P>(
  state: &mut OpState,
  has_parent: bool,
  #[string] maybe_parent_filename: Option<String>,
  #[string] maybe_parent_id: Option<String>,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  if !has_parent {
    return Ok(None);
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Ok(Some(parent_filename));
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      let fs = state.borrow::<FileSystemRc>();
      if let Ok(cwd) = fs.cwd() {
        let cwd = ensure_read_permission::<P>(state, &cwd)?;
        return Ok(Some(cwd.to_string_lossy().into_owned()));
      }
    }
  }
  Ok(None)
}

#[op2]
#[string]
pub fn op_require_try_self<P>(
  state: &mut OpState,
  #[string] parent_path: Option<String>,
  #[string] request: String,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  if parent_path.is_none() {
    return Ok(None);
  }

  let node_resolver = state.borrow::<NodeResolverRc>();
  let pkg = node_resolver
    .get_closest_package_json_from_path(&PathBuf::from(parent_path.unwrap()))
    .ok()
    .flatten();
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
    let r = node_resolver.package_exports_resolve(
      &pkg.path,
      &expansion,
      exports,
      Some(&referrer),
      NodeModuleKind::Cjs,
      REQUIRE_CONDITIONS,
      NodeResolutionMode::Execution,
    )?;
    Ok(Some(if r.scheme() == "file" {
      url_to_file_path_string(&r)?
    } else {
      r.to_string()
    }))
  } else {
    Ok(None)
  }
}

#[op2]
#[string]
pub fn op_require_read_file<P>(
  state: &mut OpState,
  #[string] file_path: String,
) -> Result<String, AnyError>
where
  P: NodePermissions + 'static,
{
  let file_path = PathBuf::from(file_path);
  let file_path = ensure_read_permission::<P>(state, &file_path)?;
  let fs = state.borrow::<FileSystemRc>();
  Ok(fs.read_text_file_lossy_sync(&file_path, None)?)
}

#[op2]
#[string]
pub fn op_require_as_file_path(#[string] file_or_url: String) -> String {
  if let Ok(url) = Url::parse(&file_or_url) {
    if let Ok(p) = url.to_file_path() {
      return p.to_string_lossy().into_owned();
    }
  }

  file_or_url
}

#[op2]
#[string]
pub fn op_require_resolve_exports<P>(
  state: &mut OpState,
  uses_local_node_modules_dir: bool,
  #[string] modules_path_str: String,
  #[string] _request: String,
  #[string] name: String,
  #[string] expansion: String,
  #[string] parent_path: String,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  let fs = state.borrow::<FileSystemRc>();
  let npm_resolver = state.borrow::<NpmResolverRc>();
  let node_resolver = state.borrow::<NodeResolverRc>();

  let modules_path = PathBuf::from(&modules_path_str);
  let pkg_path = if npm_resolver.in_npm_package_at_file_path(&modules_path)
    && !uses_local_node_modules_dir
  {
    modules_path
  } else {
    let mod_dir =
      path_resolve([modules_path_str.as_str(), name.as_str()].into_iter());
    if fs.is_dir_sync(&mod_dir) {
      mod_dir
    } else {
      modules_path
    }
  };
  let Some(pkg) =
    node_resolver.load_package_json(&pkg_path.join("package.json"))?
  else {
    return Ok(None);
  };
  let Some(exports) = &pkg.exports else {
    return Ok(None);
  };

  let referrer = Url::from_file_path(parent_path).unwrap();
  let r = node_resolver.package_exports_resolve(
    &pkg.path,
    &format!(".{expansion}"),
    exports,
    Some(&referrer),
    NodeModuleKind::Cjs,
    REQUIRE_CONDITIONS,
    NodeResolutionMode::Execution,
  )?;
  Ok(Some(if r.scheme() == "file" {
    url_to_file_path_string(&r)?
  } else {
    r.to_string()
  }))
}

#[op2]
#[serde]
pub fn op_require_read_closest_package_json<P>(
  state: &mut OpState,
  #[string] filename: String,
) -> Result<Option<PackageJsonRc>, AnyError>
where
  P: NodePermissions + 'static,
{
  let filename = PathBuf::from(filename);
  // permissions: allow reading the closest package.json files
  let node_resolver = state.borrow::<NodeResolverRc>().clone();
  node_resolver
    .get_closest_package_json_from_path(&filename)
    .map_err(AnyError::from)
}

#[op2]
#[serde]
pub fn op_require_read_package_scope<P>(
  state: &mut OpState,
  #[string] package_json_path: String,
) -> Option<PackageJsonRc>
where
  P: NodePermissions + 'static,
{
  let node_resolver = state.borrow::<NodeResolverRc>().clone();
  let package_json_path = PathBuf::from(package_json_path);
  if package_json_path.file_name() != Some("package.json".as_ref()) {
    // permissions: do not allow reading a non-package.json file
    return None;
  }
  node_resolver
    .load_package_json(&package_json_path)
    .ok()
    .flatten()
}

#[op2]
#[string]
pub fn op_require_package_imports_resolve<P>(
  state: &mut OpState,
  #[string] referrer_filename: String,
  #[string] request: String,
) -> Result<Option<String>, AnyError>
where
  P: NodePermissions + 'static,
{
  let referrer_path = PathBuf::from(&referrer_filename);
  let referrer_path = ensure_read_permission::<P>(state, &referrer_path)?;
  let node_resolver = state.borrow::<NodeResolverRc>();
  let Some(pkg) =
    node_resolver.get_closest_package_json_from_path(&referrer_path)?
  else {
    return Ok(None);
  };

  if pkg.imports.is_some() {
    let referrer_url =
      deno_core::url::Url::from_file_path(&referrer_filename).unwrap();
    let url = node_resolver.package_imports_resolve(
      &request,
      Some(&referrer_url),
      NodeModuleKind::Cjs,
      Some(&pkg),
      REQUIRE_CONDITIONS,
      NodeResolutionMode::Execution,
    )?;
    Ok(Some(url_to_file_path_string(&url)?))
  } else {
    Ok(None)
  }
}

#[op2(fast, reentrant)]
pub fn op_require_break_on_next_statement(state: Rc<RefCell<OpState>>) {
  let inspector_rc = {
    let state = state.borrow();
    state.borrow::<Rc<RefCell<JsRuntimeInspector>>>().clone()
  };
  let mut inspector = inspector_rc.borrow_mut();
  inspector.wait_for_session_and_break_on_next_statement()
}

fn url_to_file_path_string(url: &Url) -> Result<String, AnyError> {
  let file_path = url_to_file_path(url)?;
  Ok(file_path.to_string_lossy().into_owned())
}

fn url_to_file_path(url: &Url) -> Result<PathBuf, AnyError> {
  match url.to_file_path() {
    Ok(file_path) => Ok(file_path),
    Err(()) => {
      deno_core::anyhow::bail!("failed to convert '{}' to file path", url)
    }
  }
}

#[op2(fast)]
pub fn op_require_can_parse_as_esm(
  scope: &mut v8::HandleScope,
  #[string] source: &str,
) -> bool {
  let scope = &mut v8::TryCatch::new(scope);
  let Some(source) = v8::String::new(scope, source) else {
    return false;
  };
  let origin = v8::ScriptOrigin::new(
    scope,
    source.into(),
    0,
    0,
    false,
    0,
    None,
    true,
    false,
    true,
    None,
  );
  let mut source = v8::script_compiler::Source::new(source, Some(&origin));
  v8::script_compiler::compile_module(scope, &mut source).is_some()
}
