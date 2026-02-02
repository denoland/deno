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
  if path.ends_with(".tsx") {
    format!("{}.js", &path[..path.len() - 4])
  } else if path.ends_with(".ts") {
    format!("{}.js", &path[..path.len() - 3])
  } else if path.ends_with(".mts") {
    format!("{}.mjs", &path[..path.len() - 4])
  } else {
    path.to_string()
  }
}

fn convert_ts_to_dts_path(path: &str) -> String {
  let path = path.trim_start_matches("./");
  // Handle .tsx before .ts to avoid substring issues
  if path.ends_with(".tsx") {
    format!("{}.d.ts", &path[..path.len() - 4])
  } else if path.ends_with(".ts") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else if path.ends_with(".mts") {
    format!("{}.d.mts", &path[..path.len() - 4])
  } else if path.ends_with(".js") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else {
    format!("{}.d.ts", path)
  }
}

fn extract_main_and_types(exports: &Option<serde_json::Value>) -> (Option<String>, Option<String>) {
  let Some(exports) = exports else {
    return (Some("./mod.js".to_string()), Some("./mod.d.ts".to_string()));
  };

  // Handle string export
  if let Some(s) = exports.as_str() {
    let js_path = format!("./{}", convert_ts_to_js_path(s));
    let dts_path = format!("./{}", convert_ts_to_dts_path(s));
    return (Some(js_path), Some(dts_path));
  }

  // Handle object exports - look for "." entry
  if let Some(map) = exports.as_object() {
    if let Some(dot_export) = map.get(".") {
      if let Some(path) = dot_export.as_str() {
        let js_path = format!("./{}", convert_ts_to_js_path(path));
        let dts_path = format!("./{}", convert_ts_to_dts_path(path));
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
