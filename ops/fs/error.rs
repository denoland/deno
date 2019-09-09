use deno_dispatch_json::GetErrorKind;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct FsOpError<E: StdError>(E);

impl<E: StdError> fmt::Display for FsOpError<E> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.0, f)
  }
}

impl<E: StdError> StdError for FsOpError<E> {}

impl<E: StdError> From<E> for FsOpError<E> {
  fn from(e: E) -> Self {
    Self(e)
  }
}

impl GetErrorKind for FsOpError<std::io::Error> {
  fn kind(&self) -> &str {
    use std::io::ErrorKind::*;
    match self.0.kind() {
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
      WouldBlock => "WouldBlock",
      InvalidInput => "InvalidInput",
      InvalidData => "InvalidData",
      TimedOut => "TimedOut",
      Interrupted => "Interrupted",
      WriteZero => "WriteZero",
      UnexpectedEof => "UnexpectedEof",
      _ => "Other",
    }
  }
}

#[cfg(unix)]
mod unix {
  use super::{FsOpError, GetErrorKind};
  use nix::errno::Errno::*;
  pub use nix::Error;
  use nix::Error::Sys;

  impl GetErrorKind for FsOpError<Error> {
    fn kind(&self) -> &str {
      match self.0 {
        Sys(EPERM) => "PermissionDenied",
        Sys(EINVAL) => "InvalidInput",
        Sys(ENOENT) => "NotFound",
        Sys(_) => "UnixError",
        _ => "Other",
      }
    }
  }
}
