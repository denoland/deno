// Copyright 2018-2025 the Deno authors. MIT license.

pub use super::modules::ModuleConcreteError;
use crate::FastStaticString;
pub use crate::io::ResourceError;
pub use crate::modules::ModuleLoaderError;
use crate::runtime::JsRealm;
use crate::runtime::JsRuntime;
use crate::runtime::v8_static_strings;
use crate::source_map::SourceMapApplication;
use crate::url::Url;
use boxed_error::Boxed;
use deno_error::JsError;
use deno_error::JsErrorClass;
use deno_error::PropertyValue;
use deno_error::builtin_classes::*;
use std::borrow::Cow;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write as _;
use std::sync::Arc;
use thiserror::Error;

/// A generic wrapper that can encapsulate any concrete error type.
// TODO(ry) Deprecate AnyError and encourage deno_core::anyhow::Error instead.
pub type AnyError = anyhow::Error;

deno_error::js_error_wrapper!(v8::DataError, DataError, TYPE_ERROR);

impl PartialEq<DataError> for DataError {
  fn eq(&self, other: &DataError) -> bool {
    match (self.0, other.0) {
      (
        v8::DataError::BadType { actual, expected },
        v8::DataError::BadType {
          actual: other_actual,
          expected: other_expected,
        },
      ) => actual == other_actual && expected == other_expected,
      (
        v8::DataError::NoData { expected },
        v8::DataError::NoData {
          expected: other_expected,
        },
      ) => expected == other_expected,
      _ => false,
    }
  }
}

impl Eq for DataError {}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed to parse {0}")]
pub struct CoreModuleParseError(pub FastStaticString);

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed to execute {0}")]
pub struct CoreModuleExecuteError(pub FastStaticString);

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Unable to get code cache from unbound module script for {0}")]
pub struct CreateCodeCacheError(pub Url);

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "Extensions from snapshot loaded in wrong order: expected {} but got {}", .expected, .actual
)]
pub struct ExtensionSnapshotMismatchError {
  pub expected: &'static str,
  pub actual: &'static str,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "Number of lazy-initialized extensions ({}) does not match number of arguments ({})", .lazy_init_extensions_len, .arguments_len
)]
pub struct ExtensionLazyInitCountMismatchError {
  pub lazy_init_extensions_len: usize,
  pub arguments_len: usize,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error(
  "Lazy-initialized extensions loaded in wrong order: expected {} but got {}", .expected, .actual
)]
pub struct ExtensionLazyInitOrderMismatchError {
  pub expected: &'static str,
  pub actual: &'static str,
}

#[derive(Debug, Boxed, JsError)]
pub struct CoreError(pub Box<CoreErrorKind>);

#[derive(Debug, thiserror::Error, JsError)]
pub enum CoreErrorKind {
  #[class(generic)]
  #[error("Top-level await is not allowed in synchronous evaluation")]
  TLA,
  #[class(inherit)]
  #[error(transparent)]
  Js(#[from] Box<JsError>),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(transparent)]
  ExtensionTranspiler(deno_error::JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Parse(#[from] CoreModuleParseError),
  #[class(inherit)]
  #[error(transparent)]
  Execute(#[from] CoreModuleExecuteError),
  #[class(generic)]
  #[error(
    "Following modules were passed to ExtModuleLoader but never used:\n{}",
    .0.iter().map(|s| format!("  - {}\n", s)).collect::<Vec<_>>().join("")
  )]
  UnusedModules(Vec<String>),
  #[class(generic)]
  #[error(
    "Following modules were not evaluated; make sure they are imported from other code:\n{}",
    .0.iter().map(|s| format!("  - {}\n", s)).collect::<Vec<_>>().join("")
  )]
  NonEvaluatedModules(Vec<String>),
  #[class(generic)]
  #[error("{0} not present in the module map")]
  MissingFromModuleMap(String),
  #[class(generic)]
  #[error("Could not execute {specifier}")]
  CouldNotExecute {
    #[source]
    error: Box<Self>,
    specifier: String,
  },
  #[class(inherit)]
  #[error(transparent)]
  JsBox(#[from] deno_error::JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  Url(#[from] url::ParseError),
  #[class(generic)]
  #[error(
    "Cannot evaluate module, because JavaScript execution has been terminated"
  )]
  ExecutionTerminated,
  #[class(generic)]
  #[error(
    "Promise resolution is still pending but the event loop has already resolved"
  )]
  PendingPromiseResolution,
  #[class(generic)]
  #[error(
    "Cannot evaluate dynamically imported module, because JavaScript execution has been terminated"
  )]
  EvaluateDynamicImportedModule,
  #[class(inherit)]
  #[error(transparent)]
  Module(ModuleConcreteError),
  #[class(inherit)]
  #[error(transparent)]
  Data(DataError),
  #[class(inherit)]
  #[error(transparent)]
  CreateCodeCache(#[from] CreateCodeCacheError),
  #[class(inherit)]
  #[error(transparent)]
  ExtensionSnapshotMismatch(ExtensionSnapshotMismatchError),
  #[class(inherit)]
  #[error(transparent)]
  ExtensionLazyInitCountMismatch(ExtensionLazyInitCountMismatchError),
  #[class(inherit)]
  #[error(transparent)]
  ExtensionLazyInitOrderMismatch(ExtensionLazyInitOrderMismatchError),
}

impl CoreError {
  pub fn print_with_cause(&self) -> String {
    use std::error::Error;
    let mut err_message = self.to_string();

    if let Some(source) = self.source() {
      err_message.push_str(&format!(
        "\n\nCaused by:\n    {}",
        source.to_string().replace("\n", "\n    ")
      ));
    }

    err_message
  }

  pub fn to_v8_error(&self, scope: &mut v8::PinScope) -> v8::Global<v8::Value> {
    self.as_kind().to_v8_error(scope)
  }
}

impl CoreErrorKind {
  pub fn to_v8_error(&self, scope: &mut v8::PinScope) -> v8::Global<v8::Value> {
    let err_string = self.get_message().to_string();
    let mut error_chain = vec![];
    let mut intermediary_error: Option<&dyn Error> = Some(&self);

    while let Some(err) = intermediary_error {
      if let Some(source) = err.source() {
        let source_str = source.to_string();
        if source_str != err_string {
          error_chain.push(source_str);
        }

        intermediary_error = Some(source);
      } else {
        intermediary_error = None;
      }
    }

    let message = if !error_chain.is_empty() {
      format!(
        "{}\n  Caused by:\n    {}",
        err_string,
        error_chain.join("\n    ")
      )
    } else {
      err_string
    };

    let exception =
      js_class_and_message_to_exception(scope, &self.get_class(), &message);
    v8::Global::new(scope, exception)
  }
}

impl From<v8::DataError> for CoreError {
  fn from(err: v8::DataError) -> Self {
    CoreErrorKind::Data(DataError(err)).into_box()
  }
}

pub fn throw_js_error_class(
  scope: &mut v8::PinScope,
  error: &dyn JsErrorClass,
) {
  let exception = js_class_and_message_to_exception(
    scope,
    &error.get_class(),
    &error.get_message(),
  );
  scope.throw_exception(exception);
}

fn js_class_and_message_to_exception<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  _class: &str,
  message: &str,
) -> v8::Local<'s, v8::Value> {
  let message = v8::String::new(scope, message).unwrap();
  /*
  commented out since this was previously only handling type errors, but this
  change is breaking CLI, so visiting on a later date

  match class {
    TYPE_ERROR => v8::Exception::type_error(scope, message),
    RANGE_ERROR => v8::Exception::range_error(scope, message),
    REFERENCE_ERROR => v8::Exception::reference_error(scope, message),
    SYNTAX_ERROR => v8::Exception::syntax_error(scope, message),
    _ => v8::Exception::error(scope, message),
  }*/
  v8::Exception::type_error(scope, message)
}

pub fn to_v8_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: &dyn JsErrorClass,
) -> v8::Local<'s, v8::Value> {
  v8::tc_scope!(let tc_scope, scope);

  let cb = JsRealm::exception_state_from_scope(tc_scope)
    .js_build_custom_error_cb
    .borrow()
    .clone()
    .expect("Custom error builder must be set");
  let cb = cb.open(tc_scope);
  let this = v8::undefined(tc_scope).into();
  let class = v8::String::new(tc_scope, &error.get_class()).unwrap();
  let message = v8::String::new(tc_scope, &error.get_message()).unwrap();
  let mut args = vec![class.into(), message.into()];

  let additional_properties = error
    .get_additional_properties()
    .map(|(key, value)| {
      let key = v8::String::new(tc_scope, &key).unwrap().into();
      let value = match value {
        PropertyValue::String(value) => {
          v8::String::new(tc_scope, &value).unwrap().into()
        }
        PropertyValue::Number(value) => v8::Number::new(tc_scope, value).into(),
      };

      v8::Array::new_with_elements(tc_scope, &[key, value]).into()
    })
    .collect::<Vec<_>>();

  if !additional_properties.is_empty() {
    args.push(
      v8::Array::new_with_elements(tc_scope, &additional_properties).into(),
    );
  }

  let maybe_exception = cb.call(tc_scope, this, &args);

  match maybe_exception {
    Some(exception) => exception,
    None => {
      let mut msg =
        "Custom error class must have a builder registered".to_string();
      if tc_scope.has_caught() {
        let e = tc_scope.exception().unwrap();
        // If the builder threw a bare `null`/`undefined`, propagate the
        // original message instead of panicking on an opaque "Uncaught null".
        if e.is_null_or_undefined() {
          return message.into();
        }
        let js_error = JsError::from_v8_exception(tc_scope, e);
        msg = format!("{}: {}", msg, js_error.exception_message);
      }
      panic!("{}", msg);
    }
  }
}

/// Effectively throw an uncatchable error. This will terminate runtime
/// execution before any more JS code can run, except in the REPL where it
/// should just output the error to the console.
pub fn dispatch_exception<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  exception: v8::Local<'s, v8::Value>,
  promise: bool,
) {
  let state = JsRuntime::state_from(scope);
  if let Some(true) = state.with_inspector(|inspector| {
    inspector.exception_thrown(scope, exception, false);
    inspector.is_dispatching_message()
  }) {
    // This indicates that the fn is being called from a REPL. Skip termination.
    return;
  }

  JsRealm::exception_state_from_scope(scope)
    .set_dispatched_exception(v8::Global::new(scope, exception), promise);
  scope.terminate_execution();
}

#[inline(always)]
pub(crate) fn call_site_evals_key<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Private> {
  let name = v8_static_strings::CALL_SITE_EVALS.v8_string(scope).unwrap();
  v8::Private::for_api(scope, Some(name))
}

/// A `JsError` represents an exception coming from V8, with stack frames and
/// line numbers. The deno_cli crate defines another `JsError` type, which wraps
/// the one defined here, that adds source map support and colorful formatting.
/// When updating this struct, also update errors_are_equal_without_cause() in
/// fmt_error.rs.
#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsError {
  pub name: Option<String>,
  pub message: Option<String>,
  pub stack: Option<String>,
  pub cause: Option<Box<JsError>>,
  pub exception_message: String,
  pub frames: Vec<JsStackFrame>,
  pub source_line: Option<String>,
  pub source_line_frame_index: Option<usize>,
  pub aggregated: Option<Vec<JsError>>,
  pub additional_properties: Vec<(String, String)>,
}

impl JsErrorClass for JsError {
  fn get_class(&self) -> Cow<'static, str> {
    if let Some(name) = &self.name {
      Cow::Owned(name.clone())
    } else {
      Cow::Borrowed(GENERIC_ERROR)
    }
  }

  fn get_message(&self) -> Cow<'static, str> {
    if let Some(message) = &self.message {
      Cow::Owned(message.clone())
    } else {
      Cow::Borrowed("")
    }
  }

  fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
    Box::new(
      self
        .additional_properties
        // todo(dsherret): why does JsErrorClass not allow having references within this struct?
        .clone()
        .into_iter()
        .map(|(k, v)| {
          (
            Cow::Owned(k.to_string()),
            PropertyValue::String(Cow::Owned(v.to_string())),
          )
        }),
    )
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug, Eq, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
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
  pub is_wasm: bool,
  pub promise_index: Option<i64>,
}

/// Applies source map to the given location
fn apply_source_map<'a>(
  source_mapper: &mut crate::source_map::SourceMapper,
  file_name: Cow<'a, str>,
  line_number: i64,
  column_number: i64,
) -> (Cow<'a, str>, i64, i64) {
  match source_mapper.apply_source_map(
    &file_name,
    line_number as u32,
    column_number as u32,
  ) {
    SourceMapApplication::Unchanged => (file_name, line_number, column_number),
    SourceMapApplication::LineAndColumn {
      line_number,
      column_number,
    } => (file_name, line_number.into(), column_number.into()),
    SourceMapApplication::LineAndColumnAndFileName {
      file_name,
      line_number,
      column_number,
    } => (file_name.into(), line_number.into(), column_number.into()),
  }
}

/// Parses an eval origin string from V8, returning
/// the contents before the location,
/// (the file name, line number, and column number), and
/// the contents after the location.
///
/// # Example
/// ```ignore
/// assert_eq!(
///   parse_eval_origin("eval at foo (bar at (file://a.ts:1:2))"),
///   Some(("eval at foo (bar at (", ("file://a.ts", 1, 2), "))")),
/// );
/// ```
///
fn parse_eval_origin(
  eval_origin: &str,
) -> Option<(&str, (&str, i64, i64), &str)> {
  // The eval origin string we get from V8 looks like
  // `eval at ${function_name} (${origin})`
  // where origin can be either a file name, like
  // "eval at foo (file:///path/to/script.ts:1:2)"
  // or a nested eval origin, like
  // "eval at foo (eval at bar (file:///path/to/script.ts:1:2))"
  //
  let eval_at = "eval at ";
  // only the innermost eval origin can have location info, so find the last
  // "eval at", then continue parsing the rest of the string
  let mut innermost_start = eval_origin.rfind(eval_at)? + eval_at.len();
  // skip over the function name
  innermost_start += eval_origin[innermost_start..].find('(')? + 1;
  if innermost_start >= eval_origin.len() {
    // malformed
    return None;
  }

  // from the right, split by ":" to get the column number, line number, file name
  // (in that order, since we're iterating from the right). e.g.
  // eval at foo (eval at bar (file://foo.ts:1:2))
  //                           ^^^^^^^^^^^^^ ^ ^^^
  let mut parts = eval_origin[innermost_start..].rsplitn(3, ':');
  // the part with the column number will include extra stuff, the actual number ends at
  // the closing paren
  let column_number_with_rest = parts.next()?;
  let column_number_end = column_number_with_rest.find(')')?;
  let column_number = column_number_with_rest[..column_number_end]
    .parse::<i64>()
    .ok()?;
  let line_number = parts.next()?.parse::<i64>().ok()?;
  let file_name = parts.next()?;
  // The column number starts after the last occurring ":".
  let column_start = eval_origin.rfind(':')? + 1;
  // the innermost origin ends at the end of the column number
  let innermost_end = column_start + column_number_end;
  Some((
    &eval_origin[..innermost_start],
    (file_name, line_number, column_number),
    &eval_origin[innermost_end..],
  ))
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
      is_wasm: false,
      promise_index: None,
    }
  }

  /// Creates a `JsStackFrame` from a `CallSite`` JS object,
  /// provided by V8.
  fn from_callsite_object<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    callsite: v8::Local<'s, v8::Object>,
  ) -> Option<Self> {
    macro_rules! call {
      ($key: ident : $t: ty) => {{
        let res = call_method(scope, callsite, $key, &[])?;
        let res: $t = match serde_v8::from_v8(scope, res) {
          Ok(res) => res,
          Err(err) => {
            let message = format!(
              "Failed to deserialize return value from callsite property '{}' to correct type: {err:?}.",
              $key
            );
            let message = v8::String::new(scope, &message).unwrap();
            let exception = v8::Exception::type_error(scope, message);
            scope.throw_exception(exception);
            return None;
          }
        };
        res
      }};
      ($key: ident) => { call!($key : _) };
    }

    let raw_file_name = call!(GET_FILE_NAME : Option<String>);
    let is_wasm = raw_file_name
      .as_deref()
      .map(|f| f.starts_with("wasm://"))
      .unwrap_or(false);

    // For wasm frames, skip source map application — source maps don't apply to wasm.
    // V8 returns the function index via getLineNumber() and byte offset via getColumnNumber().
    let (file_name, line_number, column_number) = if is_wasm {
      (
        raw_file_name,
        call!(GET_LINE_NUMBER),
        call!(GET_COLUMN_NUMBER),
      )
    } else {
      let state = JsRuntime::state_from(scope);
      let mut source_mapper = state.source_mapper.borrow_mut();
      // apply source map
      match (
        raw_file_name,
        call!(GET_LINE_NUMBER),
        call!(GET_COLUMN_NUMBER),
      ) {
        (Some(f), Some(l), Some(c)) => {
          let (file_name, line_num, col_num) =
            apply_source_map(&mut source_mapper, f.into(), l, c);
          (Some(file_name.into_owned()), Some(line_num), Some(col_num))
        }
        (f, l, c) => (f, l, c),
      }
    };

    // apply source map to the eval origin, if the error originates from `eval`ed code
    let eval_origin = if is_wasm {
      None
    } else {
      call!(GET_EVAL_ORIGIN: Option<String>).and_then(|o| {
        let Some((before, (file, line, col), after)) = parse_eval_origin(&o)
        else {
          return Some(o);
        };
        let state = JsRuntime::state_from(scope);
        let mut source_mapper = state.source_mapper.borrow_mut();
        let (file, line, col) =
          apply_source_map(&mut source_mapper, file.into(), line, col);
        Some(format!("{before}{file}:{line}:{col}{after}"))
      })
    };

    Some(Self {
      file_name,
      line_number,
      column_number,
      eval_origin,
      type_name: call!(GET_TYPE_NAME),
      function_name: call!(GET_FUNCTION_NAME),
      method_name: call!(GET_METHOD_NAME),
      is_top_level: call!(IS_TOPLEVEL),
      is_eval: call!(IS_EVAL),
      is_native: call!(IS_NATIVE),
      is_constructor: call!(IS_CONSTRUCTOR),
      is_async: call!(IS_ASYNC),
      is_promise_all: call!(IS_PROMISE_ALL),
      is_wasm,
      promise_index: call!(GET_PROMISE_INDEX),
    })
  }

  /// Gets the source mapped stack frame corresponding to the
  /// (script_resource_name, line_number, column_number) from a v8 message.
  /// For non-syntax errors, it should also correspond to the first stack frame.
  pub fn from_v8_message<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    message: v8::Local<'s, v8::Message>,
  ) -> Option<Self> {
    let f = message.get_script_resource_name(scope)?;
    let f: v8::Local<v8::String> = f.try_into().ok()?;
    let f = f.to_rust_string_lossy(scope);
    let l = message.get_line_number(scope)? as i64;
    // V8's column numbers are 0-based, we want 1-based.
    let c = message.get_start_column() as i64 + 1;
    let state = JsRuntime::state_from(scope);
    let mut source_mapper = state.source_mapper.borrow_mut();
    let (file_name, line_num, col_num) =
      apply_source_map(&mut source_mapper, f.into(), l, c);
    Some(JsStackFrame::from_location(
      Some(file_name.into_owned()),
      Some(line_num),
      Some(col_num),
    ))
  }

  pub fn maybe_format_location(&self) -> Option<String> {
    Some(format!(
      "{}:{}:{}",
      self.file_name.as_ref()?,
      self.line_number?,
      self.column_number?
    ))
  }
}

#[inline(always)]
fn get_property<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  object: v8::Local<'s, v8::Object>,
  key: FastStaticString,
) -> Option<v8::Local<'s, v8::Value>> {
  let key = key.v8_string(scope).unwrap();
  object.get(scope, key.into())
}

fn call_method<'s, 'i, T>(
  scope: &mut v8::PinScope<'s, 'i>,
  object: v8::Local<'s, v8::Object>,
  key: FastStaticString,
  args: &[v8::Local<'s, v8::Value>],
) -> Option<v8::Local<'s, T>>
where
  v8::Local<'s, T>: TryFrom<v8::Local<'s, v8::Value>, Error: Debug>,
{
  let func = match get_property(scope, object, key)?.try_cast::<v8::Function>()
  {
    Ok(func) => func,
    Err(err) => {
      let message =
        format!("Callsite property '{key}' is not a function: {err}");
      let message = v8::String::new(scope, &message).unwrap();
      let exception = v8::Exception::type_error(scope, message);
      scope.throw_exception(exception);
      return None;
    }
  };

  let res = func.call(scope, object.into(), args)?;

  let result = match v8::Local::try_from(res) {
    Ok(result) => result,
    Err(err) => {
      let message = format!(
        "Failed to cast callsite method '{key}' return value to correct value: {err:?}."
      );
      let message = v8::String::new(scope, &message).unwrap();
      let exception = v8::Exception::type_error(scope, message);
      scope.throw_exception(exception);
      return None;
    }
  };

  Some(result)
}

#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct NativeJsError {
  pub name: Option<String>,
  pub message: Option<String>,
  // Warning! .stack is special so handled by itself
  // stack: Option<String>,
}

impl JsError {
  /// Compares all properties of JsError, except for JsError::cause. This function is used to
  /// detect that 2 JsError objects in a JsError::cause chain are identical, ie. there is a recursive cause.
  ///
  /// We don't have access to object identity here, so we do it via field comparison. Ideally this should
  /// be able to maintain object identity somehow.
  pub fn is_same_error(&self, other: &JsError) -> bool {
    let a = self;
    let b = other;
    // `a.cause == b.cause` omitted, because it is absent in recursive errors,
    // despite the error being identical to a previously seen one.
    a.name == b.name
      && a.message == b.message
      && a.stack == b.stack
      // TODO(mmastrac): we need consistency around when we insert "in promise" and when we don't. For now, we
      // are going to manually replace this part of the string.
      && (a.exception_message == b.exception_message
      || a.exception_message.replace(" (in promise) ", " ") == b.exception_message.replace(" (in promise) ", " "))
      && a.frames == b.frames
      && a.source_line == b.source_line
      && a.source_line_frame_index == b.source_line_frame_index
      && a.aggregated == b.aggregated
  }

  pub fn from_v8_exception<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    exception: v8::Local<'s, v8::Value>,
  ) -> Box<Self> {
    Box::new(Self::inner_from_v8_exception(
      scope,
      exception,
      Default::default(),
    ))
  }

  pub fn from_v8_message<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    msg: v8::Local<'s, v8::Message>,
  ) -> Box<Self> {
    // Create a new HandleScope because we're creating a lot of new local
    // handles below.
    v8::scope!(let scope, scope);

    let exception_message = msg.get(scope).to_rust_string_lossy(scope);

    // Convert them into Vec<JsStackFrame>
    let mut frames: Vec<JsStackFrame> = vec![];
    let mut source_line = None;
    let mut source_line_frame_index = None;

    if let Some(stack_frame) = JsStackFrame::from_v8_message(scope, msg) {
      frames = vec![stack_frame];
    }
    {
      let state = JsRuntime::state_from(scope);
      let mut source_mapper = state.source_mapper.borrow_mut();
      for (i, frame) in frames.iter().enumerate() {
        if let (Some(file_name), Some(line_number)) =
          (&frame.file_name, frame.line_number)
          && !file_name.trim_start_matches('[').starts_with("ext:")
        {
          source_line = source_mapper.get_source_line(file_name, line_number);
          source_line_frame_index = Some(i);
          break;
        }
      }
    }

    Box::new(Self {
      name: None,
      message: None,
      exception_message,
      cause: None,
      source_line,
      source_line_frame_index,
      frames,
      stack: None,
      aggregated: None,
      additional_properties: vec![],
    })
  }

  fn inner_from_v8_exception<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    exception: v8::Local<'s, v8::Value>,
    mut seen: HashSet<v8::Local<'s, v8::Object>>,
  ) -> Self {
    // Create a new HandleScope because we're creating a lot of new local
    // handles below.
    v8::scope!(let scope, scope);

    let msg = v8::Exception::create_message(scope, exception);

    let mut exception_message = None;
    let exception_state = JsRealm::exception_state_from_scope(scope);

    let js_format_exception_cb =
      exception_state.js_format_exception_cb.borrow().clone();
    if let Some(format_exception_cb) = js_format_exception_cb {
      let format_exception_cb = format_exception_cb.open(scope);
      let this = v8::undefined(scope).into();
      let formatted = format_exception_cb.call(scope, this, &[exception]);
      if let Some(formatted) = formatted
        && formatted.is_string()
      {
        exception_message = Some(formatted.to_rust_string_lossy(scope));
      }
    }

    if is_instance_of_error(scope, exception) {
      let v8_exception = exception;
      // The exception is a JS Error object.
      let exception: v8::Local<v8::Object> = exception.try_into().unwrap();
      let cause = get_property(scope, exception, v8_static_strings::CAUSE);
      let e: NativeJsError =
        serde_v8::from_v8(scope, exception.into()).unwrap_or_default();
      // Get the message by formatting error.name and error.message.
      let name = e.name.clone().unwrap_or_else(|| GENERIC_ERROR.to_string());
      let message_prop = e.message.clone().unwrap_or_default();
      let exception_message = exception_message.unwrap_or_else(|| {
        if !name.is_empty() && !message_prop.is_empty() {
          format!("Uncaught {name}: {message_prop}")
        } else if !name.is_empty() {
          format!("Uncaught {name}")
        } else if !message_prop.is_empty() {
          format!("Uncaught {message_prop}")
        } else {
          "Uncaught".to_string()
        }
      });
      let cause = cause.and_then(|cause| {
        if cause.is_undefined() || seen.contains(&exception) {
          None
        } else {
          seen.insert(exception);
          Some(Box::new(JsError::inner_from_v8_exception(
            scope, cause, seen,
          )))
        }
      });

      // Access error.stack to ensure that prepareStackTrace() has been called.
      // This should populate error.#callSiteEvals.
      let stack = get_property(scope, exception, v8_static_strings::STACK);
      let stack: Option<v8::Local<v8::String>> =
        stack.and_then(|s| s.try_into().ok());
      let stack = stack.map(|s| s.to_rust_string_lossy(scope));

      // Read an array of structured frames from error.#callSiteEvals.
      let frames_v8 = {
        let key = call_site_evals_key(scope);
        exception.get_private(scope, key)
      };
      // Ignore non-array values
      let frames_v8: Option<v8::Local<v8::Array>> =
        frames_v8.and_then(|a| a.try_into().ok());

      // Convert them into Vec<JsStackFrame>
      let mut frames: Vec<JsStackFrame> = match frames_v8 {
        Some(frames_v8) => {
          let mut buf = Vec::with_capacity(frames_v8.length() as usize);
          for i in 0..frames_v8.length() {
            let callsite = frames_v8.get_index(scope, i).unwrap().cast();
            v8::tc_scope!(let tc_scope, scope);

            let Some(stack_frame) =
              JsStackFrame::from_callsite_object(tc_scope, callsite)
            else {
              let message = tc_scope
                .exception()
                .expect(
                  "JsStackFrame::from_callsite_object raised an exception",
                )
                .to_rust_string_lossy(tc_scope);
              #[allow(clippy::print_stderr)]
              {
                eprintln!(
                  "warning: Failed to create JsStackFrame from callsite object: {message}. This is a bug in deno"
                );
              }
              break;
            };
            buf.push(stack_frame);
          }
          buf
        }
        None => vec![],
      };
      let mut source_line = None;
      let mut source_line_frame_index = None;

      // When the stack frame array is empty, but the source location given by
      // (script_resource_name, line_number, start_column + 1) exists, this is
      // likely a syntax error. For the sake of formatting we treat it like it
      // was given as a single stack frame.
      if frames.is_empty()
        && let Some(stack_frame) = JsStackFrame::from_v8_message(scope, msg)
      {
        frames = vec![stack_frame];
      }
      {
        let state = JsRuntime::state_from(scope);
        let mut source_mapper = state.source_mapper.borrow_mut();
        for (i, frame) in frames.iter().enumerate() {
          if let (Some(file_name), Some(line_number)) =
            (&frame.file_name, frame.line_number)
            && !file_name.trim_start_matches('[').starts_with("ext:")
          {
            source_line = source_mapper.get_source_line(file_name, line_number);
            source_line_frame_index = Some(i);
            break;
          }
        }
      }

      let mut aggregated: Option<Vec<JsError>> = None;
      if is_aggregate_error(scope, v8_exception) {
        // Read an array of stored errors, this is only defined for `AggregateError`
        let aggregated_errors =
          get_property(scope, exception, v8_static_strings::ERRORS);
        let aggregated_errors: Option<v8::Local<v8::Array>> =
          aggregated_errors.and_then(|a| a.try_into().ok());

        if let Some(errors) = aggregated_errors
          && errors.length() > 0
        {
          let mut agg = vec![];
          for i in 0..errors.length() {
            let error = errors.get_index(scope, i).unwrap();
            let js_error = Self::from_v8_exception(scope, error);
            agg.push(*js_error);
          }
          aggregated = Some(agg);
        }
      };

      let additional_properties_string =
        v8::String::new(scope, "errorAdditionalPropertyKeys").unwrap();
      let additional_properties_key =
        v8::Symbol::for_key(scope, additional_properties_string);
      let additional_properties =
        exception.get(scope, additional_properties_key.into());

      let additional_properties = if let Some(arr) =
        additional_properties.and_then(|keys| keys.try_cast::<v8::Array>().ok())
      {
        let mut out = Vec::with_capacity(arr.length() as usize);

        for i in 0..arr.length() {
          let key = arr.get_index(scope, i).unwrap();
          let key_name = key.to_rust_string_lossy(scope);

          let val = exception.get(scope, key).unwrap();
          let val_str = val.to_rust_string_lossy(scope);
          out.push((key_name, val_str));
        }

        out
      } else {
        vec![]
      };

      Self {
        name: e.name,
        message: e.message,
        exception_message,
        cause,
        source_line,
        source_line_frame_index,
        frames,
        stack,
        aggregated,
        additional_properties,
      }
    } else {
      let exception_message = exception_message
        .unwrap_or_else(|| msg.get(scope).to_rust_string_lossy(scope));
      // The exception is not a JS Error object.
      // Get the message given by V8::Exception::create_message(), and provide
      // empty frames.
      Self {
        name: None,
        message: None,
        exception_message,
        cause: None,
        source_line: None,
        source_line_frame_index: None,
        frames: vec![],
        stack: None,
        aggregated: None,
        additional_properties: vec![],
      }
    }
  }
}

impl std::error::Error for JsError {}

impl Display for JsError {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    if let Some(stack) = &self.stack {
      let stack_lines = stack.lines();
      if stack_lines.count() > 1 {
        return write!(f, "{stack}");
      }
    }
    write!(f, "{}", self.exception_message)?;
    let location = self.frames.first().and_then(|f| f.maybe_format_location());
    if let Some(location) = location {
      write!(f, "\n    at {location}")?;
    }
    Ok(())
  }
}

/// Implements `value instanceof primordials.Error` in JS. Similar to
/// `Value::is_native_error()` but more closely matches the semantics
/// of `instanceof`. `Value::is_native_error()` also checks for static class
/// inheritance rather than just scanning the prototype chain, which doesn't
/// work with our WebIDL implementation of `DOMException`.
pub(crate) fn is_instance_of_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
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
    if !prototype.is_object() {
      return false;
    }
    if prototype.strict_equals(error_prototype) {
      return true;
    }
    maybe_prototype = prototype
      .to_object(scope)
      .and_then(|o| o.get_prototype(scope));
  }
  false
}

/// Implements `value instanceof primordials.AggregateError` in JS,
/// by walking the prototype chain, and comparing each links constructor `name` property.
///
/// NOTE: There is currently no way to detect `AggregateError` via `rusty_v8`,
/// as v8 itself doesn't expose `v8__Exception__AggregateError`,
/// and we cannot create bindings for it. This forces us to rely on `name` inference.
pub(crate) fn is_aggregate_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  let mut maybe_prototype = Some(value);
  while let Some(prototype) = maybe_prototype {
    if !prototype.is_object() {
      return false;
    }

    let prototype = prototype.to_object(scope).unwrap();
    let prototype_name =
      match get_property(scope, prototype, v8_static_strings::CONSTRUCTOR) {
        Some(constructor) => {
          let ctor = constructor.to_object(scope).unwrap();
          get_property(scope, ctor, v8_static_strings::NAME)
            .map(|v| v.to_rust_string_lossy(scope))
        }
        None => return false,
      };

    if prototype_name == Some(String::from("AggregateError")) {
      return true;
    }

    maybe_prototype = prototype.get_prototype(scope);
  }

  false
}

/// Check if the error has a proper stack trace. The stack trace checked is the
/// one passed to `prepareStackTrace()`, not `msg.get_stack_trace()`.
pub(crate) fn has_call_site<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  exception: v8::Local<'s, v8::Value>,
) -> bool {
  if !exception.is_object() {
    return false;
  }
  let exception = exception.to_object(scope).unwrap();
  // Access error.stack to ensure that prepareStackTrace() has been called.
  // This should populate error.#callSiteEvals.
  get_property(scope, exception, v8_static_strings::STACK);
  let frames_v8 = {
    let key = call_site_evals_key(scope);
    exception.get_private(scope, key)
  };
  let frames_v8: Option<v8::Local<v8::Array>> =
    frames_v8.and_then(|a| a.try_into().ok());
  if let Some(frames_v8) = frames_v8
    && frames_v8.length() > 0
  {
    return true;
  }
  false
}

const DATA_URL_ABBREV_THRESHOLD: usize = 150;

fn to_percent_decoded_str(s: &str) -> String {
  match percent_encoding::percent_decode_str(s).decode_utf8() {
    Ok(s) => s.to_string(),
    // when failed to decode, return the original string
    Err(_) => s.to_string(),
  }
}

pub fn relative_specifier(from: &Url, to: &Url) -> Option<String> {
  let is_dir = to.path().ends_with('/');

  if is_dir && from == to {
    return Some("./".to_string());
  }

  let text = from.make_relative(to)?;
  let text = if text.starts_with("../") || text.starts_with("./") {
    text
  } else {
    format!("./{text}")
  };
  Some(to_percent_decoded_str(&text))
}

/// Like `relative_specifier`, but returns None if the path would require going up directories.
fn relative_specifier_within(from: &Url, to: &Url) -> Option<String> {
  let relative = relative_specifier(from, to)?;
  if relative.starts_with("../") {
    return None;
  }
  Some(relative)
}

pub struct FileNameParts<'a> {
  pub working_dir_path: Option<Cow<'a, str>>,
  pub file_name: Cow<'a, str>,
}

impl<'a> FileNameParts<'a> {
  pub fn into_owned(self) -> FileNameParts<'static> {
    FileNameParts {
      working_dir_path: self.working_dir_path.map(|s| match s {
        Cow::Borrowed(s) => Cow::Owned(s.to_owned()),
        Cow::Owned(s) => Cow::Owned(s),
      }),
      file_name: match self.file_name {
        Cow::Borrowed(s) => Cow::Owned(s.to_owned()),
        Cow::Owned(s) => Cow::Owned(s),
      },
    }
  }
}

pub fn format_file_name<'a>(
  file_name: &'a str,
  maybe_initial_cwd: Option<&Url>,
) -> FileNameParts<'a> {
  abbrev_file_name(file_name, maybe_initial_cwd).unwrap_or_else(|| {
    // same as to_percent_decoded_str() in cli/util/path.rs
    match percent_encoding::percent_decode_str(file_name).decode_utf8() {
      Ok(file_name) => FileNameParts {
        working_dir_path: None,
        file_name,
      },
      // when failing as utf-8, just return the original string
      Err(_) => FileNameParts {
        working_dir_path: None,
        file_name: file_name.into(),
      },
    }
  })
}

fn abbrev_file_name<'a>(
  file_name: &'a str,
  maybe_initial_cwd: Option<&Url>,
) -> Option<FileNameParts<'a>> {
  let Ok(url) = Url::parse(file_name) else {
    return None;
  };

  // Handle data URLs - abbreviate if too long
  if url.scheme() == "data" {
    if file_name.len() <= DATA_URL_ABBREV_THRESHOLD {
      return Some(FileNameParts {
        working_dir_path: None,
        file_name: file_name.into(),
      });
    }
    let (head, tail) = url.path().split_once(',')?;
    let len = tail.len();
    let start = tail.get(0..20)?;
    let end = tail.get(len - 20..)?;
    return Some(FileNameParts {
      working_dir_path: None,
      file_name: format!("{}:{},{}......{}", url.scheme(), head, start, end)
        .into(),
    });
  }

  // For file:// URLs, use relative paths only if the file is within the cwd
  if let Some(initial_cwd) = maybe_initial_cwd
    && let Some(relative) = relative_specifier_within(initial_cwd, &url)
  {
    return Some(FileNameParts {
      working_dir_path: Some(initial_cwd.to_string().into()),
      file_name: relative.into(),
    });
  }

  // For all other cases, use absolute paths
  None
}

fn format_eval_origin<'a>(
  eval_origin: &'a str,
  maybe_initial_cwd: Option<&Url>,
) -> Cow<'a, str> {
  let Some((before, (file, line, col), after)) = parse_eval_origin(eval_origin)
  else {
    return eval_origin.into();
  };
  let formatted_file = format_file_name(file, maybe_initial_cwd);

  Cow::Owned(format!(
    "{before}{}:{line}:{col}{after}",
    if let Some(working_dir_path) = formatted_file.working_dir_path {
      Cow::Owned(format!(
        "{}{}",
        working_dir_path,
        formatted_file.file_name.trim_start_matches("./")
      ))
    } else {
      formatted_file.file_name
    }
  ))
}

pub(crate) fn exception_to_err_result<'s, 'i, T>(
  scope: &mut v8::PinScope<'s, 'i>,
  exception: v8::Local<'s, v8::Value>,
  in_promise: bool,
  clear_error: bool,
) -> Result<T, Box<JsError>> {
  Err(exception_to_err(scope, exception, in_promise, clear_error))
}

pub(crate) fn exception_to_err<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  exception: v8::Local<'s, v8::Value>,
  mut in_promise: bool,
  clear_error: bool,
) -> Box<JsError> {
  let state = JsRealm::exception_state_from_scope(scope);

  let mut was_terminating_execution = scope.is_execution_terminating();

  // Disable running microtasks for a moment. When upgrading to V8 v11.4
  // we discovered that canceling termination here will cause the queued
  // microtasks to run which breaks some tests.
  scope.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
  // If TerminateExecution was called, cancel isolate termination so that the
  // exception can be created. Note that `scope.is_execution_terminating()` may
  // have returned false if TerminateExecution was indeed called but there was
  // no JS to execute after the call.
  scope.cancel_terminate_execution();
  let exception = match state.get_dispatched_exception_as_local(scope) {
    Some(dispatched_exception) => {
      // If termination is the result of a `reportUnhandledException` call, we want
      // to use the exception that was passed to it rather than the exception that
      // was passed to this function.
      in_promise = state.is_dispatched_exception_promise();
      if clear_error {
        state.clear_error();
        was_terminating_execution = false;
      }
      dispatched_exception
    }
    _ => {
      if was_terminating_execution && exception.is_null_or_undefined() {
        // If we are terminating and there is no exception, throw `new Error("execution terminated")``.
        let message = v8::String::new(scope, "execution terminated").unwrap();
        v8::Exception::error(scope, message)
      } else {
        // Otherwise re-use the exception
        exception
      }
    }
  };

  let mut js_error = JsError::from_v8_exception(scope, exception);
  if in_promise {
    js_error.exception_message = format!(
      "Uncaught (in promise) {}",
      js_error.exception_message.trim_start_matches("Uncaught ")
    );
  }

  if was_terminating_execution {
    // Resume exception termination.
    scope.terminate_execution();
  }
  scope.set_microtasks_policy(v8::MicrotasksPolicy::Auto);

  js_error
}

v8_static_strings::v8_static_strings! {
  ERROR = "Error",
  GET_FILE_NAME = "getFileName",
  GET_SCRIPT_NAME_OR_SOURCE_URL = "getScriptNameOrSourceURL",
  GET_THIS = "getThis",
  GET_TYPE_NAME = "getTypeName",
  GET_FUNCTION = "getFunction",
  GET_FUNCTION_NAME = "getFunctionName",
  GET_METHOD_NAME = "getMethodName",
  GET_LINE_NUMBER = "getLineNumber",
  GET_COLUMN_NUMBER = "getColumnNumber",
  GET_EVAL_ORIGIN = "getEvalOrigin",
  IS_TOPLEVEL = "isToplevel",
  IS_EVAL = "isEval",
  IS_NATIVE = "isNative",
  IS_CONSTRUCTOR = "isConstructor",
  IS_ASYNC = "isAsync",
  IS_PROMISE_ALL = "isPromiseAll",
  GET_PROMISE_INDEX = "getPromiseIndex",
  TO_STRING = "toString",
  PREPARE_STACK_TRACE = "prepareStackTrace",
  ORIGINAL = "deno_core::original_call_site",
  SOURCE_MAPPED_INFO = "deno_core::source_mapped_call_site_info",
  ERROR_RECEIVER_IS_NOT_VALID_CALLSITE_OBJECT = "The receiver is not a valid callsite object.",
}

#[inline(always)]
pub(crate) fn original_call_site_key<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Private> {
  let name = ORIGINAL.v8_string(scope).unwrap();
  v8::Private::for_api(scope, Some(name))
}

pub(crate) fn source_mapped_info_key<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Private> {
  let name = SOURCE_MAPPED_INFO.v8_string(scope).unwrap();
  v8::Private::for_api(scope, Some(name))
}

fn make_patched_callsite<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  callsite: v8::Local<'s, v8::Object>,
  prototype: v8::Local<'s, v8::Object>,
) -> v8::Local<'s, v8::Object> {
  let out_obj = v8::Object::with_prototype_and_properties(
    scope,
    prototype.into(),
    &[],
    &[],
  );
  let orig_key = original_call_site_key(scope);
  out_obj.set_private(scope, orig_key, callsite.into());
  out_obj
}

fn original_call_site<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  this: v8::Local<'s, v8::Object>,
) -> Option<v8::Local<'s, v8::Object>> {
  let orig_key = original_call_site_key(scope);
  let Some(orig) = this
    .get_private(scope, orig_key)
    .and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())
  else {
    let message = ERROR_RECEIVER_IS_NOT_VALID_CALLSITE_OBJECT
      .v8_string(scope)
      .unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
    return None;
  };
  Some(orig)
}

macro_rules! make_callsite_fn {
  ($fn:ident, $field:ident) => {
    pub fn $fn<'s, 'i>(
      scope: &mut v8::PinScope<'s, 'i>,
      args: v8::FunctionCallbackArguments<'s>,
      mut rv: v8::ReturnValue<'_>,
    ) {
      let Some(orig) = original_call_site(scope, args.this()) else {
        return;
      };
      let key = $field.v8_string(scope).unwrap().into();
      let orig_ret = orig
        .cast::<v8::Object>()
        .get(scope, key)
        .unwrap()
        .cast::<v8::Function>()
        .call(scope, orig.into(), &[]);
      rv.set(orig_ret.unwrap_or_else(|| v8::undefined(scope).into()));
    }
  };
}

fn maybe_to_path_str(string: &str) -> Option<String> {
  if string.starts_with("file://") {
    Some(
      Url::parse(string)
        .unwrap()
        .to_file_path()
        .unwrap()
        .to_string_lossy()
        .into_owned(),
    )
  } else {
    None
  }
}

pub mod callsite_fns {
  use capacity_builder::StringBuilder;

  use crate::FromV8;
  use crate::ToV8;
  use crate::convert;

  use super::*;

  enum SourceMappedCallsiteInfo<'a> {
    Ref(v8::Local<'a, v8::Array>),
    Value {
      file_name: v8::Local<'a, v8::Value>,
      line_number: v8::Local<'a, v8::Value>,
      column_number: v8::Local<'a, v8::Value>,
    },
  }
  impl<'s> SourceMappedCallsiteInfo<'s> {
    #[inline]
    fn file_name<'i>(
      &self,
      scope: &mut v8::PinScope<'s, 'i>,
    ) -> v8::Local<'s, v8::Value> {
      match self {
        Self::Ref(array) => array.get_index(scope, 0).unwrap(),
        Self::Value { file_name, .. } => *file_name,
      }
    }
    #[inline]
    fn line_number<'i>(
      &self,
      scope: &mut v8::PinScope<'s, 'i>,
    ) -> v8::Local<'s, v8::Value> {
      match self {
        Self::Ref(array) => array.get_index(scope, 1).unwrap(),
        Self::Value { line_number, .. } => *line_number,
      }
    }
    #[inline]
    fn column_number<'i>(
      &self,
      scope: &mut v8::PinScope<'s, 'i>,
    ) -> v8::Local<'s, v8::Value> {
      match self {
        Self::Ref(array) => array.get_index(scope, 2).unwrap(),
        Self::Value { column_number, .. } => *column_number,
      }
    }
  }

  type MaybeValue<'a> = Option<v8::Local<'a, v8::Value>>;

  fn maybe_apply_source_map<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    file_name: MaybeValue<'s>,
    line_number: MaybeValue<'s>,
    column_number: MaybeValue<'s>,
  ) -> Option<(String, i64, i64)> {
    let file_name = serde_v8::to_utf8(file_name?.try_cast().ok()?, scope);
    let convert::Number(line_number) =
      FromV8::from_v8(scope, line_number?).ok()?;
    let convert::Number(column_number) =
      FromV8::from_v8(scope, column_number?).ok()?;

    let state = JsRuntime::state_from(scope);
    let mut source_mapper = state.source_mapper.borrow_mut();
    let (mapped_file_name, mapped_line_number, mapped_column_number) =
      apply_source_map(
        &mut source_mapper,
        Cow::Owned(file_name),
        line_number,
        column_number,
      );
    Some((
      mapped_file_name.into_owned(),
      mapped_line_number,
      mapped_column_number,
    ))
  }
  fn source_mapped_call_site_info<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    callsite: v8::Local<'s, v8::Object>,
  ) -> Option<SourceMappedCallsiteInfo<'s>> {
    let key = source_mapped_info_key(scope);
    // return the cached value if it exists
    if let Some(info) = callsite.get_private(scope, key)
      && let Ok(array) = info.try_cast::<v8::Array>()
    {
      return Some(SourceMappedCallsiteInfo::Ref(array));
    }
    let orig_callsite = original_call_site(scope, callsite)?;

    let file_name =
      call_method::<v8::Value>(scope, orig_callsite, super::GET_FILE_NAME, &[]);
    let line_number = call_method::<v8::Value>(
      scope,
      orig_callsite,
      super::GET_LINE_NUMBER,
      &[],
    );
    let column_number = call_method::<v8::Value>(
      scope,
      orig_callsite,
      super::GET_COLUMN_NUMBER,
      &[],
    );

    // For wasm frames, skip source map application and cache the original values
    let is_wasm = file_name
      .and_then(|v| v.try_cast::<v8::String>().ok())
      .map(|s| serde_v8::to_utf8(s, scope).starts_with("wasm://"))
      .unwrap_or(false);

    let info = v8::Array::new(scope, 3);

    // if the types are right, apply the source map (unless wasm), otherwise just take them as is
    if !is_wasm
      && let Some((mapped_file_name, mapped_line_number, mapped_column_number)) =
        maybe_apply_source_map(scope, file_name, line_number, column_number)
    {
      let mapped_file_name_trimmed =
        maybe_to_path_str(&mapped_file_name).unwrap_or(mapped_file_name);
      let mapped_file_name = crate::FastString::from(mapped_file_name_trimmed)
        .v8_string(scope)
        .unwrap();
      let Ok(mapped_line_number) =
        convert::Number(mapped_line_number).to_v8(scope);
      let Ok(mapped_column_number) =
        convert::Number(mapped_column_number).to_v8(scope);
      info.set_index(scope, 0, mapped_file_name.into());
      info.set_index(scope, 1, mapped_line_number);
      info.set_index(scope, 2, mapped_column_number);
      callsite.set_private(scope, key, info.into());
      Some(SourceMappedCallsiteInfo::Value {
        file_name: mapped_file_name.into(),
        line_number: mapped_line_number,
        column_number: mapped_column_number,
      })
    } else {
      let file_name = file_name.unwrap_or_else(|| v8::undefined(scope).into());
      let line_number =
        line_number.unwrap_or_else(|| v8::undefined(scope).into());
      let column_number =
        column_number.unwrap_or_else(|| v8::undefined(scope).into());
      info.set_index(scope, 0, file_name);
      info.set_index(scope, 1, line_number);
      info.set_index(scope, 2, column_number);
      callsite.set_private(scope, key, info.into());
      Some(SourceMappedCallsiteInfo::Ref(info))
    }
  }

  make_callsite_fn!(get_this, GET_THIS);
  make_callsite_fn!(get_type_name, GET_TYPE_NAME);
  make_callsite_fn!(get_function, GET_FUNCTION);
  make_callsite_fn!(get_function_name, GET_FUNCTION_NAME);
  make_callsite_fn!(get_method_name, GET_METHOD_NAME);

  pub fn get_file_name<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    args: v8::FunctionCallbackArguments<'s>,
    mut rv: v8::ReturnValue<'_>,
  ) {
    if let Some(info) = source_mapped_call_site_info(scope, args.this()) {
      rv.set(info.file_name(scope));
    }
  }

  pub fn get_line_number<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    args: v8::FunctionCallbackArguments<'s>,
    mut rv: v8::ReturnValue<'_>,
  ) {
    if let Some(info) = source_mapped_call_site_info(scope, args.this()) {
      rv.set(info.line_number(scope));
    }
  }

  pub fn get_column_number<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    args: v8::FunctionCallbackArguments<'s>,
    mut rv: v8::ReturnValue<'_>,
  ) {
    if let Some(info) = source_mapped_call_site_info(scope, args.this()) {
      rv.set(info.column_number(scope));
    }
  }

  make_callsite_fn!(get_eval_origin, GET_EVAL_ORIGIN);
  make_callsite_fn!(is_toplevel, IS_TOPLEVEL);
  make_callsite_fn!(is_eval, IS_EVAL);
  make_callsite_fn!(is_native, IS_NATIVE);
  make_callsite_fn!(is_constructor, IS_CONSTRUCTOR);
  make_callsite_fn!(is_async, IS_ASYNC);
  make_callsite_fn!(is_promise_all, IS_PROMISE_ALL);
  make_callsite_fn!(get_promise_index, GET_PROMISE_INDEX);
  make_callsite_fn!(
    get_script_name_or_source_url,
    GET_SCRIPT_NAME_OR_SOURCE_URL
  );

  // the bulk of the to_string logic
  fn to_string_inner<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    this: v8::Local<'s, v8::Object>,
    orig: v8::Local<'s, v8::Object>,
    orig_to_string_v8: v8::Local<'s, v8::String>,
  ) -> Option<v8::Local<'s, v8::String>> {
    let orig_to_string = serde_v8::to_utf8(orig_to_string_v8, scope);
    // `this[kOriginalCallsite].getFileName()`
    let orig_file_name =
      call_method::<v8::Value>(scope, orig, GET_FILE_NAME, &[])
        .and_then(|v| v.try_cast::<v8::String>().ok())?;
    let orig_file_name = serde_v8::to_utf8(orig_file_name, scope);

    // For wasm frames, V8's original toString() already formats correctly
    // (with wasm-function[N]:0xOFFSET), so return it unchanged.
    if orig_file_name.starts_with("wasm://") {
      return Some(orig_to_string_v8);
    }

    let orig_line_number =
      call_method::<v8::Value>(scope, orig, GET_LINE_NUMBER, &[])
        .and_then(|v| v.try_cast::<v8::Number>().ok())?;
    let orig_column_number =
      call_method::<v8::Value>(scope, orig, GET_COLUMN_NUMBER, &[])
        .and_then(|v| v.try_cast::<v8::Number>().ok())?;
    let orig_line_number = orig_line_number.value() as i64;
    let orig_column_number = orig_column_number.value() as i64;
    let orig_file_name_line_col =
      fmt_file_line_col(&orig_file_name, orig_line_number, orig_column_number);
    let mapped = source_mapped_call_site_info(scope, this)?;
    let mapped_file_name = mapped.file_name(scope).to_rust_string_lossy(scope);
    let mapped_line_num = mapped
      .line_number(scope)
      .try_cast::<v8::Number>()
      .ok()
      .map(|n| n.value() as i64)?;
    let mapped_col_num =
      mapped.column_number(scope).cast::<v8::Number>().value() as i64;
    let file_name_line_col =
      fmt_file_line_col(&mapped_file_name, mapped_line_num, mapped_col_num);
    // replace file URL with file path, and source map in original `toString`
    let to_string = orig_to_string
      .replace(&orig_file_name_line_col, &file_name_line_col)
      .replace(&orig_file_name, &mapped_file_name); // maybe unnecessary?
    Some(crate::FastString::from(to_string).v8_string(scope).unwrap())
  }

  fn fmt_file_line_col(file: &str, line: i64, col: i64) -> String {
    StringBuilder::build(|builder| {
      builder.append(file);
      builder.append(':');
      builder.append(line);
      builder.append(':');
      builder.append(col);
    })
    .unwrap()
  }

  pub fn to_string<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    args: v8::FunctionCallbackArguments<'s>,
    mut rv: v8::ReturnValue<'_>,
  ) {
    let this = args.this();
    let Some(orig) = original_call_site(scope, this) else {
      return;
    };
    // `this[kOriginalCallsite].toString()`
    let Some(orig_to_string_v8) =
      call_method::<v8::String>(scope, orig, TO_STRING, &[])
    else {
      return;
    };

    if let Some(v8_str) = to_string_inner(scope, this, orig, orig_to_string_v8)
    {
      rv.set(v8_str.into());
    } else {
      rv.set(orig_to_string_v8.into());
    }
  }
}

/// Creates a template for a `Callsite`-like object, with
/// a patched `getFileName`.
/// Effectively:
/// ```js
/// const kOriginalCallsite = Symbol("_original");
/// {
///   [kOriginalCallsite]: originalCallSite,
///   getLineNumber() {
///     return this[kOriginalCallsite].getLineNumber();
///   },
///   // etc
///   getFileName() {
///     const fileName = this[kOriginalCallsite].getFileName();
///     return fileUrlToPath(fileName);
///   }
/// }
/// ```
pub(crate) fn make_callsite_prototype<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
) -> v8::Local<'s, v8::Object> {
  let template = v8::ObjectTemplate::new(scope);

  macro_rules! set_attr {
    ($scope:ident, $template:ident, $fn:ident, $field:ident) => {
      let key = $field.v8_string($scope).unwrap().into();
      $template.set_with_attr(
        key,
        v8::FunctionBuilder::<v8::FunctionTemplate>::new(callsite_fns::$fn)
          .build($scope)
          .into(),
        v8::PropertyAttribute::DONT_DELETE
          | v8::PropertyAttribute::DONT_ENUM
          | v8::PropertyAttribute::READ_ONLY,
      );
    };
  }

  set_attr!(scope, template, get_this, GET_THIS);
  set_attr!(scope, template, get_type_name, GET_TYPE_NAME);
  set_attr!(scope, template, get_function, GET_FUNCTION);
  set_attr!(scope, template, get_function_name, GET_FUNCTION_NAME);
  set_attr!(scope, template, get_method_name, GET_METHOD_NAME);
  set_attr!(scope, template, get_file_name, GET_FILE_NAME);
  set_attr!(scope, template, get_line_number, GET_LINE_NUMBER);
  set_attr!(scope, template, get_column_number, GET_COLUMN_NUMBER);
  set_attr!(scope, template, get_eval_origin, GET_EVAL_ORIGIN);
  set_attr!(scope, template, is_toplevel, IS_TOPLEVEL);
  set_attr!(scope, template, is_eval, IS_EVAL);
  set_attr!(scope, template, is_native, IS_NATIVE);
  set_attr!(scope, template, is_constructor, IS_CONSTRUCTOR);
  set_attr!(scope, template, is_async, IS_ASYNC);
  set_attr!(scope, template, is_promise_all, IS_PROMISE_ALL);
  set_attr!(scope, template, get_promise_index, GET_PROMISE_INDEX);
  set_attr!(
    scope,
    template,
    get_script_name_or_source_url,
    GET_SCRIPT_NAME_OR_SOURCE_URL
  );
  set_attr!(scope, template, to_string, TO_STRING);

  template.new_instance(scope).unwrap()
}

#[inline(always)]
fn prepare_stack_trace_inner<'s, 'i, const PATCH_CALLSITES: bool>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Value>,
  callsites: v8::Local<'s, v8::Array>,
) -> v8::Local<'s, v8::Value> {
  // stash the callsites on the error object. this is used by `JsError::from_exception_inner`
  // to get more details on an error, because the v8 StackFrame API is missing some info.
  if let Ok(obj) = error.try_cast::<v8::Object>() {
    let key = call_site_evals_key(scope);
    obj.set_private(scope, key, callsites.into());
  }

  // `globalThis.Error.prepareStackTrace`
  let global = scope.get_current_context().global(scope);
  let global_error =
    get_property(scope, global, ERROR).and_then(|g| g.try_cast().ok());
  let prepare_fn = global_error.and_then(|g| {
    get_property(scope, g, PREPARE_STACK_TRACE)
      .and_then(|f| f.try_cast::<v8::Function>().ok())
  });

  // Note that the callback is called _instead_ of `Error.prepareStackTrace`
  // so we have to explicitly call the global function
  if let Some(prepare_fn) = prepare_fn {
    let callsites = if PATCH_CALLSITES {
      // User defined `Error.prepareStackTrace`.
      // Patch the callsites to have file paths, then call
      // the user's function
      let len = callsites.length();
      let mut patched = Vec::with_capacity(len as usize);
      let template = JsRuntime::state_from(scope)
        .callsite_prototype
        .borrow()
        .clone()
        .unwrap();
      let prototype = v8::Local::new(scope, template);
      for i in 0..len {
        let callsite =
          callsites.get_index(scope, i).unwrap().cast::<v8::Object>();
        patched.push(make_patched_callsite(scope, callsite, prototype).into());
      }
      v8::Array::new_with_elements(scope, &patched)
    } else {
      callsites
    };

    // call the user's `prepareStackTrace` with our "callsite" objects
    let this = global_error.unwrap().into();
    let args = &[error, callsites.into()];
    return prepare_fn
      .call(scope, this, args)
      .unwrap_or_else(|| v8::undefined(scope).into());
  }

  // no user defined `prepareStackTrace`, just call our default
  format_stack_trace(scope, error, callsites)
}

/// Callback to prepare an error stack trace. This callback is invoked by V8 whenever a stack trace is created.
/// This will patch callsite objects to translate file URLs to file paths before they're
/// exposed to user-defined `prepareStackTrace` implementations.
///
/// This function is not used by default, but it can
/// be set directly on the [v8 isolate][v8::Isolate::set_prepare_stack_trace_callback]
pub fn prepare_stack_trace_callback_with_original_callsites<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Value>,
  callsites: v8::Local<'s, v8::Array>,
) -> v8::Local<'s, v8::Value> {
  prepare_stack_trace_inner::<false>(scope, error, callsites)
}

/// Callback to prepare an error stack trace. This callback is invoked by V8 whenever a stack trace is created.
/// This will patch callsite objects to translate file URLs to file paths before they're
/// exposed to user-defined `prepareStackTrace` implementations.
///
/// This function is the default callback, set on creating a `JsRuntime`, but the callback can also
/// be set directly on the [v8 isolate][v8::Isolate::set_prepare_stack_trace_callback]
pub fn prepare_stack_trace_callback<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Value>,
  callsites: v8::Local<'s, v8::Array>,
) -> v8::Local<'s, v8::Value> {
  prepare_stack_trace_inner::<true>(scope, error, callsites)
}

pub struct InitialCwd(pub Arc<Url>);

pub fn format_stack_trace<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Value>,
  callsites: v8::Local<'s, v8::Array>,
) -> v8::Local<'s, v8::Value> {
  let state = JsRuntime::state_from(scope);
  let maybe_initial_cwd = state
    .op_state
    .borrow()
    .try_borrow::<InitialCwd>()
    .map(|i| &i.0)
    .cloned();
  let mut result = String::new();

  if let Ok(obj) = error.try_cast() {
    // Write out the error name + message, if any
    let msg = get_property(scope, obj, v8_static_strings::MESSAGE)
      .filter(|v| !v.is_undefined())
      .map(|v| v.to_rust_string_lossy(scope))
      .unwrap_or_default();
    let name = get_property(scope, obj, v8_static_strings::NAME)
      .filter(|v| !v.is_undefined())
      .map(|v| v.to_rust_string_lossy(scope))
      .unwrap_or_else(|| GENERIC_ERROR.to_string());

    match (!msg.is_empty(), !name.is_empty()) {
      (true, true) => write!(result, "{}: {}", name, msg).unwrap(),
      (true, false) => write!(result, "{}", msg).unwrap(),
      (false, true) => write!(result, "{}", name).unwrap(),
      (false, false) => {}
    }
  }

  // format each stack frame
  for i in 0..callsites.length() {
    let callsite = callsites.get_index(scope, i).unwrap().cast::<v8::Object>();
    v8::tc_scope!(let tc_scope, scope);

    let Some(frame) = JsStackFrame::from_callsite_object(tc_scope, callsite)
    else {
      let message = tc_scope
        .exception()
        .expect("JsStackFrame::from_callsite_object raised an exception")
        .to_rust_string_lossy(tc_scope);
      #[allow(clippy::print_stderr)]
      {
        eprintln!(
          "warning: Failed to create JsStackFrame from callsite object: {message}; Result so far: {result}. This is a bug in deno"
        );
      }
      break;
    };
    write!(
      result,
      "\n    at {}",
      format_frame::<NoAnsiColors>(&frame, maybe_initial_cwd.as_deref())
    )
    .unwrap();
  }

  let result = v8::String::new(scope, &result).unwrap();
  result.into()
}

/// Formats an error without using ansi colors.
pub struct NoAnsiColors;

#[derive(Debug, Clone, Copy)]
/// Part of an error stack trace
pub enum ErrorElement {
  /// An anonymous file or method name
  Anonymous,
  /// Text signifying a native stack frame
  NativeFrame,
  /// A source line number
  LineNumber,
  /// A source column number
  ColumnNumber,
  /// The name of a function (or method)
  FunctionName,
  /// The name of a source file
  FileName,
  /// The path to the working directory, as in part of a file name
  WorkingDirPath,
  /// The origin of an error coming from `eval`ed code
  EvalOrigin,
  /// Text signifying a call to `Promise.all`
  PromiseAll,
  /// Other, plain text appearing in the error stack trace
  PlainText,
}

/// Applies formatting to various parts of error stack traces.
///
/// This is to allow the `deno` crate to reuse `format_frame` and
/// `format_location` but apply ANSI colors for terminal output,
/// without adding extra dependencies to `deno_core`.
pub trait ErrorFormat {
  fn fmt_element(
    element: ErrorElement,
    in_extension_code: bool,
    s: &str,
  ) -> Cow<'_, str>;
}

impl ErrorFormat for NoAnsiColors {
  fn fmt_element(
    _element: ErrorElement,
    _in_extension_code: bool,
    s: &str,
  ) -> Cow<'_, str> {
    s.into()
  }
}

pub fn format_location<F: ErrorFormat>(
  frame: &JsStackFrame,
  maybe_initial_cwd: Option<&Url>,
) -> String {
  use ErrorElement::*;
  let in_extension_code = frame
    .file_name
    .as_ref()
    .map(|f| f.starts_with("ext:"))
    .unwrap_or(false);
  if frame.is_native {
    return F::fmt_element(NativeFrame, in_extension_code, "native")
      .to_string();
  }
  let mut result = String::new();
  let file_name = frame.file_name.as_deref().unwrap_or("");
  if !file_name.is_empty() {
    let parts = format_file_name(file_name, maybe_initial_cwd);
    if let Some(working_dir_path) = &parts.working_dir_path {
      result +=
        &F::fmt_element(WorkingDirPath, in_extension_code, working_dir_path);
      result += &F::fmt_element(
        FileName,
        in_extension_code,
        parts.file_name.trim_start_matches("./"),
      )
    } else {
      result += &F::fmt_element(FileName, in_extension_code, &parts.file_name)
    }
  } else {
    if frame.is_eval {
      let eval_origin = frame.eval_origin.as_ref().unwrap();
      let formatted_eval_origin =
        format_eval_origin(eval_origin, maybe_initial_cwd);
      result +=
        &F::fmt_element(EvalOrigin, in_extension_code, &formatted_eval_origin);
      result += &F::fmt_element(PlainText, in_extension_code, ", ");
    }
    result += &F::fmt_element(Anonymous, in_extension_code, "<anonymous>");
  }
  if frame.is_wasm {
    // W3C WebAssembly Web API spec format:
    // {url}:wasm-function[{funcIndex}]:0x{pcOffset}
    // V8 returns function index (1-based) via getLineNumber()
    // and byte offset via getColumnNumber()
    if let Some(line_number) = frame.line_number {
      let func_index = line_number - 1;
      let wasm_func = format!("wasm-function[{func_index}]");
      result += &F::fmt_element(PlainText, in_extension_code, ":");
      result += &F::fmt_element(LineNumber, in_extension_code, &wasm_func);
    }
    if let Some(column_number) = frame.column_number {
      // V8's getColumnNumber() is 1-based, but wasm byte offsets are 0-based
      let pc_offset = format!("0x{:x}", column_number - 1);
      result += &F::fmt_element(PlainText, in_extension_code, ":");
      result += &F::fmt_element(ColumnNumber, in_extension_code, &pc_offset);
    }
  } else if let Some(line_number) = frame.line_number {
    result += &F::fmt_element(PlainText, in_extension_code, ":");
    result +=
      &F::fmt_element(LineNumber, in_extension_code, &line_number.to_string());
    if let Some(column_number) = frame.column_number {
      result += &F::fmt_element(PlainText, in_extension_code, ":");
      result += &F::fmt_element(
        ColumnNumber,
        in_extension_code,
        &column_number.to_string(),
      );
    }
  }
  result
}

pub fn format_frame<F: ErrorFormat>(
  frame: &JsStackFrame,
  maybe_initial_cwd: Option<&Url>,
) -> String {
  use ErrorElement::*;
  let in_extension_code = frame
    .file_name
    .as_ref()
    .map(|f| f.starts_with("ext:"))
    .unwrap_or(false);
  let is_method_call =
    !(frame.is_top_level.unwrap_or_default() || frame.is_constructor);
  let mut result = String::new();
  if frame.is_async {
    result += &F::fmt_element(PlainText, in_extension_code, "async ");
  }
  if frame.is_promise_all {
    result += &F::fmt_element(
      PromiseAll,
      in_extension_code,
      &format!(
        "Promise.all (index {})",
        frame.promise_index.unwrap_or_default()
      ),
    );
    return result;
  }
  if is_method_call {
    let mut formatted_method = String::new();
    if let Some(function_name) = &frame.function_name {
      if let Some(type_name) = &frame.type_name
        && !function_name.starts_with(type_name)
      {
        write!(formatted_method, "{type_name}.").unwrap();
      }
      formatted_method += function_name;
      if let Some(method_name) = &frame.method_name
        && !function_name.ends_with(method_name)
      {
        write!(formatted_method, " [as {method_name}]").unwrap();
      }
    } else {
      if let Some(type_name) = &frame.type_name {
        write!(formatted_method, "{type_name}.").unwrap();
      }
      if let Some(method_name) = &frame.method_name {
        formatted_method += method_name
      } else {
        formatted_method += "<anonymous>";
      }
    }
    result +=
      F::fmt_element(FunctionName, in_extension_code, &formatted_method)
        .as_ref();
  } else if frame.is_constructor {
    result += &F::fmt_element(PlainText, in_extension_code, "new ");
    if let Some(function_name) = &frame.function_name {
      write!(
        result,
        "{}",
        F::fmt_element(FunctionName, in_extension_code, function_name)
      )
      .unwrap();
    } else {
      result +=
        F::fmt_element(Anonymous, in_extension_code, "<anonymous>").as_ref();
    }
  } else if let Some(function_name) = &frame.function_name {
    result +=
      F::fmt_element(FunctionName, in_extension_code, function_name).as_ref();
  } else {
    result += &format_location::<F>(frame, maybe_initial_cwd);
    return result;
  }
  result += &F::fmt_element(PlainText, in_extension_code, " (");
  result += &format_location::<F>(frame, maybe_initial_cwd);
  result += &F::fmt_element(PlainText, in_extension_code, ")");
  result
}

pub fn throw_error_one_byte_info(
  info: &v8::FunctionCallbackInfo,
  message: &str,
) {
  v8::callback_scope!(unsafe scope, info);
  throw_error_one_byte(scope, message);
}

pub fn throw_error_js_error_class<'s, 'i>(
  scope: &mut v8::PinCallbackScope<'s, 'i>,
  err: &dyn JsErrorClass,
) {
  let exc = to_v8_error(scope, err);
  scope.throw_exception(exc);
}

pub fn throw_error_one_byte<'s, 'i>(
  scope: &mut v8::PinCallbackScope<'s, 'i>,
  message: &str,
) {
  let msg = deno_core::v8::String::new_from_one_byte(
    scope,
    message.as_bytes(),
    deno_core::v8::NewStringType::Normal,
  )
  .unwrap();
  let exc = deno_core::v8::Exception::type_error(scope, msg);
  scope.throw_exception(exc);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_format_file_name() {
    let file_name = format_file_name("data:,Hello%2C%20World%21", None);
    assert_eq!(file_name.file_name, "data:,Hello%2C%20World%21");

    let too_long_name = "a".repeat(DATA_URL_ABBREV_THRESHOLD + 1);
    let file_name = format_file_name(
      &format!("data:text/plain;base64,{too_long_name}_%F0%9F%A6%95"),
      None,
    )
    .into_owned();
    let expected =
      "data:text/plain;base64,aaaaaaaaaaaaaaaaaaaa......aaaaaaa_%F0%9F%A6%95";
    assert_eq!(file_name.file_name, expected,);
    let file_name = format_file_name(
      &format!("data:text/plain;base64,{too_long_name}_%F0%9F%A6%95"),
      Some(&Url::parse("file:///foo").unwrap()),
    )
    .into_owned();
    assert_eq!(file_name.file_name, expected);
    let file_name = format_file_name("file:///foo/bar.ts", None);
    assert_eq!(file_name.file_name, "file:///foo/bar.ts");

    let file_name = format_file_name(
      "file:///foo/bar.ts",
      Some(&Url::parse("file:///foo/").unwrap()),
    );
    assert_eq!(file_name.file_name, "./bar.ts");

    let file_name =
      format_file_name("file:///%E6%9D%B1%E4%BA%AC/%F0%9F%A6%95.ts", None);
    assert_eq!(file_name.file_name, "file:///東京/🦕.ts");
  }

  #[test]
  fn test_parse_eval_origin() {
    let cases = [
      (
        "eval at <anonymous> (file://path.ts:1:2)",
        Some(("eval at <anonymous> (", ("file://path.ts", 1, 2), ")")),
      ),
      (
        // malformed
        "eval at (s:1:2",
        None,
      ),
      (
        // malformed
        "at ()", None,
      ),
      (
        // url with parens
        "eval at foo (http://website.zzz/my-script).ts:1:2)",
        Some((
          "eval at foo (",
          ("http://website.zzz/my-script).ts", 1, 2),
          ")",
        )),
      ),
      (
        // nested
        "eval at foo (eval at bar (file://path.ts:1:2))",
        Some(("eval at foo (eval at bar (", ("file://path.ts", 1, 2), "))")),
      ),
    ];
    for (input, expect) in cases {
      match expect {
        Some((
          expect_before,
          (expect_file, expect_line, expect_col),
          expect_after,
        )) => {
          let (before, (file_name, line_number, column_number), after) =
            parse_eval_origin(input).unwrap();
          assert_eq!(before, expect_before);
          assert_eq!(file_name, expect_file);
          assert_eq!(line_number, expect_line);
          assert_eq!(column_number, expect_col);
          assert_eq!(after, expect_after);
        }
        None => {
          assert!(parse_eval_origin(input).is_none());
        }
      }
    }
  }

  #[test]
  fn test_relative_specifier_within() {
    // File in the same directory - should return relative path
    let from = Url::parse("file:///Users/dev/project/").unwrap();
    let to = Url::parse("file:///Users/dev/project/foo.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, Some("./foo.ts".to_string()));

    // File in a subdirectory - should return relative path
    let from = Url::parse("file:///Users/dev/project/").unwrap();
    let to = Url::parse("file:///Users/dev/project/src/bar.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, Some("./src/bar.ts".to_string()));

    // File in a deeper subdirectory - should return relative path
    let from = Url::parse("file:///Users/dev/project/").unwrap();
    let to = Url::parse("file:///Users/dev/project/src/lib/utils.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, Some("./src/lib/utils.ts".to_string()));

    // File requires going up one level - should return None
    let from = Url::parse("file:///Users/dev/project/src/").unwrap();
    let to = Url::parse("file:///Users/dev/project/main.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, None);

    // File requires going up multiple levels - should return None
    let from = Url::parse("file:///Users/dev/project/src/lib/").unwrap();
    let to = Url::parse("file:///Users/dev/other/file.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, None);

    // Completely different path (e.g., bundled synthetic path) - should return None
    let from = Url::parse("file:///Users/dev/workspace/deno/").unwrap();
    let to = Url::parse("file:///a.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, None);

    // Same directory (edge case) - should return relative path
    let from = Url::parse("file:///Users/dev/project/").unwrap();
    let to = Url::parse("file:///Users/dev/project/").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, Some("./".to_string()));

    // File in sibling directory - should return None
    let from = Url::parse("file:///Users/dev/project1/src/").unwrap();
    let to = Url::parse("file:///Users/dev/project2/foo.ts").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, None);

    // HTTP URLs (not file://) - should work if in same path
    let from = Url::parse("http://example.com/app/").unwrap();
    let to = Url::parse("http://example.com/app/main.js").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, Some("./main.js".to_string()));

    // HTTP URLs requiring going up - should return None
    let from = Url::parse("http://example.com/app/src/").unwrap();
    let to = Url::parse("http://example.com/app/main.js").unwrap();
    let result = relative_specifier_within(&from, &to);
    assert_eq!(result, None);
  }

  #[cfg(not(miri))]
  #[test]
  fn test_to_v8_error_handles_null_builder_exception() {
    let mut runtime = JsRuntime::new(Default::default());

    deno_core::scope!(scope, runtime);

    let throw_null_fn: v8::Local<v8::Function> =
      JsRuntime::eval(scope, "(function () { throw null; })").unwrap();

    // Override custom error builder so it throws null
    let exception_state = JsRealm::exception_state_from_scope(scope);
    exception_state
      .js_build_custom_error_cb
      .borrow_mut()
      .replace(v8::Global::new(scope, throw_null_fn));

    let err = CoreErrorKind::TLA;
    let expected_message = err.get_message();

    let value = to_v8_error(scope, &err);
    let value: v8::Local<v8::String> = value
      .try_into()
      .expect("should fall back to message string");

    assert_eq!(value.to_rust_string_lossy(scope), expected_message);
  }
}
