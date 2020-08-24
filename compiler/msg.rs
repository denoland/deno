// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleResolutionError;
use deno_core::ModuleSpecifier;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct EmittedFile {
  pub data: String,
  pub maybe_module_name: Option<String>,
  pub url: String,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MediaType {
  JavaScript,
  JSX,
  TypeScript,
  TSX,
  Json,
  Wasm,
  BuildInfo,
  Unknown,
}

impl MediaType {
  /// Convert a MediaType to a `ts.Extension`.
  ///
  /// *NOTE* This is defined in TypeScript as a string based enum.  Changes to
  /// that enum in TypeScript should be reflected here.
  pub fn to_ts_extension(&self, specifier: &ModuleSpecifier) -> &str {
    match self {
      MediaType::JavaScript => ".js",
      MediaType::JSX => ".jsx",
      MediaType::TypeScript => {
        let url = specifier.as_url();
        let path = url.path();
        if path.ends_with(".d.ts") {
          ".d.ts"
        } else {
          ".ts"
        }
      }
      MediaType::TSX => ".tsx",
      MediaType::Json => ".json",
      // TypeScript doesn't have an "unknown", so we will treat WASM as JS for
      // mapping purposes, though in reality, it is unlikely to ever be passed
      // to the compiler.
      MediaType::Wasm => ".js",
      MediaType::BuildInfo => ".tsbuildinfo",
      // TypeScript doesn't have an "unknown", so we will treat WASM as JS for
      // mapping purposes, though in reality, it is unlikely to ever be passed
      // to the compiler.
      MediaType::Unknown => ".js",
    }
  }
}

fn common_path(
  a: &ModuleSpecifier,
  b: &ModuleSpecifier,
) -> Option<ModuleSpecifier> {
  let mut result = a.as_url().clone();
  result.set_path("");
  let a = a.as_url();
  let b = b.as_url();
  if a.scheme() == b.scheme() {
    let a = a.path_segments();
    let b = b.path_segments();
    if let (Some(mut a), Some(mut b)) = (a, b) {
      let mut found = false;
      loop {
        let a_seg = a.next();
        let b_seg = b.next();
        if a_seg.is_some() && a_seg == b_seg {
          let input = format!("{}/", a_seg.unwrap());
          result = result.join(&input).unwrap();
          found = true;
        } else {
          break;
        }
      }

      if found {
        Some(result.into())
      } else {
        None
      }
    } else {
      None
    }
  } else {
    None
  }
}

pub fn common_path_reduce(
  specifiers: Vec<&ModuleSpecifier>,
) -> Option<ModuleSpecifier> {
  if specifiers.is_empty() {
    return None;
  }
  if specifiers.len() == 1 {
    let spec = specifiers.first().cloned().cloned().unwrap();
    let mut url = spec.as_url().to_owned();
    url.path_segments_mut().unwrap().pop().push("");

    return Some(ModuleSpecifier::from(url));
  }
  let init = specifiers.first().cloned().cloned();

  specifiers.iter().fold(init, |a, b| {
    if let Some(a) = a.as_ref() {
      common_path(a, b)
    } else {
      a
    }
  })
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilerStats {
  pub items: Vec<(String, u64)>,
}

impl<'de> Deserialize<'de> for CompilerStats {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let items: Vec<(String, u64)> = Deserialize::deserialize(deserializer)?;
    Ok(CompilerStats { items })
  }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranspileSourceFile {
  pub data: String,
  pub renamed_dependencies: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IgnoredCompilerOptions(pub Vec<String>);

impl fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.0.join(", "))?;

    Ok(())
  }
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticMessageChain {
  message_text: String,
  category: DiagnosticCategory,
  code: i64,
  next: Box<Option<DiagnosticMessageChain>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
  pub category: DiagnosticCategory,
  pub code: u64,
  pub start: Option<u64>,
  pub length: Option<u64>,
  pub message_text: Option<String>,
  pub message_chain: Option<DiagnosticMessageChain>,
  pub source: Option<String>,
  pub source_file: Option<String>,
  pub related_information: Box<Option<Diagnostic>>,
}

// impl Diagnostic {
//   fn format_category_and_code(&self) -> String {
//     let category = match self.category {
//       DiagnosticCategory::Error => "ERROR",
//       DiagnosticCategory::Warning => "WARN",
//       _ => "",
//     };
//   }

//   fn format_stack(&self) -> String {
//     let is_error = self.category == DiagnosticCategory::Error;
//     let mut s = String::new();
//     let message_line = format!("{}: {}", self.format_category_and_code())
//   }
// }

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}[TS{}] {}",
      self.category,
      self.code,
      self
        .message_text
        .clone()
        .unwrap_or_else(|| "[message chain]".to_string())
    )
    // write!(
    //   f,
    //   "{}",
    //   format_stack(
    //     match self.category {
    //       DiagnosticCategory::Error => true,
    //       _ => false,
    //     },
    //     &format!(
    //       "{}: {}",
    //       format_category_and_code(&self.category, self.code),
    //       format_message(&self.message_chain, &self.message, 0)
    //     ),
    //     self.source_line.as_deref(),
    //     self.start_column,
    //     self.end_column,
    //     // Formatter expects 1-based line and column numbers, but ours are 0-based.
    //     &[format_maybe_frame(
    //       self.script_resource_name.as_deref(),
    //       self.line_number.map(|n| n + 1),
    //       self.start_column.map(|n| n + 1)
    //     )],
    //     0
    //   )
    // )?;
    // write!(
    //   f,
    //   "{}",
    //   format_maybe_related_information(&self.related_information),
    // )
  }
}

#[derive(Clone, Debug)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl<'de> Deserialize<'de> for Diagnostics {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let items: Vec<Diagnostic> = Deserialize::deserialize(deserializer)?;
    Ok(Diagnostics(items))
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

/// Convert a module specifier into a string in the format that can be safely
/// used within the TypeScript compiler. When module specifiers exist in
/// different schemes, TypeScript fails to find a common root and then will not
/// output files to their virtual location, and will not attempt to write any
/// files that are named the same on emit (e.g. JavaScript files).
pub fn as_ts_filename(
  specifier: &ModuleSpecifier,
  maybe_shared_path: &Option<String>,
) -> String {
  match specifier.as_url().scheme() {
    "http" => specifier.as_str().replace("http://", "/http/"),
    "https" => specifier.as_str().replace("https://", "/https/"),
    "file" => {
      let specifier = specifier.as_str().replace("file:///", "/file/");
      if let Some(shared_path) = maybe_shared_path {
        specifier.replace(shared_path, "/file/")
      } else {
        specifier
      }
    }
    _ => specifier.as_str().to_string(),
  }
}

/// Take a converted string specifier used internally within the TypeScript
/// compiler and covert it to a `ModuleSpecifier`.
pub fn from_ts_filename(
  file_name: &str,
  maybe_shared_path: &Option<String>,
) -> Result<ModuleSpecifier, ModuleResolutionError> {
  if file_name == "cache:///.tsbuildinfo" {
    ModuleSpecifier::resolve_url_or_path(file_name)
  } else {
    let file_name = if file_name.starts_with("cache:///") {
      file_name.replace("cache:///", "/")
    } else {
      file_name.to_string()
    };
    let specifier = if file_name.starts_with("/http/") {
      file_name.replace("/http/", "http://")
    } else if file_name.starts_with("/https/") {
      file_name.replace("/https/", "https://")
    } else if file_name.starts_with("/file/") {
      if let Some(shared_path) = maybe_shared_path {
        file_name
          .replace("/file/", shared_path)
          .replace("/file/", "file:///")
      } else {
        file_name.replace("/file/", "file:///")
      }
    } else {
      file_name
    };

    ModuleSpecifier::resolve_url_or_path(&specifier)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_de_diagnostics() {
    let value = json!([
      {
        "messageText": "test message",
        "category": 1,
        "code": 1234,
      },
      {
        "messageText": "test message 2",
        "category": 0,
        "code": 5678,
      }, {
        "messageChain": {
          "messageText": "chain 1",
          "category": 1,
          "code": 9999,
          "next": {
            "messageText": "chain 2",
            "category": 1,
            "code": 8888,
          }
        },
        "category": 1,
        "code": 9999,
      }
    ]);
    let diagnostics: Diagnostics =
      serde_json::from_value(value).expect("cannot deserialize");
    assert_eq!(diagnostics.0.len(), 3);
    assert!(diagnostics.0[2].message_text.is_none());
    assert!(diagnostics.0[2].message_chain.is_some());
  }

  #[test]
  fn test_from_ts_filename() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("/foo/bar/baz/qat.ts").unwrap();
    let actual = as_ts_filename(&specifier, &None);
    assert_eq!(actual, "/file/foo/bar/baz/qat.ts");
  }

  #[test]
  fn test_as_ts_filename() {
    let actual = from_ts_filename("/file/foo/bar/baz/qat.ts", &None);
    assert_eq!(
      actual,
      Ok(
        ModuleSpecifier::resolve_url_or_path("file:///foo/bar/baz/qat.ts")
          .unwrap()
      )
    );
  }

  #[test]
  fn test_common_path() {
    let a =
      ModuleSpecifier::resolve_url_or_path("/foo/bar/baz/qat.ts").unwrap();
    let b =
      ModuleSpecifier::resolve_url_or_path("/foo/bar/qat/baz.ts").unwrap();
    let actual = common_path(&a, &b);
    assert_eq!(
      actual,
      Some(ModuleSpecifier::resolve_url_or_path("file:///foo/bar/").unwrap())
    );
  }

  #[test]
  fn test_common_path_none() {
    let a =
      ModuleSpecifier::resolve_url_or_path("/foo/bar/baz/qat.ts").unwrap();
    let b =
      ModuleSpecifier::resolve_url_or_path("/bar/baz/qat/foo.ts").unwrap();
    let actual = common_path(&a, &b);
    assert_eq!(actual, None);
  }

  #[test]
  fn test_common_path_reduce() {
    let fixtures = vec![
      ModuleSpecifier::resolve_url_or_path("/foo/bar/baz/qat.ts").unwrap(),
      ModuleSpecifier::resolve_url_or_path("/foo/bar/baz/qux.ts").unwrap(),
      ModuleSpecifier::resolve_url_or_path("/foo/bar/qat/qux.ts").unwrap(),
    ];

    let actual = common_path_reduce(fixtures.iter().collect());
    assert_eq!(
      actual,
      Some(ModuleSpecifier::resolve_url_or_path("file:///foo/bar/").unwrap())
    );
  }

  #[test]
  fn test_common_path_reduce_single() {
    let fixtures = vec![ModuleSpecifier::resolve_url_or_path(
      "/foo/bar/baz/qat.ts",
    )
    .unwrap()];

    let actual = common_path_reduce(fixtures.iter().collect());
    assert_eq!(
      actual,
      Some(
        ModuleSpecifier::resolve_url_or_path("file:///foo/bar/baz/").unwrap()
      )
    );
  }
}
