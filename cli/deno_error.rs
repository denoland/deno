// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::diagnostics;
use crate::fmt_errors::JSErrorColor;
use crate::import_map;
pub use crate::msg::ErrorKind;
use crate::resolve_addr::ResolveAddrError;
use crate::source_maps::apply_source_map;
use crate::source_maps::SourceMapGetter;
use deno::JSError;
use hyper;
#[cfg(unix)]
use nix::{errno::Errno, Error as UnixError};
use std;
use std::fmt;
use std::io;
use std::str;
use url;

pub type DenoResult<T> = std::result::Result<T, DenoError>;

#[derive(Debug)]
pub struct DenoError {
  repr: Repr,
}

#[derive(Debug)]
enum Repr {
  Simple(ErrorKind, String),
  IoErr(io::Error),
  UrlErr(url::ParseError),
  HyperErr(hyper::Error),
  ImportMapErr(import_map::ImportMapError),
  Diagnostic(diagnostics::Diagnostic),
  JSError(JSError),
}

/// Create a new simple DenoError.
pub fn new(kind: ErrorKind, msg: String) -> DenoError {
  DenoError {
    repr: Repr::Simple(kind, msg),
  }
}

impl DenoError {
  pub fn kind(&self) -> ErrorKind {
    match self.repr {
      Repr::Simple(kind, ref _msg) => kind,
      // Repr::Simple(kind) => kind,
      Repr::IoErr(ref err) => {
        use std::io::ErrorKind::*;
        match err.kind() {
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
          WouldBlock => ErrorKind::WouldBlock,
          InvalidInput => ErrorKind::InvalidInput,
          InvalidData => ErrorKind::InvalidData,
          TimedOut => ErrorKind::TimedOut,
          Interrupted => ErrorKind::Interrupted,
          WriteZero => ErrorKind::WriteZero,
          Other => ErrorKind::Other,
          UnexpectedEof => ErrorKind::UnexpectedEof,
          _ => unreachable!(),
        }
      }
      Repr::UrlErr(ref err) => {
        use url::ParseError::*;
        match err {
          EmptyHost => ErrorKind::EmptyHost,
          IdnaError => ErrorKind::IdnaError,
          InvalidPort => ErrorKind::InvalidPort,
          InvalidIpv4Address => ErrorKind::InvalidIpv4Address,
          InvalidIpv6Address => ErrorKind::InvalidIpv6Address,
          InvalidDomainCharacter => ErrorKind::InvalidDomainCharacter,
          RelativeUrlWithoutBase => ErrorKind::RelativeUrlWithoutBase,
          RelativeUrlWithCannotBeABaseBase => {
            ErrorKind::RelativeUrlWithCannotBeABaseBase
          }
          SetHostOnCannotBeABaseUrl => ErrorKind::SetHostOnCannotBeABaseUrl,
          Overflow => ErrorKind::Overflow,
        }
      }
      Repr::HyperErr(ref err) => {
        // For some reason hyper::errors::Kind is private.
        if err.is_parse() {
          ErrorKind::HttpParse
        } else if err.is_user() {
          ErrorKind::HttpUser
        } else if err.is_canceled() {
          ErrorKind::HttpCanceled
        } else if err.is_closed() {
          ErrorKind::HttpClosed
        } else {
          ErrorKind::HttpOther
        }
      }
      Repr::ImportMapErr(ref _err) => ErrorKind::ImportMapError,
      Repr::Diagnostic(ref _err) => ErrorKind::Diagnostic,
      Repr::JSError(ref _err) => ErrorKind::JSError,
    }
  }

  pub fn apply_source_map<G: SourceMapGetter>(self, getter: &G) -> Self {
    if let Repr::JSError(js_error) = self.repr {
      return DenoError {
        repr: Repr::JSError(apply_source_map(&js_error, getter)),
      };
    } else {
      panic!("attempt to apply source map an unremappable error")
    }
  }
}

impl fmt::Display for DenoError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.repr {
      Repr::Simple(_kind, ref err_str) => f.pad(err_str),
      Repr::IoErr(ref err) => err.fmt(f),
      Repr::UrlErr(ref err) => err.fmt(f),
      Repr::HyperErr(ref err) => err.fmt(f),
      Repr::ImportMapErr(ref err) => f.pad(&err.msg),
      Repr::Diagnostic(ref err) => err.fmt(f),
      Repr::JSError(ref err) => JSErrorColor(err).fmt(f),
    }
  }
}

impl std::error::Error for DenoError {
  fn description(&self) -> &str {
    match self.repr {
      Repr::Simple(_kind, ref msg) => msg.as_str(),
      Repr::IoErr(ref err) => err.description(),
      Repr::UrlErr(ref err) => err.description(),
      Repr::HyperErr(ref err) => err.description(),
      Repr::ImportMapErr(ref err) => &err.msg,
      Repr::Diagnostic(ref err) => &err.items[0].message,
      Repr::JSError(ref err) => &err.description(),
    }
  }

  fn cause(&self) -> Option<&dyn std::error::Error> {
    match self.repr {
      Repr::Simple(_kind, ref _msg) => None,
      Repr::IoErr(ref err) => Some(err),
      Repr::UrlErr(ref err) => Some(err),
      Repr::HyperErr(ref err) => Some(err),
      Repr::ImportMapErr(ref _err) => None,
      Repr::Diagnostic(ref _err) => None,
      Repr::JSError(ref err) => Some(err),
    }
  }
}

impl From<io::Error> for DenoError {
  #[inline]
  fn from(err: io::Error) -> Self {
    Self {
      repr: Repr::IoErr(err),
    }
  }
}

impl From<url::ParseError> for DenoError {
  #[inline]
  fn from(err: url::ParseError) -> Self {
    Self {
      repr: Repr::UrlErr(err),
    }
  }
}

impl From<hyper::Error> for DenoError {
  #[inline]
  fn from(err: hyper::Error) -> Self {
    Self {
      repr: Repr::HyperErr(err),
    }
  }
}

impl From<ResolveAddrError> for DenoError {
  fn from(e: ResolveAddrError) -> Self {
    match e {
      ResolveAddrError::Syntax => Self {
        repr: Repr::Simple(
          ErrorKind::InvalidInput,
          "invalid address syntax".to_string(),
        ),
      },
      ResolveAddrError::Resolution(io_err) => Self {
        repr: Repr::IoErr(io_err),
      },
    }
  }
}

#[cfg(unix)]
impl From<UnixError> for DenoError {
  fn from(e: UnixError) -> Self {
    match e {
      UnixError::Sys(Errno::EPERM) => Self {
        repr: Repr::Simple(
          ErrorKind::PermissionDenied,
          Errno::EPERM.desc().to_owned(),
        ),
      },
      UnixError::Sys(Errno::EINVAL) => Self {
        repr: Repr::Simple(
          ErrorKind::InvalidInput,
          Errno::EINVAL.desc().to_owned(),
        ),
      },
      UnixError::Sys(Errno::ENOENT) => Self {
        repr: Repr::Simple(
          ErrorKind::NotFound,
          Errno::ENOENT.desc().to_owned(),
        ),
      },
      UnixError::Sys(err) => Self {
        repr: Repr::Simple(ErrorKind::UnixError, err.desc().to_owned()),
      },
      _ => Self {
        repr: Repr::Simple(ErrorKind::Other, format!("{}", e)),
      },
    }
  }
}

impl From<import_map::ImportMapError> for DenoError {
  fn from(err: import_map::ImportMapError) -> Self {
    Self {
      repr: Repr::ImportMapErr(err),
    }
  }
}

impl From<diagnostics::Diagnostic> for DenoError {
  fn from(diagnostic: diagnostics::Diagnostic) -> Self {
    Self {
      repr: Repr::Diagnostic(diagnostic),
    }
  }
}

impl From<JSError> for DenoError {
  fn from(err: JSError) -> Self {
    Self {
      repr: Repr::JSError(err),
    }
  }
}

pub fn bad_resource() -> DenoError {
  new(ErrorKind::BadResource, String::from("bad resource id"))
}

pub fn permission_denied() -> DenoError {
  new(
    ErrorKind::PermissionDenied,
    String::from("permission denied"),
  )
}

pub fn op_not_implemented() -> DenoError {
  new(
    ErrorKind::OpNotAvailable,
    String::from("op not implemented"),
  )
}

pub fn worker_init_failed() -> DenoError {
  // TODO(afinch7) pass worker error data through here
  new(
    ErrorKind::WorkerInitFailed,
    String::from("worker init failed"),
  )
}

pub fn no_buffer_specified() -> DenoError {
  new(ErrorKind::InvalidInput, String::from("no buffer specified"))
}

pub fn no_async_support() -> DenoError {
  new(
    ErrorKind::NoAsyncSupport,
    String::from("op doesn't support async calls"),
  )
}

pub fn no_sync_support() -> DenoError {
  new(
    ErrorKind::NoSyncSupport,
    String::from("op doesn't support sync calls"),
  )
}

pub fn err_check<R>(r: Result<R, DenoError>) {
  if let Err(e) = r {
    panic!(e.to_string());
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ansi::strip_ansi_codes;
  use crate::diagnostics::Diagnostic;
  use crate::diagnostics::DiagnosticCategory;
  use crate::diagnostics::DiagnosticItem;
  use crate::import_map::ImportMapError;
  use deno::StackFrame;

  fn js_error() -> JSError {
    JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: None,
      end_column: None,
      frames: vec![
        StackFrame {
          line: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ],
    }
  }

  fn diagnostic() -> Diagnostic {
    Diagnostic {
      items: vec![
        DiagnosticItem {
          message: "Example 1".to_string(),
          message_chain: None,
          code: 2322,
          category: DiagnosticCategory::Error,
          start_position: Some(267),
          end_position: Some(273),
          source_line: Some("  values: o => [".to_string()),
          line_number: Some(18),
          script_resource_name: Some(
            "deno/tests/complex_diagnostics.ts".to_string(),
          ),
          start_column: Some(2),
          end_column: Some(8),
          related_information: None,
        },
        DiagnosticItem {
          message: "Example 2".to_string(),
          message_chain: None,
          code: 2000,
          category: DiagnosticCategory::Error,
          start_position: Some(2),
          end_position: Some(2),
          source_line: Some("  values: undefined,".to_string()),
          line_number: Some(128),
          script_resource_name: Some("/foo/bar.ts".to_string()),
          start_column: Some(2),
          end_column: Some(8),
          related_information: None,
        },
      ],
    }
  }

  struct MockSourceMapGetter {}

  impl SourceMapGetter for MockSourceMapGetter {
    fn get_source_map(&self, _script_name: &str) -> Option<Vec<u8>> {
      Some(vec![])
    }

    fn get_source_line(
      &self,
      _script_name: &str,
      _line: usize,
    ) -> Option<String> {
      None
    }
  }

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
    let err = new(ErrorKind::NoError, "foo".to_string());
    assert_eq!(err.kind(), ErrorKind::NoError);
    assert_eq!(err.to_string(), "foo");
  }

  #[test]
  fn test_io_error() {
    let err = DenoError::from(io_error());
    assert_eq!(err.kind(), ErrorKind::NotFound);
    assert_eq!(err.to_string(), "entity not found");
  }

  #[test]
  fn test_url_error() {
    let err = DenoError::from(url_error());
    assert_eq!(err.kind(), ErrorKind::EmptyHost);
    assert_eq!(err.to_string(), "empty host");
  }

  // TODO find a way to easily test tokio errors and unix errors

  #[test]
  fn test_diagnostic() {
    let err = DenoError::from(diagnostic());
    assert_eq!(err.kind(), ErrorKind::Diagnostic);
    assert_eq!(strip_ansi_codes(&err.to_string()), "error TS2322: Example 1\n\n► deno/tests/complex_diagnostics.ts:19:3\n\n19   values: o => [\n     ~~~~~~\n\nerror TS2000: Example 2\n\n► /foo/bar.ts:129:3\n\n129   values: undefined,\n      ~~~~~~\n\n\nFound 2 errors.\n");
  }

  #[test]
  fn test_js_error() {
    let err = DenoError::from(js_error());
    assert_eq!(err.kind(), ErrorKind::JSError);
    assert_eq!(strip_ansi_codes(&err.to_string()), "error: Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2");
  }

  #[test]
  fn test_import_map_error() {
    let err = DenoError::from(import_map_error());
    assert_eq!(err.kind(), ErrorKind::ImportMapError);
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
  fn test_op_not_implemented() {
    let err = op_not_implemented();
    assert_eq!(err.kind(), ErrorKind::OpNotAvailable);
    assert_eq!(err.to_string(), "op not implemented");
  }

  #[test]
  fn test_worker_init_failed() {
    let err = worker_init_failed();
    assert_eq!(err.kind(), ErrorKind::WorkerInitFailed);
    assert_eq!(err.to_string(), "worker init failed");
  }

  #[test]
  fn test_no_buffer_specified() {
    let err = no_buffer_specified();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(err.to_string(), "no buffer specified");
  }

  #[test]
  fn test_no_async_support() {
    let err = no_async_support();
    assert_eq!(err.kind(), ErrorKind::NoAsyncSupport);
    assert_eq!(err.to_string(), "op doesn't support async calls");
  }

  #[test]
  fn test_no_sync_support() {
    let err = no_sync_support();
    assert_eq!(err.kind(), ErrorKind::NoSyncSupport);
    assert_eq!(err.to_string(), "op doesn't support sync calls");
  }

  #[test]
  #[should_panic]
  fn test_apply_source_map_invalid() {
    let getter = MockSourceMapGetter {};
    let err = new(ErrorKind::NotFound, "not found".to_string());
    err.apply_source_map(&getter);
  }

  #[test]
  #[should_panic]
  fn test_err_check() {
    err_check(
      Err(new(ErrorKind::NotFound, "foo".to_string())) as Result<(), DenoError>
    );
  }
}
