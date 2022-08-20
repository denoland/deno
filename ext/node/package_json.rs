// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::DenoDirNpmResolver;
use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use serde::Serialize;
use std::io::ErrorKind;
use std::path::PathBuf;

// TODO(bartlomieju): deduplicate with cli/compat/esm_resolver.rs
#[derive(Clone, Debug, Serialize)]
pub struct PackageJson {
  pub exists: bool,
  pub exports: Option<Map<String, Value>>,
  pub imports: Option<Map<String, Value>>,
  pub bin: Option<Value>,
  pub main: Option<String>,
  pub name: Option<String>,
  pub path: PathBuf,
  pub typ: String,
  pub types: Option<String>,
}

impl PackageJson {
  pub fn empty(path: PathBuf) -> PackageJson {
    PackageJson {
      exists: false,
      exports: None,
      imports: None,
      bin: None,
      main: None,
      name: None,
      path,
      typ: "none".to_string(),
      types: None,
    }
  }

  pub fn load(
    resolver: &dyn DenoDirNpmResolver,
    path: PathBuf,
  ) -> Result<PackageJson, AnyError> {
    resolver.ensure_read_permission(&path)?;
    let source = match std::fs::read_to_string(&path) {
      Ok(source) => source,
      Err(err) if err.kind() == ErrorKind::NotFound => {
        return Ok(PackageJson::empty(path));
      }
      Err(err) => bail!(
        "Error loading package.json at {}. {:#}",
        path.display(),
        err
      ),
    };

    if source.trim().is_empty() {
      return Ok(PackageJson::empty(path));
    }

    let package_json: Value = serde_json::from_str(&source)
      .map_err(|err| anyhow::anyhow!("malformed package.json {}", err))?;

    let imports_val = package_json.get("imports");
    let main_val = package_json.get("main");
    let name_val = package_json.get("name");
    let type_val = package_json.get("type");
    let bin = package_json.get("bin").map(ToOwned::to_owned);
    let exports = package_json.get("exports").map(|exports| {
      if is_conditional_exports_main_sugar(exports) {
        let mut map = Map::new();
        map.insert(".".to_string(), exports.to_owned());
        map
      } else {
        exports.as_object().unwrap().to_owned()
      }
    });

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
    let typ = if let Some(t) = type_val {
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

    // for typescript, it looks for "typings" first, then "types"
    let types = package_json
      .get("typings")
      .or_else(|| package_json.get("types"))
      .and_then(|t| t.as_str().map(|s| s.to_string()));

    let package_json = PackageJson {
      exists: true,
      path,
      main,
      name,
      typ,
      types,
      exports,
      imports,
      bin,
    };
    Ok(package_json)
  }
}

fn is_conditional_exports_main_sugar(exports: &Value) -> bool {
  if exports.is_string() || exports.is_array() {
    return true;
  }

  if exports.is_null() || !exports.is_object() {
    return false;
  }

  let exports_obj = exports.as_object().unwrap();
  let mut is_conditional_sugar = false;
  let mut i = 0;
  for key in exports_obj.keys() {
    let cur_is_conditional_sugar = key.is_empty() || !key.starts_with('.');
    if i == 0 {
      is_conditional_sugar = cur_is_conditional_sugar;
      i += 1;
    } else if is_conditional_sugar != cur_is_conditional_sugar {
      panic!("\"exports\" cannot contains some keys starting with \'.\' and some not.
        The exports object must either be an object of package subpath keys
        or an object of main entry condition name keys only.")
    }
  }

  is_conditional_sugar
}
