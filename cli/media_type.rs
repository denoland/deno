// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use data_url::DataUrl;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::ModuleSpecifier;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

// Warning! The values in this enum are duplicated in tsc/99_main_compiler.js
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum MediaType {
  JavaScript = 0,
  Jsx = 1,
  TypeScript = 2,
  Dts = 3,
  Tsx = 4,
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
      MediaType::Jsx => "JSX",
      MediaType::TypeScript => "TypeScript",
      MediaType::Dts => "Dts",
      MediaType::Tsx => "TSX",
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
    Self::from_path(path)
  }
}

impl<'a> From<&'a PathBuf> for MediaType {
  fn from(path: &'a PathBuf) -> Self {
    Self::from_path(path)
  }
}

impl<'a> From<&'a String> for MediaType {
  fn from(specifier: &'a String) -> Self {
    Self::from_path(&PathBuf::from(specifier))
  }
}

impl<'a> From<&'a ModuleSpecifier> for MediaType {
  fn from(specifier: &'a ModuleSpecifier) -> Self {
    if specifier.scheme() != "data" {
      let path = if specifier.scheme() == "file" {
        if let Ok(path) = specifier.to_file_path() {
          path
        } else {
          PathBuf::from(specifier.path())
        }
      } else {
        PathBuf::from(specifier.path())
      };
      Self::from_path(&path)
    } else if let Ok(data_url) = DataUrl::process(specifier.as_str()) {
      Self::from_content_type(specifier, data_url.mime_type().to_string())
    } else {
      Self::Unknown
    }
  }
}

impl Default for MediaType {
  fn default() -> Self {
    MediaType::Unknown
  }
}

impl MediaType {
  pub fn from_content_type<S: AsRef<str>>(
    specifier: &ModuleSpecifier,
    content_type: S,
  ) -> Self {
    match content_type.as_ref().trim().to_lowercase().as_ref() {
      "application/typescript"
      | "text/typescript"
      | "video/vnd.dlna.mpeg-tts"
      | "video/mp2t"
      | "application/x-typescript" => {
        map_js_like_extension(specifier, Self::TypeScript)
      }
      "application/javascript"
      | "text/javascript"
      | "application/ecmascript"
      | "text/ecmascript"
      | "application/x-javascript"
      | "application/node" => {
        map_js_like_extension(specifier, Self::JavaScript)
      }
      "text/jsx" => Self::Jsx,
      "text/tsx" => Self::Tsx,
      "application/json" | "text/json" => Self::Json,
      "application/wasm" => Self::Wasm,
      // Handle plain and possibly webassembly
      "text/plain" | "application/octet-stream"
        if specifier.scheme() != "data" =>
      {
        Self::from(specifier)
      }
      _ => Self::Unknown,
    }
  }

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
        Some("ts") => {
          if let Some(os_str) = path.file_stem() {
            if let Some(file_name) = os_str.to_str() {
              if file_name.ends_with(".d") {
                return MediaType::Dts;
              }
            }
          }
          MediaType::TypeScript
        }
        Some("tsx") => MediaType::Tsx,
        Some("js") => MediaType::JavaScript,
        Some("jsx") => MediaType::Jsx,
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
  pub fn as_ts_extension(&self) -> &str {
    match self {
      MediaType::JavaScript => ".js",
      MediaType::Jsx => ".jsx",
      MediaType::TypeScript => ".ts",
      MediaType::Dts => ".d.ts",
      MediaType::Tsx => ".tsx",
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
      // TypeScript doesn't have an "unknown", so we will treat unknowns as JS
      // for mapping purposes, though in reality, it is unlikely to ever be
      // passed to the compiler.
      MediaType::Unknown => ".js",
    }
  }

  /// Map the media type to a `ts.ScriptKind`
  pub fn as_ts_script_kind(&self) -> i32 {
    match self {
      MediaType::JavaScript => 1,
      MediaType::Jsx => 2,
      MediaType::TypeScript => 3,
      MediaType::Dts => 3,
      MediaType::Tsx => 4,
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
      MediaType::JavaScript => 0_i32,
      MediaType::Jsx => 1_i32,
      MediaType::TypeScript => 2_i32,
      MediaType::Dts => 3_i32,
      MediaType::Tsx => 4_i32,
      MediaType::Json => 5_i32,
      MediaType::Wasm => 6_i32,
      MediaType::TsBuildInfo => 7_i32,
      MediaType::SourceMap => 8_i32,
      MediaType::Unknown => 9_i32,
    };
    Serialize::serialize(&value, serializer)
  }
}

/// Serialize a `MediaType` enum into a human readable string.  The default
/// serialization for media types is and integer.
///
/// TODO(@kitsonk) remove this once we stop sending MediaType into tsc.
pub fn serialize_media_type<S>(
  mmt: &Option<MediaType>,
  s: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  match *mmt {
    Some(ref mt) => s.serialize_some(&mt.to_string()),
    None => s.serialize_none(),
  }
}

/// Used to augment media types by using the path part of a module specifier to
/// resolve to a more accurate media type.
fn map_js_like_extension(
  specifier: &ModuleSpecifier,
  default: MediaType,
) -> MediaType {
  let path = if specifier.scheme() == "file" {
    if let Ok(path) = specifier.to_file_path() {
      path
    } else {
      PathBuf::from(specifier.path())
    }
  } else {
    PathBuf::from(specifier.path())
  };
  match path.extension() {
    None => default,
    Some(os_str) => match os_str.to_str() {
      None => default,
      Some("jsx") => MediaType::Jsx,
      Some("tsx") => MediaType::Tsx,
      // Because DTS files do not have a separate media type, or a unique
      // extension, we have to "guess" at those things that we consider that
      // look like TypeScript, and end with `.d.ts` are DTS files.
      Some("ts") => {
        if default == MediaType::TypeScript {
          match path.file_stem() {
            None => default,
            Some(os_str) => {
              if let Some(file_stem) = os_str.to_str() {
                if file_stem.ends_with(".d") {
                  MediaType::Dts
                } else {
                  default
                }
              } else {
                default
              }
            }
          }
        } else {
          default
        }
      }
      Some(_) => default,
    },
  }
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
    assert_eq!(MediaType::from(Path::new("foo/bar.tsx")), MediaType::Tsx);
    assert_eq!(MediaType::from(Path::new("foo/bar.d.ts")), MediaType::Dts);
    assert_eq!(
      MediaType::from(Path::new("foo/bar.js")),
      MediaType::JavaScript
    );
    assert_eq!(MediaType::from(Path::new("foo/bar.jsx")), MediaType::Jsx);
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
  fn test_from_specifier() {
    let fixtures = vec![
      ("file:///a/b/c.ts", MediaType::TypeScript),
      ("file:///a/b/c.js", MediaType::JavaScript),
      ("file:///a/b/c.txt", MediaType::Unknown),
      ("https://deno.land/x/mod.ts", MediaType::TypeScript),
      ("https://deno.land/x/mod.js", MediaType::JavaScript),
      ("https://deno.land/x/mod.txt", MediaType::Unknown),
      ("data:application/typescript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=", MediaType::TypeScript),
      ("data:application/javascript;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=", MediaType::JavaScript),
      ("data:text/plain;base64,ZXhwb3J0IGNvbnN0IGEgPSAiYSI7CgpleHBvcnQgZW51bSBBIHsKICBBLAogIEIsCiAgQywKfQo=", MediaType::Unknown),
    ];

    for (specifier, expected) in fixtures {
      let actual = deno_core::resolve_url_or_path(specifier).unwrap();
      assert_eq!(MediaType::from(&actual), expected);
    }
  }

  #[test]
  fn test_from_content_type() {
    let fixtures = vec![
      (
        "https://deno.land/x/mod.ts",
        "application/typescript",
        MediaType::TypeScript,
      ),
      (
        "https://deno.land/x/mod.d.ts",
        "application/typescript",
        MediaType::Dts,
      ),
      ("https://deno.land/x/mod.tsx", "text/tsx", MediaType::Tsx),
      (
        "https://deno.land/x/mod.js",
        "application/javascript",
        MediaType::JavaScript,
      ),
      ("https://deno.land/x/mod.jsx", "text/jsx", MediaType::Jsx),
      (
        "https://deno.land/x/mod.ts",
        "text/plain",
        MediaType::TypeScript,
      ),
      (
        "https://deno.land/x/mod.js",
        "text/plain",
        MediaType::JavaScript,
      ),
      (
        "https://deno.land/x/mod.wasm",
        "text/plain",
        MediaType::Wasm,
      ),
    ];

    for (specifier, content_type, expected) in fixtures {
      let fixture = deno_core::resolve_url_or_path(specifier).unwrap();
      assert_eq!(
        MediaType::from_content_type(&fixture, content_type),
        expected
      );
    }
  }

  #[test]
  fn test_serialization() {
    assert_eq!(json!(MediaType::JavaScript), json!(0));
    assert_eq!(json!(MediaType::Jsx), json!(1));
    assert_eq!(json!(MediaType::TypeScript), json!(2));
    assert_eq!(json!(MediaType::Dts), json!(3));
    assert_eq!(json!(MediaType::Tsx), json!(4));
    assert_eq!(json!(MediaType::Json), json!(5));
    assert_eq!(json!(MediaType::Wasm), json!(6));
    assert_eq!(json!(MediaType::TsBuildInfo), json!(7));
    assert_eq!(json!(MediaType::SourceMap), json!(8));
    assert_eq!(json!(MediaType::Unknown), json!(9));
  }

  #[test]
  fn test_display() {
    assert_eq!(MediaType::JavaScript.to_string(), "JavaScript");
    assert_eq!(MediaType::Jsx.to_string(), "JSX");
    assert_eq!(MediaType::TypeScript.to_string(), "TypeScript");
    assert_eq!(MediaType::Dts.to_string(), "Dts");
    assert_eq!(MediaType::Tsx.to_string(), "TSX");
    assert_eq!(MediaType::Json.to_string(), "Json");
    assert_eq!(MediaType::Wasm.to_string(), "Wasm");
    assert_eq!(MediaType::TsBuildInfo.to_string(), "TsBuildInfo");
    assert_eq!(MediaType::SourceMap.to_string(), "SourceMap");
    assert_eq!(MediaType::Unknown.to_string(), "Unknown");
  }
}
