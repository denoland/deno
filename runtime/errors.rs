// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - AnyError: a generic wrapper that can encapsulate any type of error.
//! - JsError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JsError, in that they have line numbers. But
//!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
//!   exceptions.

use deno_core::error::AnyError;
use std::sync::Arc;

fn get_regex_error_class(error: &regex::Error) -> &'static str {
  use regex::Error::*;
  match error {
    Syntax(_) => "SyntaxError",
    CompiledTooBig(_) => "RangeError",
    _ => "Error",
  }
}

fn get_hyper_error_class(_error: &hyper::Error) -> &'static str {
  "Http"
}

fn get_hyper_util_error_class(
  _error: &hyper_util::client::legacy::Error,
) -> &'static str {
  "Http"
}

fn get_hyper_v014_error_class(_error: &hyper_v014::Error) -> &'static str {
  "Http"
}

#[cfg(unix)]
pub fn get_nix_error_class(error: &nix::Error) -> &'static str {
  match error {
    nix::Error::ECHILD => "NotFound",
    nix::Error::EINVAL => "TypeError",
    nix::Error::ENOENT => "NotFound",
    nix::Error::ENOTTY => "BadResource",
    nix::Error::EPERM => "PermissionDenied",
    nix::Error::ESRCH => "NotFound",
    nix::Error::ELOOP => "FilesystemLoop",
    nix::Error::ENOTDIR => "NotADirectory",
    nix::Error::ENETUNREACH => "NetworkUnreachable",
    nix::Error::EISDIR => "IsADirectory",
    nix::Error::UnknownErrno => "Error",
    &nix::Error::ENOTSUP => unreachable!(),
    _ => "Error",
  }
}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<hyper::Error>().map(get_hyper_error_class)
    .or_else(|| {
      e.downcast_ref::<hyper_util::client::legacy::Error>()
        .map(get_hyper_util_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<Arc<hyper_v014::Error>>()
        .map(|e| get_hyper_v014_error_class(e))
    })
    .or_else(|| e.downcast_ref::<regex::Error>().map(get_regex_error_class))
    .or_else(|| {
      #[cfg(unix)]
      let maybe_get_nix_error_class =
        || e.downcast_ref::<nix::Error>().map(get_nix_error_class);
      #[cfg(not(unix))]
      let maybe_get_nix_error_class = || Option::<&'static str>::None;
      (maybe_get_nix_error_class)()
    })
}
