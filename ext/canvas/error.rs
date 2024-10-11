// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use std::borrow::Cow;
use std::fmt;

#[derive(Debug)]
pub struct DOMExceptionInvalidStateError {
  pub msg: String,
}

impl DOMExceptionInvalidStateError {
  pub fn new(msg: &str) -> Self {
    DOMExceptionInvalidStateError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DOMExceptionInvalidStateError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DOMExceptionInvalidStateError {}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DOMExceptionInvalidStateError>()
    .map(|_| "DOMExceptionInvalidStateError")
}

/// Returns a string that represents the error message for the image.
pub(crate) fn image_error_message<'a, T: Into<Cow<'a, str>>>(
  opreation: T,
  reason: T,
) -> String {
  format!(
    "An error has occurred while {}.
reason: {}",
    opreation.into(),
    reason.into(),
  )
}
