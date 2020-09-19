// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use serde::Serialize;
use serde::Serializer;
use std::path::Path;
use std::path::PathBuf;

// Warning! The values in this enum are duplicated in tsc/99_main_compiler.js
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  Dts = 3,
  TSX = 4,
  Json = 5,
  Wasm = 6,
  BuildInfo = 7,
  Unknown = 8,
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

impl MediaType {
  fn from_path(path: &Path) -> Self {
    match path.extension() {
      None => MediaType::Unknown,
      Some(os_str) => match os_str.to_str() {
        Some("ts") => MediaType::TypeScript,
        Some("tsx") => MediaType::TSX,
        Some("js") => MediaType::JavaScript,
        Some("jsx") => MediaType::JSX,
        Some("mjs") => MediaType::JavaScript,
        Some("cjs") => MediaType::JavaScript,
        Some("json") => MediaType::Json,
        Some("wasm") => MediaType::Wasm,
        _ => MediaType::Unknown,
      },
    }
  }

  /// Convert a MediaType to a `ts.Extension`.
  ///
  /// *NOTE* This is defined in TypeScript as a string based enum.  Changes to
  /// that enum in TypeScript should be reflected here.
  pub fn as_ts_extension(&self) -> &str {
    match self {
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
      MediaType::BuildInfo => ".tsbuildinfo",
      // TypeScript doesn't have an "unknown", so we will treat WASM as JS for
      // mapping purposes, though in reality, it is unlikely to ever be passed
      // to the compiler.
      MediaType::Unknown => ".js",
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
      MediaType::BuildInfo => 7 as i32,
      MediaType::Unknown => 8 as i32,
    };
    Serialize::serialize(&value, serializer)
  }
}

pub fn enum_name_media_type(mt: MediaType) -> &'static str {
  match mt {
    MediaType::JavaScript => "JavaScript",
    MediaType::JSX => "JSX",
    MediaType::TypeScript => "TypeScript",
    MediaType::Dts => "Dts",
    MediaType::TSX => "TSX",
    MediaType::Json => "Json",
    MediaType::Wasm => "Wasm",
    MediaType::BuildInfo => "BuildInfo",
    MediaType::Unknown => "Unknown",
  }
}

#[test]
fn test_map_file_extension() {
  assert_eq!(
    MediaType::from(Path::new("foo/bar.ts")),
    MediaType::TypeScript
  );
  assert_eq!(MediaType::from(Path::new("foo/bar.tsx")), MediaType::TSX);
  assert_eq!(
    MediaType::from(Path::new("foo/bar.d.ts")),
    MediaType::TypeScript
  );
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
    MediaType::from(Path::new("foo/bar.txt")),
    MediaType::Unknown
  );
  assert_eq!(MediaType::from(Path::new("foo/bar")), MediaType::Unknown);
}
