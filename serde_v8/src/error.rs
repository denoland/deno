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
      _ => formatter.write_str("serde_v8 error: TODO"),
    }
  }
}

impl std::error::Error for Error {}
