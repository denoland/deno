// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::v8;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[property("code" = self.code())]
pub enum Error {
  #[class(type)]
  #[error("{0}")]
  InvalidArgType(&'static str),
}

impl Error {
  pub fn code(&self) -> ErrorCode {
    match self {
      Self::InvalidArgType(_) => ErrorCode::ERR_INVALID_ARG_TYPE,
    }
  }
}

#[allow(non_camel_case_types)]
pub enum ErrorCode {
  ERR_INVALID_ARG_TYPE,
}

impl std::fmt::Display for ErrorCode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl ErrorCode {
  pub fn as_str(&self) -> &str {
    match self {
      Self::ERR_INVALID_ARG_TYPE => "ERR_INVALID_ARG_TYPE",
    }
  }
}

impl From<ErrorCode> for deno_error::PropertyValue {
  fn from(code: ErrorCode) -> Self {
    deno_error::PropertyValue::from(code.as_str().to_string())
  }
}

pub(super) fn sql_str(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_string() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"sql\" argument must be a string.",
  ))
}

pub(super) fn changeset_buffer(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_uint8_array() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"changeset\" argument must be a Uint8Array.",
  ))
}

pub(super) fn name_str(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_string() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"name\" argument must be a string.",
  ))
}

pub(super) fn path_str(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_string() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"path\" argument must be a string.",
  ))
}

pub(super) fn allow_bare_named_params_bool(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_boolean() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"allowBareNamedParameters\" argument must be a boolean.",
  ))
}

pub(super) fn allow_unknown_named_params_bool(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_boolean() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"enabled\" argument must be a boolean.",
  ))
}

pub(super) fn read_big_ints_bool(
  _: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<(), Error> {
  if value.is_boolean() {
    return Ok(());
  }

  Err(Error::InvalidArgType(
    "The \"readBigInts\" argument must be a boolean.",
  ))
}
