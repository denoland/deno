// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Note that source_map_mappings requires 0-indexed line and column numbers but
// V8 Exceptions are 1-indexed.

// TODO: This currently only applies to uncaught exceptions. It would be nice to
// also have source maps for situations like this:
//   const err = new Error("Boo!");
//   console.log(err.stack);
// It would require calling into Rust from Error.prototype.prepareStackTrace.

use serde_json;
use serde_json::value::Value;
use std::fmt;
use std::str;

const K_MESSAGE_LOG: i64 = (1 << 0);
const K_MESSAGE_DEBUG: i64 = (1 << 1);
const K_MESSAGE_INFO: i64 = (1 << 2);
const K_MESSAGE_ERROR: i64 = (1 << 3);
const K_MESSAGE_WARNING: i64 = (1 << 4);

#[derive(Debug, PartialEq, Clone)]
pub enum DenoDiagnosticCategory {
  Log,        // 0
  Debug,      // 1
  Info,       // 2
  Error,      // 3
  Warning,    // 4
  Suggestion, // 5
}

impl DenoDiagnosticCategory {
  fn from_v8_i64(value: i64) -> DenoDiagnosticCategory {
    match value {
      K_MESSAGE_LOG => DenoDiagnosticCategory::Log,
      K_MESSAGE_DEBUG => DenoDiagnosticCategory::Debug,
      K_MESSAGE_INFO => DenoDiagnosticCategory::Info,
      K_MESSAGE_ERROR => DenoDiagnosticCategory::Error,
      K_MESSAGE_WARNING => DenoDiagnosticCategory::Warning,
      _ => panic!("Unknown value: {}", value),
    }
  }

  fn from_i64(value: i64) -> DenoDiagnosticCategory {
    match value {
      0 => DenoDiagnosticCategory::Log,
      1 => DenoDiagnosticCategory::Debug,
      2 => DenoDiagnosticCategory::Info,
      3 => DenoDiagnosticCategory::Error,
      4 => DenoDiagnosticCategory::Warning,
      5 => DenoDiagnosticCategory::Suggestion,
      _ => panic!("Unknown value: {}", value),
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DenoDiagnosticSources {
  V8,         // 0
  Rust,       // 1
  TypeScript, // 2
  Runtime,    // 3
}

impl DenoDiagnosticSources {
  fn from_i64(value: i64) -> DenoDiagnosticSources {
    match value {
      0 => DenoDiagnosticSources::V8,
      1 => DenoDiagnosticSources::Rust,
      2 => DenoDiagnosticSources::TypeScript,
      3 => DenoDiagnosticSources::Runtime,
      _ => panic!("Unknown value: {}", value),
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DenoDiagnosticFrame {
  pub line: i64,   // zero indexed
  pub column: i64, // zero indexed
  pub script_name: String,
  pub function_name: String,
  pub is_eval: bool,
  pub is_constructor: bool,
  pub is_wasm: bool,
}

impl DenoDiagnosticFrame {
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

    Some(Self {
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

impl fmt::Display for DenoDiagnosticFrame {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let function_name = self.function_name.clone();
    // Note when we print to string, we change from 0-indexed to 1-indexed.
    let line = (1 + self.line).to_string();
    let column = (1 + self.column).to_string();
    let script_name = self.script_name.to_string();
    let script_line_column = format!("{}:{}:{}", script_name, line, column);

    if !self.function_name.is_empty() {
      write!(f, "    at {} ({})", function_name, script_line_column)
    } else if self.is_eval {
      write!(f, "    at eval ({})", script_line_column)
    } else {
      write!(f, "    at {}", script_line_column)
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DenoDiagnostic {
  pub message: String,
  pub diagnostics: Option<Vec<DenoDiagnostic>>,
  pub related_information: Option<Vec<DenoDiagnostic>>,
  pub source_line: Option<String>,
  pub line_number: Option<i64>,
  pub script_resource_name: Option<String>,
  pub start_position: Option<i64>,
  pub end_position: Option<i64>,
  pub category: DenoDiagnosticCategory,
  pub code: Option<i64>,
  pub source: DenoDiagnosticSources,
  pub start_column: Option<i64>,
  pub end_column: Option<i64>,
  pub frames: Option<Vec<DenoDiagnosticFrame>>,
  pub next: Option<Box<DenoDiagnostic>>,
}

impl DenoDiagnostic {
  pub fn from_compiler_json_value(v: &serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    // required properties
    let message_v = &obj["message"];
    if !message_v.is_string() {
      return None;
    }
    let message = String::from(message_v.as_str().unwrap());
    let category = DenoDiagnosticCategory::from_i64(
      obj.get("category").and_then(Value::as_i64).unwrap(),
    );
    let source = DenoDiagnosticSources::from_i64(
      obj.get("source").and_then(Value::as_i64).unwrap(),
    );

    let source_line = obj
      .get("sourceLine")
      .and_then(|v| v.as_str().map(String::from));
    let script_resource_name = obj
      .get("scriptResourceName")
      .and_then(|v| v.as_str().map(String::from));

    let code = obj.get("code").and_then(Value::as_i64);
    let line_number = obj.get("lineNumber").and_then(Value::as_i64);
    let start_position = obj.get("startPosition").and_then(Value::as_i64);
    let end_position = obj.get("endPosition").and_then(Value::as_i64);
    let start_column = obj.get("startColumn").and_then(Value::as_i64);
    let end_column = obj.get("endColumn").and_then(Value::as_i64);

    let diagnostics_v = obj.get("diagnostics");
    let diagnostics = match diagnostics_v {
      Some(d) => {
        let mut diagnostics = Vec::<DenoDiagnostic>::new();
        let diagnostic_values = d.as_array().unwrap();

        for diagnostic_v in diagnostic_values {
          match DenoDiagnostic::from_compiler_json_value(diagnostic_v) {
            Some(diagnostic) => diagnostics.push(diagnostic),
            None => return None,
          }
        }

        Some(diagnostics)
      }
      _ => None,
    };

    let related_information_v = obj.get("relatedInformation");
    let related_information = match related_information_v {
      Some(r) => {
        let mut related_information = Vec::<DenoDiagnostic>::new();
        let related_info_values = r.as_array().unwrap();

        for related_info_v in related_info_values {
          match DenoDiagnostic::from_compiler_json_value(related_info_v) {
            Some(related_info) => related_information.push(related_info),
            _ => return None,
          }
        }

        Some(related_information)
      }
      _ => None,
    };

    let next_v = obj.get("next");
    let next = match next_v {
      Some(n) => Some(Box::new(
        DenoDiagnostic::from_compiler_json_value(n).unwrap(),
      )),
      _ => None,
    };

    Some(DenoDiagnostic {
      message,
      diagnostics,
      related_information,
      code,
      source_line,
      script_resource_name,
      line_number,
      start_position,
      end_position,
      category,
      source,
      start_column,
      end_column,
      frames: None,
      next,
    })
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
    let category = match error_level {
      Some(v) => DenoDiagnosticCategory::from_v8_i64(v),
      _ => DenoDiagnosticCategory::Info,
    };
    let start_column = obj.get("startColumn").and_then(Value::as_i64);
    let end_column = obj.get("endColumn").and_then(Value::as_i64);

    let frames_v = &obj["frames"];
    if !frames_v.is_array() {
      return None;
    }
    let frame_values = frames_v.as_array().unwrap();

    let mut frames = Vec::<DenoDiagnosticFrame>::new();
    for frame_v in frame_values {
      match DenoDiagnosticFrame::from_json_value(frame_v) {
        None => return None,
        Some(frame) => frames.push(frame),
      }
    }

    Some(DenoDiagnostic {
      message,
      diagnostics: None,
      related_information: None,
      code: None,
      source_line,
      script_resource_name,
      line_number,
      start_position,
      end_position,
      category,
      source: DenoDiagnosticSources::V8,
      start_column,
      end_column,
      frames: Some(frames),
      next: None,
    })
  }

  pub fn from_v8_exception(json_str: &str) -> Option<Self> {
    let v = serde_json::from_str::<serde_json::Value>(json_str);
    if v.is_err() {
      return None;
    }
    let v = v.unwrap();
    Self::from_json_value(v)
  }
}

impl fmt::Display for DenoDiagnostic {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}{}{}{}{}",
      format_source_name(self, 0),
      format_category_and_code(self),
      format_message(self, 0),
      format_source_line(self, 0),
      format_related_info(self),
    )?;

    match &self.frames {
      Some(frames) => {
        for frame in frames {
          write!(f, "\n{}", &frame.to_string())?;
        }
      }
      _ => {}
    }

    match &self.next {
      Some(next) => write!(f, "\n{}", next.to_string())?,
      _ => (),
    }

    Ok(())
  }
}

impl std::error::Error for DenoDiagnostic {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    None
  }
}

/// Format the category and code for a given diagnostic.  This currently only
/// pertains to diagnostics coming from TypeScript.
fn format_category_and_code(diagnostic: &DenoDiagnostic) -> String {
  if diagnostic.source != DenoDiagnosticSources::TypeScript {
    return "".to_owned();
  }

  let category = match diagnostic.category {
    DenoDiagnosticCategory::Error => "- error",
    DenoDiagnosticCategory::Warning => "- warn",
    DenoDiagnosticCategory::Debug => "- debug",
    DenoDiagnosticCategory::Info => "- info",
    _ => "",
  };

  let code = match diagnostic.code {
    Some(code_int) => format!(" TS{}:", code_int.to_string()),
    None => "".to_owned(),
  };

  format!("{}{} ", category, code)
}

/// Format the message of a diagnostic.
fn format_message(diagnostic: &DenoDiagnostic, level: usize) -> String {
  if diagnostic.diagnostics.is_none() {
    return format!("{:indent$}{}", "", diagnostic.message, indent = level);
  }

  let mut s = String::new();
  let mut i = level / 2;
  for diagnostic in diagnostic.diagnostics.clone().unwrap() {
    s.push_str(&std::iter::repeat(" ").take(i * 2).collect::<String>());
    s.push_str(&diagnostic.message);
    s.push('\n');
    i += 1;
  }
  s.pop();

  s
}

/// Format the related information from a diagnostic.  Currently only TypeScript
/// diagnostics optionally contain related information, where the compiler
/// can advise on the source of a diagnostic error.
fn format_related_info(diagnostic: &DenoDiagnostic) -> String {
  if diagnostic.related_information.is_none() {
    return "".to_string();
  }

  let mut s = String::new();
  for related_diagnostic in diagnostic.related_information.clone().unwrap() {
    let rd = &related_diagnostic;
    s.push_str(&format!(
      "\n{}{}{}\n",
      format_source_name(rd, 2),
      format_source_line(rd, 4),
      format_message(rd, 4),
    ));
  }

  s
}

/// If a diagnostic contains a source line, return a string that formats it
/// underlining the span of code related to the diagnostic
fn format_source_line(diagnostic: &DenoDiagnostic, level: usize) -> String {
  if diagnostic.source_line.is_none() {
    return "".to_owned();
  }

  let source_line = diagnostic.source_line.as_ref().unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here
  if source_line.len() == 0 {
    return "".to_owned();
  }
  assert!(diagnostic.line_number.is_some());
  assert!(diagnostic.start_column.is_some());
  assert!(diagnostic.end_column.is_some());
  let line = match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.line_number.unwrap()).to_string()
    }
    _ => diagnostic.line_number.unwrap().to_string(),
  };
  let line_len = line.clone().len();
  let line_padding =
    format!("{:indent$}", "", indent = line_len);
  let mut s = String::new();
  let start_column = diagnostic.start_column.unwrap();
  let end_column = diagnostic.end_column.unwrap();
  // TypeScript uses `~` always, but V8 would utilise `^` always, even when
  // doing ranges, so here, if we only have one marker (very common with V8
  // errors) we will use `^` instead.
  let underline_char =
    if (end_column - start_column) <= 1 {
      '^'
    } else {
      '~'
    };
  for i in 0..end_column {
    if i >= start_column {
      s.push(underline_char);
    } else {
      s.push(' ');
    }
  }

  let indent = format!("{:indent$}", "", indent = level);

  format!(
    "\n\n{}{} {}\n{}{} {}\n",
    indent, line, source_line, indent, line_padding, s
  )
}

/// Format the source resource name, along with line and column information from
/// a diagnostic into a single line.
fn format_source_name(
  diagnostic: &DenoDiagnostic,
  level: usize,
) -> String {
  if diagnostic.script_resource_name.is_none() {
    return "".to_owned();
  }

  let script_name = diagnostic.script_resource_name.clone().unwrap();
  assert!(diagnostic.line_number.is_some());
  assert!(diagnostic.start_column.is_some());
  let line = match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.line_number.unwrap()).to_string()
    }
    _ => diagnostic.line_number.unwrap().to_string(),
  };
  let column = match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.start_column.unwrap()).to_string()
    }
    _ => diagnostic.start_column.unwrap().to_string(),
  };
  format!(
    "{:indent$}{}:{}:{} ",
    "",
    script_name,
    line,
    column,
    indent = level
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn diagnostic_js() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Error: foo bar".to_string(),
      diagnostics: None,
      related_information: None,
      code: None,
      source: DenoDiagnosticSources::V8,
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      category: DenoDiagnosticCategory::Error,
      start_column: None,
      end_column: None,
      frames: Some(vec![
        DenoDiagnosticFrame {
          line: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ]),
      next: None,
    }
  }

  fn diagnostic_ts1() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
      diagnostics: Some(vec![
        DenoDiagnostic {
          message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2322),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
        DenoDiagnostic {
          message: "Types of parameters 'o' and 'r' are incompatible.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2328),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
        DenoDiagnostic {
          message: "Type 'B' is not assignable to type 'T'.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2322),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
      ]),
      code: Some(2322),
      category: DenoDiagnosticCategory::Error,
      source: DenoDiagnosticSources::TypeScript,
      start_position: Some(267),
      end_position: Some(273),
      source_line: Some("  values: o => [".to_string()),
      line_number: Some(18),
      script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
      start_column: Some(2),
      end_column: Some(8),
      related_information: Some(vec![
        DenoDiagnostic {
          message: "The expected type comes from property 'values' which is declared here on type 'SettingsInterface<B>'".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(6500),
          source_line: Some("  values?: (r: T) => Array<Value<T>>;".to_string()),
          script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
          line_number: Some(6),
          start_position: Some(94),
          end_position: Some(100),
          category: DenoDiagnosticCategory::Info,
          start_column: Some(2),
          end_column: Some(8),
          frames: None,
          next: None,
        }
      ]),
      frames: None,
      next: None,
    }
  }

  fn diagnostic_ts2() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Example 1".to_string(),
      diagnostics: None,
      code: Some(2322),
      category: DenoDiagnosticCategory::Error,
      source: DenoDiagnosticSources::TypeScript,
      start_position: Some(267),
      end_position: Some(273),
      source_line: Some("  values: o => [".to_string()),
      line_number: Some(18),
      script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
      start_column: Some(2),
      end_column: Some(8),
      related_information: None,
      frames: None,
      next: Some(Box::new(DenoDiagnostic {
        message: "Example 2".to_string(),
        diagnostics: None,
        code: Some(2000),
        category: DenoDiagnosticCategory::Error,
        source: DenoDiagnosticSources::TypeScript,
        start_position: Some(2),
        end_position: Some(2),
        source_line: Some("  values: undefined,".to_string()),
        line_number: Some(128),
        script_resource_name: Some("/foo/bar.ts".to_string()),
        start_column: Some(2),
        end_column: Some(8),
        related_information: None,
        frames: None,
        next: None,
      })),
    }
  }

  #[test]
  fn stack_frame_from_json_value_1() {
    let v = serde_json::from_str::<serde_json::Value>(
      r#"{
        "line":2,
        "column":11,
        "functionName":"foo",
        "scriptName":"/deno/tests/error_001.ts",
        "isEval":true,
        "isConstructor":false,
        "isWasm":false
      }"#,
    ).unwrap();
    let r = DenoDiagnosticFrame::from_json_value(&v);
    assert_eq!(
      r,
      Some(DenoDiagnosticFrame {
        line: 1,
        column: 10,
        script_name: "/deno/tests/error_001.ts".to_string(),
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
        "scriptName": "/deno/tests/error_001.ts",
        "line": 2,
        "column": 11
      }"#,
    ).unwrap();
    let r = DenoDiagnosticFrame::from_json_value(&v);
    assert!(r.is_some());
    let f = r.unwrap();
    assert_eq!(f.line, 1);
    assert_eq!(f.column, 10);
    assert_eq!(f.script_name, "/deno/tests/error_001.ts");
  }

  #[test]
  fn js_error_from_v8_exception() {
    let r = DenoDiagnostic::from_v8_exception(
      r#"{
        "message":"Uncaught Error: bad",
        "frames":[
          {
            "line":2,
            "column":11,
            "functionName":"foo",
            "scriptName":"/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }, {
            "line":5,
            "column":5,
            "functionName":"bar",
            "scriptName":"/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }
        ]}"#,
    );
    assert!(r.is_some());
    let e = r.unwrap();
    assert_eq!(e.message, "Uncaught Error: bad");
    let frames = e.frames.unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(
      frames[0],
      DenoDiagnosticFrame {
        line: 1,
        column: 10,
        script_name: "/deno/tests/error_001.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: true,
        is_constructor: false,
        is_wasm: false,
      }
    )
  }

  #[test]
  fn js_error_from_v8_exception2() {
    let r = DenoDiagnostic::from_v8_exception(
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
    assert_eq!(e.category, DenoDiagnosticCategory::Error);
    assert_eq!(e.start_column, Some(6));
    assert_eq!(e.end_column, Some(7));
    assert_eq!(e.frames.unwrap().len(), 1);
  }

  #[test]
  fn stack_frame_to_string() {
    let e = diagnostic_js();
    let frames = e.frames.unwrap();
    assert_eq!("    at foo (foo_bar.ts:5:17)", &frames[0].to_string());
    assert_eq!("    at qat (bar_baz.ts:6:21)", &frames[1].to_string());
  }

  #[test]
  fn js_error_to_string() {
    let e = diagnostic_js();
    let expected = "Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2";
    assert_eq!(expected, &e.to_string());
  }

  #[test]
  fn ts_diagnostic_to_string1() {
    let d = diagnostic_ts1();
    let expected = "deno/tests/complex_diagnostics.ts:19:3 - error TS2322: Type \'(o: T) => { v: any; f: (x: B) => string; }[]\' is not assignable to type \'(r: B) => Value<B>[]\'.\n  Types of parameters \'o\' and \'r\' are incompatible.\n    Type \'B\' is not assignable to type \'T\'.\n\n19   values: o => [\n     ~~~~~~\n\n  deno/tests/complex_diagnostics.ts:7:3 \n\n    7   values?: (r: T) => Array<Value<T>>;\n        ~~~~~~\n    The expected type comes from property \'values\' which is declared here on type \'SettingsInterface<B>\'\n";
    assert_eq!(expected, &d.to_string());
  }

  #[test]
  fn ts_diagnostic_to_string2() {
    let d = diagnostic_ts2();
    let expected = "deno/tests/complex_diagnostics.ts:19:3 - error TS2322: Example 1\n\n19   values: o => [\n     ~~~~~~\n\n/foo/bar.ts:129:3 - error TS2000: Example 2\n\n129   values: undefined,\n      ~~~~~~\n";
    assert_eq!(expected, &d.to_string());
  }
}
