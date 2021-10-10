// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::module_graph::TypeLib;
use crate::proc_state::ProcState;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::ModuleLoadId;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;
use std::str;

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
fn node_resolve(
  &self,
  specifier: &str,
  referrer: &str,
  is_main: bool,
) -> Result<ModuleSpecifier, AnyError> {
  // TODO(bartlomieju): shipped "policy" part

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data:" {
      return Ok(url);
    }

    let protocol = url.protocol();

    if protocol == "node:" {
      return Ok(url);
    }

    if protocol != "file:" && protocol != "data:" {
      return Err(generic_error(format!("Only file and data URLs are supported by the default ESM loader. Received protocol '{}'", protocol)));
    }

    // In Deno there's no way to expose internal Node modules anyway,
    // so calls to NativeModule.canBeRequiredByUsers would only work for built-in modules.

    if referrer.starts_with("data:") {
      return Url::parse(specifier, referrer).map_err(AnyError::from);
    }

    let referrer = if is_main {
      // path_to_file_url()
      referrer
    } else {
      referrer
    };

    let url = module_resolve(specifier, referrer)?;

    // TODO: check codes

    Ok(url)
  }

  Ok(module_specifier)
}

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier == "" {
    return false;
  }

  if specifier[0] == "/" {
    return true;
  }

  is_relative_specifier(specifier)
}

fn is_relative_specifier(specifier: &str) -> bool {
  if specifier[0] == "." {
    if specifier.len() == 1 || specifier[1] == "/" {
      return true;
    }
    if specifier[1] == "." {
      if specifier.len() == 2 || specifier[2] == "/" {
        return true;
      }
    }
  }
  false
}

fn module_resolve(specifier: &str, base: &str) {
  let resolved = if should_be_treated_as_relative_or_absolute_path(specifier) {
    Url::parse(specifier, base)
    // TODO(bartlomieju): check len, can panic
  } else if specifier[0] == "#" {
    package_imports_resolve(specifier, base)
  } else {
    if let Ok(resolved) = Url::parse(specifier) {
      resolved
    } else {
      package_resolve(specifier, base)
    }
  };
  finalize_resolution(resolved, base)
}

fn finalize_resolution(resolved: &str, base: &str) {
  todo!()
}

fn package_imports_resolve(specifier: &str, base: &str) {
  todo!()
}

fn package_resolve(specifier: &str, base: &str) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, is_scoped) = parse_package_name(specifier, base);

  todo!()
}

fn parse_package_name(specifier: &str, base: &str) -> (&str, &str, &str) {
    todo!()
  }
  
