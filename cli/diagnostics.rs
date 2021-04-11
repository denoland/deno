// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;

use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use regex::Regex;
use std::error::Error;
use std::fmt;

const MAX_SOURCE_LINE_LENGTH: usize = 150;

const UNSTABLE_DENO_PROPS: &[&str] = &[
  "CompilerOptions",
  "CreateHttpClientOptions",
  "DatagramConn",
  "Diagnostic",
  "DiagnosticCategory",
  "DiagnosticItem",
  "DiagnosticMessageChain",
  "EmitOptions",
  "EmitResult",
  "HttpClient",
  "HttpConn",
  "LinuxSignal",
  "Location",
  "MXRecord",
  "MacOSSignal",
  "Metrics",
  "OpMetrics",
  "RecordType",
  "RequestEvent",
  "ResolveDnsOptions",
  "SRVRecord",
  "SetRawOptions",
  "Signal",
  "SignalStream",
  "StartTlsOptions",
  "SystemCpuInfo",
  "SystemMemoryInfo",
  "UnixConnectOptions",
  "UnixListenOptions",
  "applySourceMap",
  "connect",
  "consoleSize",
  "createHttpClient",
  "emit",
  "formatDiagnostics",
  "ftruncate",
  "ftruncateSync",
  "futime",
  "futimeSync",
  "hostname",
  "kill",
  "listen",
  "listenDatagram",
  "loadavg",
  "openPlugin",
  "osRelease",
  "ppid",
  "resolveDns",
  "serveHttp",
  "setRaw",
  "shutdown",
  "signal",
  "signals",
  "sleepSync",
  "startTls",
  "systemCpuInfo",
  "systemMemoryInfo",
  "umask",
  "utime",
  "utimeSync",
];

lazy_static::lazy_static! {
  static ref MSG_MISSING_PROPERTY_DENO: Regex =
    Regex::new(r#"Property '([^']+)' does not exist on type 'typeof Deno'"#)
      .unwrap();
  static ref MSG_SUGGESTION: Regex =
    Regex::new(r#" Did you mean '([^']+)'\?"#).unwrap();
}

/// Potentially convert a "raw" diagnostic message from TSC to something that
/// provides a more sensible error message given a Deno runtime context.
fn format_message(msg: &str, code: &u64) -> String {
  match code {
    2339 => {
      if let Some(captures) = MSG_MISSING_PROPERTY_DENO.captures(msg) {
        if let Some(property) = captures.get(1) {
          if UNSTABLE_DENO_PROPS.contains(&property.as_str()) {
            return format!("{} 'Deno.{}' is an unstable API. Did you forget to run with the '--unstable' flag?", msg, property.as_str());
          }
        }
      }

      msg.to_string()
    }
    2551 => {
      if let (Some(caps_property), Some(caps_suggestion)) = (
        MSG_MISSING_PROPERTY_DENO.captures(msg),
        MSG_SUGGESTION.captures(msg),
      ) {
        if let (Some(property), Some(suggestion)) =
          (caps_property.get(1), caps_suggestion.get(1))
        {
          if UNSTABLE_DENO_PROPS.contains(&property.as_str()) {
            return format!("{} 'Deno.{}' is an unstable API. Did you forget to run with the '--unstable' flag, or did you mean '{}'?", MSG_SUGGESTION.replace(msg, ""), property.as_str(), suggestion.as_str());
          }
        }
      }

      msg.to_string()
    }
    _ => msg.to_string(),
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticCategory {
  Warning,
  Error,
  Suggestion,
  Message,
}

impl fmt::Display for DiagnosticCategory {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}",
      match self {
        DiagnosticCategory::Warning => "WARN ",
        DiagnosticCategory::Error => "ERROR ",
        DiagnosticCategory::Suggestion => "",
        DiagnosticCategory::Message => "",
      }
    )
  }
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

impl Serialize for DiagnosticCategory {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value = match self {
      DiagnosticCategory::Warning => 0_i32,
      DiagnosticCategory::Error => 1_i32,
      DiagnosticCategory::Suggestion => 2_i32,
      DiagnosticCategory::Message => 3_i32,
    };
    Serialize::serialize(&value, serializer)
  }
}

impl From<i64> for DiagnosticCategory {
  fn from(value: i64) -> Self {
    match value {
      0 => DiagnosticCategory::Warning,
      1 => DiagnosticCategory::Error,
      2 => DiagnosticCategory::Suggestion,
      3 => DiagnosticCategory::Message,
      _ => panic!("Unknown value: {}", value),
    }
  }
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticMessageChain {
  message_text: String,
  category: DiagnosticCategory,
  code: i64,
  next: Option<Vec<DiagnosticMessageChain>>,
}

impl DiagnosticMessageChain {
  pub fn format_message(&self, level: usize) -> String {
    let mut s = String::new();

    s.push_str(&std::iter::repeat(" ").take(level * 2).collect::<String>());
    s.push_str(&self.message_text);
    if let Some(next) = &self.next {
      s.push('\n');
      let arr = next.clone();
      for dm in arr {
        s.push_str(&dm.format_message(level + 1));
      }
    }

    s
  }
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
  pub line: u64,
  pub character: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
  pub category: DiagnosticCategory,
  pub code: u64,
  pub start: Option<Position>,
  pub end: Option<Position>,
  pub message_text: Option<String>,
  pub message_chain: Option<DiagnosticMessageChain>,
  pub source: Option<String>,
  pub source_line: Option<String>,
  pub file_name: Option<String>,
  pub related_information: Option<Vec<Diagnostic>>,
}

impl Diagnostic {
  fn fmt_category_and_code(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let category = match self.category {
      DiagnosticCategory::Error => "ERROR",
      DiagnosticCategory::Warning => "WARN",
      _ => "",
    };

    if !category.is_empty() {
      write!(
        f,
        "{} [{}]: ",
        colors::bold(&format!("TS{}", self.code)),
        category
      )
    } else {
      Ok(())
    }
  }

  fn fmt_frame(&self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
    if let (Some(file_name), Some(start)) =
      (self.file_name.as_ref(), self.start.as_ref())
    {
      write!(
        f,
        "\n{:indent$}    at {}:{}:{}",
        "",
        colors::cyan(file_name),
        colors::yellow(&(start.line + 1).to_string()),
        colors::yellow(&(start.character + 1).to_string()),
        indent = level
      )
    } else {
      Ok(())
    }
  }

  fn fmt_message(&self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
    if let Some(message_chain) = &self.message_chain {
      write!(f, "{}", message_chain.format_message(level))
    } else {
      write!(
        f,
        "{:indent$}{}",
        "",
        format_message(&self.message_text.clone().unwrap(), &self.code),
        indent = level,
      )
    }
  }

  fn fmt_source_line(
    &self,
    f: &mut fmt::Formatter,
    level: usize,
  ) -> fmt::Result {
    if let (Some(source_line), Some(start), Some(end)) =
      (&self.source_line, &self.start, &self.end)
    {
      if !source_line.is_empty() && source_line.len() <= MAX_SOURCE_LINE_LENGTH
      {
        write!(f, "\n{:indent$}{}", "", source_line, indent = level)?;
        let length = if start.line == end.line {
          end.character - start.character
        } else {
          1
        };
        let mut s = String::new();
        for i in 0..start.character {
          s.push(if source_line.chars().nth(i as usize).unwrap() == '\t' {
            '\t'
          } else {
            ' '
          });
        }
        // TypeScript always uses `~` when underlining, but v8 always uses `^`.
        // We will use `^` to indicate a single point, or `~` when spanning
        // multiple characters.
        let ch = if length > 1 { '~' } else { '^' };
        for _i in 0..length {
          s.push(ch)
        }
        let underline = if self.is_error() {
          colors::red(&s).to_string()
        } else {
          colors::cyan(&s).to_string()
        };
        write!(f, "\n{:indent$}{}", "", underline, indent = level)?;
      }
    }

    Ok(())
  }

  fn fmt_related_information(&self, f: &mut fmt::Formatter) -> fmt::Result {
    if let Some(related_information) = self.related_information.as_ref() {
      write!(f, "\n\n")?;
      for info in related_information {
        info.fmt_stack(f, 4)?;
      }
    }

    Ok(())
  }

  fn fmt_stack(&self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
    self.fmt_category_and_code(f)?;
    self.fmt_message(f, level)?;
    self.fmt_source_line(f, level)?;
    self.fmt_frame(f, level)
  }

  fn is_error(&self) -> bool {
    self.category == DiagnosticCategory::Error
  }
}

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.fmt_stack(f, 0)?;
    self.fmt_related_information(f)
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
  #[cfg(test)]
  pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
    Diagnostics(diagnostics)
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

impl<'de> Deserialize<'de> for Diagnostics {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let items: Vec<Diagnostic> = Deserialize::deserialize(deserializer)?;
    Ok(Diagnostics(items))
  }
}

impl Serialize for Diagnostics {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.0, serializer)
  }
}

impl fmt::Display for Diagnostics {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut i = 0;
    for item in &self.0 {
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

impl Error for Diagnostics {}

#[cfg(test)]
mod tests {
  use super::*;
  use colors::strip_ansi_codes;
  use deno_core::serde_json;
  use deno_core::serde_json::json;

  #[test]
  fn test_de_diagnostics() {
    let value = json!([
      {
        "messageText": "Unknown compiler option 'invalid'.",
        "category": 1,
        "code": 5023
      },
      {
        "start": {
          "line": 0,
          "character": 0
        },
        "end": {
          "line": 0,
          "character": 7
        },
        "fileName": "test.ts",
        "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the `lib` compiler option to include 'dom'.",
        "sourceLine": "console.log(\"a\");",
        "category": 1,
        "code": 2584
      },
      {
        "start": {
          "line": 7,
          "character": 0
        },
        "end": {
          "line": 7,
          "character": 7
        },
        "fileName": "test.ts",
        "messageText": "Cannot find name 'foo_Bar'. Did you mean 'foo_bar'?",
        "sourceLine": "foo_Bar();",
        "relatedInformation": [
          {
            "start": {
              "line": 3,
              "character": 9
            },
            "end": {
              "line": 3,
              "character": 16
            },
            "fileName": "test.ts",
            "messageText": "'foo_bar' is declared here.",
            "sourceLine": "function foo_bar() {",
            "category": 3,
            "code": 2728
          }
        ],
        "category": 1,
        "code": 2552
      },
      {
        "start": {
          "line": 18,
          "character": 0
        },
        "end": {
          "line": 18,
          "character": 1
        },
        "fileName": "test.ts",
        "messageChain": {
          "messageText": "Type '{ a: { b: { c(): { d: number; }; }; }; }' is not assignable to type '{ a: { b: { c(): { d: string; }; }; }; }'.",
          "category": 1,
          "code": 2322,
          "next": [
            {
              "messageText": "The types of 'a.b.c().d' are incompatible between these types.",
              "category": 1,
              "code": 2200,
              "next": [
                {
                  "messageText": "Type 'number' is not assignable to type 'string'.",
                  "category": 1,
                  "code": 2322
                }
              ]
            }
          ]
        },
        "sourceLine": "x = y;",
        "code": 2322,
        "category": 1
      }
    ]);
    let diagnostics: Diagnostics =
      serde_json::from_value(value).expect("cannot deserialize");
    assert_eq!(diagnostics.0.len(), 4);
    assert!(diagnostics.0[0].source_line.is_none());
    assert!(diagnostics.0[0].file_name.is_none());
    assert!(diagnostics.0[0].start.is_none());
    assert!(diagnostics.0[0].end.is_none());
    assert!(diagnostics.0[0].message_text.is_some());
    assert!(diagnostics.0[0].message_chain.is_none());
    assert!(diagnostics.0[0].related_information.is_none());
    assert!(diagnostics.0[1].source_line.is_some());
    assert!(diagnostics.0[1].file_name.is_some());
    assert!(diagnostics.0[1].start.is_some());
    assert!(diagnostics.0[1].end.is_some());
    assert!(diagnostics.0[1].message_text.is_some());
    assert!(diagnostics.0[1].message_chain.is_none());
    assert!(diagnostics.0[1].related_information.is_none());
    assert!(diagnostics.0[2].source_line.is_some());
    assert!(diagnostics.0[2].file_name.is_some());
    assert!(diagnostics.0[2].start.is_some());
    assert!(diagnostics.0[2].end.is_some());
    assert!(diagnostics.0[2].message_text.is_some());
    assert!(diagnostics.0[2].message_chain.is_none());
    assert!(diagnostics.0[2].related_information.is_some());
  }

  #[test]
  fn test_diagnostics_no_source() {
    let value = json!([
      {
        "messageText": "Unknown compiler option 'invalid'.",
        "category":1,
        "code":5023
      }
    ]);
    let diagnostics: Diagnostics = serde_json::from_value(value).unwrap();
    let actual = diagnostics.to_string();
    assert_eq!(
      strip_ansi_codes(&actual),
      "TS5023 [ERROR]: Unknown compiler option \'invalid\'."
    );
  }

  #[test]
  fn test_diagnostics_basic() {
    let value = json!([
      {
        "start": {
          "line": 0,
          "character": 0
        },
        "end": {
          "line": 0,
          "character": 7
        },
        "fileName": "test.ts",
        "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the `lib` compiler option to include 'dom'.",
        "sourceLine": "console.log(\"a\");",
        "category": 1,
        "code": 2584
      }
    ]);
    let diagnostics: Diagnostics = serde_json::from_value(value).unwrap();
    let actual = diagnostics.to_string();
    assert_eq!(strip_ansi_codes(&actual), "TS2584 [ERROR]: Cannot find name \'console\'. Do you need to change your target library? Try changing the `lib` compiler option to include \'dom\'.\nconsole.log(\"a\");\n~~~~~~~\n    at test.ts:1:1");
  }

  #[test]
  fn test_diagnostics_related_info() {
    let value = json!([
      {
        "start": {
          "line": 7,
          "character": 0
        },
        "end": {
          "line": 7,
          "character": 7
        },
        "fileName": "test.ts",
        "messageText": "Cannot find name 'foo_Bar'. Did you mean 'foo_bar'?",
        "sourceLine": "foo_Bar();",
        "relatedInformation": [
          {
            "start": {
              "line": 3,
              "character": 9
            },
            "end": {
              "line": 3,
              "character": 16
            },
            "fileName": "test.ts",
            "messageText": "'foo_bar' is declared here.",
            "sourceLine": "function foo_bar() {",
            "category": 3,
            "code": 2728
          }
        ],
        "category": 1,
        "code": 2552
      }
    ]);
    let diagnostics: Diagnostics = serde_json::from_value(value).unwrap();
    let actual = diagnostics.to_string();
    assert_eq!(strip_ansi_codes(&actual), "TS2552 [ERROR]: Cannot find name \'foo_Bar\'. Did you mean \'foo_bar\'?\nfoo_Bar();\n~~~~~~~\n    at test.ts:8:1\n\n    \'foo_bar\' is declared here.\n    function foo_bar() {\n             ~~~~~~~\n        at test.ts:4:10");
  }

  #[test]
  fn test_unstable_suggestion() {
    let value = json![
      {
        "start": {
          "line": 0,
          "character": 17
        },
        "end": {
          "line": 0,
          "character": 21
        },
        "fileName": "file:///cli/tests/unstable_ts2551.ts",
        "messageText": "Property 'ppid' does not exist on type 'typeof Deno'. Did you mean 'pid'?",
        "sourceLine": "console.log(Deno.ppid);",
        "relatedInformation": [
          {
            "start": {
              "line": 89,
              "character": 15
            },
            "end": {
              "line": 89,
              "character": 18
            },
            "fileName": "asset:///lib.deno.ns.d.ts",
            "messageText": "'pid' is declared here.",
            "sourceLine": "  export const pid: number;",
            "category": 3,
            "code": 2728
          }
        ],
        "category": 1,
        "code": 2551
      }
    ];
    let diagnostics: Diagnostic = serde_json::from_value(value).unwrap();
    let actual = diagnostics.to_string();
    assert_eq!(strip_ansi_codes(&actual), "TS2551 [ERROR]: Property \'ppid\' does not exist on type \'typeof Deno\'. \'Deno.ppid\' is an unstable API. Did you forget to run with the \'--unstable\' flag, or did you mean \'pid\'?\nconsole.log(Deno.ppid);\n                 ~~~~\n    at file:///cli/tests/unstable_ts2551.ts:1:18\n\n    \'pid\' is declared here.\n      export const pid: number;\n                   ~~~\n        at asset:///lib.deno.ns.d.ts:90:16");
  }
}
