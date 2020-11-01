// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::ModuleSpecifier;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

// Warning! The values in this enum are duplicated in tsc/99_main_compiler.js
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  Dts = 3,
  TSX = 4,
  Json = 5,
  Wasm = 6,
  TsBuildInfo = 7,
  SourceMap = 8,
  Unknown = 9,
}

impl fmt::Display for MediaType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let value = match self {
      MediaType::JavaScript => "JavaScript",
      MediaType::JSX => "JSX",
      MediaType::TypeScript => "TypeScript",
      MediaType::Dts => "Dts",
      MediaType::TSX => "TSX",
      MediaType::Json => "Json",
      MediaType::Wasm => "Wasm",
      MediaType::TsBuildInfo => "TsBuildInfo",
      MediaType::SourceMap => "SourceMap",
      MediaType::Unknown => "Unknown",
    };
    write!(f, "{}", value)
  }
}

impl<'a> From<&'a Path> for MediaType {
  fn from(path: &'a Path) -> Self {
    MediaType::from_path(path)
  }
}

impl<'a> From<&'a PathBuf> for MediaType {
  fn from(path: &'a PathBuf) -> Self {
    MediaType::from_path(path)
  }
}

impl<'a> From<&'a String> for MediaType {
  fn from(specifier: &'a String) -> Self {
    MediaType::from_path(&PathBuf::from(specifier))
  }
}

impl<'a> From<&'a ModuleSpecifier> for MediaType {
  fn from(specifier: &'a ModuleSpecifier) -> Self {
    let url = specifier.as_url();
    let path = if url.scheme() == "file" {
      if let Ok(path) = url.to_file_path() {
        path
      } else {
        PathBuf::from(url.path())
      }
    } else {
      PathBuf::from(url.path())
    };
    MediaType::from_path(&path)
  }
}

impl Default for MediaType {
  fn default() -> Self {
    MediaType::Unknown
  }
}

impl MediaType {
  fn from_path(path: &Path) -> Self {
    match path.extension() {
      None => match path.file_name() {
        None => MediaType::Unknown,
        Some(os_str) => match os_str.to_str() {
          Some(".tsbuildinfo") => MediaType::TsBuildInfo,
          _ => MediaType::Unknown,
        },
      },
      Some(os_str) => match os_str.to_str() {
        Some("ts") => match path.file_stem() {
          Some(os_str) => match os_str.to_str() {
            Some(file_name) => {
              if file_name.ends_with(".d") {
                MediaType::Dts
              } else {
                MediaType::TypeScript
              }
            }
            None => MediaType::TypeScript,
          },
          None => MediaType::TypeScript,
        },
        Some("tsx") => MediaType::TSX,
        Some("js") => MediaType::JavaScript,
        Some("jsx") => MediaType::JSX,
        Some("mjs") => MediaType::JavaScript,
        Some("cjs") => MediaType::JavaScript,
        Some("json") => MediaType::Json,
        Some("wasm") => MediaType::Wasm,
        Some("tsbuildinfo") => MediaType::TsBuildInfo,
        Some("map") => MediaType::SourceMap,
        _ => MediaType::Unknown,
      },
    }
  }

  /// Convert a MediaType to a `ts.Extension`.
  ///
  /// *NOTE* This is defined in TypeScript as a string based enum.  Changes to
  /// that enum in TypeScript should be reflected here.
  pub fn as_ts_extension(&self) -> String {
    let ext = match self {
      MediaType::JavaScript => ".js",
      MediaType::JSX => ".jsx",
      MediaType::TypeScript => ".ts",
      MediaType::Dts => ".d.ts",
      MediaType::TSX => ".tsx",
      MediaType::Json => ".json",
      // TypeScript doesn't have an "unknown", so we will treat WASM as JS for
      // mapping purposes, though in reality, it is unlikely to ever be passed
      // to the compiler.
      MediaType::Wasm => ".js",
      MediaType::TsBuildInfo => ".tsbuildinfo",
      // TypeScript doesn't have an "source map", so we will treat SourceMap as
      // JS for mapping purposes, though in reality, it is unlikely to ever be
      // passed to the compiler.
      MediaType::SourceMap => ".js",
      // TypeScript doesn't have an "unknown", so we will treat WASM as JS for
      // mapping purposes, though in reality, it is unlikely to ever be passed
      // to the compiler.
      MediaType::Unknown => ".js",
    };

    ext.into()
  }

  /// Map the media type to a `ts.ScriptKind`
  pub fn as_ts_script_kind(&self) -> i32 {
    match self {
      MediaType::JavaScript => 1,
      MediaType::JSX => 2,
      MediaType::TypeScript => 3,
      MediaType::Dts => 3,
      MediaType::TSX => 4,
      MediaType::Json => 5,
      _ => 0,
    }
  }
}

impl Serialize for MediaType {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value = match self {
      MediaType::JavaScript => 0 as i32,
      MediaType::JSX => 1 as i32,
      MediaType::TypeScript => 2 as i32,
      MediaType::Dts => 3 as i32,
      MediaType::TSX => 4 as i32,
      MediaType::Json => 5 as i32,
      MediaType::Wasm => 6 as i32,
      MediaType::TsBuildInfo => 7 as i32,
      MediaType::SourceMap => 8 as i32,
      MediaType::Unknown => 9 as i32,
    };
    Serialize::serialize(&value, serializer)
  }
}

/// Serialize a `MediaType` enum into a human readable string.  The default
/// serialization for media types is and integer.
///
/// TODO(@kitsonk) remove this once we stop sending MediaType into tsc.
pub fn serialize_media_type<S>(mt: &MediaType, s: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  s.serialize_str(&format!("{}", mt))
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json::json;

  #[test]
  fn test_map_file_extension() {
    assert_eq!(
      MediaType::from(Path::new("foo/bar.ts")),
      MediaType::TypeScript
    );
    assert_eq!(MediaType::from(Path::new("foo/bar.tsx")), MediaType::TSX);
    assert_eq!(MediaType::from(Path::new("foo/bar.d.ts")), MediaType::Dts);
    assert_eq!(
      MediaType::from(Path::new("foo/bar.js")),
      MediaType::JavaScript
    );
    assert_eq!(MediaType::from(Path::new("foo/bar.jsx")), MediaType::JSX);
    assert_eq!(MediaType::from(Path::new("foo/bar.json")), MediaType::Json);
    assert_eq!(MediaType::from(Path::new("foo/bar.wasm")), MediaType::Wasm);
    assert_eq!(
      MediaType::from(Path::new("foo/bar.cjs")),
      MediaType::JavaScript
    );
    assert_eq!(
      MediaType::from(Path::new("foo/.tsbuildinfo")),
      MediaType::TsBuildInfo
    );
    assert_eq!(
      MediaType::from(Path::new("foo/bar.js.map")),
      MediaType::SourceMap
    );
    assert_eq!(
      MediaType::from(Path::new("foo/bar.txt")),
      MediaType::Unknown
    );
    assert_eq!(MediaType::from(Path::new("foo/bar")), MediaType::Unknown);
  }

  #[test]
  fn test_serialization() {
    assert_eq!(json!(MediaType::JavaScript), json!(0));
    assert_eq!(json!(MediaType::JSX), json!(1));
    assert_eq!(json!(MediaType::TypeScript), json!(2));
    assert_eq!(json!(MediaType::Dts), json!(3));
    assert_eq!(json!(MediaType::TSX), json!(4));
    assert_eq!(json!(MediaType::Json), json!(5));
    assert_eq!(json!(MediaType::Wasm), json!(6));
    assert_eq!(json!(MediaType::TsBuildInfo), json!(7));
    assert_eq!(json!(MediaType::SourceMap), json!(8));
    assert_eq!(json!(MediaType::Unknown), json!(9));
  }

  #[test]
  fn test_display() {
    assert_eq!(format!("{}", MediaType::JavaScript), "JavaScript");
    assert_eq!(format!("{}", MediaType::JSX), "JSX");
    assert_eq!(format!("{}", MediaType::TypeScript), "TypeScript");
    assert_eq!(format!("{}", MediaType::Dts), "Dts");
    assert_eq!(format!("{}", MediaType::TSX), "TSX");
    assert_eq!(format!("{}", MediaType::Json), "Json");
    assert_eq!(format!("{}", MediaType::Wasm), "Wasm");
    assert_eq!(format!("{}", MediaType::TsBuildInfo), "TsBuildInfo");
    assert_eq!(format!("{}", MediaType::SourceMap), "SourceMap");
    assert_eq!(format!("{}", MediaType::Unknown), "Unknown");
  }
}
