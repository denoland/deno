// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use boxed_error::Boxed;
use deno_core::FastString;
use deno_core::JsRuntimeInspector;
use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_package_json::PackageJsonRc;
use deno_path_util::normalize_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_permissions::PermissionsContainer;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
use node_resolver::UrlOrPath;
use node_resolver::UrlOrPathRef;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use node_resolver::errors::PackageJsonLoadError;
use sys_traits::FsMetadataValue;

use crate::ExtNodeSys;
use crate::NodeRequireLoaderRc;
use crate::NodeResolverRc;
use crate::PackageJsonResolverRc;

#[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
fn ensure_read_permission<'a>(
  state: &mut OpState,
  file_path: Cow<'a, Path>,
) -> Result<Cow<'a, Path>, JsErrorBox> {
  let loader = state.borrow::<NodeRequireLoaderRc>().clone();
  let permissions = state.borrow_mut::<PermissionsContainer>();
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
  #[properties(inherit)]
  #[error(transparent)]
  PackageExportsResolve(
    #[from] node_resolver::errors::PackageExportsResolveError,
  ),
  #[class(generic)]
  #[properties(inherit)]
  #[error(transparent)]
  PackageJsonLoad(#[from] node_resolver::errors::PackageJsonLoadError),
  #[class(generic)]
  #[properties(inherit)]
  #[error(transparent)]
  PackageImportsResolve(
    #[from] node_resolver::errors::PackageImportsResolveError,
  ),
  #[class(generic)]
  #[properties(inherit)]
  #[error(transparent)]
  FilePathConversion(#[from] deno_path_util::UrlToFilePathError),
  #[class(generic)]
  #[properties(inherit)]
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
pub fn op_require_node_module_paths<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] from: &str,
) -> Result<Vec<String>, RequireError> {
  let sys = state.borrow::<TSys>();
  // Guarantee that "from" is absolute.
  let from = if from.starts_with("file:///") {
    Cow::Owned(url_to_file_path(&Url::parse(from)?)?)
  } else {
    let current_dir = &sys
      .env_current_dir()
      .map_err(|e| RequireErrorKind::UnableToGetCwd(UnableToGetCwdError(e)))?;
    normalize_path(Cow::Owned(current_dir.join(from)))
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

  let loader = state.borrow::<NodeRequireLoaderRc>();
  Ok(loader.resolve_require_node_module_paths(&from))
}

#[op2]
#[string]
pub fn op_require_proxy_path(#[string] filename: &str) -> Option<String> {
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
    let p = Path::new(filename);
    Some(p.join("noop.js").to_string_lossy().into_owned())
  } else {
    None // filename as-is
  }
}

#[op2(fast)]
pub fn op_require_is_request_relative(#[string] request: &str) -> bool {
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
  #[string] request: &str,
  #[string] parent_filename: &str,
) -> Result<Option<String>, deno_path_util::PathToUrlError> {
  let resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();

  let path = Path::new(parent_filename);
  Ok(
    resolver
      .resolve_package_folder_from_package(
        request,
        &UrlOrPathRef::from_path(path),
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
  #[string] path: &str,
) -> bool {
  let resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();
  match deno_path_util::url_from_file_path(Path::new(path)) {
    Ok(specifier) => resolver.in_npm_package(&specifier),
    Err(_) => false,
  }
}

#[op2]
#[serde]
pub fn op_require_resolve_lookup_paths(
  #[string] request: &str,
  #[serde] maybe_parent_paths: Option<Vec<String>>,
  #[string] parent_filename: &str,
) -> Option<Vec<String>> {
  if !request.starts_with('.')
    || (request.len() > 1
      && !request.starts_with("..")
      && !request.starts_with("./")
      && (!cfg!(windows) || !request.starts_with(".\\")))
  {
    let module_paths = vec![];
    let mut paths = module_paths;
    if let Some(mut parent_paths) = maybe_parent_paths
      && !parent_paths.is_empty()
    {
      paths.append(&mut parent_paths);
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

  let p = Path::new(parent_filename);
  Some(vec![p.parent().unwrap().to_string_lossy().into_owned()])
}

#[op2(fast)]
pub fn op_require_path_is_absolute(#[string] p: &str) -> bool {
  Path::new(p).is_absolute()
}

#[op2(fast, stack_trace)]
pub fn op_require_stat<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<i32, JsErrorBox> {
  let path = Cow::Borrowed(Path::new(path));
  let path = if path.ends_with("node_modules") {
    // skip stat permission checks for node_modules directories
    // because they're noisy and it's fine
    path
  } else {
    ensure_read_permission(state, path)?
  };
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
pub fn op_require_real_path<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] request: &str,
) -> Result<String, RequireError> {
  let path = Cow::Borrowed(Path::new(request));
  let path = ensure_read_permission(state, path)
    .map_err(RequireErrorKind::Permission)?;
  let sys = state.borrow::<TSys>();
  let canonicalized_path =
    deno_path_util::strip_unc_prefix(match sys.fs_canonicalize(&path) {
      Ok(path) => path,
      Err(err) => {
        if path.ends_with("$deno$eval.cjs")
          || path.ends_with("$deno$eval.cts")
          || path.ends_with("$deno$stdin.cjs")
          || path.ends_with("$deno$stdin.cts")
        {
          path.to_path_buf()
        } else {
          return Err(RequireErrorKind::Io(err).into_box());
        }
      }
    });
  Ok(canonicalized_path.to_string_lossy().into_owned())
}

fn path_resolve<'a>(mut parts: impl Iterator<Item = &'a str>) -> PathBuf {
  let mut p = PathBuf::from(parts.next().unwrap());
  for part in parts {
    p = p.join(part);
  }
  normalize_path(Cow::Owned(p)).into_owned()
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
  #[string] request: &str,
) -> Result<String, JsErrorBox> {
  let p = Path::new(request);
  if let Some(parent) = p.parent() {
    Ok(parent.to_string_lossy().into_owned())
  } else {
    Err(JsErrorBox::generic("Path doesn't have a parent"))
  }
}

#[op2]
#[string]
pub fn op_require_path_basename(
  #[string] request: &str,
) -> Result<String, JsErrorBox> {
  let p = Path::new(request);
  if let Some(path) = p.file_name() {
    Ok(path.to_string_lossy().into_owned())
  } else {
    Err(JsErrorBox::generic("Path doesn't have a file name"))
  }
}

#[op2(stack_trace)]
#[string]
pub fn op_require_try_self<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] parent_path: &str,
  #[string] request: &str,
) -> Result<Option<String>, RequireError> {
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();
  let pkg = pkg_json_resolver
    .get_closest_package_json(Path::new(parent_path))
    .ok()
    .flatten();
  let Some(pkg) = pkg else {
    return Ok(None);
  };

  if pkg.exports.is_none() {
    return Ok(None);
  }
  let Some(pkg_name) = &pkg.name else {
    return Ok(None);
  };

  let expansion = if request == pkg_name {
    Cow::Borrowed(".")
  } else if let Some(slash_with_export) = request
    .strip_prefix(pkg_name)
    .filter(|t| t.starts_with('/'))
  {
    Cow::Owned(format!(".{}", slash_with_export))
  } else {
    return Ok(None);
  };

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
      node_resolver.require_conditions(),
      NodeResolutionKind::Execution,
    )?;
    Ok(Some(url_or_path_to_string(r)?))
  } else {
    Ok(None)
  }
}

#[op2(stack_trace)]
#[to_v8]
pub fn op_require_read_file(
  state: &mut OpState,
  #[string] file_path: &str,
) -> Result<FastString, RequireError> {
  let file_path = Cow::Borrowed(Path::new(file_path));
  // todo(dsherret): there's multiple borrows to NodeRequireLoaderRc here
  let file_path = ensure_read_permission(state, file_path)
    .map_err(RequireErrorKind::Permission)?;
  let loader = state.borrow::<NodeRequireLoaderRc>();
  loader
    .load_text_file_lossy(&file_path)
    .map_err(|e| RequireErrorKind::ReadModule(e).into_box())
}

#[op2]
#[string]
pub fn op_require_as_file_path(#[string] file_or_url: &str) -> Option<String> {
  if let Ok(url) = Url::parse(file_or_url)
    && let Ok(p) = url.to_file_path()
  {
    return Some(p.to_string_lossy().into_owned());
  }

  None // use original input
}

#[op2(stack_trace)]
#[string]
pub fn op_require_resolve_exports<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  uses_local_node_modules_dir: bool,
  #[string] modules_path_str: &str,
  #[string] _request: &str,
  #[string] name: &str,
  #[string] expansion: &str,
  #[string] parent_path: &str,
) -> Result<Option<String>, RequireError> {
  let sys = state.borrow::<TSys>();
  let node_resolver = state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>();
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();

  let modules_path = Path::new(&modules_path_str);
  let modules_specifier = deno_path_util::url_from_file_path(modules_path)?;
  let pkg_path = if node_resolver.in_npm_package(&modules_specifier)
    && !uses_local_node_modules_dir
  {
    Cow::Borrowed(modules_path)
  } else {
    let mod_dir = path_resolve([modules_path_str, name].into_iter());
    if sys.fs_is_dir_no_err(&mod_dir) {
      Cow::Owned(mod_dir)
    } else {
      Cow::Borrowed(modules_path)
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
    node_resolver.require_conditions(),
    NodeResolutionKind::Execution,
  )?;
  Ok(Some(url_or_path_to_string(r)?))
}

deno_error::js_error_wrapper!(
  PackageJsonLoadError,
  JsPackageJsonLoadError,
  "Error"
);

#[op2(fast)]
pub fn op_require_is_maybe_cjs(
  state: &mut OpState,
  #[string] filename: &str,
) -> Result<bool, JsPackageJsonLoadError> {
  let filename = Path::new(filename);
  let Ok(url) = url_from_file_path(filename) else {
    return Ok(false);
  };
  let loader = state.borrow::<NodeRequireLoaderRc>();
  loader.is_maybe_cjs(&url).map_err(Into::into)
}

#[op2(stack_trace)]
#[serde]
pub fn op_require_read_package_scope<TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  #[string] package_json_path: &str,
) -> Option<PackageJsonRc> {
  let pkg_json_resolver = state.borrow::<PackageJsonResolverRc<TSys>>();
  let package_json_path = Path::new(package_json_path);
  if package_json_path.file_name() != Some("package.json".as_ref()) {
    // permissions: do not allow reading a non-package.json file
    return None;
  }
  pkg_json_resolver
    .load_package_json(package_json_path)
    .ok()
    .flatten()
}

#[op2(stack_trace)]
#[string]
pub fn op_require_package_imports_resolve<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] referrer_filename: &str,
  #[string] request: &str,
) -> Result<Option<String>, RequireError> {
  let referrer_path = Cow::Borrowed(Path::new(referrer_filename));
  let referrer_path = ensure_read_permission(state, referrer_path)
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
    let url = node_resolver.resolve_package_import(
      request,
      Some(&UrlOrPathRef::from_path(&referrer_path)),
      Some(&pkg),
      ResolutionMode::Require,
      NodeResolutionKind::Execution,
    )?;
    Ok(Some(url_or_path_to_string(url)?))
  } else {
    Ok(None)
  }
}

#[op2(fast, reentrant)]
pub fn op_require_break_on_next_statement(state: Rc<RefCell<OpState>>) {
  let inspector = { state.borrow().borrow::<Rc<JsRuntimeInspector>>().clone() };
  inspector.wait_for_session_and_break_on_next_statement()
}

#[op2(fast)]
pub fn op_require_can_parse_as_esm(
  scope: &mut v8::PinScope<'_, '_>,
  #[string] source: &str,
) -> bool {
  v8::tc_scope!(scope, scope);
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
    Ok(url.into_path()?.to_string_lossy().into_owned())
  } else {
    Ok(url.to_string_lossy().into_owned())
  }
}
