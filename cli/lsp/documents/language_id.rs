// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_core::error::AnyError;
use once_cell::sync::Lazy;
use std::{collections::HashMap, str::FromStr};

static JS_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([(
    "content-type".to_string(),
    "application/javascript".to_string(),
  )])
  .into_iter()
  .collect()
});

static JSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/jsx".to_string())])
    .into_iter()
    .collect()
});

static TS_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([(
    "content-type".to_string(),
    "application/typescript".to_string(),
  )])
  .into_iter()
  .collect()
});

static TSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/tsx".to_string())])
    .into_iter()
    .collect()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
  JavaScript,
  Jsx,
  TypeScript,
  Tsx,
  Json,
  JsonC,
  Markdown,
  Unknown,
}

impl LanguageId {
  pub fn as_media_type(&self) -> MediaType {
    match self {
      LanguageId::JavaScript => MediaType::JavaScript,
      LanguageId::Jsx => MediaType::Jsx,
      LanguageId::TypeScript => MediaType::TypeScript,
      LanguageId::Tsx => MediaType::Tsx,
      LanguageId::Json => MediaType::Json,
      LanguageId::JsonC => MediaType::Json,
      LanguageId::Markdown | LanguageId::Unknown => MediaType::Unknown,
    }
  }

  pub fn as_extension(&self) -> Option<&'static str> {
    match self {
      LanguageId::JavaScript => Some("js"),
      LanguageId::Jsx => Some("jsx"),
      LanguageId::TypeScript => Some("ts"),
      LanguageId::Tsx => Some("tsx"),
      LanguageId::Json => Some("json"),
      LanguageId::JsonC => Some("jsonc"),
      LanguageId::Markdown => Some("md"),
      LanguageId::Unknown => None,
    }
  }

  /// Get the HTTP headers for the language.
  ///
  /// Returns `None` if the language is not diagnosable.
  pub fn as_headers(&self) -> Option<&HashMap<String, String>> {
    match self {
      Self::JavaScript => Some(&JS_HEADERS),
      Self::Jsx => Some(&JSX_HEADERS),
      Self::TypeScript => Some(&TS_HEADERS),
      Self::Tsx => Some(&TSX_HEADERS),
      _ => None,
    }
  }

  /// Returns `true` if the language is diagnosable.
  ///
  /// Only JavaScript, JSX, TypeScript, and TSX are diagnosable.
  pub fn is_diagnosable(&self) -> bool {
    matches!(
      self,
      Self::JavaScript | Self::Jsx | Self::TypeScript | Self::Tsx
    )
  }
}

impl FromStr for LanguageId {
  type Err = AnyError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "javascript" => Ok(Self::JavaScript),
      "javascriptreact" | "jsx" => Ok(Self::Jsx),
      "typescript" => Ok(Self::TypeScript),
      "typescriptreact" | "tsx" => Ok(Self::Tsx),
      "json" => Ok(Self::Json),
      "jsonc" => Ok(Self::JsonC),
      "markdown" => Ok(Self::Markdown),
      _ => Ok(Self::Unknown),
    }
  }
}
