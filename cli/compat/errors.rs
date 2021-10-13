// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
    "[ERR_MODULE_NOT_FOUND] Cannot find {} '{}' imported from {}",
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
