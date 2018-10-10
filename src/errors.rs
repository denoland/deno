// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use http::uri;
use hyper;
pub use msg::ErrorKind;
use std;
use std::fmt;
use std::io;

pub type DenoResult<T> = std::result::Result<T, DenoError>;

#[derive(Debug)]
pub struct DenoError {
  repr: Repr,
}

#[derive(Debug)]
enum Repr {
  Simple(ErrorKind, String),
  IoErr(io::Error),
  HyperErr(hyper::Error),
}

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
    }
  }
}

impl fmt::Display for DenoError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self.repr {
      Repr::Simple(_kind, ref err_str) => f.pad(err_str),
      Repr::IoErr(ref err) => err.fmt(f),
      Repr::HyperErr(ref err) => err.fmt(f),
    }
  }
}

impl std::error::Error for DenoError {
  fn description(&self) -> &str {
    match self.repr {
      Repr::Simple(_kind, ref msg) => msg.as_str(),
      Repr::IoErr(ref err) => err.description(),
      Repr::HyperErr(ref err) => err.description(),
    }
  }

  fn cause(&self) -> Option<&std::error::Error> {
    match self.repr {
      Repr::Simple(_kind, ref _msg) => None,
      Repr::IoErr(ref err) => Some(err),
      Repr::HyperErr(ref err) => Some(err),
    }
  }
}

impl From<io::Error> for DenoError {
  #[inline]
  fn from(err: io::Error) -> DenoError {
    DenoError {
      repr: Repr::IoErr(err),
    }
  }
}

impl From<hyper::Error> for DenoError {
  #[inline]
  fn from(err: hyper::Error) -> DenoError {
    DenoError {
      repr: Repr::HyperErr(err),
    }
  }
}

impl From<uri::InvalidUri> for DenoError {
  #[inline]
  fn from(_err: uri::InvalidUri) -> DenoError {
    DenoError {
      repr: Repr::Simple(ErrorKind::InvalidUri, String::from("TODO")),
    }
  }
}

impl From<uri::InvalidUriParts> for DenoError {
  #[inline]
  fn from(_err: uri::InvalidUriParts) -> DenoError {
    DenoError {
      repr: Repr::Simple(ErrorKind::InvalidUri, String::from("TODO")),
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
