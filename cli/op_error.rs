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

#[derive(Debug)]
pub struct OpError {
  pub kind: &'static str,
  pub msg: String,
}

impl OpError {
  fn new(kind: &'static str, msg: String) -> Self {
    Self { kind, msg }
  }

  pub fn not_found(msg: String) -> Self {
    Self::new("NotFound", msg)
  }

  pub fn not_implemented() -> Self {
    Self::other("not implemented".to_string())
  }

  pub fn other(msg: String) -> Self {
    Self::new("Other", msg)
  }

  pub fn type_error(msg: String) -> Self {
    Self::new("TypeError", msg)
  }

  pub fn http(msg: String) -> Self {
    Self::new("Http", msg)
  }

  pub fn uri_error(msg: String) -> Self {
    Self::new("URIError", msg)
  }

  pub fn permission_denied(msg: String) -> OpError {
    Self::new("PermissionDenied", msg)
  }

  pub fn bad_resource(msg: String) -> OpError {
    Self::new("BadResource", msg)
  }

  // BadResource usually needs no additional detail, hence this helper.
  pub fn bad_resource_id() -> OpError {
    Self::new("BadResource", "Bad resource ID".to_string())
  }

  pub fn invalid_utf8() -> OpError {
    Self::new("InvalidData", "invalid utf8".to_string())
  }

  pub fn resource_unavailable() -> OpError {
    Self::new(
      "Busy",
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
    Self::new("Other", error.to_string())
  }
}

impl From<ModuleResolutionError> for OpError {
  fn from(error: ModuleResolutionError) -> Self {
    OpError::from(&error)
  }
}

impl From<&ModuleResolutionError> for OpError {
  fn from(error: &ModuleResolutionError) -> Self {
    Self::new("URIError", error.to_string())
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
      NotPresent => "NotFound",
      NotUnicode(..) => "InvalidData",
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
    Self::new("URIError", error.to_string())
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
        .unwrap_or_else(|| Self::new("Http", error.to_string())),
      None => Self::new("Http", error.to_string()),
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
      Eof => "UnexpectedEof",
      Interrupted => "Interrupted",
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
      Category::Io => "TypeError",
      Category::Syntax => "TypeError",
      Category::Data => "InvalidData",
      Category::Eof => "UnexpectedEof",
    };

    Self::new(kind, error.to_string())
  }
}

#[cfg(unix)]
impl From<nix::Error> for OpError {
  fn from(error: nix::Error) -> Self {
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
      NullCharacter(_) => "Other",
      OpeningLibraryError(e) => return e.into(),
      SymbolGettingError(e) => return e.into(),
      AddrNotMatchingDll(e) => return e.into(),
      NullSymbol => "Other",
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
      Generic(_) => "Other",
      Io(ref e) => return e.into(),
      PathNotFound => "NotFound",
      WatchNotFound => "NotFound",
      InvalidConfig(_) => "InvalidData",
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
    Self::new("Other", error.diagnostics.join(", "))
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
        error
          .downcast_ref::<OpError>()
          .map(|e| OpError::new(&e.kind, e.msg.to_string()))
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
    assert_eq!(err.kind, "NotFound");
    assert_eq!(err.to_string(), "foo");
  }

  #[test]
  fn test_io_error() {
    let err = OpError::from(io_error());
    assert_eq!(err.kind, "NotFound");
    assert_eq!(err.to_string(), "entity not found");
  }

  #[test]
  fn test_url_error() {
    let err = OpError::from(url_error());
    assert_eq!(err.kind, "URIError");
    assert_eq!(err.to_string(), "empty host");
  }

  // TODO find a way to easily test tokio errors and unix errors

  #[test]
  fn test_import_map_error() {
    let err = OpError::from(import_map_error());
    assert_eq!(err.kind, "Other");
    assert_eq!(err.to_string(), "an import map error");
  }

  #[test]
  fn test_bad_resource() {
    let err = OpError::bad_resource("Resource has been closed".to_string());
    assert_eq!(err.kind, "BadResource");
    assert_eq!(err.to_string(), "Resource has been closed");
  }

  #[test]
  fn test_bad_resource_id() {
    let err = OpError::bad_resource_id();
    assert_eq!(err.kind, "BadResource");
    assert_eq!(err.to_string(), "Bad resource ID");
  }

  #[test]
  fn test_permission_denied() {
    let err = OpError::permission_denied(
      "run again with the --allow-net flag".to_string(),
    );
    assert_eq!(err.kind, "PermissionDenied");
    assert_eq!(err.to_string(), "run again with the --allow-net flag");
  }
}
