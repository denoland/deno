// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use serde::Serialize;

use super::ProcessedFile;

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

  // Collect dependencies from all files
  let mut dependencies = HashMap::new();

  if include_deno_shim {
    dependencies.insert("@deno/shim-deno".to_string(), "~0.19.0".to_string());
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
    description: None, // TODO: extract from config if available
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
    let js_path = convert_ts_to_js_path(s);
    return Ok(json!({
      "types": format!("./{}", convert_ts_to_dts_path(s)),
      "import": format!("./{}", js_path),
      "default": format!("./{}", js_path)
    }));
  }

  // Handle object exports
  if let Some(map) = exports.as_object() {
    let mut result = serde_json::Map::new();

    for (key, value) in map.iter() {
      if let Some(path) = value.as_str() {
        let js_path = convert_ts_to_js_path(path);
        result.insert(
          key.clone(),
          json!({
            "types": format!("./{}", convert_ts_to_dts_path(path)),
            "import": format!("./{}", js_path),
            "default": format!("./{}", js_path)
          }),
        );
      }
    }

    return Ok(serde_json::Value::Object(result));
  }

  // Fallback
  Ok(json!("./mod.js"))
}

fn convert_ts_to_js_path(path: &str) -> String {
  let path = path.trim_start_matches("./");
  if path.ends_with(".ts") {
    path.replace(".ts", ".js")
  } else if path.ends_with(".tsx") {
    path.replace(".tsx", ".js")
  } else if path.ends_with(".mts") {
    path.replace(".mts", ".mjs")
  } else {
    path.to_string()
  }
}

fn convert_ts_to_dts_path(path: &str) -> String {
  let path = path.trim_start_matches("./");
  if path.ends_with(".ts") || path.ends_with(".tsx") {
    path.replace(".ts", ".d.ts").replace(".tsx", ".d.ts")
  } else if path.ends_with(".mts") {
    path.replace(".mts", ".d.mts")
  } else {
    format!("{}.d.ts", path.trim_end_matches(".js"))
  }
}
