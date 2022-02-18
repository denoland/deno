// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::errors;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use std::path::PathBuf;
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct PackageConfig {
  pub exists: bool,
  pub exports: Option<Value>,
  pub imports: Option<Map<String, Value>>,
  pub main: Option<String>,
  pub name: Option<String>,
  pub pjsonpath: PathBuf,
  pub typ: String,
}

pub(crate) fn get_package_config(
  path: &Path,
  specifier: &str,
  maybe_base: Option<&Path>,
) -> Result<PackageConfig, AnyError> {
  // TODO(bartlomieju):
  // if let Some(existing) = package_json_cache.get(path) {
  //   return Ok(existing.clone());
  // }

  let result = std::fs::read_to_string(&path);

  let source = result.unwrap_or_else(|_| "".to_string());
  if source.is_empty() {
    let package_config = PackageConfig {
      pjsonpath: path.to_path_buf(),
      exists: false,
      main: None,
      name: None,
      typ: "none".to_string(),
      exports: None,
      imports: None,
    };
    // TODO(bartlomieju):
    // package_json_cache.set(package_json_path, package_config.clone());
    return Ok(package_config);
  }

  let package_json: Value = serde_json::from_str(&source).map_err(|err| {
    let base_msg = maybe_base.map(|base| {
      format!("\"{}\" from \"{}\"", specifier, base.to_string_lossy())
    });
    errors::err_invalid_package_config(
      &path.display().to_string(),
      base_msg,
      Some(err.to_string()),
    )
  })?;

  let imports_val = package_json.get("imports");
  let main_val = package_json.get("main");
  let name_val = package_json.get("name");
  let typ_val = package_json.get("type");
  let exports = package_json.get("exports").map(|e| e.to_owned());

  let imports = if let Some(imp) = imports_val {
    imp.as_object().map(|imp| imp.to_owned())
  } else {
    None
  };
  let main = if let Some(m) = main_val {
    m.as_str().map(|m| m.to_string())
  } else {
    None
  };
  let name = if let Some(n) = name_val {
    n.as_str().map(|n| n.to_string())
  } else {
    None
  };

  // Ignore unknown types for forwards compatibility
  let typ = if let Some(t) = typ_val {
    if let Some(t) = t.as_str() {
      if t != "module" && t != "commonjs" {
        "none".to_string()
      } else {
        t.to_string()
      }
    } else {
      "none".to_string()
    }
  } else {
    "none".to_string()
  };

  let package_config = PackageConfig {
    pjsonpath: path.to_path_buf(),
    exists: true,
    main,
    name,
    typ,
    exports,
    imports,
  };
  // TODO(bartlomieju):
  // package_json_cache.set(package_json_path, package_config.clone());
  Ok(package_config)
}

pub(crate) fn get_package_scope_config(
  resolved: &Path,
) -> Result<PackageConfig, AnyError> {
  let mut package_json_path = resolved.join("./package.json");

  loop {
    if package_json_path.ends_with("node_modules/package.json") {
      break;
    }

    let package_config = get_package_config(
      &package_json_path,
      &resolved.to_string_lossy().to_string(),
      None,
    )?;

    if package_config.exists {
      return Ok(package_config);
    }

    let last_package_json_path = package_json_path.clone();
    package_json_path = package_json_path.join("../package.json");

    // TODO(bartlomieju): I'm not sure this will work properly
    // Terminates at root where ../package.json equals ../../package.json
    // (can't just check "/package.json" for Windows support)
    if package_json_path == last_package_json_path {
      break;
    }
  }

  let package_config = PackageConfig {
    pjsonpath: package_json_path.to_owned(),
    exists: false,
    main: None,
    name: None,
    typ: "none".to_string(),
    exports: None,
    imports: None,
  };

  // TODO(bartlomieju):
  // package_json_cache.set(package_json_path, package_config.clone());

  Ok(package_config)
}
