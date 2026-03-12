// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

/// The type of a module, determining how it is parsed and processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
  Js,
  Jsx,
  Ts,
  Tsx,
  Css,
  Json,
  Html,
  Text,
  Binary,
  Asset,
}

impl Loader {
  pub fn from_extension(ext: &str) -> Option<Loader> {
    match ext {
      "js" | "mjs" | "cjs" => Some(Loader::Js),
      "jsx" | "mjsx" => Some(Loader::Jsx),
      "ts" | "mts" | "cts" => Some(Loader::Ts),
      "tsx" | "mtsx" => Some(Loader::Tsx),
      "css" => Some(Loader::Css),
      "json" => Some(Loader::Json),
      "html" | "htm" => Some(Loader::Html),
      "txt" => Some(Loader::Text),
      "wasm" => Some(Loader::Binary),
      "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" | "avif"
      | "woff" | "woff2" | "ttf" | "otf" | "eot" | "mp3" | "mp4"
      | "webm" | "ogg" | "wav" | "flac" | "aac" | "pdf" => {
        Some(Loader::Asset)
      }
      _ => None,
    }
  }

  pub fn from_path(path: &Path) -> Option<Loader> {
    path
      .extension()
      .and_then(|ext| ext.to_str())
      .and_then(Self::from_extension)
  }

  /// Whether this loader type can contain dependencies (imports).
  pub fn has_dependencies(&self) -> bool {
    matches!(
      self,
      Loader::Js
        | Loader::Jsx
        | Loader::Ts
        | Loader::Tsx
        | Loader::Css
        | Loader::Html
    )
  }

  /// Whether this is a static asset (no code transform needed).
  pub fn is_asset(&self) -> bool {
    matches!(self, Loader::Asset | Loader::Binary)
  }
}

/// Check if a file path has an explicit CJS extension (.cjs, .cts).
pub fn is_explicit_cjs(path: &Path) -> bool {
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| ext == "cjs" || ext == "cts")
}

/// Check if a file path has an explicit ESM extension (.mjs, .mts).
pub fn is_explicit_esm(path: &Path) -> bool {
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| ext == "mjs" || ext == "mts" || ext == "mjsx" || ext == "mtsx")
}
