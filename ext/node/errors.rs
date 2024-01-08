// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::url::Url;

use crate::NodeResolutionMode;

pub fn err_invalid_module_specifier(
  request: &str,
  reason: &str,
  maybe_base: Option<String>,
) -> AnyError {
  let mut msg = format!(
    "[ERR_INVALID_MODULE_SPECIFIER] Invalid module \"{request}\" {reason}"
  );

  if let Some(base) = maybe_base {
    msg = format!("{msg} imported from {base}");
  }

  type_error(msg)
}

#[allow(unused)]
pub fn err_invalid_package_config(
  path: &str,
  maybe_base: Option<String>,
  maybe_message: Option<String>,
) -> AnyError {
  let mut msg =
    format!("[ERR_INVALID_PACKAGE_CONFIG] Invalid package config {path}");

  if let Some(base) = maybe_base {
    msg = format!("{msg} while importing {base}");
  }

  if let Some(message) = maybe_message {
    msg = format!("{msg}. {message}");
  }

  generic_error(msg)
}

pub fn err_module_not_found(path: &str, base: &str, typ: &str) -> AnyError {
  generic_error(format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find {typ} \"{path}\" imported from \"{base}\""
  ))
}

pub fn err_invalid_package_target(
  pkg_path: &str,
  key: &str,
  target: &str,
  is_import: bool,
  maybe_referrer: Option<String>,
) -> AnyError {
  let rel_error = !is_import && !target.is_empty() && !target.starts_with("./");
  let mut msg = "[ERR_INVALID_PACKAGE_TARGET]".to_string();
  let pkg_json_path = PathBuf::from(pkg_path).join("package.json");

  if key == "." {
    assert!(!is_import);
    msg = format!(
      "{} Invalid \"exports\" main target {} defined in the package config {}",
      msg,
      target,
      pkg_json_path.display()
    )
  } else {
    let ie = if is_import { "imports" } else { "exports" };
    msg = format!(
      "{} Invalid \"{}\" target {} defined for '{}' in the package config {}",
      msg,
      ie,
      target,
      key,
      pkg_json_path.display()
    )
  };

  if let Some(base) = maybe_referrer {
    msg = format!("{msg} imported from {base}");
  };
  if rel_error {
    msg = format!("{msg}; target must start with \"./\"");
  }

  generic_error(msg)
}

pub fn err_package_path_not_exported(
  mut pkg_path: String,
  subpath: &str,
  maybe_referrer: Option<String>,
  mode: NodeResolutionMode,
) -> AnyError {
  let mut msg = "[ERR_PACKAGE_PATH_NOT_EXPORTED]".to_string();

  #[cfg(windows)]
  {
    if !pkg_path.ends_with('\\') {
      pkg_path.push('\\');
    }
  }
  #[cfg(not(windows))]
  {
    if !pkg_path.ends_with('/') {
      pkg_path.push('/');
    }
  }

  let types_msg = match mode {
    NodeResolutionMode::Execution => String::new(),
    NodeResolutionMode::Types => " for types".to_string(),
  };
  if subpath == "." {
    msg =
      format!("{msg} No \"exports\" main defined{types_msg} in '{pkg_path}package.json'");
  } else {
    msg = format!("{msg} Package subpath '{subpath}' is not defined{types_msg} by \"exports\" in '{pkg_path}package.json'");
  };

  if let Some(referrer) = maybe_referrer {
    msg = format!("{msg} imported from '{referrer}'");
  }

  generic_error(msg)
}

pub fn err_package_import_not_defined(
  specifier: &str,
  package_path: Option<String>,
  base: &str,
) -> AnyError {
  let mut msg = format!(
    "[ERR_PACKAGE_IMPORT_NOT_DEFINED] Package import specifier \"{specifier}\" is not defined"
  );

  if let Some(package_path) = package_path {
    let pkg_json_path = PathBuf::from(package_path).join("package.json");
    msg = format!("{} in package {}", msg, pkg_json_path.display());
  }

  msg = format!("{msg} imported from {base}");

  type_error(msg)
}

pub fn err_unsupported_dir_import(path: &str, base: &str) -> AnyError {
  generic_error(format!("[ERR_UNSUPPORTED_DIR_IMPORT] Directory import '{path}' is not supported resolving ES modules imported from {base}"))
}

pub fn err_unsupported_esm_url_scheme(url: &Url) -> AnyError {
  let mut msg =
    "[ERR_UNSUPPORTED_ESM_URL_SCHEME] Only file and data URLS are supported by the default ESM loader"
      .to_string();

  if cfg!(window) && url.scheme().len() == 2 {
    msg =
      format!("{msg}. On Windows, absolute path must be valid file:// URLs");
  }

  msg = format!("{}. Received protocol '{}'", msg, url.scheme());
  generic_error(msg)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn types_resolution_package_path_not_exported() {
    let separator_char = if cfg!(windows) { '\\' } else { '/' };
    assert_eq!(
      err_package_path_not_exported(
        "test_path".to_string(),
        "./jsx-runtime",
        None,
        NodeResolutionMode::Types,
      )
      .to_string(),
      format!("[ERR_PACKAGE_PATH_NOT_EXPORTED] Package subpath './jsx-runtime' is not defined for types by \"exports\" in 'test_path{separator_char}package.json'")
    );
    assert_eq!(
      err_package_path_not_exported(
        "test_path".to_string(),
        ".",
        None,
        NodeResolutionMode::Types,
      )
      .to_string(),
      format!("[ERR_PACKAGE_PATH_NOT_EXPORTED] No \"exports\" main defined for types in 'test_path{separator_char}package.json'")
    );
  }
}
