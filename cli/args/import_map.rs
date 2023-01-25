// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::url::Url;
use import_map::ImportMap;
use import_map::ImportMapDiagnostic;
use log::warn;

pub fn import_map_from_text(
  specifier: &Url,
  json_text: &str,
  ignore_unknown_keys: bool,
) -> Result<ImportMap, AnyError> {
  debug_assert!(
    !specifier.as_str().contains("../"),
    "Import map specifier incorrectly contained ../: {}",
    specifier.as_str()
  );
  let result = import_map::parse_from_json(specifier, json_text)?;
  print_import_map_diagnostics(&result.diagnostics, ignore_unknown_keys);
  Ok(result.import_map)
}

fn print_import_map_diagnostics(
  diagnostics: &[ImportMapDiagnostic],
  ignore_unknown_keys: bool,
) {
  let diagnostics = diagnostics
    .iter()
    .filter(|d| {
      if ignore_unknown_keys {
        if let ImportMapDiagnostic::InvalidTopLevelKey(_) = d {
          return false;
        }
      }

      true
    })
    .collect::<Vec<_>>();

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
