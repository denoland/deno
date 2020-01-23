// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Note that source_map_mappings requires 0-indexed line and column numbers but
// V8 Exceptions are 1-indexed.

// TODO: This currently only applies to uncaught exceptions. It would be nice to
// also have source maps for situations like this:
//   const err = new Error("Boo!");
//   console.log(err.stack);
// It would require calling into Rust from Error.prototype.prepareStackTrace.

use crate::any_error::ErrBox;
use serde_json;
use serde_json::value::Value;
use std::error::Error;
use std::fmt;
use std::str;

#[derive(Debug, PartialEq, Clone)]
pub struct StackFrame {
  pub line: i64,   // zero indexed
  pub column: i64, // zero indexed
  pub script_name: String,
  pub function_name: String,
  pub is_eval: bool,
  pub is_constructor: bool,
  pub is_wasm: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct V8Exception {
  pub message: String,

  pub source_line: Option<String>,
  pub script_resource_name: Option<String>,
  pub line_number: Option<i64>,
  pub start_position: Option<i64>,
  pub end_position: Option<i64>,
  pub error_level: Option<i64>,
  pub start_column: Option<i64>,
  pub end_column: Option<i64>,

  pub frames: Vec<StackFrame>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct CoreJSError(V8Exception);

impl StackFrame {
  // TODO Maybe use serde_derive?
  fn from_json_value(v: &serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    let line_v = &obj["line"];
    if !line_v.is_u64() {
      return None;
    }
    let line = line_v.as_u64().unwrap() as i64;

    let column_v = &obj["column"];
    if !column_v.is_u64() {
      return None;
    }
    let column = column_v.as_u64().unwrap() as i64;

    let script_name_v = &obj["scriptName"];
    if !script_name_v.is_string() {
      return None;
    }
    let script_name = String::from(script_name_v.as_str().unwrap());

    // Optional fields. See EncodeExceptionAsJSON() in libdeno.
    // Sometimes V8 doesn't provide all the frame information.

    let mut function_name = String::from(""); // default
    if obj.contains_key("functionName") {
      let function_name_v = &obj["functionName"];
      if function_name_v.is_string() {
        function_name = String::from(function_name_v.as_str().unwrap());
      }
    }

    let mut is_eval = false; // default
    if obj.contains_key("isEval") {
      let is_eval_v = &obj["isEval"];
      if is_eval_v.is_boolean() {
        is_eval = is_eval_v.as_bool().unwrap();
      }
    }

    let mut is_constructor = false; // default
    if obj.contains_key("isConstructor") {
      let is_constructor_v = &obj["isConstructor"];
      if is_constructor_v.is_boolean() {
        is_constructor = is_constructor_v.as_bool().unwrap();
      }
    }

    let mut is_wasm = false; // default
    if obj.contains_key("isWasm") {
      let is_wasm_v = &obj["isWasm"];
      if is_wasm_v.is_boolean() {
        is_wasm = is_wasm_v.as_bool().unwrap();
      }
    }

    Some(StackFrame {
      line: line - 1,
      column: column - 1,
      script_name,
      function_name,
      is_eval,
      is_constructor,
      is_wasm,
    })
  }
}

impl V8Exception {
  /// Creates a new V8Exception by parsing the raw exception JSON string from V8.
  pub fn from_json(json_str: &str) -> Option<Self> {
    let v = serde_json::from_str::<serde_json::Value>(json_str);
    if let Err(err) = v {
      eprintln!("V8Exception::from_json got problem {}", err);
      return None;
    }
    let v = v.unwrap();
    Self::from_json_value(v)
  }

  pub fn from_json_value(v: serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    let message_v = &obj["message"];
    if !message_v.is_string() {
      return None;
    }
    let message = String::from(message_v.as_str().unwrap());

    let source_line = obj
      .get("sourceLine")
      .and_then(|v| v.as_str().map(String::from));
    let script_resource_name = obj
      .get("scriptResourceName")
      .and_then(|v| v.as_str().map(String::from));
    let line_number = obj.get("lineNumber").and_then(Value::as_i64);
    let start_position = obj.get("startPosition").and_then(Value::as_i64);
    let end_position = obj.get("endPosition").and_then(Value::as_i64);
    let error_level = obj.get("errorLevel").and_then(Value::as_i64);
    let start_column = obj.get("startColumn").and_then(Value::as_i64);
    let end_column = obj.get("endColumn").and_then(Value::as_i64);

    let frames_v = &obj["frames"];
    if !frames_v.is_array() {
      return None;
    }
    let frame_values = frames_v.as_array().unwrap();

    let mut frames = Vec::<StackFrame>::new();
    for frame_v in frame_values {
      match StackFrame::from_json_value(frame_v) {
        None => return None,
        Some(frame) => frames.push(frame),
      }
    }

    Some(V8Exception {
      message,
      source_line,
      script_resource_name,
      line_number,
      start_position,
      end_position,
      error_level,
      start_column,
      end_column,
      frames,
    })
  }
}

impl CoreJSError {
  pub fn from_v8_exception(v8_exception: V8Exception) -> ErrBox {
    let error = Self(v8_exception);
    ErrBox::from(error)
  }
}

fn format_source_loc(script_name: &str, line: i64, column: i64) -> String {
  // TODO match this style with how typescript displays errors.
  let line = line + 1;
  let column = column + 1;
  format!("{}:{}:{}", script_name, line, column)
}

fn format_stack_frame(frame: &StackFrame) -> String {
  // Note when we print to string, we change from 0-indexed to 1-indexed.
  let source_loc =
    format_source_loc(&frame.script_name, frame.line, frame.column);

  if !frame.function_name.is_empty() {
    format!("    at {} ({})", frame.function_name, source_loc)
  } else if frame.is_eval {
    format!("    at eval ({})", source_loc)
  } else {
    format!("    at {}", source_loc)
  }
}

impl fmt::Display for CoreJSError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if self.0.script_resource_name.is_some() {
      let script_resource_name = self.0.script_resource_name.as_ref().unwrap();
      if self.0.line_number.is_some() && self.0.start_column.is_some() {
        assert!(self.0.line_number.is_some());
        assert!(self.0.start_column.is_some());
        let source_loc = format_source_loc(
          script_resource_name,
          self.0.line_number.unwrap() - 1,
          self.0.start_column.unwrap() - 1,
        );
        write!(f, "{}", source_loc)?;
      }
      if self.0.source_line.is_some() {
        write!(f, "\n{}\n", self.0.source_line.as_ref().unwrap())?;
        let mut s = String::new();
        for i in 0..self.0.end_column.unwrap() {
          if i >= self.0.start_column.unwrap() {
            s.push('^');
          } else {
            s.push(' ');
          }
        }
        writeln!(f, "{}", s)?;
      }
    }

    write!(f, "{}", self.0.message)?;

    for frame in &self.0.frames {
      write!(f, "\n{}", format_stack_frame(frame))?;
    }
    Ok(())
  }
}

impl Error for CoreJSError {}

#[cfg(test)]
mod tests {
  use super::*;

  fn error1() -> V8Exception {
    V8Exception {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: None,
      end_column: None,
      frames: vec![
        StackFrame {
          line: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ],
    }
  }

  #[test]
  fn stack_frame_from_json_value_1() {
    let v = serde_json::from_str::<serde_json::Value>(
      r#"{
        "line":2,
        "column":11,
        "functionName":"foo",
        "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
        "isEval":true,
        "isConstructor":false,
        "isWasm":false
      }"#,
    )
    .unwrap();
    let r = StackFrame::from_json_value(&v);
    assert_eq!(
      r,
      Some(StackFrame {
        line: 1,
        column: 10,
        script_name: "/Users/rld/src/deno/tests/error_001.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: true,
        is_constructor: false,
        is_wasm: false,
      })
    );
  }

  #[test]
  fn stack_frame_from_json_value_2() {
    let v = serde_json::from_str::<serde_json::Value>(
      r#"{
        "scriptName": "/Users/rld/src/deno/tests/error_001.ts",
        "line": 2,
        "column": 11
      }"#,
    )
    .unwrap();
    let r = StackFrame::from_json_value(&v);
    assert!(r.is_some());
    let f = r.unwrap();
    assert_eq!(f.line, 1);
    assert_eq!(f.column, 10);
    assert_eq!(f.script_name, "/Users/rld/src/deno/tests/error_001.ts");
  }

  #[test]
  fn v8_exception_from_json() {
    let r = V8Exception::from_json(
      r#"{
        "message":"Uncaught Error: bad",
        "frames":[
          {
            "line":2,
            "column":11,
            "functionName":"foo",
            "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }, {
            "line":5,
            "column":5,
            "functionName":"bar",
            "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }
        ]}"#,
    );
    assert!(r.is_some());
    let e = r.unwrap();
    assert_eq!(e.message, "Uncaught Error: bad");
    assert_eq!(e.frames.len(), 2);
    assert_eq!(
      e.frames[0],
      StackFrame {
        line: 1,
        column: 10,
        script_name: "/Users/rld/src/deno/tests/error_001.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: true,
        is_constructor: false,
        is_wasm: false,
      }
    )
  }

  #[test]
  fn v8_exception_from_json_2() {
    let r = V8Exception::from_json(
      "{\"message\":\"Error: boo\",\"sourceLine\":\"throw Error('boo');\",\"scriptResourceName\":\"a.js\",\"lineNumber\":3,\"startPosition\":8,\"endPosition\":9,\"errorLevel\":8,\"startColumn\":6,\"endColumn\":7,\"isSharedCrossOrigin\":false,\"isOpaque\":false,\"frames\":[{\"line\":3,\"column\":7,\"functionName\":\"\",\"scriptName\":\"a.js\",\"isEval\":false,\"isConstructor\":false,\"isWasm\":false}]}"
    );
    assert!(r.is_some());
    let e = r.unwrap();
    assert_eq!(e.message, "Error: boo");
    assert_eq!(e.source_line, Some("throw Error('boo');".to_string()));
    assert_eq!(e.script_resource_name, Some("a.js".to_string()));
    assert_eq!(e.line_number, Some(3));
    assert_eq!(e.start_position, Some(8));
    assert_eq!(e.end_position, Some(9));
    assert_eq!(e.error_level, Some(8));
    assert_eq!(e.start_column, Some(6));
    assert_eq!(e.end_column, Some(7));
    assert_eq!(e.frames.len(), 1);
  }

  #[test]
  fn js_error_to_string() {
    let e = CoreJSError(error1());
    let expected = "Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2";
    assert_eq!(expected, &e.to_string());
  }
}
