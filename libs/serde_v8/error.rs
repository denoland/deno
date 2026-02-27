// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Display;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[non_exhaustive]
pub enum Error {
  #[error("{0}")]
  Message(String),

  #[error("serde_v8 error: invalid type; expected: boolean, got: {0}")]
  ExpectedBoolean(&'static str),

  #[error("serde_v8 error: invalid type; expected: integer, got: {0}")]
  ExpectedInteger(&'static str),

  #[error("serde_v8 error: invalid type; expected: number, got: {0}")]
  ExpectedNumber(&'static str),

  #[error("serde_v8 error: invalid type; expected: string, got: {0}")]
  ExpectedString(&'static str),

  #[error("serde_v8 error: invalid type; expected: array, got: {0}")]
  ExpectedArray(&'static str),

  #[error("serde_v8 error: invalid type; expected: map, got: {0}")]
  ExpectedMap(&'static str),

  #[error("serde_v8 error: invalid type; expected: enum, got: {0}")]
  ExpectedEnum(&'static str),

  #[error("serde_v8 error: invalid type; expected: object, got: {0}")]
  ExpectedObject(&'static str),

  #[error("serde_v8 error: invalid type; expected: buffer, got: {0}")]
  ExpectedBuffer(&'static str),

  #[error("serde_v8 error: invalid type; expected: detachable, got: {0}")]
  ExpectedDetachable(&'static str),

  #[error("serde_v8 error: invalid type; expected: external, got: {0}")]
  ExpectedExternal(&'static str),

  #[error("serde_v8 error: invalid type; expected: bigint, got: {0}")]
  ExpectedBigInt(&'static str),

  #[error("serde_v8 error: invalid type, expected: utf8")]
  ExpectedUtf8,
  #[error("serde_v8 error: invalid type, expected: latin1")]
  ExpectedLatin1,

  #[error("serde_v8 error: unsupported type")]
  UnsupportedType,

  #[error("serde_v8 error: length mismatch, got: {0}, expected: {1}")]
  LengthMismatch(usize, usize),

  #[error("serde_v8 error: can't create slice from resizable ArrayBuffer")]
  ResizableBackingStoreNotSupported,

  #[error("{0}")]
  Custom(Box<dyn std::error::Error + Send + Sync + 'static>),
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
