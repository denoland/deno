// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
  #[error("{0}")]
  Message(String),

  #[error("serde_v8 error: invalid type, expected: boolean")]
  ExpectedBoolean,
  #[error("serde_v8 error: invalid type, expected: integer")]
  ExpectedInteger,
  #[error("serde_v8 error: invalid type, expected: number")]
  ExpectedNumber,
  #[error("serde_v8 error: invalid type, expected: string")]
  ExpectedString,
  #[error("serde_v8 error: invalid type, expected: array")]
  ExpectedArray,
  #[error("serde_v8 error: invalid type, expected: map")]
  ExpectedMap,
  #[error("serde_v8 error: invalid type, expected: enum")]
  ExpectedEnum,
  #[error("serde_v8 error: invalid type, expected: object")]
  ExpectedObject,
  #[error("serde_v8 error: invalid type, expected: buffer")]
  ExpectedBuffer,
  #[error("serde_v8 error: invalid type, expected: detachable")]
  ExpectedDetachable,
  #[error("serde_v8 error: invalid type, expected: external")]
  ExpectedExternal,
  #[error("serde_v8 error: invalid type, expected: bigint")]
  ExpectedBigInt,

  #[error("serde_v8 error: invalid type, expected: utf8")]
  ExpectedUtf8,
  #[error("serde_v8 error: invalid type, expected: latin1")]
  ExpectedLatin1,

  #[error("serde_v8 error: unsupported type")]
  UnsupportedType,

  #[error("serde_v8 error: length mismatch, got: {0}, expected: {1}")]
  LengthMismatch(usize, usize),
}

impl serde::ser::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}

impl serde::de::Error for Error {
  fn custom<T: Display>(msg: T) -> Self {
    Error::Message(msg.to_string())
  }
}
