use crate::key::KeyUsage;
use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub enum WebCryptoError {
  MissingArgument(String),
  Unsupported,
}

impl fmt::Display for WebCryptoError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      WebCryptoError::MissingArgument(s) => {
        write!(f, "Missing argument {}", &s)
      }
      WebCryptoError::Unsupported => write!(f, "Unsupported algorithm"),
    }
  }
}

impl std::error::Error for WebCryptoError {}

#[derive(Debug, Serialize)]
pub struct DOMError(pub String);

impl fmt::Display for DOMError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", &self.0)
  }
}

impl std::error::Error for DOMError {}
