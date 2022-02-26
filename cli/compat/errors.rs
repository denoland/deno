// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::url::Url;

pub(crate) fn err_invalid_module_specifier(
  request: &str,
  reason: &str,
  maybe_base: Option<String>,
) -> AnyError {
  let mut msg = format!(
    "[ERR_INVALID_MODULE_SPECIFIER] Invalid module \"{}\" {}",
    request, reason
  );

  if let Some(base) = maybe_base {
    msg = format!("{} imported from {}", msg, base);
  }

  type_error(msg)
}

pub(crate) fn err_invalid_package_config(
  path: &str,
  maybe_base: Option<String>,
  maybe_message: Option<String>,
) -> AnyError {
  let mut msg = format!(
    "[ERR_INVALID_PACKAGE_CONFIG] Invalid package config {}",
    path
  );

  if let Some(base) = maybe_base {
    msg = format!("{} while importing {}", msg, base);
  }

  if let Some(message) = maybe_message {
    msg = format!("{}. {}", msg, message);
  }

  generic_error(msg)
}

pub(crate) fn err_module_not_found(
  path: &str,
  base: &str,
  typ: &str,
) -> AnyError {
  generic_error(format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find {} \"{}\" imported from \"{}\"",
    typ, path, base
  ))
}

pub(crate) fn err_unsupported_dir_import(path: &str, base: &str) -> AnyError {
  generic_error(format!("[ERR_UNSUPPORTED_DIR_IMPORT] Directory import '{}' is not supported resolving ES modules imported from {}", path, base))
}

pub(crate) fn err_unsupported_esm_url_scheme(url: &Url) -> AnyError {
  let mut msg =
    "[ERR_UNSUPPORTED_ESM_URL_SCHEME] Only file and data URLS are supported by the default ESM loader"
      .to_string();

  if cfg!(window) && url.scheme().len() == 2 {
    msg = format!(
      "{}. On Windows, absolute path must be valid file:// URLs",
      msg
    );
  }

  msg = format!("{}. Received protocol '{}'", msg, url.scheme());
  generic_error(msg)
}

pub(crate) fn err_invalid_package_target(
  pkg_path: String,
  key: String,
  target: String,
  is_import: bool,
  maybe_base: Option<String>,
) -> AnyError {
  let rel_error = !is_import && !target.is_empty() && !target.starts_with("./");
  let mut msg = "[ERR_INVALID_PACKAGE_TARGET]".to_string();

  if key == "." {
    assert!(!is_import);
    msg = format!("{} Invalid \"exports\" main target {} defined in the package config {}package.json", msg, target, pkg_path)
  } else {
    let ie = if is_import { "imports" } else { "exports" };
    msg = format!("{} Invalid \"{}\" target {} defined for '{}' in the package config {}package.json", msg, ie, target, key, pkg_path)
  };

  if let Some(base) = maybe_base {
    msg = format!("{} imported from {}", msg, base);
  };
  if rel_error {
    msg = format!("{}; target must start with \"./\"", msg);
  }

  generic_error(msg)
}

pub(crate) fn err_package_path_not_exported(
  pkg_path: String,
  subpath: String,
  maybe_base: Option<String>,
) -> AnyError {
  let mut msg = "[ERR_PACKAGE_PATH_NOT_EXPORTED]".to_string();

  if subpath == "." {
    msg = format!(
      "{} No \"exports\" main defined in {}package.json",
      msg, pkg_path
    );
  } else {
    msg = format!("{} Package subpath \'{}\' is not defined by \"exports\" in {}package.json", msg, subpath, pkg_path);
  };

  if let Some(base) = maybe_base {
    msg = format!("{} imported from {}", msg, base);
  }

  generic_error(msg)
}

pub(crate) fn err_package_import_not_defined(
  specifier: &str,
  package_path: Option<String>,
  base: &str,
) -> AnyError {
  let mut msg = format!(
    "[ERR_PACKAGE_IMPORT_NOT_DEFINED] Package import specifier \"{}\" is not defined in",
    specifier
  );

  if let Some(package_path) = package_path {
    msg = format!("{} in package {}package.json", msg, package_path);
  }

  msg = format!("{} imported from {}", msg, base);

  type_error(msg)
}
