// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use serde::Serialize;

use super::extensions::ts_to_dts_extension;
use super::extensions::ts_to_js_extension;
use super::ProcessedFile;

const DENO_SHIM_VERSION: &str = "~0.19.0";

#[derive(Serialize)]
struct PackageJson {
  name: String,
  version: String,
  #[serde(rename = "type")]
  module_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  license: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  description: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  main: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  types: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  exports: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  dependencies: Option<HashMap<String, String>>,
}

pub fn generate_package_json(
  config_file: &ConfigFile,
  version: &str,
  files: &[ProcessedFile],
  include_deno_shim: bool,
) -> Result<String, AnyError> {
  let name = config_file
    .json
    .name
    .as_ref()
    .ok_or_else(|| deno_core::anyhow::anyhow!("Missing name in config"))?;

  // Convert exports from deno.json
  let exports = convert_exports(&config_file.json.exports)?;

  // Extract main and types from exports if "." entry exists
  let (main, types) = extract_main_and_types(&config_file.json.exports);

  // Collect dependencies from all files
  let mut dependencies = HashMap::new();

  if include_deno_shim {
    dependencies.insert("@deno/shim-deno".to_string(), DENO_SHIM_VERSION.to_string());
  }

  // Merge dependencies from all processed files
  for file in files {
    for (name, version) in &file.dependencies {
      dependencies.insert(name.clone(), version.clone());
    }
  }

  let license = config_file.json.license.as_ref().and_then(|l| {
    l.as_str().map(|s| s.to_string())
  });

  let pkg = PackageJson {
    name: name.clone(),
    version: version.to_string(),
    module_type: "module".to_string(),
    license,
    description: None,
    main,
    types,
    exports: Some(exports),
    dependencies: if dependencies.is_empty() {
      None
    } else {
      Some(dependencies)
    },
  };

  let json = serde_json::to_string_pretty(&pkg)?;
  Ok(json)
}

fn convert_exports(
  exports: &Option<serde_json::Value>,
) -> Result<serde_json::Value, AnyError> {
  let Some(exports) = exports else {
    return Ok(json!("./mod.js"));
  };

  // Handle string export
  if let Some(s) = exports.as_str() {
    let js_path = ts_to_js_extension(s);
    return Ok(json!({
      "types": format!("./{}", ts_to_dts_extension(s)),
      "import": format!("./{}", js_path),
      "default": format!("./{}", js_path)
    }));
  }

  // Handle object exports
  if let Some(map) = exports.as_object() {
    let mut result = serde_json::Map::new();

    for (key, value) in map.iter() {
      if let Some(path) = value.as_str() {
        let js_path = ts_to_js_extension(path);
        result.insert(
          key.clone(),
          json!({
            "types": format!("./{}", ts_to_dts_extension(path)),
            "import": format!("./{}", js_path),
            "default": format!("./{}", js_path)
          }),
        );
      } else if let Some(obj) = value.as_object() {
        // Conditional exports (e.g., {"types": "...", "import": "..."})
        // Rewrite .ts → .js paths within nested object values
        let mut rewritten = serde_json::Map::new();
        for (condition, cond_value) in obj.iter() {
          if let Some(path) = cond_value.as_str() {
            let rewritten_path = if condition == "types" {
              format!("./{}", ts_to_dts_extension(path))
            } else {
              format!("./{}", ts_to_js_extension(path))
            };
            rewritten
              .insert(condition.clone(), serde_json::Value::String(rewritten_path));
          } else {
            // Pass through non-string values as-is
            rewritten.insert(condition.clone(), cond_value.clone());
          }
        }
        result.insert(key.clone(), serde_json::Value::Object(rewritten));
      }
    }

    return Ok(serde_json::Value::Object(result));
  }

  // Fallback
  Ok(json!("./mod.js"))
}


fn extract_main_and_types(exports: &Option<serde_json::Value>) -> (Option<String>, Option<String>) {
  let Some(exports) = exports else {
    return (Some("./mod.js".to_string()), Some("./mod.d.ts".to_string()));
  };

  // Handle string export
  if let Some(s) = exports.as_str() {
    let js_path = format!("./{}", ts_to_js_extension(s));
    let dts_path = format!("./{}", ts_to_dts_extension(s));
    return (Some(js_path), Some(dts_path));
  }

  // Handle object exports - look for "." entry
  if let Some(map) = exports.as_object() {
    if let Some(dot_export) = map.get(".") {
      if let Some(path) = dot_export.as_str() {
        let js_path = format!("./{}", ts_to_js_extension(path));
        let dts_path = format!("./{}", ts_to_dts_extension(path));
        return (Some(js_path), Some(dts_path));
      } else if let Some(obj) = dot_export.as_object() {
        // Conditional exports - extract from "import" or "default"
        let main = obj.get("import")
          .or_else(|| obj.get("default"))
          .and_then(|v| v.as_str())
          .map(|s| s.to_string());
        let types = obj.get("types")
          .and_then(|v| v.as_str())
          .map(|s| s.to_string());
        return (main, types);
      }
    }
  }

  (None, None)
}
