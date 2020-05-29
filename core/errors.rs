// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use rusty_v8 as v8;
use std::any::Any;
use std::any::TypeId;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

// The Send and Sync traits are required because deno is multithreaded and we
// need to be able to handle errors across threads.
pub trait AnyError: Any + Error + Send + Sync + 'static {}
impl<T> AnyError for T where T: Any + Error + Send + Sync + Sized + 'static {}

#[derive(Debug)]
pub struct ErrBox(Box<dyn AnyError>);

impl dyn AnyError {
  pub fn downcast_ref<T: AnyError>(&self) -> Option<&T> {
    if Any::type_id(self) == TypeId::of::<T>() {
      let target = self as *const Self as *const T;
      let target = unsafe { &*target };
      Some(target)
    } else {
      None
    }
  }
}

impl ErrBox {
  pub fn downcast<T: AnyError>(self) -> Result<T, Self> {
    if Any::type_id(&*self.0) == TypeId::of::<T>() {
      let target = Box::into_raw(self.0) as *mut T;
      let target = unsafe { Box::from_raw(target) };
      Ok(*target)
    } else {
      Err(self)
    }
  }
}

impl AsRef<dyn AnyError> for ErrBox {
  fn as_ref(&self) -> &dyn AnyError {
    self.0.as_ref()
  }
}

impl Deref for ErrBox {
  type Target = Box<dyn AnyError>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: AnyError> From<T> for ErrBox {
  fn from(error: T) -> Self {
    Self(Box::new(error))
  }
}

impl From<Box<dyn AnyError>> for ErrBox {
  fn from(boxed: Box<dyn AnyError>) -> Self {
    Self(boxed)
  }
}

impl fmt::Display for ErrBox {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

/// A `JSError` represents an exception coming from V8, with stack frames and
/// line numbers. The deno_cli crate defines another `JSError` type, which wraps
/// the one defined here, that adds source map support and colorful formatting.
#[derive(Debug, PartialEq, Clone)]
pub struct JSError {
  pub message: String,
  pub source_line: Option<String>,
  pub script_resource_name: Option<String>,
  pub line_number: Option<i64>,
  pub start_column: Option<i64>, // 0-based
  pub end_column: Option<i64>,   // 0-based
  pub frames: Vec<JSStackFrame>,
  pub formatted_frames: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct JSStackFrame {
  pub type_name: Option<String>,
  pub function_name: Option<String>,
  pub method_name: Option<String>,
  pub file_name: Option<String>,
  pub line_number: Option<i64>,
  pub column_number: Option<i64>,
  pub eval_origin: Option<String>,
  pub is_top_level: Option<bool>,
  pub is_eval: bool,
  pub is_native: bool,
  pub is_constructor: bool,
  pub is_async: bool,
  pub is_promise_all: bool,
  pub promise_index: Option<i64>,
}

fn get_property<'a>(
  scope: &mut impl v8::ToLocal<'a>,
  context: v8::Local<v8::Context>,
  object: v8::Local<v8::Object>,
  key: &str,
) -> Option<v8::Local<'a, v8::Value>> {
  let key = v8::String::new(scope, key).unwrap();
  object.get(scope, context, key.into())
}

impl JSError {
  pub(crate) fn create(js_error: Self) -> ErrBox {
    ErrBox::from(js_error)
  }

  pub fn from_v8_exception(
    scope: &mut impl v8::InIsolate,
    exception: v8::Local<v8::Value>,
  ) -> Self {
    // Create a new HandleScope because we're creating a lot of new local
    // handles below.
    let mut hs = v8::HandleScope::new(scope);
    let scope = hs.enter();
    let context = { scope.get_current_context().unwrap() };

    let msg = v8::Exception::create_message(scope, exception);

    let (message, frames, formatted_frames) = if exception.is_native_error() {
      // The exception is a JS Error object.
      let exception: v8::Local<v8::Object> =
        exception.clone().try_into().unwrap();

      // Get the message by formatting error.name and error.message.
      let name = get_property(scope, context, exception, "name")
        .and_then(|m| m.to_string(scope))
        .map(|s| s.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "undefined".to_string());
      let message_prop = get_property(scope, context, exception, "message")
        .and_then(|m| m.to_string(scope))
        .map(|s| s.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "undefined".to_string());
      let message = format!("Uncaught {}: {}", name, message_prop);

      // Access error.stack to ensure that prepareStackTrace() has been called.
      // This should populate error.__callSiteEvals and error.__formattedFrames.
      let _ = get_property(scope, context, exception, "stack");

      // Read an array of structured frames from error.__callSiteEvals.
      let frames_v8 =
        get_property(scope, context, exception, "__callSiteEvals");
      let frames_v8: Option<v8::Local<v8::Array>> =
        frames_v8.and_then(|a| a.try_into().ok());

      // Read an array of pre-formatted frames from error.__formattedFrames.
      let formatted_frames_v8 =
        get_property(scope, context, exception, "__formattedFrames");
      let formatted_frames_v8: Option<v8::Local<v8::Array>> =
        formatted_frames_v8.and_then(|a| a.try_into().ok());

      // Convert them into Vec<JSStack> and Vec<String> respectively.
      let mut frames: Vec<JSStackFrame> = vec![];
      let mut formatted_frames: Vec<String> = vec![];
      if let (Some(frames_v8), Some(formatted_frames_v8)) =
        (frames_v8, formatted_frames_v8)
      {
        for i in 0..frames_v8.length() {
          let call_site: v8::Local<v8::Object> = frames_v8
            .get_index(scope, context, i)
            .unwrap()
            .try_into()
            .unwrap();
          let type_name: Option<v8::Local<v8::String>> =
            get_property(scope, context, call_site, "typeName")
              .unwrap()
              .try_into()
              .ok();
          let type_name = type_name.map(|s| s.to_rust_string_lossy(scope));
          let function_name: Option<v8::Local<v8::String>> =
            get_property(scope, context, call_site, "functionName")
              .unwrap()
              .try_into()
              .ok();
          let function_name =
            function_name.map(|s| s.to_rust_string_lossy(scope));
          let method_name: Option<v8::Local<v8::String>> =
            get_property(scope, context, call_site, "methodName")
              .unwrap()
              .try_into()
              .ok();
          let method_name = method_name.map(|s| s.to_rust_string_lossy(scope));
          let file_name: Option<v8::Local<v8::String>> =
            get_property(scope, context, call_site, "fileName")
              .unwrap()
              .try_into()
              .ok();
          let file_name = file_name.map(|s| s.to_rust_string_lossy(scope));
          let line_number: Option<v8::Local<v8::Integer>> =
            get_property(scope, context, call_site, "lineNumber")
              .unwrap()
              .try_into()
              .ok();
          let line_number = line_number.map(|n| n.value());
          let column_number: Option<v8::Local<v8::Integer>> =
            get_property(scope, context, call_site, "columnNumber")
              .unwrap()
              .try_into()
              .ok();
          let column_number = column_number.map(|n| n.value());
          let eval_origin: Option<v8::Local<v8::String>> =
            get_property(scope, context, call_site, "evalOrigin")
              .unwrap()
              .try_into()
              .ok();
          let eval_origin = eval_origin.map(|s| s.to_rust_string_lossy(scope));
          let is_top_level: Option<v8::Local<v8::Boolean>> =
            get_property(scope, context, call_site, "isTopLevel")
              .unwrap()
              .try_into()
              .ok();
          let is_top_level = is_top_level.map(|b| b.is_true());
          let is_eval: v8::Local<v8::Boolean> =
            get_property(scope, context, call_site, "isEval")
              .unwrap()
              .try_into()
              .unwrap();
          let is_eval = is_eval.is_true();
          let is_native: v8::Local<v8::Boolean> =
            get_property(scope, context, call_site, "isNative")
              .unwrap()
              .try_into()
              .unwrap();
          let is_native = is_native.is_true();
          let is_constructor: v8::Local<v8::Boolean> =
            get_property(scope, context, call_site, "isConstructor")
              .unwrap()
              .try_into()
              .unwrap();
          let is_constructor = is_constructor.is_true();
          let is_async: v8::Local<v8::Boolean> =
            get_property(scope, context, call_site, "isAsync")
              .unwrap()
              .try_into()
              .unwrap();
          let is_async = is_async.is_true();
          let is_promise_all: v8::Local<v8::Boolean> =
            get_property(scope, context, call_site, "isPromiseAll")
              .unwrap()
              .try_into()
              .unwrap();
          let is_promise_all = is_promise_all.is_true();
          let promise_index: Option<v8::Local<v8::Integer>> =
            get_property(scope, context, call_site, "columnNumber")
              .unwrap()
              .try_into()
              .ok();
          let promise_index = promise_index.map(|n| n.value());
          frames.push(JSStackFrame {
            type_name,
            function_name,
            method_name,
            file_name,
            line_number,
            column_number,
            eval_origin,
            is_top_level,
            is_eval,
            is_native,
            is_constructor,
            is_async,
            is_promise_all,
            promise_index,
          });
          let formatted_frame: v8::Local<v8::String> = formatted_frames_v8
            .get_index(scope, context, i)
            .unwrap()
            .try_into()
            .unwrap();
          let formatted_frame = formatted_frame.to_rust_string_lossy(scope);
          formatted_frames.push(formatted_frame)
        }
      }
      (message, frames, formatted_frames)
    } else {
      // The exception is not a JS Error object.
      // Get the message given by V8::Exception::create_message(), and provide
      // empty frames.
      (msg.get(scope).to_rust_string_lossy(scope), vec![], vec![])
    };

    Self {
      message,
      script_resource_name: msg
        .get_script_resource_name(scope)
        .and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
        .map(|v| v.to_rust_string_lossy(scope)),
      source_line: msg
        .get_source_line(scope, context)
        .map(|v| v.to_rust_string_lossy(scope)),
      line_number: msg.get_line_number(context).and_then(|v| v.try_into().ok()),
      start_column: msg.get_start_column().try_into().ok(),
      end_column: msg.get_end_column().try_into().ok(),
      frames,
      formatted_frames,
    }
  }
}

impl Error for JSError {}

fn format_source_loc(
  file_name: &str,
  line_number: i64,
  column_number: i64,
) -> String {
  let line_number = line_number;
  let column_number = column_number;
  format!("{}:{}:{}", file_name, line_number, column_number)
}

impl fmt::Display for JSError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if let Some(script_resource_name) = &self.script_resource_name {
      if self.line_number.is_some() && self.start_column.is_some() {
        assert!(self.line_number.is_some());
        assert!(self.start_column.is_some());
        let source_loc = format_source_loc(
          script_resource_name,
          self.line_number.unwrap(),
          self.start_column.unwrap(),
        );
        write!(f, "{}", source_loc)?;
      }
      if self.source_line.is_some() {
        let source_line = self.source_line.as_ref().unwrap();
        write!(f, "\n{}\n", source_line)?;
        let mut s = String::new();
        for i in 0..self.end_column.unwrap() {
          if i >= self.start_column.unwrap() {
            s.push('^');
          } else if source_line.chars().nth(i as usize).unwrap() == '\t' {
            s.push('\t');
          } else {
            s.push(' ');
          }
        }
        writeln!(f, "{}", s)?;
      }
    }

    write!(f, "{}", self.message)?;

    for formatted_frame in &self.formatted_frames {
      // TODO: Strip ANSI color from formatted_frame.
      write!(f, "\n    at {}", formatted_frame)?;
    }
    Ok(())
  }
}

pub(crate) fn attach_handle_to_error(
  scope: &mut impl v8::InIsolate,
  err: ErrBox,
  handle: v8::Local<v8::Value>,
) -> ErrBox {
  ErrWithV8Handle::new(scope, err, handle).into()
}

// TODO(piscisaureus): rusty_v8 should implement the Error trait on
// values of type v8::Global<T>.
pub struct ErrWithV8Handle {
  err: ErrBox,
  handle: v8::Global<v8::Value>,
}

impl ErrWithV8Handle {
  pub fn new(
    scope: &mut impl v8::InIsolate,
    err: ErrBox,
    handle: v8::Local<v8::Value>,
  ) -> Self {
    let handle = v8::Global::new_from(scope, handle);
    Self { err, handle }
  }

  pub fn get_handle(&self) -> &v8::Global<v8::Value> {
    &self.handle
  }
}

unsafe impl Send for ErrWithV8Handle {}
unsafe impl Sync for ErrWithV8Handle {}

impl Error for ErrWithV8Handle {}

impl fmt::Display for ErrWithV8Handle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.err.fmt(f)
  }
}

impl fmt::Debug for ErrWithV8Handle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.err.fmt(f)
  }
}
