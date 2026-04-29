// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;

use deno_config::deno_json::ConfigFile;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use serde::Serialize;

use super::ProcessedFile;
use super::extensions::ts_to_dts_extension;
use super::extensions::ts_to_js_extension;

/// Build the lookup of source paths (as written in `deno.json` exports,
/// normalized) for which fast-check generated a usable `.d.ts`. We need
/// this so the `types` field is only emitted for entries whose declarations
/// actually exist in the tarball; otherwise TypeScript would resolve a
/// missing or empty stub and conclude the module exports nothing.
fn dts_available_set(files: &[ProcessedFile]) -> HashSet<String> {
  files
    .iter()
    .filter(|f| f.dts_content.is_some())
    .map(|f| f.output_path.clone())
    .collect()
}

/// Returns true if the deno.json export source path (e.g. "./mod.ts") has
/// a corresponding generated .d.ts in the tarball.
fn has_dts(set: &HashSet<String>, source_path: &str) -> bool {
  set.contains(&ts_to_js_extension(source_path))
}

/// Pinned version range of the @deno/shim-deno polyfill that pack injects
/// when it detects Deno API usage. Bump this when shim-deno publishes a
/// release we want users to pick up by default; check
/// https://www.npmjs.com/package/@deno/shim-deno for the latest. We use a
/// `~` range so consumers receive patch fixes but not breaking changes.
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

  let dts_set = dts_available_set(files);

  let exports = convert_exports(&config_file.json.exports, &dts_set)?;
  let (main, types) =
    extract_main_and_types(&config_file.json.exports, &dts_set);

  // Collect dependencies from all files
  let mut dependencies = HashMap::new();

  if include_deno_shim {
    dependencies
      .insert("@deno/shim-deno".to_string(), DENO_SHIM_VERSION.to_string());
  }

  // Merge dependencies from all processed files
  for file in files {
    for (name, version) in &file.dependencies {
      dependencies.insert(name.clone(), version.clone());
    }
  }

  let license = config_file
    .json
    .license
    .as_ref()
    .and_then(|l| l.as_str().map(|s| s.to_string()));

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
  dts_set: &HashSet<String>,
) -> Result<serde_json::Value, AnyError> {
  let Some(exports) = exports else {
    return Ok(json!("./mod.js"));
  };

  if let Some(s) = exports.as_str() {
    let js_path = ts_to_js_extension(s);
    let mut entry = serde_json::Map::new();
    if has_dts(dts_set, s) {
      entry.insert(
        "types".to_string(),
        json!(format!("./{}", ts_to_dts_extension(s))),
      );
    }
    entry.insert("import".to_string(), json!(format!("./{}", js_path)));
    entry.insert("default".to_string(), json!(format!("./{}", js_path)));
    return Ok(json!({ ".": entry }));
  }

  if let Some(map) = exports.as_object() {
    let mut result = serde_json::Map::new();

    for (key, value) in map.iter() {
      if let Some(path) = value.as_str() {
        let js_path = ts_to_js_extension(path);
        let mut entry = serde_json::Map::new();
        if has_dts(dts_set, path) {
          entry.insert(
            "types".to_string(),
            json!(format!("./{}", ts_to_dts_extension(path))),
          );
        }
        entry.insert("import".to_string(), json!(format!("./{}", js_path)));
        entry.insert("default".to_string(), json!(format!("./{}", js_path)));
        result.insert(key.clone(), serde_json::Value::Object(entry));
      } else if let Some(obj) = value.as_object() {
        let mut rewritten = serde_json::Map::new();
        for (condition, cond_value) in obj.iter() {
          if let Some(path) = cond_value.as_str() {
            if condition == "types" {
              if has_dts(dts_set, path) {
                rewritten.insert(
                  condition.clone(),
                  json!(format!("./{}", ts_to_dts_extension(path))),
                );
              }
              // omit `types` when no .d.ts was generated
            } else {
              rewritten.insert(
                condition.clone(),
                json!(format!("./{}", ts_to_js_extension(path))),
              );
            }
          } else {
            rewritten.insert(condition.clone(), cond_value.clone());
          }
        }
        result.insert(key.clone(), serde_json::Value::Object(rewritten));
      }
    }

    return Ok(serde_json::Value::Object(result));
  }

  deno_core::anyhow::bail!(
    "unsupported 'exports' shape in deno.json: expected a string or object, got {}",
    exports
  )
}

fn extract_main_and_types(
  exports: &Option<serde_json::Value>,
  dts_set: &HashSet<String>,
) -> (Option<String>, Option<String>) {
  let Some(exports) = exports else {
    let types = if dts_set.contains("mod.js") {
      Some("./mod.d.ts".to_string())
    } else {
      None
    };
    return (Some("./mod.js".to_string()), types);
  };

  if let Some(s) = exports.as_str() {
    let js_path = format!("./{}", ts_to_js_extension(s));
    let dts_path = if has_dts(dts_set, s) {
      Some(format!("./{}", ts_to_dts_extension(s)))
    } else {
      None
    };
    return (Some(js_path), dts_path);
  }

  if let Some(map) = exports.as_object()
    && let Some(dot_export) = map.get(".")
  {
    if let Some(path) = dot_export.as_str() {
      let js_path = format!("./{}", ts_to_js_extension(path));
      let dts_path = if has_dts(dts_set, path) {
        Some(format!("./{}", ts_to_dts_extension(path)))
      } else {
        None
      };
      return (Some(js_path), dts_path);
    } else if let Some(obj) = dot_export.as_object() {
      let main = obj
        .get("import")
        .or_else(|| obj.get("default"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
      let types = obj
        .get("types")
        .and_then(|v| v.as_str())
        .filter(|s| has_dts(dts_set, s))
        .map(|s| format!("./{}", ts_to_dts_extension(s)));
      return (main, types);
    }
  }

  (None, None)
}
