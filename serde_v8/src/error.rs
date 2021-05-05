// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use std::fmt::{self, Display};

use serde::{de, ser};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
  Message(String),

  ExpectedBoolean,
  ExpectedInteger,
  ExpectedString,
  ExpectedNull,
  ExpectedArray,
  ExpectedMap,
  ExpectedEnum,

  ExpectedUtf8,

  LengthMismatch,
}

impl ser::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl de::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl Display for Error {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Error::Message(msg) => formatter.write_str(msg),
      err => formatter.write_str(format!("serde_v8 error: {:?}", err).as_ref()),
    }
  }
}

impl std::error::Error for Error {}
