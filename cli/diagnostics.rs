// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//! This module encodes TypeScript errors (diagnostics) into Rust structs and
//! contains code for printing them to the console.
use crate::ansi;
use crate::fmt_errors::format_maybe_source_line;
use crate::fmt_errors::format_maybe_source_name;
use crate::fmt_errors::DisplayFormatter;
use serde_json;
use serde_json::value::Value;
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
        items.push(DiagnosticItem::from_json_value(item_v));
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
        writeln!(f)?;
      }
      write!(f, "{}", item.to_string())?;
      i += 1;
    }

    if i > 1 {
      write!(f, "\n\nFound {} errors.\n", i)?;
    }

    Ok(())
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DiagnosticItem {
  /// The top level message relating to the diagnostic item.
  pub message: String,

  /// A chain of messages, code, and categories of messages which indicate the
  /// full diagnostic information.
  pub message_chain: Option<Box<DiagnosticMessageChain>>,

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
  pub fn from_json_value(v: &serde_json::Value) -> Self {
    let obj = v.as_object().unwrap();

    // required attributes
    let message = obj
      .get("message")
      .and_then(|v| v.as_str().map(String::from))
      .unwrap();
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
            .push(DiagnosticItem::from_json_value(related_info_v));
        }

        Some(related_information)
      }
      _ => None,
    };

    Self {
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
    }
  }
}

impl DisplayFormatter for DiagnosticItem {
  fn format_category_and_code(&self) -> String {
    let category = match self.category {
      DiagnosticCategory::Error => {
        format!("{}", ansi::red_bold("error".to_string()))
      }
      DiagnosticCategory::Warning => "warn".to_string(),
      DiagnosticCategory::Debug => "debug".to_string(),
      DiagnosticCategory::Info => "info".to_string(),
      _ => "".to_string(),
    };

    let code = ansi::bold(format!(" TS{}", self.code.to_string())).to_string();

    format!("{}{}: ", category, code)
  }

  fn format_message(&self, level: usize) -> String {
    if self.message_chain.is_none() {
      return format!("{:indent$}{}", "", self.message, indent = level);
    }

    let mut s = String::new();
    let mut i = level / 2;
    let mut item_o = self.message_chain.clone();
    while item_o.is_some() {
      let item = item_o.unwrap();
      s.push_str(&std::iter::repeat(" ").take(i * 2).collect::<String>());
      s.push_str(&item.message);
      s.push('\n');
      item_o = item.next.clone();
      i += 1;
    }
    s.pop();

    s
  }

  fn format_related_info(&self) -> String {
    if self.related_information.is_none() {
      return "".to_string();
    }

    let mut s = String::new();
    let related_information = self.related_information.clone().unwrap();
    for related_diagnostic in related_information {
      let rd = &related_diagnostic;
      s.push_str(&format!(
        "\n{}\n\n    ► {}{}\n",
        rd.format_message(2),
        rd.format_source_name(),
        rd.format_source_line(4),
      ));
    }

    s
  }

  fn format_source_line(&self, level: usize) -> String {
    format_maybe_source_line(
      self.source_line.clone(),
      self.line_number,
      self.start_column,
      self.end_column,
      match self.category {
        DiagnosticCategory::Error => true,
        _ => false,
      },
      level,
    )
  }

  fn format_source_name(&self) -> String {
    format_maybe_source_name(
      self.script_resource_name.clone(),
      self.line_number,
      self.start_column,
    )
  }
}

impl fmt::Display for DiagnosticItem {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}{}\n\n► {}{}{}",
      self.format_category_and_code(),
      self.format_message(0),
      self.format_source_name(),
      self.format_source_line(0),
      self.format_related_info(),
    )
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DiagnosticMessageChain {
  pub message: String,
  pub code: i64,
  pub category: DiagnosticCategory,
  pub next: Option<Box<DiagnosticMessageChain>>,
}

impl DiagnosticMessageChain {
  pub fn from_json_value(v: &serde_json::Value) -> Option<Box<Self>> {
    if !v.is_object() {
      return None;
    }

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
      Some(n) => DiagnosticMessageChain::from_json_value(n),
      _ => None,
    };

    Some(Box::new(Self {
      message,
      code,
      category,
      next,
    }))
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
  use crate::ansi::strip_ansi_codes;

  fn diagnostic1() -> Diagnostic {
    Diagnostic {
      items: vec![
        DiagnosticItem {
          message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
          message_chain: Some(Box::new(DiagnosticMessageChain {
            message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
            code: 2322,
            category: DiagnosticCategory::Error,
            next: Some(Box::new(DiagnosticMessageChain {
              message: "Types of parameters 'o' and 'r' are incompatible.".to_string(),
              code: 2328,
              category: DiagnosticCategory::Error,
              next: Some(Box::new(DiagnosticMessageChain {
                message: "Type 'B' is not assignable to type 'T'.".to_string(),
                code: 2322,
                category: DiagnosticCategory::Error,
                next: None,
              })),
            })),
          })),
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
            "message": "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.",
            "messageChain": {
              "message": "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.",
              "code": 2322,
              "category": 3,
              "next": {
                "message": "Types of parameters 'o' and 'r' are incompatible.",
                "code": 2328,
                "category": 3,
                "next": {
                  "message": "Type 'B' is not assignable to type 'T'.",
                  "code": 2322,
                  "category": 3
                }
              }
            },
            "code": 2322,
            "category": 3,
            "startPosition": 235,
            "endPosition": 241,
            "sourceLine": "  values: o => [",
            "lineNumber": 18,
            "scriptResourceName": "/deno/tests/complex_diagnostics.ts",
            "startColumn": 2,
            "endColumn": 8,
            "relatedInformation": [
              {
                "message": "The expected type comes from property 'values' which is declared here on type 'C<B>'",
                "code": 6500,
                "category": 2,
                "startPosition": 78,
                "endPosition": 84,
                "sourceLine": "  values?: (r: T) => Array<Value<T>>;",
                "lineNumber": 6,
                "scriptResourceName": "/deno/tests/complex_diagnostics.ts",
                "startColumn": 2,
                "endColumn": 8
              }
            ]
          },
          {
            "message": "Property 't' does not exist on type 'T'.",
            "code": 2339,
            "category": 3,
            "startPosition": 267,
            "endPosition": 268,
            "sourceLine": "      v: o.t,",
            "lineNumber": 20,
            "scriptResourceName": "/deno/tests/complex_diagnostics.ts",
            "startColumn": 11,
            "endColumn": 12
          }
        ]
      }"#,
    ).unwrap();
    let r = Diagnostic::from_json_value(&v);
    let expected = Some(Diagnostic {
      items: vec![
        DiagnosticItem {
          message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
          message_chain: Some(Box::new(DiagnosticMessageChain {
            message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
            code: 2322,
            category: DiagnosticCategory::Error,
            next: Some(Box::new(DiagnosticMessageChain {
              message: "Types of parameters 'o' and 'r' are incompatible.".to_string(),
              code: 2328,
              category: DiagnosticCategory::Error,
              next: Some(Box::new(DiagnosticMessageChain {
                message: "Type 'B' is not assignable to type 'T'.".to_string(),
                code: 2322,
                category: DiagnosticCategory::Error,
                next: None,
              })),
            })),
          })),
          related_information: Some(vec![
            DiagnosticItem {
              message: "The expected type comes from property 'values' which is declared here on type 'C<B>'".to_string(),
              message_chain: None,
              related_information: None,
              source_line: Some("  values?: (r: T) => Array<Value<T>>;".to_string()),
              line_number: Some(6),
              script_resource_name: Some("/deno/tests/complex_diagnostics.ts".to_string()),
              start_position: Some(78),
              end_position: Some(84),
              category: DiagnosticCategory::Info,
              code: 6500,
              start_column: Some(2),
              end_column: Some(8),
            }
          ]),
          source_line: Some("  values: o => [".to_string()),
          line_number: Some(18),
          script_resource_name: Some("/deno/tests/complex_diagnostics.ts".to_string()),
          start_position: Some(235),
          end_position: Some(241),
          category: DiagnosticCategory::Error,
          code: 2322,
          start_column: Some(2),
          end_column: Some(8),
        },
        DiagnosticItem {
          message: "Property 't' does not exist on type 'T'.".to_string(),
          message_chain: None,
          related_information: None,
          source_line: Some("      v: o.t,".to_string()),
          line_number: Some(20),
          script_resource_name: Some("/deno/tests/complex_diagnostics.ts".to_string()),
          start_position: Some(267),
          end_position: Some(268),
          category: DiagnosticCategory::Error,
          code: 2339,
          start_column: Some(11),
          end_column: Some(12),
        },
      ],
    });
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
    let expected = "error TS2322: Type \'(o: T) => { v: any; f: (x: B) => string; }[]\' is not assignable to type \'(r: B) => Value<B>[]\'.\n  Types of parameters \'o\' and \'r\' are incompatible.\n    Type \'B\' is not assignable to type \'T\'.\n\n► deno/tests/complex_diagnostics.ts:19:3\n\n19   values: o => [\n     ~~~~~~\n\n  The expected type comes from property \'values\' which is declared here on type \'SettingsInterface<B>\'\n\n    ► deno/tests/complex_diagnostics.ts:7:3\n\n    7   values?: (r: T) => Array<Value<T>>;\n        ~~~~~~\n\n";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }

  #[test]
  fn diagnostic_to_string2() {
    let d = diagnostic2();
    let expected = "error TS2322: Example 1\n\n► deno/tests/complex_diagnostics.ts:19:3\n\n19   values: o => [\n     ~~~~~~\n\nerror TS2000: Example 2\n\n► /foo/bar.ts:129:3\n\n129   values: undefined,\n      ~~~~~~\n\n\nFound 2 errors.\n";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }
}
