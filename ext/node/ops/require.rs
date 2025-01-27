// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use boxed_error::Boxed;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::FastString;
use deno_core::JsRuntimeInspector;
use deno_core::OpState;
use deno_error::JsErrorBox;
use deno_package_json::PackageJsonRc;
use deno_path_util::normalize_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
use node_resolver::UrlOrPath;
use node_resolver::UrlOrPathRef;
use node_resolver::REQUIRE_CONDITIONS;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;

use crate::ExtNodeSys;
use crate::NodePermissions;
use crate::NodeRequireLoaderRc;
use crate::NodeResolverRc;
use crate::PackageJsonResolverRc;

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a, P>(
  state: &mut OpState,
  file_path: &'a Path,
) -> Result<Cow<'a, Path>, JsErrorBox>
where
  P: NodePermissions + 'static,
{
  let loader = state.borrow::<NodeRequireLoaderRc>().clone();
  let permissions = state.borrow_mut::<P>();
  loader.ensure_read_permission(permissions, file_path)
}

#[derive(Debug, Boxed, deno_error::JsError)]
pub struct RequireError(pub Box<RequireErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum RequireErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  UrlParse(
    #[from]
    #[inherit]
    url::ParseError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[inherit] JsErrorBox),
  #[class(generic)]
  #[error(transparent)]
  PackageExportsResolve(
    #[from] node_resolver::errors::PackageExportsResolveError,
  ),
  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoad(#[from] node_resolver::errors::PackageJsonLoadError),
  #[class(generic)]
  #[error(transparent)]
  ClosestPkgJson(#[from] ClosestPkgJsonError),
  #[class(generic)]
  #[error(transparent)]
  PackageImportsResolve(
    #[from] node_resolver::errors::PackageImportsResolveError,
  ),
  #[class(generic)]
  #[error(transparent)]
  FilePathConversion(#[from] deno_path_util::UrlToFilePathError),
  #[class(generic)]
  #[error(transparent)]
  UrlConversion(#[from] deno_path_util::PathToUrlError),
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
  #[class(inherit)]
  #[error(transparent)]
  ReadModule(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
  #[class(inherit)]
  #[error(transparent)]
  UnableToGetCwd(UnableToGetCwdError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error("Unable to get CWD")]
#[class(inherit)]
pub struct UnableToGetCwdError(#[source] pub std::io::Error);

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

#[op2(stack_trace)]
#[serde]
pub fn op_require_node_module_paths<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] from: String,
) -> Result<Vec<String>, RequireError> {
  let sys = state.borrow::<TSys>();
  // Guarantee that "from" is absolute.
  let from = if from.starts_with("file:///") {
    url_to_file_path(&Url::parse(&from)?)?
  } else {
    let current_dir = &sys
      .env_current_dir()
      .map_err(|e| RequireErrorKind::UnableToGetCwd(UnableToGetCwdError(e)))?;
    normalize_path(current_dir.join(from))
  };

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
  let mut current_path = from.as_path();
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
pub fn op_require_resolve_deno_dir<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] request: String,
  #[string] parent_filename: String,
) -> Result<Option<String>, deno_path_util::PathToUrlError> {
  let resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();

  let path = PathBuf::from(parent_filename);
  Ok(
    resolver
      .resolve_package_folder_from_package(
        &request,
        &UrlOrPathRef::from_path(&path),
      )
      .ok()
      .map(|p| p.to_string_lossy().into_owned()),
  )
}

#[op2(fast)]
pub fn op_require_is_deno_dir_package<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] path: String,
) -> bool {
  let resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();
  match deno_path_util::url_from_file_path(&PathBuf::from(path)) {
    Ok(specifier) => resolver.in_npm_package(&specifier),
    Err(_) => false,
  }
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

#[op2(fast, stack_trace)]
pub fn op_require_stat<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] path: String,
) -> Result<i32, JsErrorBox> {
  let path = PathBuf::from(path);
  let path = ensure_read_permission::<P>(state, &path)?;
  let sys = state.borrow::<TSys>();
  if let Ok(metadata) = sys.fs_metadata(&path) {
    if metadata.file_type().is_file() {
      return Ok(0);
    } else {
      return Ok(1);
    }
  }

  Ok(-1)
}

#[op2(stack_trace)]
#[string]
pub fn op_require_real_path<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] request: String,
) -> Result<String, RequireError> {
  let path = PathBuf::from(request);
  let path = ensure_read_permission::<P>(state, &path)
    .map_err(RequireErrorKind::Permission)?;
  let sys = state.borrow::<TSys>();
  let canonicalized_path = deno_path_util::strip_unc_prefix(
    sys.fs_canonicalize(&path).map_err(RequireErrorKind::Io)?,
  );
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
) -> Result<String, JsErrorBox> {
  let p = PathBuf::from(request);
  if let Some(parent) = p.parent() {
    Ok(parent.to_string_lossy().into_owned())
  } else {
    Err(JsErrorBox::generic("Path doesn't have a parent"))
  }
}

#[op2]
#[string]
pub fn op_require_path_basename(
  #[string] request: String,
) -> Result<String, JsErrorBox> {
  let p = PathBuf::from(request);
  if let Some(path) = p.file_name() {
    Ok(path.to_string_lossy().into_owned())
  } else {
    Err(JsErrorBox::generic("Path doesn't have a file name"))
  }
}

#[op2(stack_trace)]
#[string]
pub fn op_require_try_self_parent_path<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  has_parent: bool,
  #[string] maybe_parent_filename: Option<String>,
  #[string] maybe_parent_id: Option<String>,
) -> Result<Option<String>, JsErrorBox> {
  if !has_parent {
    return Ok(None);
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Ok(Some(parent_filename));
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      let sys = state.borrow::<TSys>();
      if let Ok(cwd) = sys.env_current_dir() {
        let cwd = ensure_read_permission::<P>(state, &cwd)?;
        return Ok(Some(cwd.to_string_lossy().into_owned()));
      }
    }
  }
  Ok(None)
}

#[op2(stack_trace)]
#[string]
pub fn op_require_try_self<
  P: NodePermissions + 'static,
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] parent_path: Option<String>,
  #[string] request: String,
) -> Result<Option<String>, RequireError> {
  if parent_path.is_none() {
    return Ok(None);
  }

  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();
  let pkg = pkg_json_resolver
    .get_closest_package_json(&PathBuf::from(parent_path.unwrap()))
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

  if let Some(exports) = &pkg.exports {
    let node_resolver = state.borrow::<NodeResolverRc<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TSys,
    >>();
    let referrer = UrlOrPathRef::from_path(&pkg.path);
    // invalidate the resolution cache in case things have changed
    NodeResolutionThreadLocalCache::clear();
    let r = node_resolver.package_exports_resolve(
      &pkg.path,
      &expansion,
      exports,
      Some(&referrer),
      ResolutionMode::Require,
      REQUIRE_CONDITIONS,
      NodeResolutionKind::Execution,
    )?;
    Ok(Some(url_or_path_to_string(r)?))
  } else {
    Ok(None)
  }
}

#[op2(stack_trace)]
#[to_v8]
pub fn op_require_read_file<P>(
  state: &mut OpState,
  #[string] file_path: String,
) -> Result<FastString, RequireError>
where
  P: NodePermissions + 'static,
{
  let file_path = PathBuf::from(file_path);
  // todo(dsherret): there's multiple borrows to NodeRequireLoaderRc here
  let file_path = ensure_read_permission::<P>(state, &file_path)
    .map_err(RequireErrorKind::Permission)?;
  let loader = state.borrow::<NodeRequireLoaderRc>();
  loader
    .load_text_file_lossy(&file_path)
    .map(|s| match s {
      Cow::Borrowed(s) => FastString::from_static(s),
      Cow::Owned(s) => s.into(),
    })
    .map_err(|e| RequireErrorKind::ReadModule(e).into_box())
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

#[op2(stack_trace)]
#[string]
pub fn op_require_resolve_exports<
  P: NodePermissions + 'static,
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  uses_local_node_modules_dir: bool,
  #[string] modules_path_str: String,
  #[string] _request: String,
  #[string] name: String,
  #[string] expansion: String,
  #[string] parent_path: String,
) -> Result<Option<String>, RequireError> {
  let sys = state.borrow::<TSys>();
  let node_resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();

  let modules_path = PathBuf::from(&modules_path_str);
  let modules_specifier = deno_path_util::url_from_file_path(&modules_path)?;
  let pkg_path = if node_resolver.in_npm_package(&modules_specifier)
    && !uses_local_node_modules_dir
  {
    modules_path
  } else {
    let mod_dir =
      path_resolve([modules_path_str.as_str(), name.as_str()].into_iter());
    if sys.fs_is_dir_no_err(&mod_dir) {
      mod_dir
    } else {
      modules_path
    }
  };
  let Some(pkg) =
    pkg_json_resolver.load_package_json(&pkg_path.join("package.json"))?
  else {
    return Ok(None);
  };
  let Some(exports) = &pkg.exports else {
    return Ok(None);
  };

  let referrer = if parent_path.is_empty() {
    None
  } else {
    Some(PathBuf::from(parent_path))
  };
  NodeResolutionThreadLocalCache::clear();
  let r = node_resolver.package_exports_resolve(
    &pkg.path,
    &format!(".{expansion}"),
    exports,
    referrer
      .as_ref()
      .map(|r| UrlOrPathRef::from_path(r))
      .as_ref(),
    ResolutionMode::Require,
    REQUIRE_CONDITIONS,
    NodeResolutionKind::Execution,
  )?;
  Ok(Some(url_or_path_to_string(r)?))
}

deno_error::js_error_wrapper!(
  ClosestPkgJsonError,
  JsClosestPkgJsonError,
  "Error"
);

#[op2(fast)]
pub fn op_require_is_maybe_cjs(
  state: &mut OpState,
  #[string] filename: String,
) -> Result<bool, JsClosestPkgJsonError> {
  let filename = PathBuf::from(filename);
  let Ok(url) = url_from_file_path(&filename) else {
    return Ok(false);
  };
  let loader = state.borrow::<NodeRequireLoaderRc>();
  loader.is_maybe_cjs(&url).map_err(Into::into)
}

#[op2(stack_trace)]
#[serde]
pub fn op_require_read_package_scope<
  P: NodePermissions + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] package_json_path: String,
) -> Option<PackageJsonRc> {
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();
  let package_json_path = PathBuf::from(package_json_path);
  if package_json_path.file_name() != Some("package.json".as_ref()) {
    // permissions: do not allow reading a non-package.json file
    return None;
  }
  pkg_json_resolver
    .load_package_json(&package_json_path)
    .ok()
    .flatten()
}

#[op2(stack_trace)]
#[string]
pub fn op_require_package_imports_resolve<
  P: NodePermissions + 'static,
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] referrer_filename: String,
  #[string] request: String,
) -> Result<Option<String>, RequireError> {
  let referrer_path = PathBuf::from(&referrer_filename);
  let referrer_path = ensure_read_permission::<P>(state, &referrer_path)
    .map_err(RequireErrorKind::Permission)?;
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();
  let Some(pkg) = pkg_json_resolver.get_closest_package_json(&referrer_path)?
  else {
    return Ok(None);
  };

  if pkg.imports.is_some() {
    let node_resolver = state.borrow::<NodeResolverRc<
      TInNpmPackageChecker,
      TNpmPackageFolderResolver,
      TSys,
    >>();
    NodeResolutionThreadLocalCache::clear();
    let url = node_resolver.package_imports_resolve(
      &request,
      Some(&UrlOrPathRef::from_path(&referrer_path)),
      ResolutionMode::Require,
      Some(&pkg),
      REQUIRE_CONDITIONS,
      NodeResolutionKind::Execution,
    )?;
    Ok(Some(url_or_path_to_string(url)?))
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

fn url_or_path_to_string(
  url: UrlOrPath,
) -> Result<String, deno_path_util::UrlToFilePathError> {
  if url.is_file() {
    Ok(url.into_path()?.to_string_lossy().to_string())
  } else {
    Ok(url.to_string_lossy().to_string())
  }
}
