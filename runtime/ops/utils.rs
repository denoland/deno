// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::JsNativeError;
use deno_core::error::AnyError;

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {s:?} is not valid UTF-8");
    JsNativeError::new("InvalidData", message).into()
  })
}
