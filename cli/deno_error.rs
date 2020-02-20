// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! This module implements error serialization; it
//! allows to serialize Rust errors to be sent to JS runtime.
//!
//! Currently it is deeply intertwined with `ErrBox` which is
//! not optimal since not every ErrBox can be "JS runtime error";
//! eg. there's no way to throw JSError/Diagnostic from within JS runtime
//!
//! There are many types of errors in Deno:
//! - ErrBox: a generic boxed object. This is the super type of all
//!   errors handled in Rust.
//! - JSError: exceptions thrown from V8 into Rust. Usually a user exception.
//!   These are basically a big JSON structure which holds information about
//!   line numbers. We use this to pretty-print stack traces. These are
//!   never passed back into the runtime.
//! - DenoError: these are errors that happen during ops, which are passed
//!   back into the runtime, where an exception object is created and thrown.
//!   DenoErrors have an integer code associated with them - access this via the kind() method.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JSError, in that they have line numbers.
//!   But Diagnostics are compile-time type errors, whereas JSErrors are runtime exceptions.
//!
//! TODO:
//! - rename DenoError to OpError?
//! - rename JSError to RuntimeException. merge V8Exception?
//! - rename ErrorKind::Other. This corresponds to a generic exception thrown as the
//!   global `Error` in JS:
//!   https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error

use crate::import_map::ImportMapError;
use deno_core::AnyError;
use deno_core::ErrBox;
use deno_core::ModuleResolutionError;
use dlopen::Error as DlopenError;
use reqwest;
use rustyline::error::ReadlineError;
use std;
use std::env::VarError;
use std::error::Error;
use std::fmt;
use std::io;
use url;

// Warning! The values in this enum are duplicated in js/errors.ts
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ErrorKind {
  NotFound = 1,
  PermissionDenied = 2,
  ConnectionRefused = 3,
  ConnectionReset = 4,
  ConnectionAborted = 5,
  NotConnected = 6,
  AddrInUse = 7,
  AddrNotAvailable = 8,
  BrokenPipe = 9,
  AlreadyExists = 10,
  InvalidData = 13,
  TimedOut = 14,
  Interrupted = 15,
  WriteZero = 16,
  UnexpectedEof = 17,
  BadResource = 18,
  Http = 19,
  URIError = 20,
  TypeError = 21,
  Other = 22,
}

#[derive(Debug)]
pub struct DenoError {
  kind: ErrorKind,
  msg: String,
}

pub fn print_msg_and_exit(msg: &str) {
  eprintln!("{}", msg);
  std::process::exit(1);
}

pub fn print_err_and_exit(err: ErrBox) {
  eprintln!("{}", err.to_string());
  std::process::exit(1);
}

pub fn js_check(r: Result<(), ErrBox>) {
  if let Err(err) = r {
    print_err_and_exit(err);
  }
}

impl DenoError {
  pub fn new(kind: ErrorKind, msg: String) -> Self {
    Self { kind, msg }
  }
}

impl Error for DenoError {}

impl fmt::Display for DenoError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(self.msg.as_str())
  }
}

#[derive(Debug)]
struct StaticError(ErrorKind, &'static str);

impl Error for StaticError {}

impl fmt::Display for StaticError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(self.1)
  }
}

pub fn bad_resource() -> ErrBox {
  StaticError(ErrorKind::BadResource, "bad resource id").into()
}

pub fn permission_denied() -> ErrBox {
  StaticError(ErrorKind::PermissionDenied, "permission denied").into()
}

pub fn permission_denied_msg(msg: String) -> ErrBox {
  DenoError::new(ErrorKind::PermissionDenied, msg).into()
}

pub fn no_buffer_specified() -> ErrBox {
  StaticError(ErrorKind::TypeError, "no buffer specified").into()
}

pub fn invalid_address_syntax() -> ErrBox {
  StaticError(ErrorKind::TypeError, "invalid address syntax").into()
}

pub fn other_error(msg: String) -> ErrBox {
  DenoError::new(ErrorKind::Other, msg).into()
}

pub trait GetErrorKind {
  fn kind(&self) -> ErrorKind;
}

impl GetErrorKind for DenoError {
  fn kind(&self) -> ErrorKind {
    self.kind
  }
}

impl GetErrorKind for StaticError {
  fn kind(&self) -> ErrorKind {
    self.0
  }
}

impl GetErrorKind for ImportMapError {
  fn kind(&self) -> ErrorKind {
    ErrorKind::Other
  }
}

impl GetErrorKind for ModuleResolutionError {
  fn kind(&self) -> ErrorKind {
    ErrorKind::URIError
  }
}

impl GetErrorKind for VarError {
  fn kind(&self) -> ErrorKind {
    use VarError::*;
    match self {
      NotPresent => ErrorKind::NotFound,
      NotUnicode(..) => ErrorKind::InvalidData,
    }
  }
}

impl GetErrorKind for io::Error {
  fn kind(&self) -> ErrorKind {
    use io::ErrorKind::*;
    match self.kind() {
      NotFound => ErrorKind::NotFound,
      PermissionDenied => ErrorKind::PermissionDenied,
      ConnectionRefused => ErrorKind::ConnectionRefused,
      ConnectionReset => ErrorKind::ConnectionReset,
      ConnectionAborted => ErrorKind::ConnectionAborted,
      NotConnected => ErrorKind::NotConnected,
      AddrInUse => ErrorKind::AddrInUse,
      AddrNotAvailable => ErrorKind::AddrNotAvailable,
      BrokenPipe => ErrorKind::BrokenPipe,
      AlreadyExists => ErrorKind::AlreadyExists,
      InvalidInput => ErrorKind::TypeError,
      InvalidData => ErrorKind::InvalidData,
      TimedOut => ErrorKind::TimedOut,
      Interrupted => ErrorKind::Interrupted,
      WriteZero => ErrorKind::WriteZero,
      UnexpectedEof => ErrorKind::UnexpectedEof,
      WouldBlock => unreachable!(),
      _ => ErrorKind::Other,
    }
  }
}

impl GetErrorKind for url::ParseError {
  fn kind(&self) -> ErrorKind {
    ErrorKind::URIError
  }
}

impl GetErrorKind for reqwest::Error {
  fn kind(&self) -> ErrorKind {
    use self::GetErrorKind as Get;

    match self.source() {
      Some(err_ref) => None
        .or_else(|| err_ref.downcast_ref::<url::ParseError>().map(Get::kind))
        .or_else(|| err_ref.downcast_ref::<io::Error>().map(Get::kind))
        .or_else(|| {
          err_ref
            .downcast_ref::<serde_json::error::Error>()
            .map(Get::kind)
        })
        .unwrap_or_else(|| ErrorKind::Http),
      None => ErrorKind::Http,
    }
  }
}

impl GetErrorKind for ReadlineError {
  fn kind(&self) -> ErrorKind {
    use ReadlineError::*;
    match self {
      Io(err) => GetErrorKind::kind(err),
      Eof => ErrorKind::UnexpectedEof,
      Interrupted => ErrorKind::Interrupted,
      #[cfg(unix)]
      Errno(err) => err.kind(),
      _ => unimplemented!(),
    }
  }
}

impl GetErrorKind for serde_json::error::Error {
  fn kind(&self) -> ErrorKind {
    use serde_json::error::*;
    match self.classify() {
      Category::Io => ErrorKind::TypeError,
      Category::Syntax => ErrorKind::TypeError,
      Category::Data => ErrorKind::InvalidData,
      Category::Eof => ErrorKind::UnexpectedEof,
    }
  }
}

#[cfg(unix)]
mod unix {
  use super::{ErrorKind, GetErrorKind};
  use nix::errno::Errno::*;
  pub use nix::Error;
  use nix::Error::Sys;

  impl GetErrorKind for Error {
    fn kind(&self) -> ErrorKind {
      match self {
        Sys(EPERM) => ErrorKind::PermissionDenied,
        Sys(EINVAL) => ErrorKind::TypeError,
        Sys(ENOENT) => ErrorKind::NotFound,
        Sys(UnknownErrno) => unreachable!(),
        Sys(_) => unreachable!(),
        Error::InvalidPath => ErrorKind::TypeError,
        Error::InvalidUtf8 => ErrorKind::InvalidData,
        Error::UnsupportedOperation => unreachable!(),
      }
    }
  }
}

impl GetErrorKind for DlopenError {
  fn kind(&self) -> ErrorKind {
    use dlopen::Error::*;
    match self {
      NullCharacter(_) => ErrorKind::Other,
      OpeningLibraryError(e) => GetErrorKind::kind(e),
      SymbolGettingError(e) => GetErrorKind::kind(e),
      NullSymbol => ErrorKind::Other,
      AddrNotMatchingDll(e) => GetErrorKind::kind(e),
    }
  }
}

impl GetErrorKind for dyn AnyError {
  fn kind(&self) -> ErrorKind {
    use self::GetErrorKind as Get;

    #[cfg(unix)]
    fn unix_error_kind(err: &dyn AnyError) -> Option<ErrorKind> {
      err.downcast_ref::<unix::Error>().map(Get::kind)
    }

    #[cfg(not(unix))]
    fn unix_error_kind(_: &dyn AnyError) -> Option<ErrorKind> {
      None
    }

    None
      .or_else(|| self.downcast_ref::<DenoError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<reqwest::Error>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ImportMapError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<io::Error>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ModuleResolutionError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<StaticError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<url::ParseError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<VarError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ReadlineError>().map(Get::kind))
      .or_else(|| {
        self
          .downcast_ref::<serde_json::error::Error>()
          .map(Get::kind)
      })
      .or_else(|| self.downcast_ref::<DlopenError>().map(Get::kind))
      .or_else(|| unix_error_kind(self))
      .unwrap_or_else(|| {
        panic!("Can't get ErrorKind for {:?}", self);
      })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::ErrBox;

  fn io_error() -> io::Error {
    io::Error::from(io::ErrorKind::NotFound)
  }

  fn url_error() -> url::ParseError {
    url::ParseError::EmptyHost
  }

  fn import_map_error() -> ImportMapError {
    ImportMapError {
      msg: "an import map error".to_string(),
    }
  }

  #[test]
  fn test_simple_error() {
    let err =
      ErrBox::from(DenoError::new(ErrorKind::NotFound, "foo".to_string()));
    assert_eq!(err.kind(), ErrorKind::NotFound);
    assert_eq!(err.to_string(), "foo");
  }

  #[test]
  fn test_io_error() {
    let err = ErrBox::from(io_error());
    assert_eq!(err.kind(), ErrorKind::NotFound);
    assert_eq!(err.to_string(), "entity not found");
  }

  #[test]
  fn test_url_error() {
    let err = ErrBox::from(url_error());
    assert_eq!(err.kind(), ErrorKind::URIError);
    assert_eq!(err.to_string(), "empty host");
  }

  // TODO find a way to easily test tokio errors and unix errors

  #[test]
  fn test_import_map_error() {
    let err = ErrBox::from(import_map_error());
    assert_eq!(err.kind(), ErrorKind::Other);
    assert_eq!(err.to_string(), "an import map error");
  }

  #[test]
  fn test_bad_resource() {
    let err = bad_resource();
    assert_eq!(err.kind(), ErrorKind::BadResource);
    assert_eq!(err.to_string(), "bad resource id");
  }

  #[test]
  fn test_permission_denied() {
    let err = permission_denied();
    assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    assert_eq!(err.to_string(), "permission denied");
  }

  #[test]
  fn test_permission_denied_msg() {
    let err =
      permission_denied_msg("run again with the --allow-net flag".to_string());
    assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    assert_eq!(err.to_string(), "run again with the --allow-net flag");
  }

  #[test]
  fn test_no_buffer_specified() {
    let err = no_buffer_specified();
    assert_eq!(err.kind(), ErrorKind::TypeError);
    assert_eq!(err.to_string(), "no buffer specified");
  }
}
