// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use import_map::ImportMap;
use log::warn;

pub fn import_map_from_value(
  specifier: &Url,
  value: serde_json::Value,
) -> Result<ImportMap, AnyError> {
  debug_assert!(
    !specifier.as_str().contains("../"),
    "Import map specifier incorrectly contained ../: {}",
    specifier.as_str()
  );
  let result = import_map::parse_from_value(specifier, value)?;
  print_import_map_diagnostics(&result.diagnostics);
  Ok(result.import_map)
}

pub fn import_map_from_text(
  specifier: &Url,
  json_text: &str,
) -> Result<ImportMap, AnyError> {
  debug_assert!(
    !specifier.as_str().contains("../"),
    "Import map specifier incorrectly contained ../: {}",
    specifier.as_str()
  );
  let result = import_map::parse_from_json(specifier, json_text)?;
  print_import_map_diagnostics(&result.diagnostics);
  Ok(result.import_map)
}

fn print_import_map_diagnostics(diagnostics: &[String]) {
  if !diagnostics.is_empty() {
    warn!(
      "Import map diagnostics:\n{}",
      diagnostics
        .iter()
        .map(|d| format!("  - {}", d))
        .collect::<Vec<_>>()
        .join("\n")
    );
  }
}
