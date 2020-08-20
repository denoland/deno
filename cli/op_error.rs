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
}

impl Error for OpError {}

impl fmt::Display for OpError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(self.msg.as_str())
  }
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

impl From<OpError> for ErrBox {
  fn from(error: OpError) -> Self {
    ErrBox::new_text(error.kind, error.msg)
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

pub fn var_to_errbox(error: VarError) -> ErrBox {
  use VarError::*;
  let kind = match error {
    NotPresent => "NotFound",
    NotUnicode(..) => "InvalidData",
  };

  ErrBox::new(kind, error)
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

pub fn io_to_errbox(error: io::Error) -> ErrBox {
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
  ErrBox::new(kind, error)
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

pub fn url_to_errbox(error: url::ParseError) -> ErrBox {
  ErrBox::new("URIError", error)
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

pub fn reqwest_to_errbox(error: reqwest::Error) -> ErrBox {
  match error.source() {
    Some(err_ref) => None
      .or_else(|| {
        err_ref.downcast_ref::<url::ParseError>().map(|_e| {
          // url_to_errbox(e.to_owned())
          todo!()
        })
      })
      .or_else(|| {
        err_ref.downcast_ref::<io::Error>().map(|_e| {
          // io_to_errbox(e.to_owned())
          todo!()
        })
      })
      .or_else(|| {
        err_ref
          .downcast_ref::<serde_json::error::Error>()
          .map(|_e| {
            // serde_to_errbox(e.to_owned())
            todo!()
          })
      })
      .unwrap_or_else(|| ErrBox::new("Http", error)),
    None => ErrBox::new("Http", error),
  }
}

pub fn readline_to_errbox(error: ReadlineError) -> ErrBox {
  use ReadlineError::*;
  let kind = match error {
    Io(err) => return io_to_errbox(err),
    Eof => "UnexpectedEof",
    Interrupted => "Interrupted",
    #[cfg(unix)]
    Errno(_err) => todo!(),
    _ => unimplemented!(),
  };
  ErrBox::new(kind, error)
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

pub fn serde_to_errbox(error: serde_json::error::Error) -> ErrBox {
  use serde_json::error::*;
  let kind = match error.classify() {
    Category::Io => "TypeError",
    Category::Syntax => "TypeError",
    Category::Data => "InvalidData",
    Category::Eof => "UnexpectedEof",
  };
  ErrBox::new(kind, error)
}

#[cfg(unix)]
pub fn nix_to_errbox(error: nix::Error) -> ErrBox {
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

pub fn dl_to_errbox(error: dlopen::Error) -> ErrBox {
  use dlopen::Error::*;
  let kind = match error {
    NullCharacter(_) => "Other",
    OpeningLibraryError(e) => return io_to_errbox(e),
    SymbolGettingError(e) => return io_to_errbox(e),
    AddrNotMatchingDll(e) => return io_to_errbox(e),
    NullSymbol => "Other",
  };

  ErrBox::new(kind, error)
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
      .or_else(|| error.downcast_ref::<io::Error>().map(|e| e.into()))
      .or_else(|| {
        error
          .downcast_ref::<ModuleResolutionError>()
          .map(|e| e.into())
      })
      .or_else(|| error.downcast_ref::<url::ParseError>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<VarError>().map(|e| e.into()))
      .or_else(|| {
        error
          .downcast_ref::<serde_json::error::Error>()
          .map(|e| e.into())
      })
      .or_else(|| error.downcast_ref::<dlopen::Error>().map(|e| e.into()))
      .or_else(|| error.downcast_ref::<notify::Error>().map(|e| e.into()))
      .or_else(|| unix_error_kind(&error))
      .or_else(|| Some(OpError::new(error.1, error.0.to_string())))
      .unwrap()
  }
}
