// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - ErrBox: a generic boxed object. This is the super type of all
//!   errors handled in Rust.
//! - JSError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JSError, in that they have line numbers.
//!   But Diagnostics are compile-time type errors, whereas JSErrors are runtime
//!   exceptions.

use crate::import_map::ImportMapError;
use crate::swc_util::SwcDiagnosticBuffer;
use deno_core::ErrBox;
use deno_core::ModuleResolutionError;
use rustyline::error::ReadlineError;
use std::env::VarError;
use std::error::Error;
use std::io;

fn from_var(error: &VarError) -> &'static str {
  use VarError::*;
  match error {
    NotPresent => "NotFound",
    NotUnicode(..) => "InvalidData",
  }
}

fn get_io_kind(error_kind: &io::ErrorKind) -> &'static str {
  use io::ErrorKind::*;
  match error_kind {
    NotFound => "NotFound",
    PermissionDenied => "PermissionDenied",
    ConnectionRefused => "ConnectionRefused",
    ConnectionReset => "ConnectionReset",
    ConnectionAborted => "ConnectionAborted",
    NotConnected => "NotConnected",
    AddrInUse => "AddrInUse",
    AddrNotAvailable => "AddrNotAvailable",
    BrokenPipe => "BrokenPipe",
    AlreadyExists => "AlreadyExists",
    InvalidInput => "TypeError",
    InvalidData => "InvalidData",
    TimedOut => "TimedOut",
    Interrupted => "Interrupted",
    WriteZero => "WriteZero",
    UnexpectedEof => "UnexpectedEof",
    Other => "Other",
    WouldBlock => unreachable!(),
    // Non-exhaustive enum - might add new variants
    // in the future
    _ => unreachable!(),
  }
}

fn get_io_error_class(error: &io::Error) -> &'static str {
  get_io_kind(&error.kind())
}

fn get_url_parse_error_class(error: &url::ParseError) -> &'static str {
  "URIError"
}

fn get_request_error_class(error: &reqwest::Error) -> &'static str {
  match error.source() {
    Some(err_ref) => None
      .or_else(|| {
        err_ref
          .downcast_ref::<url::ParseError>()
          .map(get_url_parse_error_class)
      })
      .or_else(|| err_ref.downcast_ref::<io::Error>().map(get_io_error_class))
      .or_else(|| {
        err_ref
          .downcast_ref::<serde_json::error::Error>()
          .map(get_serde_json_error_class)
      })
      .unwrap_or("Http"),
    None => "Http",
  }
}

fn get_import_map_error_class(_: &ImportMapError) -> &'static str {
  "URIError"
}

fn get_mpdule_resolution_error_class(
  _: &ModuleResolutionError,
) -> &'static str {
  "URIError"
}

fn from_readline(error: &ReadlineError) -> &'static str {
  use ReadlineError::*;
  match error {
    Io(err) => get_io_error_class(err),
    Eof => "UnexpectedEof",
    Interrupted => "Interrupted",
    #[cfg(unix)]
    Errno(err) => from_nix(err),
    _ => unimplemented!(),
  }
}

fn get_serde_json_error_class(
  error: &serde_json::error::Error,
) -> &'static str {
  use serde_json::error::*;
  match error.classify() {
    Category::Io => "TypeError", // TODO(piscisaureus): this is not correct.
    Category::Syntax => "SyntaxError",
    Category::Data => "InvalidData",
    Category::Eof => "UnexpectedEof",
  }
}

#[cfg(unix)]
fn from_nix(error: &nix::Error) -> &'static str {
  use nix::errno::Errno::*;
  match error {
    nix::Error::Sys(EPERM) => "PermissionDenied",
    nix::Error::Sys(EINVAL) => "TypeError",
    nix::Error::Sys(ENOENT) => "NotFound",
    nix::Error::Sys(ENOTTY) => "BadResource",
    nix::Error::Sys(UnknownErrno) => unreachable!(),
    nix::Error::Sys(_) => unreachable!(),
    nix::Error::InvalidPath => "TypeError",
    nix::Error::InvalidUtf8 => "InvalidData",
    nix::Error::UnsupportedOperation => unreachable!(),
  }
}

fn get_dlopen_error_class(error: &dlopen::Error) -> &'static str {
  use dlopen::Error::*;
  match error {
    NullCharacter(_) => "Other",
    OpeningLibraryError(e) => get_io_error_class(e),
    SymbolGettingError(e) => get_io_error_class(e),
    AddrNotMatchingDll(e) => get_io_error_class(e),
    NullSymbol => "Other",
  }
}

fn get_notify_error_class(error: &notify::Error) -> &'static str {
  use notify::ErrorKind::*;
  match error.kind {
    Generic(_) => "Other",
    Io(ref e) => get_io_error_class(e),
    PathNotFound => "NotFound",
    WatchNotFound => "NotFound",
    InvalidConfig(_) => "InvalidData",
  }
}

fn get_swc_diagnostic_error_class(_: &SwcDiagnosticBuffer) -> &'static str {
  "Other"
}

pub fn get_error_class(error: &ErrBox) -> &'static str {
  None
    .or_else(|| {
      error
        .downcast_ref::<reqwest::Error>()
        .map(get_request_error_class)
    })
    .or_else(|| {
      error
        .downcast_ref::<ImportMapError>()
        .map(get_import_map_error_class)
    })
    .or_else(|| {
      error
        .downcast_ref::<ModuleResolutionError>()
        .map(get_mpdule_resolution_error_class)
    })
    .or_else(|| error.downcast_ref::<io::Error>().map(get_io_error_class))
    .or_else(|| {
      error
        .downcast_ref::<url::ParseError>()
        .map(get_url_parse_error_class)
    })
    .or_else(|| error.downcast_ref::<VarError>().map(from_var))
    .or_else(|| error.downcast_ref::<ReadlineError>().map(from_readline))
    .or_else(|| {
      error
        .downcast_ref::<serde_json::error::Error>()
        .map(get_serde_json_error_class)
    })
    .or_else(|| {
      error
        .downcast_ref::<dlopen::Error>()
        .map(get_dlopen_error_class)
    })
    .or_else(|| {
      error
        .downcast_ref::<notify::Error>()
        .map(get_notify_error_class)
    })
    .or_else(|| {
      error
        .downcast_ref::<SwcDiagnosticBuffer>()
        .map(get_swc_diagnostic_error_class)
    })
    .or_else(|| {
      #[cfg(unix)]
      fn get_os_error_class(error: &ErrBox) -> Option<&'static str> {
        error.downcast_ref::<nix::Error>().map(from_nix)
      }
      #[cfg(not(unix))]
      fn get_os_error_class(_: &ErrBox) -> Option<&'static str> {
        None
      }
      get_os_error_class(error)
    })
    .unwrap_or_else(|| {
      panic!("Can't downcast {:?} to ErrBox", error);
    })
}

pub fn rust_err_to_json(error: &ErrBox) -> Box<[u8]> {
  let error_value =
    json!({ "kind": get_error_class(error), "message": error.to_string()});
  serde_json::to_vec(&error_value).unwrap().into_boxed_slice()
}

pub fn not_implemented() -> ErrBox {
  ErrBox::other("not implemented".to_string())
}

pub fn invalid_utf8() -> ErrBox {
  ErrBox::new_text("InvalidData", "invalid utf8".to_string())
}

pub fn invalid_domain_error() -> ErrBox {
  ErrBox::type_error("Invalid domain.".to_string())
}

pub fn uri_error(msg: String) -> ErrBox {
  ErrBox::new_text("URIError", msg)
}

pub fn from_resolution(error: &ModuleResolutionError) -> ErrBox {
  uri_error(error.to_string())
}

pub fn permission_escalation_error() -> ErrBox {
  permission_denied("Arguments escalate parent permissions.".to_string())
}

pub fn resource_unavailable() -> ErrBox {
  ErrBox::new_text(
    "Busy",
    "resource is unavailable because it is in use by a promise".to_string(),
  )
}

pub fn permission_denied(msg: String) -> ErrBox {
  ErrBox::new_text("PermissionDenied", msg)
}
