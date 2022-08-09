// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::normalize_path;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use std::path::PathBuf;

pub struct Unstable(pub bool);

pub fn init(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/node",
      "01_require.js",
    ))
    .ops(vec![
      op_require_init_paths::decl(),
      op_require_node_module_paths::decl(),
      op_require_proxy_path::decl(),
      op_require_is_request_relative::decl(),
      op_require_resolve_lookup_paths::decl(),
      op_require_try_self_parent_path::decl(),
      op_require_try_self::decl(),
      op_require_real_path::decl(),
      op_require_path_is_absolute::decl(),
      op_require_path_dirname::decl(),
      op_require_stat::decl(),
      op_require_path_resolve::decl(),
      op_require_path_basename::decl(),
      op_require_read_file::decl(),
    ])
    .state(move |state| {
      state.put(Unstable(unstable));
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

#[op]
pub fn op_require_init_paths(state: &mut OpState) -> Vec<String> {
  check_unstable(state);

  let (home_dir, node_path) = if cfg!(windows) {
    (
      std::env::var("USERPROFILE").unwrap_or_else(|_| "".into()),
      std::env::var("NODE_PATH").unwrap_or_else(|_| "".into()),
    )
  } else {
    (
      std::env::var("HOME").unwrap_or_else(|_| "".into()),
      std::env::var("NODE_PATH").unwrap_or_else(|_| "".into()),
    )
  };

  let mut prefix_dir = std::env::current_exe().unwrap();
  if cfg!(windows) {
    prefix_dir = prefix_dir.join("..").join("..")
  } else {
    prefix_dir = prefix_dir.join("..")
  }

  let mut paths = vec![prefix_dir.join("lib").join("node")];

  if !home_dir.is_empty() {
    paths.insert(0, PathBuf::from(&home_dir).join(".node_libraries"));
    paths.insert(0, PathBuf::from(&home_dir).join(".nod_modules"));
  }

  let mut paths = paths
    .into_iter()
    .map(|p| p.to_string_lossy().to_string())
    .collect();

  if !node_path.is_empty() {
    let delimiter = if cfg!(windows) { ";" } else { ":" };
    let mut node_paths: Vec<String> = node_path
      .split(delimiter)
      .filter(|e| !e.is_empty())
      .map(|s| s.to_string())
      .collect();
    node_paths.append(&mut paths);
    paths = node_paths;
  }

  paths
}

#[op]
pub fn op_require_node_module_paths(
  state: &mut OpState,
  from: String,
) -> Vec<String> {
  check_unstable(state);
  // Guarantee that "from" is absolute.
  let from = deno_core::resolve_path(&from)
    .unwrap()
    .to_file_path()
    .unwrap();

  if cfg!(windows) {
    // return root node_modules when path is 'D:\\'.
    let from_str = from.to_str().unwrap();
    if from_str.len() >= 3 {
      let bytes = from_str.as_bytes();
      if bytes[from_str.len() - 1] == b'\\' && bytes[from_str.len() - 2] == b':'
      {
        let p = from_str.to_owned() + "node_modules";
        return vec![p];
      }
    }
  } else {
    // Return early not only to avoid unnecessary work, but to *avoid* returning
    // an array of two items for a root: [ '//node_modules', '/node_modules' ]
    if from.to_string_lossy() == "/" {
      return vec!["/node_modules".to_string()];
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

  paths
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
  if request.starts_with("./") {
    return true;
  }

  if request.starts_with("../") {
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
fn op_require_stat(state: &mut OpState, filename: String) -> i32 {
  check_unstable(state);
  if let Ok(metadata) = std::fs::metadata(&filename) {
    if metadata.is_file() {
      return 0;
    } else {
      return 1;
    }
  }

  -1
}

#[op]
fn op_require_real_path(
  state: &mut OpState,
  request: String,
) -> Result<String, AnyError> {
  check_unstable(state);
  let mut canonicalized_path = PathBuf::from(request).canonicalize()?;
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

#[op]
fn op_require_path_resolve(state: &mut OpState, parts: Vec<String>) -> String {
  check_unstable(state);
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
fn op_require_try_self_parent_path(
  state: &mut OpState,
  has_parent: bool,
  maybe_parent_filename: Option<String>,
  maybe_parent_id: Option<String>,
) -> Option<String> {
  check_unstable(state);
  if !has_parent {
    return None;
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Some(parent_filename);
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      if let Ok(cwd) = std::env::current_dir() {
        return Some(cwd.to_string_lossy().to_string());
      }
    }
  }
  None
}

#[op]
fn op_require_try_self(
  state: &mut OpState,
  has_parent: bool,
  maybe_parent_filename: Option<String>,
  maybe_parent_id: Option<String>,
) -> Option<String> {
  check_unstable(state);
  if !has_parent {
    return None;
  }

  if let Some(parent_filename) = maybe_parent_filename {
    return Some(parent_filename);
  }

  if let Some(parent_id) = maybe_parent_id {
    if parent_id == "<repl>" || parent_id == "internal/preload" {
      if let Ok(cwd) = std::env::current_dir() {
        return Some(cwd.to_string_lossy().to_string());
      }
    }
  }
  None
}

#[op]
fn op_require_read_file(
  state: &mut OpState,
  _filename: String,
) -> Result<String, AnyError> {
  check_unstable(state);
  todo!("not implemented");
}
