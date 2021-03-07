// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub enum WebCryptoError {
  MissingArgument(String),
  Unsupported,
  UnsupportedHash,
}

impl fmt::Display for WebCryptoError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      WebCryptoError::MissingArgument(s) => {
        write!(f, "Missing argument {}", &s)
      }
      WebCryptoError::Unsupported => write!(f, "Unsupported algorithm"),
      WebCryptoError::UnsupportedHash => write!(f, "Unsupported hash"),
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
