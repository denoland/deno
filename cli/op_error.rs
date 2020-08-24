// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - ErrBox: a generic boxed object. This is the super type of all
//!   errors handled in Rust.
//! - JSError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - OpError: these are errors that happen during ops, which are passed
//!   back into the runtime, where an exception object is created and thrown.
//!   OpErrors have an integer code associated with them - access this via the
//!   `kind` field.
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
use std::fmt;
use std::io;

// Warning! The values in this enum are duplicated in js/errors.ts
// Update carefully!
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
  /// This maps to window.Error - ie. a generic error type
  /// if no better context is available.
  /// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error
  Other = 22,
  Busy = 23,
}

impl From<ErrorKind> for String {
  fn from(kind: ErrorKind) -> Self {
    let s = match kind {
      ErrorKind::NotFound => "NotFound",
      ErrorKind::PermissionDenied => "PermissionDenied",
      ErrorKind::ConnectionRefused => "ConnectionRefused",
      ErrorKind::ConnectionReset => "ConnectionReset",
      ErrorKind::ConnectionAborted => "ConnectionAborted",
      ErrorKind::NotConnected => "NotConnected",
      ErrorKind::AddrInUse => "AddrInUse",
      ErrorKind::AddrNotAvailable => "AddrNotAvailable",
      ErrorKind::BrokenPipe => "BrokenPipe",
      ErrorKind::AlreadyExists => "AlreadyExists",
      ErrorKind::InvalidData => "InvalidData",
      ErrorKind::TimedOut => "TimedOut",
      ErrorKind::Interrupted => "Interrupted",
      ErrorKind::WriteZero => "WriteZero",
      ErrorKind::UnexpectedEof => "UnexpectedEof",
      ErrorKind::BadResource => "BadResource",
      ErrorKind::Http => "Http",
      ErrorKind::URIError => "URIError",
      ErrorKind::TypeError => "TypeError",
      ErrorKind::Other => "Other",
      ErrorKind::Busy => "Busy",
    };

    s.to_string()
  }
}

fn error_str_to_kind(kind_str: &str) -> ErrorKind {
  match kind_str {
    "NotFound" => ErrorKind::NotFound,
    "PermissionDenied" => ErrorKind::PermissionDenied,
    "ConnectionRefused" => ErrorKind::ConnectionRefused,
    "ConnectionReset" => ErrorKind::ConnectionReset,
    "ConnectionAborted" => ErrorKind::ConnectionAborted,
    "NotConnected" => ErrorKind::NotConnected,
    "AddrInUse" => ErrorKind::AddrInUse,
    "AddrNotAvailable" => ErrorKind::AddrNotAvailable,
    "BrokenPipe" => ErrorKind::BrokenPipe,
    "AlreadyExists" => ErrorKind::AlreadyExists,
    "InvalidData" => ErrorKind::InvalidData,
    "TimedOut" => ErrorKind::TimedOut,
    "Interrupted" => ErrorKind::Interrupted,
    "WriteZero" => ErrorKind::WriteZero,
    "UnexpectedEof" => ErrorKind::UnexpectedEof,
    "BadResource" => ErrorKind::BadResource,
    "Http" => ErrorKind::Http,
    "URIError" => ErrorKind::URIError,
    "TypeError" => ErrorKind::TypeError,
    "Other" => ErrorKind::Other,
    "Busy" => ErrorKind::Busy,
    _ => panic!("unknown error kind"),
  }
}

#[derive(Debug)]
pub struct OpError {
  pub kind_str: String,
  pub msg: String,
}

impl OpError {
  fn new(kind: ErrorKind, msg: String) -> Self {
    Self {
      kind_str: kind.into(),
      msg,
    }
  }

  pub fn not_found(msg: String) -> Self {
    Self::new(ErrorKind::NotFound, msg)
  }

  pub fn not_implemented() -> Self {
    Self::other("not implemented".to_string())
  }

  pub fn other(msg: String) -> Self {
    Self::new(ErrorKind::Other, msg)
  }

  pub fn type_error(msg: String) -> Self {
    Self::new(ErrorKind::TypeError, msg)
  }

  pub fn http(msg: String) -> Self {
    Self::new(ErrorKind::Http, msg)
  }

  pub fn uri_error(msg: String) -> Self {
    Self::new(ErrorKind::URIError, msg)
  }

  pub fn permission_denied(msg: String) -> OpError {
    Self::new(ErrorKind::PermissionDenied, msg)
  }

  pub fn bad_resource(msg: String) -> OpError {
    Self::new(ErrorKind::BadResource, msg)
  }

  // BadResource usually needs no additional detail, hence this helper.
  pub fn bad_resource_id() -> OpError {
    Self::new(ErrorKind::BadResource, "Bad resource ID".to_string())
  }

  pub fn invalid_utf8() -> OpError {
    Self::new(ErrorKind::InvalidData, "invalid utf8".to_string())
  }

  pub fn resource_unavailable() -> OpError {
    Self::new(
      ErrorKind::Busy,
      "resource is unavailable because it is in use by a promise".to_string(),
    )
  }

  pub fn invalid_domain_error() -> OpError {
    OpError::type_error("Invalid domain.".to_string())
  }

  pub fn permission_escalation_error() -> OpError {
    OpError::permission_denied(
      "Arguments escalate parent permissions.".to_string(),
    )
  }
}

impl Error for OpError {}

impl fmt::Display for OpError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(self.msg.as_str())
  }
}

impl From<ImportMapError> for OpError {
  fn from(error: ImportMapError) -> Self {
    OpError::from(&error)
  }
}

impl From<&ImportMapError> for OpError {
  fn from(error: &ImportMapError) -> Self {
    Self::new(ErrorKind::Other, error.to_string())
  }
}

impl From<ModuleResolutionError> for OpError {
  fn from(error: ModuleResolutionError) -> Self {
    OpError::from(&error)
  }
}

impl From<&ModuleResolutionError> for OpError {
  fn from(error: &ModuleResolutionError) -> Self {
    Self::new(ErrorKind::URIError, error.to_string())
  }
}

impl From<VarError> for OpError {
  fn from(error: VarError) -> Self {
    OpError::from(&error)
  }
}

impl From<&VarError> for OpError {
  fn from(error: &VarError) -> Self {
    use VarError::*;
    let kind = match error {
      NotPresent => ErrorKind::NotFound,
      NotUnicode(..) => ErrorKind::InvalidData,
    };

    Self::new(kind, error.to_string())
  }
}

impl From<io::Error> for OpError {
  fn from(error: io::Error) -> Self {
    OpError::from(&error)
  }
}

impl From<&io::Error> for OpError {
  fn from(error: &io::Error) -> Self {
    use io::ErrorKind::*;
    let kind = match error.kind() {
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
      Other => ErrorKind::Other,
      WouldBlock => unreachable!(),
      // Non-exhaustive enum - might add new variants
      // in the future
      _ => unreachable!(),
    };

    Self::new(kind, error.to_string())
  }
}

impl From<url::ParseError> for OpError {
  fn from(error: url::ParseError) -> Self {
    OpError::from(&error)
  }
}

impl From<&url::ParseError> for OpError {
  fn from(error: &url::ParseError) -> Self {
    Self::new(ErrorKind::URIError, error.to_string())
  }
}
impl From<reqwest::Error> for OpError {
  fn from(error: reqwest::Error) -> Self {
    OpError::from(&error)
  }
}

impl From<&reqwest::Error> for OpError {
  fn from(error: &reqwest::Error) -> Self {
    match error.source() {
      Some(err_ref) => None
        .or_else(|| {
          err_ref
            .downcast_ref::<url::ParseError>()
            .map(|e| e.clone().into())
        })
        .or_else(|| {
          err_ref
            .downcast_ref::<io::Error>()
            .map(|e| e.to_owned().into())
        })
        .or_else(|| {
          err_ref
            .downcast_ref::<serde_json::error::Error>()
            .map(|e| e.into())
        })
        .unwrap_or_else(|| Self::new(ErrorKind::Http, error.to_string())),
      None => Self::new(ErrorKind::Http, error.to_string()),
    }
  }
}

impl From<ReadlineError> for OpError {
  fn from(error: ReadlineError) -> Self {
    OpError::from(&error)
  }
}

impl From<&ReadlineError> for OpError {
  fn from(error: &ReadlineError) -> Self {
    use ReadlineError::*;
    let kind = match error {
      Io(err) => return OpError::from(err),
      Eof => ErrorKind::UnexpectedEof,
      Interrupted => ErrorKind::Interrupted,
      #[cfg(unix)]
      Errno(err) => return (*err).into(),
      _ => unimplemented!(),
    };

    Self::new(kind, error.to_string())
  }
}

impl From<serde_json::error::Error> for OpError {
  fn from(error: serde_json::error::Error) -> Self {
    OpError::from(&error)
  }
}

impl From<&serde_json::error::Error> for OpError {
  fn from(error: &serde_json::error::Error) -> Self {
    use serde_json::error::*;
    let kind = match error.classify() {
      Category::Io => ErrorKind::TypeError,
      Category::Syntax => ErrorKind::TypeError,
      Category::Data => ErrorKind::InvalidData,
      Category::Eof => ErrorKind::UnexpectedEof,
    };

    Self::new(kind, error.to_string())
  }
}

#[cfg(unix)]
impl From<nix::Error> for OpError {
  fn from(error: nix::Error) -> Self {
    use nix::errno::Errno::*;
    let kind = match error {
      nix::Error::Sys(EPERM) => ErrorKind::PermissionDenied,
      nix::Error::Sys(EINVAL) => ErrorKind::TypeError,
      nix::Error::Sys(ENOENT) => ErrorKind::NotFound,
      nix::Error::Sys(ENOTTY) => ErrorKind::BadResource,
      nix::Error::Sys(UnknownErrno) => unreachable!(),
      nix::Error::Sys(_) => unreachable!(),
      nix::Error::InvalidPath => ErrorKind::TypeError,
      nix::Error::InvalidUtf8 => ErrorKind::InvalidData,
      nix::Error::UnsupportedOperation => unreachable!(),
    };

    Self::new(kind, error.to_string())
  }
}

impl From<dlopen::Error> for OpError {
  fn from(error: dlopen::Error) -> Self {
    OpError::from(&error)
  }
}

impl From<&dlopen::Error> for OpError {
  fn from(error: &dlopen::Error) -> Self {
    use dlopen::Error::*;
    let kind = match error {
      NullCharacter(_) => ErrorKind::Other,
      OpeningLibraryError(e) => return e.into(),
      SymbolGettingError(e) => return e.into(),
      AddrNotMatchingDll(e) => return e.into(),
      NullSymbol => ErrorKind::Other,
    };

    Self::new(kind, error.to_string())
  }
}

impl From<notify::Error> for OpError {
  fn from(error: notify::Error) -> Self {
    OpError::from(&error)
  }
}

impl From<&notify::Error> for OpError {
  fn from(error: &notify::Error) -> Self {
    use notify::ErrorKind::*;
    let kind = match error.kind {
      Generic(_) => ErrorKind::Other,
      Io(ref e) => return e.into(),
      PathNotFound => ErrorKind::NotFound,
      WatchNotFound => ErrorKind::NotFound,
      InvalidConfig(_) => ErrorKind::InvalidData,
    };

    Self::new(kind, error.to_string())
  }
}

impl From<SwcDiagnosticBuffer> for OpError {
  fn from(error: SwcDiagnosticBuffer) -> Self {
    OpError::from(&error)
  }
}

impl From<&SwcDiagnosticBuffer> for OpError {
  fn from(error: &SwcDiagnosticBuffer) -> Self {
    Self::new(ErrorKind::Other, error.diagnostics.join(", "))
  }
}

impl From<ErrBox> for OpError {
  fn from(error: ErrBox) -> Self {
    #[cfg(unix)]
    fn unix_error_kind(err: &ErrBox) -> Option<OpError> {
      err.downcast_ref::<nix::Error>().map(|e| (*e).into())
    }

    #[cfg(not(unix))]
    fn unix_error_kind(_: &ErrBox) -> Option<OpError> {
      None
    }

    None
      .or_else(|| {
        error.downcast_ref::<OpError>().map(|e| {
          OpError::new(error_str_to_kind(&e.kind_str), e.msg.to_string())
        })
      })
      .or_else(|| error.downcast_ref::<reqwest::Error>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<ImportMapError>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<io::Error>().map(|e| e.into()))
      .or_else(|| {
        error
          .downcast_ref::<ModuleResolutionError>()
          .map(|e| e.into())
      })
      .or_else(|| error.downcast_ref::<url::ParseError>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<VarError>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<ReadlineError>().map(|e| e.into()))
      .or_else(|| {
        error
          .downcast_ref::<serde_json::error::Error>()
          .map(|e| e.into())
      })
      .or_else(|| error.downcast_ref::<dlopen::Error>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<notify::Error>().map(|e| e.into()))
      .or_else(|| {
        error
          .downcast_ref::<SwcDiagnosticBuffer>()
          .map(|e| e.into())
      })
      .or_else(|| unix_error_kind(&error))
      .unwrap_or_else(|| {
        panic!("Can't downcast {:?} to OpError", error);
      })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
    let err = OpError::not_found("foo".to_string());
    assert_eq!(err.kind_str, "NotFound");
    assert_eq!(err.to_string(), "foo");
  }

  #[test]
  fn test_io_error() {
    let err = OpError::from(io_error());
    assert_eq!(err.kind_str, "NotFound");
    assert_eq!(err.to_string(), "entity not found");
  }

  #[test]
  fn test_url_error() {
    let err = OpError::from(url_error());
    assert_eq!(err.kind_str, "URIError");
    assert_eq!(err.to_string(), "empty host");
  }

  // TODO find a way to easily test tokio errors and unix errors

  #[test]
  fn test_import_map_error() {
    let err = OpError::from(import_map_error());
    assert_eq!(err.kind_str, "Other");
    assert_eq!(err.to_string(), "an import map error");
  }

  #[test]
  fn test_bad_resource() {
    let err = OpError::bad_resource("Resource has been closed".to_string());
    assert_eq!(err.kind_str, "BadResource");
    assert_eq!(err.to_string(), "Resource has been closed");
  }

  #[test]
  fn test_bad_resource_id() {
    let err = OpError::bad_resource_id();
    assert_eq!(err.kind_str, "BadResource");
    assert_eq!(err.to_string(), "Bad resource ID");
  }

  #[test]
  fn test_permission_denied() {
    let err = OpError::permission_denied(
      "run again with the --allow-net flag".to_string(),
    );
    assert_eq!(err.kind_str, "PermissionDenied");
    assert_eq!(err.to_string(), "run again with the --allow-net flag");
  }
}
