// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::diagnostics::Diagnostic;
use crate::fmt_errors::JSError;
use crate::import_map::ImportMapError;
pub use crate::msg::ErrorKind;
use deno::AnyError;
use deno::ErrBox;
use deno::ModuleResolutionError;
use http::uri;
use hyper;
use reqwest;
use rustyline::error::ReadlineError;
use std;
use std::env::VarError;
use std::error::Error;
use std::fmt;
use std::io;
use url;

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

pub fn op_not_implemented() -> ErrBox {
  StaticError(ErrorKind::OpNotAvailable, "op not implemented").into()
}

pub fn no_buffer_specified() -> ErrBox {
  StaticError(ErrorKind::InvalidInput, "no buffer specified").into()
}

pub fn no_async_support() -> ErrBox {
  StaticError(ErrorKind::NoAsyncSupport, "op doesn't support async calls")
    .into()
}

pub fn no_sync_support() -> ErrBox {
  StaticError(ErrorKind::NoSyncSupport, "op doesn't support sync calls").into()
}

pub fn invalid_address_syntax() -> ErrBox {
  StaticError(ErrorKind::InvalidInput, "invalid address syntax").into()
}

pub fn too_many_redirects() -> ErrBox {
  StaticError(ErrorKind::TooManyRedirects, "too many redirects").into()
}

pub fn type_error(msg: String) -> ErrBox {
  DenoError::new(ErrorKind::TypeError, msg).into()
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

impl GetErrorKind for JSError {
  fn kind(&self) -> ErrorKind {
    ErrorKind::JSError
  }
}

impl GetErrorKind for Diagnostic {
  fn kind(&self) -> ErrorKind {
    ErrorKind::Diagnostic
  }
}

impl GetErrorKind for ImportMapError {
  fn kind(&self) -> ErrorKind {
    ErrorKind::ImportMapError
  }
}

impl GetErrorKind for ModuleResolutionError {
  fn kind(&self) -> ErrorKind {
    use ModuleResolutionError::*;
    match self {
      InvalidUrl(ref err) | InvalidBaseUrl(ref err) => err.kind(),
      InvalidPath(_) => ErrorKind::InvalidPath,
      ImportPrefixMissing(_, _) => ErrorKind::ImportPrefixMissing,
    }
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
      WouldBlock => ErrorKind::WouldBlock,
      InvalidInput => ErrorKind::InvalidInput,
      InvalidData => ErrorKind::InvalidData,
      TimedOut => ErrorKind::TimedOut,
      Interrupted => ErrorKind::Interrupted,
      WriteZero => ErrorKind::WriteZero,
      UnexpectedEof => ErrorKind::UnexpectedEof,
      _ => ErrorKind::Other,
    }
  }
}

impl GetErrorKind for uri::InvalidUri {
  fn kind(&self) -> ErrorKind {
    // The http::uri::ErrorKind exists and is similar to url::ParseError.
    // However it is also private, so we can't get any details out.
    ErrorKind::InvalidUri
  }
}

impl GetErrorKind for url::ParseError {
  fn kind(&self) -> ErrorKind {
    use url::ParseError::*;
    match self {
      EmptyHost => ErrorKind::EmptyHost,
      IdnaError => ErrorKind::IdnaError,
      InvalidDomainCharacter => ErrorKind::InvalidDomainCharacter,
      InvalidIpv4Address => ErrorKind::InvalidIpv4Address,
      InvalidIpv6Address => ErrorKind::InvalidIpv6Address,
      InvalidPort => ErrorKind::InvalidPort,
      Overflow => ErrorKind::Overflow,
      RelativeUrlWithCannotBeABaseBase => {
        ErrorKind::RelativeUrlWithCannotBeABaseBase
      }
      RelativeUrlWithoutBase => ErrorKind::RelativeUrlWithoutBase,
      SetHostOnCannotBeABaseUrl => ErrorKind::SetHostOnCannotBeABaseUrl,
    }
  }
}

impl GetErrorKind for hyper::Error {
  fn kind(&self) -> ErrorKind {
    match self {
      e if e.is_canceled() => ErrorKind::HttpCanceled,
      e if e.is_closed() => ErrorKind::HttpClosed,
      e if e.is_parse() => ErrorKind::HttpParse,
      e if e.is_user() => ErrorKind::HttpUser,
      _ => ErrorKind::HttpOther,
    }
  }
}

impl GetErrorKind for reqwest::Error {
  fn kind(&self) -> ErrorKind {
    use self::GetErrorKind as Get;

    match self.get_ref() {
      Some(err_ref) => None
        .or_else(|| err_ref.downcast_ref::<hyper::Error>().map(Get::kind))
        .or_else(|| err_ref.downcast_ref::<url::ParseError>().map(Get::kind))
        .or_else(|| err_ref.downcast_ref::<io::Error>().map(Get::kind))
        .or_else(|| {
          err_ref
            .downcast_ref::<serde_json::error::Error>()
            .map(Get::kind)
        })
        .unwrap_or_else(|| ErrorKind::HttpOther),
      _ => ErrorKind::HttpOther,
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
      Category::Io => ErrorKind::InvalidInput,
      Category::Syntax => ErrorKind::InvalidInput,
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
        Sys(EINVAL) => ErrorKind::InvalidInput,
        Sys(ENOENT) => ErrorKind::NotFound,
        Sys(_) => ErrorKind::UnixError,
        _ => ErrorKind::Other,
      }
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
      .or_else(|| self.downcast_ref::<Diagnostic>().map(Get::kind))
      .or_else(|| self.downcast_ref::<hyper::Error>().map(Get::kind))
      .or_else(|| self.downcast_ref::<reqwest::Error>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ImportMapError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<io::Error>().map(Get::kind))
      .or_else(|| self.downcast_ref::<JSError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ModuleResolutionError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<StaticError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<uri::InvalidUri>().map(Get::kind))
      .or_else(|| self.downcast_ref::<url::ParseError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<VarError>().map(Get::kind))
      .or_else(|| self.downcast_ref::<ReadlineError>().map(Get::kind))
      .or_else(|| {
        self
          .downcast_ref::<serde_json::error::Error>()
          .map(Get::kind)
      })
      .or_else(|| unix_error_kind(self))
      .unwrap_or_else(|| {
        panic!("Can't get ErrorKind for {:?}", self);
      })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::colors::strip_ansi_codes;
  use crate::diagnostics::Diagnostic;
  use crate::diagnostics::DiagnosticCategory;
  use crate::diagnostics::DiagnosticItem;
  use deno::ErrBox;
  use deno::StackFrame;
  use deno::V8Exception;

  fn js_error() -> JSError {
    JSError::new(V8Exception {
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
    })
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
      ErrBox::from(DenoError::new(ErrorKind::NoError, "foo".to_string()));
    assert_eq!(err.kind(), ErrorKind::NoError);
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
    assert_eq!(err.kind(), ErrorKind::EmptyHost);
    assert_eq!(err.to_string(), "empty host");
  }

  // TODO find a way to easily test tokio errors and unix errors

  #[test]
  fn test_diagnostic() {
    let err = ErrBox::from(diagnostic());
    assert_eq!(err.kind(), ErrorKind::Diagnostic);
    assert_eq!(strip_ansi_codes(&err.to_string()), "error TS2322: Example 1\n\n► deno/tests/complex_diagnostics.ts:19:3\n\n19   values: o => [\n     ~~~~~~\n\nerror TS2000: Example 2\n\n► /foo/bar.ts:129:3\n\n129   values: undefined,\n      ~~~~~~\n\n\nFound 2 errors.\n");
  }

  #[test]
  fn test_js_error() {
    let err = ErrBox::from(js_error());
    assert_eq!(err.kind(), ErrorKind::JSError);
    assert_eq!(strip_ansi_codes(&err.to_string()), "error: Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2");
  }

  #[test]
  fn test_import_map_error() {
    let err = ErrBox::from(import_map_error());
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
  fn test_permission_denied_msg() {
    let err =
      permission_denied_msg("run again with the --allow-net flag".to_string());
    assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    assert_eq!(err.to_string(), "run again with the --allow-net flag");
  }

  #[test]
  fn test_op_not_implemented() {
    let err = op_not_implemented();
    assert_eq!(err.kind(), ErrorKind::OpNotAvailable);
    assert_eq!(err.to_string(), "op not implemented");
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
}
