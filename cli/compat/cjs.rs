// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use std::path::PathBuf;

fn try_self_parent_path(parent: Option<&Module>) -> Option<PathBuf> {
    if let Some(parent) = parent {
        if let Some(parent_path) = &parent.filename {
            return Some(parent_path.clone())
        } else if parent.id == "<repl>" || parent.id == "internal/preload" {
            if let Ok(cwd) = std::env::current_dir() {
                return Some(cwd);
            }
        }
    }

    return None;
}

fn try_self(parent_path: Option<PathBuf>, request: &str) -> Result<Option<PathBuf>, AnyError> {
    if parent_path.is_none() {
        return Ok(None);
    }

    let package = read_package_scope(parent_path.unwrap());
    todo!()
}

fn read_package_scope(check_path: &Path) {

}

struct Module {
  id: String,
  // path: PathBuf,
  // exports:
  // module_parent_cache:
  filename: Option<PathBuf>,
  loaded: bool,
  // children: Vec<Module?>
  paths: Option<Vec<String>>,
}

impl Module {
  fn new(id: &str /*parent: Option<>*/) -> Self {
    Self {
      id: id.to_string(),
      filename: None,
      loaded: false,
      paths: None,
    }
  }

  fn _resolve_lookup_paths(
    request: &str,
    parent: Option<&Module>,
  ) -> Vec<String> {
    // if NativeModule.can_be_required_by_users(request) {
    //     return vec![];
    // }

    // Check for node modules paths.
    if !request.is_empty() {
      let request_bytes = request.as_bytes();
      if request_bytes[0] != b'.'
        || (request.len() > 1
          && request_bytes[1] != b'.'
          && request_bytes[1] != b'/'
          && (!cfg!(windows) || request_bytes[1] != b'\\'))
      {
        // TODO(bartlomieju): and add _init_paths() function
        // let paths = GLOBAL_MODULE_PATHS;
        let mut paths = vec![];
        if let Some(parent) = parent {
          if let Some(parent_paths) = &parent.paths {
            paths.append(&mut parent_paths.clone());
          }
        }

        return paths;
      }
    }

    // In REPL, parent.filename is None
    let mut no_parent = false;
    if parent.is_none() {
      no_parent = true;
    } else if let Some(parent) = parent {
      if parent.id.is_empty() || parent.filename.is_none() {
        no_parent = true;
      }
    }
    if no_parent {
      // Make require('./path/to/foo') work - normally the path is taken
      // from realpath(__filename) but in REPL the is no filename
      let main_paths = vec![".".to_string()];
      return main_paths;
    }

    let parent = parent.unwrap();
    let filename = parent.filename.as_ref().unwrap();
    let mut parent_dir = vec![];

    if let Some(parent) = filename.parent() {
      parent_dir.push(parent.to_string_lossy().to_string());
    }
    return parent_dir;
  }

  fn _resolve_filename(
    request: &str,
    parent: Option<&Module>,
    is_main: bool,
    options: Option<()>,
  ) -> String {
    if request.starts_with("node:")
    /*|| NativeModule.can_be_required_by_users(request)*/
    {
      return request.to_string();
    }

    let mut paths = vec![];

    if let Some(opts) = options {
      todo!()
    } else {
      paths = Module::_resolve_lookup_paths(request, parent);
    }

    if let Some(parent) = parent {
        if let Some(parent_filename) = &parent.filename {
            if request.as_bytes()[0] == b'#' {
                todo!()
                // let pkg = read_package_scope(parent_filename).unwrap_or_default();
                // if pkg.data.imports.is_some() {

                // }
            }
        }
    }

    // Try module self resolution first
    let parent_path = try_self_parent_path(parent);
    let self_resolved = try_self(parent_path, request);
    // if let Some(self_resolved) = self_resolved {
    //     let cache_key = format!("{}\x00{}", request, if paths.len() == 1 {
    //         paths[0]
    //     } else {
    //         paths.join("\x00")
    //     });
    //     Module::_path_cache[cache_key] = self_resolved;
    //     return self_resolved;
    // }

    // // Look up the filename first, since that's the cache key.
    // let filename = Module::_find_path(request, paths, is_main, false);
    // if let Some(filename) = filename {
    //     return filename;
    // }

    // let mut require_stack = vec![];

    // loop {
    //     let cursor = parent;
    //     //
    // }

    todo!();
  }
}
