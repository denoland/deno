// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

pub use anyhow::anyhow;
pub use anyhow::bail;
pub use anyhow::Context;
use rusty_v8 as v8;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;

/// A generic wrapper that can encapsulate any concrete error type.
pub type AnyError = anyhow::Error;

/// Creates a new error with a caller-specified error class name and message.
pub fn custom_error(
  class: &'static str,
  message: impl Into<Cow<'static, str>>,
) -> AnyError {
  CustomError {
    class,
    message: message.into(),
  }
  .into()
}

pub fn generic_error(message: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("Error", message)
}

pub fn type_error(message: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("TypeError", message)
}

pub fn uri_error(message: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("URIError", message)
}

pub fn bad_resource(message: impl Into<Cow<'static, str>>) -> AnyError {
  custom_error("BadResource", message)
}

pub fn bad_resource_id() -> AnyError {
  custom_error("BadResource", "Bad resource ID")
}

pub fn not_supported() -> AnyError {
  custom_error("NotSupported", "The operation is not supported")
}

pub fn resource_unavailable() -> AnyError {
  custom_error(
    "Busy",
    "Resource is unavailable because it is in use by a promise",
  )
}

/// A simple error type that lets the creator specify both the error message and
/// the error class name. This type is private; externally it only ever appears
/// wrapped in an `AnyError`. To retrieve the error class name from a wrapped
/// `CustomError`, use the function `get_custom_error_class()`.
#[derive(Debug)]
struct CustomError {
  class: &'static str,
  message: Cow<'static, str>,
}

impl Display for CustomError {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for CustomError {}

/// If this error was crated with `custom_error()`, return the specified error
/// class name. In all other cases this function returns `None`.
pub fn get_custom_error_class(error: &AnyError) -> Option<&'static str> {
  error.downcast_ref::<CustomError>().map(|e| e.class)
}

/// A `JsError` represents an exception coming from V8, with stack frames and
/// line numbers. The deno_cli crate defines another `JsError` type, which wraps
/// the one defined here, that adds source map support and colorful formatting.
#[derive(Debug, PartialEq, Clone)]
pub struct JsError {
  pub message: String,
  pub source_line: Option<String>,
  pub script_resource_name: Option<String>,
  pub line_number: Option<i64>,
  pub start_column: Option<i64>, // 0-based
  pub end_column: Option<i64>,   // 0-based
  pub frames: Vec<JsStackFrame>,
  pub stack: Option<String>,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsStackFrame {
  pub type_name: Option<String>,
  pub function_name: Option<String>,
  pub method_name: Option<String>,
  pub file_name: Option<String>,
  pub line_number: Option<i64>,
  pub column_number: Option<i64>,
  pub eval_origin: Option<String>,
  // Warning! isToplevel has inconsistent snake<>camel case, "typo" originates in v8:
  // https://source.chromium.org/search?q=isToplevel&sq=&ss=chromium%2Fchromium%2Fsrc:v8%2F
  #[serde(rename = "isToplevel")]
  pub is_top_level: Option<bool>,
  pub is_eval: bool,
  pub is_native: bool,
  pub is_constructor: bool,
  pub is_async: bool,
  pub is_promise_all: bool,
  pub promise_index: Option<i64>,
}

impl JsStackFrame {
  pub fn from_location(
    file_name: Option<String>,
    line_number: Option<i64>,
    column_number: Option<i64>,
  ) -> Self {
    Self {
      type_name: None,
      function_name: None,
      method_name: None,
      file_name,
      line_number,
      column_number,
      eval_origin: None,
      is_top_level: None,
      is_eval: false,
      is_native: false,
      is_constructor: false,
      is_async: false,
      is_promise_all: false,
      promise_index: None,
    }
  }
}

fn get_property<'a>(
  scope: &mut v8::HandleScope<'a>,
  object: v8::Local<v8::Object>,
  key: &str,
) -> Option<v8::Local<'a, v8::Value>> {
  let key = v8::String::new(scope, key).unwrap();
  object.get(scope, key.into())
}

#[derive(serde::Deserialize)]
struct NativeJsError {
  name: Option<String>,
  message: Option<String>,
  stack: Option<String>,
}

impl JsError {
  pub(crate) fn create(js_error: Self) -> AnyError {
    js_error.into()
  }

  pub fn from_v8_exception(
    scope: &mut v8::HandleScope,
    exception: v8::Local<v8::Value>,
  ) -> Self {
    // Create a new HandleScope because we're creating a lot of new local
    // handles below.
    let scope = &mut v8::HandleScope::new(scope);

    let msg = v8::Exception::create_message(scope, exception);

    let (message, frames, stack) = if exception.is_native_error() {
      // The exception is a JS Error object.
      let exception: v8::Local<v8::Object> =
        exception.clone().try_into().unwrap();

      // Decode native JS error, internally this should access error.stack
      // ensuring that prepareStackTrace() has been called.
      // Which in turn should populate error.__callSiteEvals.
      let e: NativeJsError =
        serde_v8::from_v8(scope, exception.into()).unwrap();

      // Get the message by formatting error.name and error.message.
      let name = e.name.unwrap_or_else(|| "Error".to_string());
      let message_prop = e.message.unwrap_or_else(|| "".to_string());
      let message = if !name.is_empty() && !message_prop.is_empty() {
        format!("Uncaught {}: {}", name, message_prop)
      } else if !name.is_empty() {
        format!("Uncaught {}", name)
      } else if !message_prop.is_empty() {
        format!("Uncaught {}", message_prop)
      } else {
        "Uncaught".to_string()
      };

      // Read an array of structured frames from error.__callSiteEvals.
      let frames_v8 = get_property(scope, exception, "__callSiteEvals");

      // Convert them into Vec<JSStack>
      let frames: Vec<JsStackFrame> = match frames_v8 {
        Some(frames_v8) => serde_v8::from_v8(scope, frames_v8).unwrap(),
        None => vec![],
      };
      (message, frames, e.stack)
    } else {
      // The exception is not a JS Error object.
      // Get the message given by V8::Exception::create_message(), and provide
      // empty frames.
      (msg.get(scope).to_rust_string_lossy(scope), vec![], None)
    };

    Self {
      message,
      script_resource_name: msg
        .get_script_resource_name(scope)
        .and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
        .map(|v| v.to_rust_string_lossy(scope)),
      source_line: msg
        .get_source_line(scope)
        .map(|v| v.to_rust_string_lossy(scope)),
      line_number: msg.get_line_number(scope).and_then(|v| v.try_into().ok()),
      start_column: msg.get_start_column().try_into().ok(),
      end_column: msg.get_end_column().try_into().ok(),
      frames,
      stack,
    }
  }
}

impl Error for JsError {}

fn format_source_loc(
  file_name: &str,
  line_number: i64,
  column_number: i64,
) -> String {
  let line_number = line_number;
  let column_number = column_number;
  format!("{}:{}:{}", file_name, line_number, column_number)
}

impl Display for JsError {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    if let Some(stack) = &self.stack {
      let stack_lines = stack.lines();
      if stack_lines.count() > 1 {
        return write!(f, "{}", stack);
      }
    }

    write!(f, "{}", self.message)?;
    if let Some(script_resource_name) = &self.script_resource_name {
      if self.line_number.is_some() && self.start_column.is_some() {
        let source_loc = format_source_loc(
          script_resource_name,
          self.line_number.unwrap(),
          self.start_column.unwrap(),
        );
        write!(f, "\n    at {}", source_loc)?;
      }
    }
    Ok(())
  }
}

pub(crate) fn attach_handle_to_error(
  scope: &mut v8::Isolate,
  err: AnyError,
  handle: v8::Local<v8::Value>,
) -> AnyError {
  ErrWithV8Handle::new(scope, err, handle).into()
}

// TODO(piscisaureus): rusty_v8 should implement the Error trait on
// values of type v8::Global<T>.
pub(crate) struct ErrWithV8Handle {
  err: AnyError,
  handle: v8::Global<v8::Value>,
}

impl ErrWithV8Handle {
  pub fn new(
    scope: &mut v8::Isolate,
    err: AnyError,
    handle: v8::Local<v8::Value>,
  ) -> Self {
    let handle = v8::Global::new(scope, handle);
    Self { err, handle }
  }

  pub fn get_handle<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::Local<'s, v8::Value> {
    v8::Local::new(scope, &self.handle)
  }
}

unsafe impl Send for ErrWithV8Handle {}
unsafe impl Sync for ErrWithV8Handle {}

impl Error for ErrWithV8Handle {}

impl Display for ErrWithV8Handle {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    <AnyError as Display>::fmt(&self.err, f)
  }
}

impl Debug for ErrWithV8Handle {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    <Self as Display>::fmt(self, f)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_bad_resource() {
    let err = bad_resource("Resource has been closed");
    assert_eq!(err.to_string(), "Resource has been closed");
  }

  #[test]
  fn test_bad_resource_id() {
    let err = bad_resource_id();
    assert_eq!(err.to_string(), "Bad resource ID");
  }
}
