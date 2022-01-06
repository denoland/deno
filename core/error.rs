// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use anyhow::Error;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;

/// A generic wrapper that can encapsulate any concrete error type.
// TODO(ry) Deprecate AnyError and encourage deno_core::anyhow::Error instead.
pub type AnyError = anyhow::Error;

/// Creates a new error with a caller-specified error class name and message.
pub fn custom_error(
  class: &'static str,
  message: impl Into<Cow<'static, str>>,
) -> Error {
  CustomError {
    class,
    message: message.into(),
  }
  .into()
}

pub fn generic_error(message: impl Into<Cow<'static, str>>) -> Error {
  custom_error("Error", message)
}

pub fn type_error(message: impl Into<Cow<'static, str>>) -> Error {
  custom_error("TypeError", message)
}

pub fn range_error(message: impl Into<Cow<'static, str>>) -> Error {
  custom_error("RangeError", message)
}

pub fn invalid_hostname(hostname: &str) -> Error {
  type_error(format!("Invalid hostname: '{}'", hostname))
}

pub fn uri_error(message: impl Into<Cow<'static, str>>) -> Error {
  custom_error("URIError", message)
}

pub fn bad_resource(message: impl Into<Cow<'static, str>>) -> Error {
  custom_error("BadResource", message)
}

pub fn bad_resource_id() -> Error {
  custom_error("BadResource", "Bad resource ID")
}

pub fn not_supported() -> Error {
  custom_error("NotSupported", "The operation is not supported")
}

pub fn resource_unavailable() -> Error {
  custom_error(
    "Busy",
    "Resource is unavailable because it is in use by a promise",
  )
}

/// A simple error type that lets the creator specify both the error message and
/// the error class name. This type is private; externally it only ever appears
/// wrapped in an `anyhow::Error`. To retrieve the error class name from a wrapped
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

impl std::error::Error for CustomError {}

/// If this error was crated with `custom_error()`, return the specified error
/// class name. In all other cases this function returns `None`.
pub fn get_custom_error_class(error: &Error) -> Option<&'static str> {
  error.downcast_ref::<CustomError>().map(|e| e.class)
}

/// A `JsError` represents an exception coming from V8, with stack frames and
/// line numbers. The deno_cli crate defines another `JsError` type, which wraps
/// the one defined here, that adds source map support and colorful formatting.
#[derive(Debug, PartialEq, Clone)]
pub struct JsError {
  pub message: String,
  pub cause: Option<Box<JsError>>,
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
pub(crate) struct NativeJsError {
  pub name: Option<String>,
  pub message: Option<String>,
  // Warning! .stack is special so handled by itself
  // stack: Option<String>,
}

impl JsError {
  pub(crate) fn create(js_error: Self) -> Error {
    js_error.into()
  }

  pub fn from_v8_exception(
    scope: &mut v8::HandleScope,
    exception: v8::Local<v8::Value>,
  ) -> Self {
    Self::inner_from_v8_exception(scope, exception, Default::default())
  }

  fn inner_from_v8_exception<'a>(
    scope: &'a mut v8::HandleScope,
    exception: v8::Local<'a, v8::Value>,
    mut seen: HashSet<v8::Local<'a, v8::Value>>,
  ) -> Self {
    // Create a new HandleScope because we're creating a lot of new local
    // handles below.
    let scope = &mut v8::HandleScope::new(scope);

    let msg = v8::Exception::create_message(scope, exception);

    let (message, frames, stack, cause) =
      if is_instance_of_error(scope, exception) {
        // The exception is a JS Error object.
        let exception: v8::Local<v8::Object> = exception.try_into().unwrap();
        let cause = get_property(scope, exception, "cause");
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
        let cause = cause.and_then(|cause| {
          if cause.is_undefined() || seen.contains(&cause) {
            None
          } else {
            seen.insert(cause);
            Some(Box::new(JsError::inner_from_v8_exception(
              scope, cause, seen,
            )))
          }
        });

        // Access error.stack to ensure that prepareStackTrace() has been called.
        // This should populate error.__callSiteEvals.
        let stack = get_property(scope, exception, "stack");
        let stack: Option<v8::Local<v8::String>> =
          stack.and_then(|s| s.try_into().ok());
        let stack = stack.map(|s| s.to_rust_string_lossy(scope));

        // Read an array of structured frames from error.__callSiteEvals.
        let frames_v8 = get_property(scope, exception, "__callSiteEvals");
        // Ignore non-array values
        let frames_v8: Option<v8::Local<v8::Array>> =
          frames_v8.and_then(|a| a.try_into().ok());

        // Convert them into Vec<JsStackFrame>
        let frames: Vec<JsStackFrame> = match frames_v8 {
          Some(frames_v8) => {
            serde_v8::from_v8(scope, frames_v8.into()).unwrap()
          }
          None => vec![],
        };
        (message, frames, stack, cause)
      } else {
        // The exception is not a JS Error object.
        // Get the message given by V8::Exception::create_message(), and provide
        // empty frames.
        (
          msg.get(scope).to_rust_string_lossy(scope),
          vec![],
          None,
          None,
        )
      };

    Self {
      message,
      cause,
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

impl std::error::Error for JsError {}

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
  err: Error,
  handle: v8::Local<v8::Value>,
) -> Error {
  ErrWithV8Handle::new(scope, err, handle).into()
}

/// Implements `value instanceof primordials.Error` in JS. Similar to
/// `Value::is_native_error()` but more closely matches the semantics
/// of `instanceof`. `Value::is_native_error()` also checks for static class
/// inheritance rather than just scanning the prototype chain, which doesn't
/// work with our WebIDL implementation of `DOMException`.
pub(crate) fn is_instance_of_error<'s>(
  scope: &mut v8::HandleScope<'s>,
  value: v8::Local<v8::Value>,
) -> bool {
  if !value.is_object() {
    return false;
  }
  let message = v8::String::empty(scope);
  let error_prototype = v8::Exception::error(scope, message)
    .to_object(scope)
    .unwrap()
    .get_prototype(scope)
    .unwrap();
  let mut maybe_prototype =
    value.to_object(scope).unwrap().get_prototype(scope);
  while let Some(prototype) = maybe_prototype {
    if prototype.strict_equals(error_prototype) {
      return true;
    }
    maybe_prototype = prototype
      .to_object(scope)
      .and_then(|o| o.get_prototype(scope));
  }
  false
}

// TODO(piscisaureus): rusty_v8 should implement the Error trait on
// values of type v8::Global<T>.
pub(crate) struct ErrWithV8Handle {
  err: Error,
  handle: v8::Global<v8::Value>,
}

impl ErrWithV8Handle {
  pub fn new(
    scope: &mut v8::Isolate,
    err: Error,
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

impl std::error::Error for ErrWithV8Handle {}

impl Display for ErrWithV8Handle {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    <Error as Display>::fmt(&self.err, f)
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
