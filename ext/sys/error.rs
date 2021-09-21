use deno_core::error::AnyError;

pub fn get_notify_error_class(e: &AnyError) -> Option<&'static str> {
  use notify::ErrorKind::*;
  e.downcast_ref::<notify::Error>().map(|e| match e.kind {
    Generic(_) => "Error",
    Io(ref e) => get_io_error_class(e),
    PathNotFound => "NotFound",
    WatchNotFound => "NotFound",
    InvalidConfig(_) => "InvalidData",
    MaxFilesWatch => "Error",
  })
}

pub fn get_io_error_class(error: &std::io::Error) -> &'static str {
  use std::io::ErrorKind::*;
  match error.kind() {
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
    Other => "Error",
    WouldBlock => unreachable!(),
    // Non-exhaustive enum - might add new variants
    // in the future
    _ => "Error",
  }
}

pub fn get_env_var_error_class(error: &std::env::VarError) -> &'static str {
  use std::env::VarError::*;
  match error {
    NotPresent => "NotFound",
    NotUnicode(..) => "InvalidData",
  }
}
