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

use deno_core::ErrBox;
use deno_core::ModuleResolutionError;
use rustyline::error::ReadlineError;
use std::env::VarError;
use std::error::Error;
use std::io;

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

pub fn resolve_to_errbox(error: ModuleResolutionError) -> ErrBox {
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

pub fn from_var(error: VarError) -> ErrBox {
  use VarError::*;
  let kind = match error {
    NotPresent => "NotFound",
    NotUnicode(..) => "InvalidData",
  };

  ErrBox::new(kind, error)
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

pub fn from_io(error: io::Error) -> ErrBox {
  let kind = get_io_kind(&error.kind());
  ErrBox::new(kind, error)
}

pub fn from_io_ref(error: &io::Error) -> ErrBox {
  let kind = get_io_kind(&error.kind());
  ErrBox::new_text(kind, error.to_string())
}

pub fn from_url(error: url::ParseError) -> ErrBox {
  ErrBox::new("URIError", error)
}

fn from_url_ref(error: &url::ParseError) -> ErrBox {
  ErrBox::new_text("URIError", error.to_string())
}

pub fn from_reqwest(error: reqwest::Error) -> ErrBox {
  match error.source() {
    Some(err_ref) => None
      .or_else(|| err_ref.downcast_ref::<url::ParseError>().map(from_url_ref))
      .or_else(|| err_ref.downcast_ref::<io::Error>().map(from_io_ref))
      .or_else(|| {
        err_ref
          .downcast_ref::<serde_json::error::Error>()
          .map(from_serde_ref)
      })
      .unwrap_or_else(|| ErrBox::new("Http", error)),
    None => ErrBox::new("Http", error),
  }
}

pub fn from_readline(error: ReadlineError) -> ErrBox {
  use ReadlineError::*;
  let kind = match error {
    Io(err) => return from_io(err),
    Eof => "UnexpectedEof",
    Interrupted => "Interrupted",
    #[cfg(unix)]
    Errno(err) => return from_nix(err),
    _ => unimplemented!(),
  };
  ErrBox::new(kind, error)
}

fn get_serde_kind(category: serde_json::error::Category) -> &'static str {
  use serde_json::error::*;
  match category {
    Category::Io => "TypeError",
    Category::Syntax => "TypeError",
    Category::Data => "InvalidData",
    Category::Eof => "UnexpectedEof",
  }
}

pub fn from_serde(error: serde_json::error::Error) -> ErrBox {
  let kind = get_serde_kind(error.classify());
  ErrBox::new(kind, error)
}

pub fn from_serde_ref(error: &serde_json::error::Error) -> ErrBox {
  let kind = get_serde_kind(error.classify());
  ErrBox::new_text(kind, error.to_string())
}

#[cfg(unix)]
pub fn from_nix(error: nix::Error) -> ErrBox {
  use nix::errno::Errno::*;
  let kind = match error {
    nix::Error::Sys(EPERM) => "PermissionDenied",
    nix::Error::Sys(EINVAL) => "TypeError",
    nix::Error::Sys(ENOENT) => "NotFound",
    nix::Error::Sys(ENOTTY) => "BadResource",
    nix::Error::Sys(UnknownErrno) => unreachable!(),
    nix::Error::Sys(_) => unreachable!(),
    nix::Error::InvalidPath => "TypeError",
    nix::Error::InvalidUtf8 => "InvalidData",
    nix::Error::UnsupportedOperation => unreachable!(),
  };

  ErrBox::new(kind, error)
}

pub fn from_dlopen(error: dlopen::Error) -> ErrBox {
  use dlopen::Error::*;
  let kind = match error {
    NullCharacter(_) => "Other",
    OpeningLibraryError(e) => return from_io(e),
    SymbolGettingError(e) => return from_io(e),
    AddrNotMatchingDll(e) => return from_io(e),
    NullSymbol => "Other",
  };

  ErrBox::new(kind, error)
}
