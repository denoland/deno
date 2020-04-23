// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This module encodes TypeScript errors (diagnostics) into Rust structs and
//! contains code for printing them to the console.

// TODO(ry) This module does a lot of JSON parsing manually. It should use
// serde_json.

use crate::colors;
use crate::fmt_errors::format_stack;
use serde_json::value::Value;
use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub struct Diagnostic {
  pub items: Vec<DiagnosticItem>,
}

impl Diagnostic {
  /// Take a JSON value and attempt to map it to a
  pub fn from_json_value(v: &serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    let mut items = Vec::<DiagnosticItem>::new();
    let items_v = &obj["items"];
    if items_v.is_array() {
      let items_values = items_v.as_array().unwrap();

      for item_v in items_values {
        items.push(DiagnosticItem::from_json_value(item_v)?);
      }
    }

    Some(Self { items })
  }

  pub fn from_emit_result(json_str: &str) -> Option<Self> {
    let v = serde_json::from_str::<serde_json::Value>(json_str)
      .expect("Error decoding JSON string.");
    let diagnostics_o = v.get("diagnostics");
    if let Some(diagnostics_v) = diagnostics_o {
      return Self::from_json_value(diagnostics_v);
    }

    None
  }
}

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut i = 0;
    for item in &self.items {
      if i > 0 {
        write!(f, "\n\n")?;
      }
      write!(f, "{}", item.to_string())?;
      i += 1;
    }

    if i > 1 {
      write!(f, "\n\nFound {} errors.", i)?;
    }

    Ok(())
  }
}

impl Error for Diagnostic {
  fn description(&self) -> &str {
    &self.items[0].message
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DiagnosticItem {
  /// The top level message relating to the diagnostic item.
  pub message: String,

  /// A chain of messages, code, and categories of messages which indicate the
  /// full diagnostic information.
  pub message_chain: Option<DiagnosticMessageChain>,

  /// Other diagnostic items that are related to the diagnostic, usually these
  /// are suggestions of why an error occurred.
  pub related_information: Option<Vec<DiagnosticItem>>,

  /// The source line the diagnostic is in reference to.
  pub source_line: Option<String>,

  /// Zero-based index to the line number of the error.
  pub line_number: Option<i64>,

  /// The resource name provided to the TypeScript compiler.
  pub script_resource_name: Option<String>,

  /// Zero-based index to the start position in the entire script resource.
  pub start_position: Option<i64>,

  /// Zero-based index to the end position in the entire script resource.
  pub end_position: Option<i64>,
  pub category: DiagnosticCategory,

  /// This is defined in TypeScript and can be referenced via
  /// [diagnosticMessages.json](https://github.com/microsoft/TypeScript/blob/master/src/compiler/diagnosticMessages.json).
  pub code: i64,

  /// Zero-based index to the start column on `line_number`.
  pub start_column: Option<i64>,

  /// Zero-based index to the end column on `line_number`.
  pub end_column: Option<i64>,
}

impl DiagnosticItem {
  pub fn from_json_value(v: &serde_json::Value) -> Option<Self> {
    let obj = v.as_object().unwrap();

    // required attributes
    let message = obj
      .get("message")
      .and_then(|v| v.as_str().map(String::from))?;
    let category = DiagnosticCategory::from(
      obj.get("category").and_then(Value::as_i64).unwrap(),
    );
    let code = obj.get("code").and_then(Value::as_i64).unwrap();

    // optional attributes
    let source_line = obj
      .get("sourceLine")
      .and_then(|v| v.as_str().map(String::from));
    let script_resource_name = obj
      .get("scriptResourceName")
      .and_then(|v| v.as_str().map(String::from));
    let line_number = obj.get("lineNumber").and_then(Value::as_i64);
    let start_position = obj.get("startPosition").and_then(Value::as_i64);
    let end_position = obj.get("endPosition").and_then(Value::as_i64);
    let start_column = obj.get("startColumn").and_then(Value::as_i64);
    let end_column = obj.get("endColumn").and_then(Value::as_i64);

    let message_chain_v = obj.get("messageChain");
    let message_chain = match message_chain_v {
      Some(v) => DiagnosticMessageChain::from_json_value(v),
      _ => None,
    };

    let related_information_v = obj.get("relatedInformation");
    let related_information = match related_information_v {
      Some(r) => {
        let mut related_information = Vec::<DiagnosticItem>::new();
        let related_info_values = r.as_array().unwrap();

        for related_info_v in related_info_values {
          related_information
            .push(DiagnosticItem::from_json_value(related_info_v)?);
        }

        Some(related_information)
      }
      _ => None,
    };

    Some(Self {
      message,
      message_chain,
      related_information,
      code,
      source_line,
      script_resource_name,
      line_number,
      start_position,
      end_position,
      category,
      start_column,
      end_column,
    })
  }
}

fn format_category_and_code(
  category: &DiagnosticCategory,
  code: i64,
) -> String {
  let category = match category {
    DiagnosticCategory::Error => {
      format!("{}", colors::red_bold("error".to_string()))
    }
    DiagnosticCategory::Warning => "warn".to_string(),
    DiagnosticCategory::Debug => "debug".to_string(),
    DiagnosticCategory::Info => "info".to_string(),
    _ => "".to_string(),
  };

  let code = colors::bold(format!("TS{}", code.to_string())).to_string();

  format!("{} {}", category, code)
}

fn format_message(
  message_chain: &Option<DiagnosticMessageChain>,
  message: &str,
  level: usize,
) -> String {
  debug!("format_message");
  if message_chain.is_none() {
    return format!("{:indent$}{}", "", message, indent = level);
  }

  let mut s = message_chain.clone().unwrap().format_message(level);
  s.pop();

  s
}

/// Formats optional source, line and column numbers into a single string.
fn format_maybe_frame(
  file_name: Option<String>,
  line_number: Option<i64>,
  column_number: Option<i64>,
) -> String {
  if file_name.is_none() {
    return "".to_string();
  }

  assert!(line_number.is_some());
  assert!(column_number.is_some());

  let line_number = line_number.unwrap();
  let column_number = column_number.unwrap();
  let file_name_c = colors::cyan(file_name.unwrap());
  let line_c = colors::yellow(line_number.to_string());
  let column_c = colors::yellow(column_number.to_string());
  format!("{}:{}:{}", file_name_c, line_c, column_c)
}

fn format_maybe_related_information(
  related_information: &Option<Vec<DiagnosticItem>>,
) -> String {
  if related_information.is_none() {
    return "".to_string();
  }

  let mut s = String::new();
  let related_information = related_information.clone().unwrap();
  for rd in related_information {
    s.push_str("\n\n");
    s.push_str(&format_stack(
      match rd.category {
        DiagnosticCategory::Error => true,
        _ => false,
      },
      format_message(&rd.message_chain, &rd.message, 0),
      rd.source_line.clone(),
      rd.start_column,
      rd.end_column,
      // Formatter expects 1-based line and column numbers, but ours are 0-based.
      &[format_maybe_frame(
        rd.script_resource_name.clone(),
        rd.line_number.map(|n| n + 1),
        rd.start_column.map(|n| n + 1),
      )],
      4,
    ));
  }

  s
}

impl fmt::Display for DiagnosticItem {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}",
      format_stack(
        match self.category {
          DiagnosticCategory::Error => true,
          _ => false,
        },
        format!(
          "{}: {}",
          format_category_and_code(&self.category, self.code),
          format_message(&self.message_chain, &self.message, 0)
        ),
        self.source_line.clone(),
        self.start_column,
        self.end_column,
        // Formatter expects 1-based line and column numbers, but ours are 0-based.
        &[format_maybe_frame(
          self.script_resource_name.clone(),
          self.line_number.map(|n| n + 1),
          self.start_column.map(|n| n + 1)
        )],
        0
      )
    )?;
    write!(
      f,
      "{}",
      format_maybe_related_information(&self.related_information),
    )
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DiagnosticMessageChain {
  pub message: String,
  pub code: i64,
  pub category: DiagnosticCategory,
  pub next: Option<Vec<DiagnosticMessageChain>>,
}

impl DiagnosticMessageChain {
  fn from_value(v: &serde_json::Value) -> Self {
    let obj = v.as_object().unwrap();
    let message = obj
      .get("message")
      .and_then(|v| v.as_str().map(String::from))
      .unwrap();
    let code = obj.get("code").and_then(Value::as_i64).unwrap();
    let category = DiagnosticCategory::from(
      obj.get("category").and_then(Value::as_i64).unwrap(),
    );

    let next_v = obj.get("next");
    let next = match next_v {
      Some(n) => DiagnosticMessageChain::from_next_array(n),
      _ => None,
    };

    Self {
      message,
      code,
      category,
      next,
    }
  }

  fn from_next_array(v: &serde_json::Value) -> Option<Vec<Self>> {
    if !v.is_array() {
      return None;
    }

    let vec = v
      .as_array()
      .unwrap()
      .iter()
      .map(|item| Self::from_value(&item))
      .collect::<Vec<Self>>();

    Some(vec)
  }

  pub fn from_json_value(v: &serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }

    Some(Self::from_value(v))
  }

  pub fn format_message(&self, level: usize) -> String {
    let mut s = String::new();

    s.push_str(&std::iter::repeat(" ").take(level * 2).collect::<String>());
    s.push_str(&self.message);
    s.push('\n');
    if self.next.is_some() {
      let arr = self.next.clone().unwrap();
      for dm in arr {
        s.push_str(&dm.format_message(level + 1));
      }
    }

    s
  }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DiagnosticCategory {
  Log,        // 0
  Debug,      // 1
  Info,       // 2
  Error,      // 3
  Warning,    // 4
  Suggestion, // 5
}

impl From<i64> for DiagnosticCategory {
  fn from(value: i64) -> Self {
    match value {
      0 => DiagnosticCategory::Log,
      1 => DiagnosticCategory::Debug,
      2 => DiagnosticCategory::Info,
      3 => DiagnosticCategory::Error,
      4 => DiagnosticCategory::Warning,
      5 => DiagnosticCategory::Suggestion,
      _ => panic!("Unknown value: {}", value),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::colors::strip_ansi_codes;

  fn diagnostic1() -> Diagnostic {
    Diagnostic {
      items: vec![
        DiagnosticItem {
          message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
          message_chain: Some(DiagnosticMessageChain {
            message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
            code: 2322,
            category: DiagnosticCategory::Error,
            next: Some(vec![DiagnosticMessageChain {
              message: "Types of parameters 'o' and 'r' are incompatible.".to_string(),
              code: 2328,
              category: DiagnosticCategory::Error,
              next: Some(vec![DiagnosticMessageChain {
                message: "Type 'B' is not assignable to type 'T'.".to_string(),
                code: 2322,
                category: DiagnosticCategory::Error,
                next: None,
              }]),
            }]),
          }),
          code: 2322,
          category: DiagnosticCategory::Error,
          start_position: Some(267),
          end_position: Some(273),
          source_line: Some("  values: o => [".to_string()),
          line_number: Some(18),
          script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
          start_column: Some(2),
          end_column: Some(8),
          related_information: Some(vec![
            DiagnosticItem {
              message: "The expected type comes from property 'values' which is declared here on type 'SettingsInterface<B>'".to_string(),
              message_chain: None,
              related_information: None,
              code: 6500,
              source_line: Some("  values?: (r: T) => Array<Value<T>>;".to_string()),
              script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
              line_number: Some(6),
              start_position: Some(94),
              end_position: Some(100),
              category: DiagnosticCategory::Info,
              start_column: Some(2),
              end_column: Some(8),
            }
          ])
        }
      ]
    }
  }

  fn diagnostic2() -> Diagnostic {
    Diagnostic {
      items: vec![
        DiagnosticItem {
          message: "Example 1".to_string(),
          message_chain: None,
          code: 2322,
          category: DiagnosticCategory::Error,
          start_position: Some(267),
          end_position: Some(273),
          source_line: Some("  values: o => [".to_string()),
          line_number: Some(18),
          script_resource_name: Some(
            "deno/tests/complex_diagnostics.ts".to_string(),
          ),
          start_column: Some(2),
          end_column: Some(8),
          related_information: None,
        },
        DiagnosticItem {
          message: "Example 2".to_string(),
          message_chain: None,
          code: 2000,
          category: DiagnosticCategory::Error,
          start_position: Some(2),
          end_position: Some(2),
          source_line: Some("  values: undefined,".to_string()),
          line_number: Some(128),
          script_resource_name: Some("/foo/bar.ts".to_string()),
          start_column: Some(2),
          end_column: Some(8),
          related_information: None,
        },
      ],
    }
  }

  #[test]
  fn from_json() {
    let v = serde_json::from_str::<serde_json::Value>(
      &r#"{
        "items": [
          {
            "message": "Type '{ a(): { b: number; }; }' is not assignable to type '{ a(): { b: string; }; }'.",
            "messageChain": {
              "message": "Type '{ a(): { b: number; }; }' is not assignable to type '{ a(): { b: string; }; }'.",
              "code": 2322,
              "category": 3,
              "next": [
                {
                  "message": "Types of property 'a' are incompatible.",
                  "code": 2326,
                  "category": 3
                }
              ]
            },
            "code": 2322,
            "category": 3,
            "startPosition": 352,
            "endPosition": 353,
            "sourceLine": "x = y;",
            "lineNumber": 29,
            "scriptResourceName": "/deno/tests/error_003_typescript.ts",
            "startColumn": 0,
            "endColumn": 1
          }
        ]
      }"#,
    ).unwrap();
    let r = Diagnostic::from_json_value(&v);
    let expected = Some(
      Diagnostic {
        items: vec![
          DiagnosticItem {
            message: "Type \'{ a(): { b: number; }; }\' is not assignable to type \'{ a(): { b: string; }; }\'.".to_string(),
            message_chain: Some(
              DiagnosticMessageChain {
                message: "Type \'{ a(): { b: number; }; }\' is not assignable to type \'{ a(): { b: string; }; }\'.".to_string(),
                code: 2322,
                category: DiagnosticCategory::Error,
                next: Some(vec![
                  DiagnosticMessageChain {
                    message: "Types of property \'a\' are incompatible.".to_string(),
                    code: 2326,
                    category: DiagnosticCategory::Error,
                    next: None,
                  }
                ])
              }
            ),
            related_information: None,
            source_line: Some("x = y;".to_string()),
            line_number: Some(29),
            script_resource_name: Some("/deno/tests/error_003_typescript.ts".to_string()),
            start_position: Some(352),
            end_position: Some(353),
            category: DiagnosticCategory::Error,
            code: 2322,
            start_column: Some(0),
            end_column: Some(1)
          }
        ]
      }
    );
    assert_eq!(expected, r);
  }

  #[test]
  fn from_emit_result() {
    let r = Diagnostic::from_emit_result(
      &r#"{
      "emitSkipped": false,
      "diagnostics": {
        "items": [
          {
            "message": "foo bar",
            "code": 9999,
            "category": 3
          }
        ]
      }
    }"#,
    );
    let expected = Some(Diagnostic {
      items: vec![DiagnosticItem {
        message: "foo bar".to_string(),
        message_chain: None,
        related_information: None,
        source_line: None,
        line_number: None,
        script_resource_name: None,
        start_position: None,
        end_position: None,
        category: DiagnosticCategory::Error,
        code: 9999,
        start_column: None,
        end_column: None,
      }],
    });
    assert_eq!(expected, r);
  }

  #[test]
  fn from_emit_result_none() {
    let r = &r#"{"emitSkipped":false}"#;
    assert!(Diagnostic::from_emit_result(r).is_none());
  }

  #[test]
  fn diagnostic_to_string1() {
    let d = diagnostic1();
    let expected = "error TS2322: Type \'(o: T) => { v: any; f: (x: B) => string; }[]\' is not assignable to type \'(r: B) => Value<B>[]\'.\n  Types of parameters \'o\' and \'r\' are incompatible.\n    Type \'B\' is not assignable to type \'T\'.\n  values: o => [\n  ~~~~~~\n    at deno/tests/complex_diagnostics.ts:19:3\n\n    The expected type comes from property \'values\' which is declared here on type \'SettingsInterface<B>\'\n      values?: (r: T) => Array<Value<T>>;\n      ~~~~~~\n        at deno/tests/complex_diagnostics.ts:7:3";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }

  #[test]
  fn diagnostic_to_string2() {
    let d = diagnostic2();
    let expected = "error TS2322: Example 1\n  values: o => [\n  ~~~~~~\n    at deno/tests/complex_diagnostics.ts:19:3\n\nerror TS2000: Example 2\n  values: undefined,\n  ~~~~~~\n    at /foo/bar.ts:129:3\n\nFound 2 errors.";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }

  #[test]
  fn test_format_none_frame() {
    let actual = format_maybe_frame(None, None, None);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_frame() {
    let actual = format_maybe_frame(
      Some("file://foo/bar.ts".to_string()),
      Some(1),
      Some(2),
    );
    assert_eq!(strip_ansi_codes(&actual), "file://foo/bar.ts:1:2");
  }
}
