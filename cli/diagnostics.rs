// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This module encodes TypeScript errors (diagnostics) into Rust structs and
//! contains code for printing them to the console.

use crate::colors;
use crate::fmt_errors::format_stack;
use serde::Deserialize;
use serde::Deserializer;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
  pub items: Vec<DiagnosticItem>,
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

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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

fn format_category_and_code(
  category: &DiagnosticCategory,
  code: i64,
) -> String {
  let category = match category {
    DiagnosticCategory::Error => "ERROR".to_string(),
    DiagnosticCategory::Warning => "WARN".to_string(),
    DiagnosticCategory::Debug => "DEBUG".to_string(),
    DiagnosticCategory::Info => "INFO".to_string(),
    _ => "".to_string(),
  };

  let code = colors::bold(&format!("TS{}", code.to_string())).to_string();

  format!("{} [{}]", code, category)
}

fn format_message(
  message_chain: &Option<DiagnosticMessageChain>,
  message: &str,
  level: usize,
) -> String {
  debug!("format_message");

  if let Some(message_chain) = message_chain {
    let mut s = message_chain.format_message(level);
    s.pop();

    s
  } else {
    format!("{:indent$}{}", "", message, indent = level)
  }
}

/// Formats optional source, line and column numbers into a single string.
fn format_maybe_frame(
  file_name: Option<&str>,
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
  let line_c = colors::yellow(&line_number.to_string());
  let column_c = colors::yellow(&column_number.to_string());
  format!("{}:{}:{}", file_name_c, line_c, column_c)
}

fn format_maybe_related_information(
  related_information: &Option<Vec<DiagnosticItem>>,
) -> String {
  if related_information.is_none() {
    return "".to_string();
  }

  let mut s = String::new();

  if let Some(related_information) = related_information {
    for rd in related_information {
      s.push_str("\n\n");
      s.push_str(&format_stack(
        matches!(rd.category, DiagnosticCategory::Error),
        &format_message(&rd.message_chain, &rd.message, 0),
        rd.source_line.as_deref(),
        rd.start_column,
        rd.end_column,
        // Formatter expects 1-based line and column numbers, but ours are 0-based.
        &[format_maybe_frame(
          rd.script_resource_name.as_deref(),
          rd.line_number.map(|n| n + 1),
          rd.start_column.map(|n| n + 1),
        )],
        4,
      ));
    }
  }

  s
}

impl fmt::Display for DiagnosticItem {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}",
      format_stack(
        matches!(self.category, DiagnosticCategory::Error),
        &format!(
          "{}: {}",
          format_category_and_code(&self.category, self.code),
          format_message(&self.message_chain, &self.message, 0)
        ),
        self.source_line.as_deref(),
        self.start_column,
        self.end_column,
        // Formatter expects 1-based line and column numbers, but ours are 0-based.
        &[format_maybe_frame(
          self.script_resource_name.as_deref(),
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

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticMessageChain {
  pub message: String,
  pub code: i64,
  pub category: DiagnosticCategory,
  pub next: Option<Vec<DiagnosticMessageChain>>,
}

impl DiagnosticMessageChain {
  pub fn format_message(&self, level: usize) -> String {
    let mut s = String::new();

    s.push_str(&std::iter::repeat(" ").take(level * 2).collect::<String>());
    s.push_str(&self.message);
    s.push('\n');
    if let Some(next) = &self.next {
      let arr = next.clone();
      for dm in arr {
        s.push_str(&dm.format_message(level + 1));
      }
    }

    s
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagnosticCategory {
  Log,        // 0
  Debug,      // 1
  Info,       // 2
  Error,      // 3
  Warning,    // 4
  Suggestion, // 5
}

impl<'de> Deserialize<'de> for DiagnosticCategory {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s: i64 = Deserialize::deserialize(deserializer)?;
    Ok(DiagnosticCategory::from(s))
  }
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
    let r = serde_json::from_str::<Diagnostic>(
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
    let expected =
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
      };
    assert_eq!(expected, r);
  }

  #[test]
  fn diagnostic_to_string1() {
    let d = diagnostic1();
    let expected = "TS2322 [ERROR]: Type \'(o: T) => { v: any; f: (x: B) => string; }[]\' is not assignable to type \'(r: B) => Value<B>[]\'.\n  Types of parameters \'o\' and \'r\' are incompatible.\n    Type \'B\' is not assignable to type \'T\'.\n  values: o => [\n  ~~~~~~\n    at deno/tests/complex_diagnostics.ts:19:3\n\n    The expected type comes from property \'values\' which is declared here on type \'SettingsInterface<B>\'\n      values?: (r: T) => Array<Value<T>>;\n      ~~~~~~\n        at deno/tests/complex_diagnostics.ts:7:3";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }

  #[test]
  fn diagnostic_to_string2() {
    let d = diagnostic2();
    let expected = "TS2322 [ERROR]: Example 1\n  values: o => [\n  ~~~~~~\n    at deno/tests/complex_diagnostics.ts:19:3\n\nTS2000 [ERROR]: Example 2\n  values: undefined,\n  ~~~~~~\n    at /foo/bar.ts:129:3\n\nFound 2 errors.";
    assert_eq!(expected, strip_ansi_codes(&d.to_string()));
  }

  #[test]
  fn test_format_none_frame() {
    let actual = format_maybe_frame(None, None, None);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_frame() {
    let actual =
      format_maybe_frame(Some("file://foo/bar.ts"), Some(1), Some(2));
    assert_eq!(strip_ansi_codes(&actual), "file://foo/bar.ts:1:2");
  }
}
