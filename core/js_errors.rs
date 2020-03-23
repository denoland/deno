// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Note that source_map_mappings requires 0-indexed line and column numbers but
// V8 Exceptions are 1-indexed.

// TODO: This currently only applies to uncaught exceptions. It would be nice to
// also have source maps for situations like this:
//   const err = new Error("Boo!");
//   console.log(err.stack);
// It would require calling into Rust from Error.prototype.prepareStackTrace.

use crate::ErrBox;
use rusty_v8 as v8;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;

/// A `JSError` represents an exception coming from V8, with stack frames and
/// line numbers. The deno_cli crate defines another `JSError` type, which wraps
/// the one defined here, that adds source map support and colorful formatting.  
#[derive(Debug, PartialEq, Clone)]
pub struct JSError {
  pub message: String,
  pub source_line: Option<String>,
  pub script_resource_name: Option<String>,
  pub line_number: Option<i64>,
  pub start_column: Option<i64>,
  pub end_column: Option<i64>,
  pub frames: Vec<JSStackFrame>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct JSStackFrame {
  pub line_number: i64, // zero indexed
  pub column: i64,      // zero indexed
  pub script_name: String,
  pub function_name: String,
  pub is_eval: bool,
  pub is_constructor: bool,
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
    let context = scope.get_current_context().unwrap();

    let msg = v8::Exception::create_message(scope, exception);

    Self {
      message: msg.get(scope).to_rust_string_lossy(scope),
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
      frames: msg
        .get_stack_trace(scope)
        .map(|stack_trace| {
          (0..stack_trace.get_frame_count())
            .map(|i| {
              let frame = stack_trace.get_frame(scope, i).unwrap();
              JSStackFrame {
                line_number: frame
                  .get_line_number()
                  .checked_sub(1)
                  .and_then(|v| v.try_into().ok())
                  .unwrap(),
                column: frame
                  .get_column()
                  .checked_sub(1)
                  .and_then(|v| v.try_into().ok())
                  .unwrap(),
                script_name: frame
                  .get_script_name_or_source_url(scope)
                  .map(|v| v.to_rust_string_lossy(scope))
                  .unwrap_or_else(|| "<unknown>".to_owned()),
                function_name: frame
                  .get_function_name(scope)
                  .map(|v| v.to_rust_string_lossy(scope))
                  .unwrap_or_else(|| "".to_owned()),
                is_constructor: frame.is_constructor(),
                is_eval: frame.is_eval(),
              }
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_else(Vec::<_>::new),
    }
  }
}

impl Error for JSError {}

fn format_source_loc(
  script_name: &str,
  line_number: i64,
  column: i64,
) -> String {
  // TODO match this style with how typescript displays errors.
  let line_number = line_number + 1;
  let column = column + 1;
  format!("{}:{}:{}", script_name, line_number, column)
}

fn format_stack_frame(frame: &JSStackFrame) -> String {
  // Note when we print to string, we change from 0-indexed to 1-indexed.
  let source_loc =
    format_source_loc(&frame.script_name, frame.line_number, frame.column);

  if !frame.function_name.is_empty() {
    format!("    at {} ({})", frame.function_name, source_loc)
  } else if frame.is_eval {
    format!("    at eval ({})", source_loc)
  } else {
    format!("    at {}", source_loc)
  }
}

impl fmt::Display for JSError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if self.script_resource_name.is_some() {
      let script_resource_name = self.script_resource_name.as_ref().unwrap();
      if self.line_number.is_some() && self.start_column.is_some() {
        assert!(self.line_number.is_some());
        assert!(self.start_column.is_some());
        let source_loc = format_source_loc(
          script_resource_name,
          self.line_number.unwrap() - 1,
          self.start_column.unwrap() - 1,
        );
        write!(f, "{}", source_loc)?;
      }
      if self.source_line.is_some() {
        write!(f, "\n{}\n", self.source_line.as_ref().unwrap())?;
        let mut s = String::new();
        for i in 0..self.end_column.unwrap() {
          if i >= self.start_column.unwrap() {
            s.push('^');
          } else {
            s.push(' ');
          }
        }
        writeln!(f, "{}", s)?;
      }
    }

    write!(f, "{}", self.message)?;

    for frame in &self.frames {
      write!(f, "\n{}", format_stack_frame(frame))?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn js_error_to_string() {
    let js_error = JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_column: None,
      end_column: None,
      frames: vec![
        JSStackFrame {
          line_number: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
        },
      ],
    };
    let actual = js_error.to_string();
    let expected = "Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2";
    assert_eq!(actual, expected);
  }
}
