// Copyright 2018-2025 the Deno authors. MIT license.

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[property("code" = self.code())]
pub enum Error {
  #[class(type)]
  #[error("{0}")]
  #[allow(dead_code)]
  InvalidArgType(&'static str),
  #[class(type)]
  #[error("Cannot call constructor without `new`")]
  ConstructCallRequired,
}

impl Error {
  pub fn code(&self) -> ErrorCode {
    match self {
      Self::InvalidArgType(_) => ErrorCode::ERR_INVALID_ARG_TYPE,
      Self::ConstructCallRequired => ErrorCode::ERR_CONSTRUCT_CALL_REQUIRED,
    }
  }
}

#[allow(non_camel_case_types)]
pub enum ErrorCode {
  ERR_INVALID_ARG_TYPE,
  ERR_CONSTRUCT_CALL_REQUIRED,
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
      Self::ERR_CONSTRUCT_CALL_REQUIRED => "ERR_CONSTRUCT_CALL_REQUIRED",
    }
  }
}

impl From<ErrorCode> for deno_error::PropertyValue {
  fn from(code: ErrorCode) -> Self {
    deno_error::PropertyValue::from(code.as_str().to_string())
  }
}
