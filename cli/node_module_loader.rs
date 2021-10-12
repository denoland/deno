// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
// fn node_resolve(
//   specifier: &str,
//   referrer: &str,
//   is_main: bool,
// ) -> Result<ModuleSpecifier, AnyError> {
//   // TODO(bartlomieju): shipped "policy" part

//   if let Ok(url) = Url::parse(specifier) {
//     if url.scheme() == "data:" {
//       return Ok(url);
//     }

//     let protocol = url.scheme();

//     if protocol == "node" {
//       return Ok(url);
//     }

//     if protocol != "file" && protocol != "data" {
//       return Err(generic_error(format!("Only file and data URLs are supported by the default ESM loader. Received protocol '{}'", protocol)));
//     }

//     // In Deno there's no way to expose internal Node modules anyway,
//     // so calls to NativeModule.canBeRequiredByUsers would only work for built-in modules.

//     if referrer.starts_with("data:") {
//       let referrer_url = Url::parse(referrer)?;
//       return referrer_url.join(specifier).map_err(AnyError::from);
//     }

//     let referrer = if is_main {
//       // path_to_file_url()
//       referrer
//     } else {
//       referrer
//     };

//     let url = module_resolve(specifier, referrer)?;

//     // TODO: check codes

//     Ok(url)
//   }

//   // Ok(module_specifier)
//   todo!()
// }

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier == "" {
    return false;
  }

  if specifier.chars().nth(0) == Some('/') {
    return true;
  }

  is_relative_specifier(specifier)
}

fn is_relative_specifier(specifier: &str) -> bool {
  let specifier_len = specifier.len();
  let mut specifier_chars = specifier.chars();

  if specifier_chars.nth(0) == Some('.') {
    if specifier_len == 1 || specifier_chars.nth(1) == Some('/') {
      return true;
    }
    if specifier_chars.nth(1) == Some('.') {
      if specifier_len == 2 || specifier_chars.nth(2) == Some('/') {
        return true;
      }
    }
  }
  false
}

fn module_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let resolved = if should_be_treated_as_relative_or_absolute_path(specifier) {
    base.join(specifier)?
  } else if specifier.chars().nth(0) == Some('#') {
    package_imports_resolve(specifier, base)?
  } else {
    if let Ok(resolved) = Url::parse(specifier) {
      resolved
    } else {
      package_resolve(specifier, base)?
    }
  };
  finalize_resolution(resolved, base)
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let encoded_sep_re = Regex::new(r"%2F|%2C").expect("bad regex");

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(generic_error(format!(
      "{} must not include encoded \"/\" or \"\\\\\" characters {}",
      resolved.path(),
      base.to_file_path().unwrap().display()
    )));
  }

  let path = resolved.to_file_path().unwrap();

  // TODO(bartlomieju): currently not supported
  // if (getOptionValue('--experimental-specifier-resolution') === 'node') {
  //   ...
  // }

  let p_str = path.to_str().unwrap();
  let p = if p_str.ends_with('/') {
    p_str[p_str.len() - 1..].to_string()
  } else {
    p_str.to_string()
  };

  let stats = std::fs::metadata(&p)?;
  if stats.is_dir() {
    return Err(
      generic_error(
        format!("Directory import {} is not supported resolving ES modules imported from {}",
          path.display(), base.to_file_path().unwrap().display()
        )
    ));
  } else if !stats.is_file() {
    return Err(generic_error(format!(
      "Cannot find module {} imported from {}",
      path.display(),
      base.to_file_path().unwrap().display()
    )));
  }

  Ok(resolved)
}

fn package_imports_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  todo!()
}

fn is_conditional_exports_main_sugar(
  exports: &Value,
  package_json_url: &Url,
  base: &ModuleSpecifier,
) -> Result<bool, AnyError> {
  if exports.is_string() || exports.is_array() {
    return Ok(true);
  }

  if exports.is_null() || !exports.is_object() {
    return Ok(false);
  }

  let exports_obj = exports.as_object().unwrap();
  let mut is_conditional_sugar = false;
  let mut i = 0;
  for key in exports_obj.keys() {
    let cur_is_conditional_sugar = key == "" || !key.starts_with('.');
    if i == 0 {
      is_conditional_sugar = cur_is_conditional_sugar;
      i += 1;
    } else if is_conditional_sugar != cur_is_conditional_sugar {
      let msg = format!(
        "Invalid package config {} while importing {}.
      \"exports\" cannot contains some keys starting with \'.\' and some not.
      The exports object must either be an object of package subpath keys
      or an object of main entry condition name keys only.",
        package_json_url.to_file_path().unwrap().display(),
        base.as_str()
      );
      return Err(generic_error(msg));
    }
  }

  Ok(is_conditional_sugar)
}

// TODO(bartlomieju): last argument "conditions" was skipped
fn resolve_package_target_string(
  target: String,
  subpath: String,
  match_: String,
  package_json_url: Url,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
) -> Result<ModuleSpecifier, AnyError> {
  if subpath == "" && !pattern && !target.ends_with('/') {
    todo!()
  }

  if !target.starts_with("./") {
    if internal && !target.starts_with("../") && !target.starts_with('/') {
      todo!()
    }
    todo!()
  }

  let invalid_segment_re =
    Regex::new(r"(^|\|/)(..?|node_modules)(\|/|$)").expect("bad regex");
  let pattern_re = Regex::new(r"*").expect("bad regex");

  if invalid_segment_re.is_match(&target[2..]) {
    todo!()
  }

  let resolved = package_json_url.join(&target)?;
  let resolved_path = resolved.path();
  let package_url = package_json_url.join(".").unwrap();
  let package_path = package_url.path();

  if !resolved_path.starts_with(package_path) {
    todo!()
  }

  if subpath == "" {
    return Ok(resolved);
  }

  if invalid_segment_re.is_match(&subpath) {
    todo!()
  }

  if pattern {
    let replaced = pattern_re
      .replace(resolved.as_str(), |_caps: &regex::Captures| subpath.clone());
    let url = Url::parse(&replaced)?;
    return Ok(url);
  }

  Ok(resolved.join(&subpath)?)
}

// TODO(bartlomieju): last argument "conditions" was skipped
fn resolve_package_target(
  package_json_url: Url,
  target: Value,
  subpath: String,
  package_subpath: String,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
) -> Result<ModuleSpecifier, AnyError> {
  if let Some(target) = target.as_str() {
    return resolve_package_target_string(
      target.to_string(),
      subpath,
      package_subpath,
      package_json_url,
      base,
      pattern,
      internal,
      // TODO(bartlomieju): last argument "conditions" was skipped
    );
  } else if let Some(target_arr) = target.as_array() {
    if target_arr.is_empty() {
      todo!()
    }

    todo!()
  } else if let Some(target_obj) = target.as_object() {
    todo!()
  } else if target.is_null() {
    todo!()
  }

  todo!()
}

// TODO(bartlomieju): last argument "conditions" was skipped
fn package_exports_resolve(
  package_json_url: Url,
  package_subpath: String,
  package_config: PackageConfig,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let exports = &package_config.exports.unwrap();

  let exports_map =
    if is_conditional_exports_main_sugar(exports, &package_json_url, base)? {
      let mut map = Map::new();
      map.insert(".".to_string(), exports.to_owned());
      map
    } else {
      exports.as_object().unwrap().to_owned()
    };

  if exports_map.contains_key(&package_subpath)
    && package_subpath.find('*').is_none()
    && !package_subpath.ends_with('/')
  {
    let target = exports_map.get(&package_subpath).unwrap().to_owned();
    // TODO(bartlomieju): last argument "conditions" was skipped
    let resolved = resolve_package_target(
      package_json_url,
      target,
      "".to_string(),
      package_subpath,
      base,
      false,
      false,
    )?;
    // TODO()
    return Ok(resolved);
  }

  todo!()
}

fn package_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, is_scoped) =
    parse_package_name(specifier, base)?;

  // ResolveSelf
  let package_config = get_package_scope_config(base)?;
  if package_config.exists {
    let package_json_url =
      Url::from_file_path(&package_config.pjsonpath).unwrap();
    if package_config.name == Some(package_name) {
      if let Some(exports) = &package_config.exports {
        if !exports.is_null() {
          // TODO(bartlomieju): last argument "conditions" was skipped
          return package_exports_resolve(
            package_json_url,
            package_subpath,
            package_config,
            base,
          );
        }
      }
    }
  }

  todo!()
}

fn parse_package_name(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<(String, String, bool), AnyError> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = false;
  let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else {
    if specifier.chars().nth(0) == Some('@') {
      is_scoped = true;
      if let Some(index) = separator_index {
        separator_index = specifier[index + 1..].find('/');
      } else {
        valid_package_name = false;
      }
    }
  }

  let package_name = if let Some(index) = separator_index {
    specifier[0..index].to_string()
  } else {
    specifier.to_string()
  };

  // Package name cannot have leading . and cannot have percent-encoding or separators.
  for ch in package_name.chars() {
    if ch == '%' || ch == '\\' {
      valid_package_name = false;
      break;
    }
  }

  if !valid_package_name {
    return Err(generic_error(format!(
      "{} is not a valid package name {}",
      specifier,
      base.to_file_path().unwrap().display()
    )));
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
      .to_string()
  } else {
    ".".to_string()
  };

  Ok((package_name, package_subpath, is_scoped))
}

// enum ExportConfig {
//   Str(String),
//   StrArray(Vec<String>),
// }

// enum PackageType {
//   Module,
//   CommonJs,
// }

#[derive(Clone, Debug)]
struct PackageConfig {
  exists: bool,
  exports: Option<Value>,
  imports: Option<Map<String, Value>>,
  main: Option<String>,
  name: Option<String>,
  pjsonpath: PathBuf,
  typ: String,
}

fn get_package_config(
  path: PathBuf,
  specifier: &ModuleSpecifier,
  maybe_base: Option<&ModuleSpecifier>,
) -> Result<PackageConfig, AnyError> {
  // TODO(bartlomieju):
  // if let Some(existing) = package_json_cache.get(path) {
  //   return Ok(existing.clone());
  // }

  // TODO: maybe shouldn't error be return empty package
  let source = std::fs::read_to_string(&path)?;
  if source.is_empty() {
    let package_config = PackageConfig {
      pjsonpath: path,
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

  let package_json: Value = serde_json::from_str(&source).map_err(|_err| {
    let mut msg = format!("Invalid package config {}", path.display());

    if let Some(base) = maybe_base {
      msg = format!(
        "{} \"{}\" from {}",
        msg,
        specifier.as_str(),
        base.to_file_path().unwrap().display()
      );
    }

    generic_error(msg)
  })?;

  let imports_val = package_json.get("imports");
  let main_val = package_json.get("main");
  let name_val = package_json.get("name");
  let typ_val = package_json.get("type");
  let exports = package_json.get("exports").map(|e| e.to_owned());

  // TODO(bartlomieju): refactor
  let imports = if let Some(imp) = imports_val {
    if let Some(imp) = imp.as_object() {
      Some(imp.to_owned())
    } else {
      None
    }
  } else {
    None
  };
  let main = if let Some(m) = main_val {
    if let Some(m) = m.as_str() {
      Some(m.to_string())
    } else {
      None
    }
  } else {
    None
  };
  let name = if let Some(n) = name_val {
    if let Some(n) = n.as_str() {
      Some(n.to_string())
    } else {
      None
    }
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
    pjsonpath: path,
    exists: false,
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

fn get_package_scope_config(
  resolved: &ModuleSpecifier,
) -> Result<PackageConfig, AnyError> {
  let mut package_json_url = resolved.join("./package.json")?;

  loop {
    let package_json_path = package_json_url.path();

    if package_json_path.ends_with("node_modules/package.json") {
      break;
    }

    let package_config = get_package_config(
      package_json_url.to_file_path().unwrap(),
      resolved,
      None,
    )?;
    if package_config.exists {
      return Ok(package_config);
    }

    let last_package_json_url = package_json_url.clone();
    package_json_url = package_json_url.join("../package.json")?;

    // Terminates at root where ../package.json equals ../../package.json
    // (can't just check "/package.json" for Windows support)
    if package_json_url.path() == last_package_json_url.path() {
      break;
    }
  }

  let package_json_path = package_json_url.to_file_path().unwrap();
  let package_config = PackageConfig {
    pjsonpath: package_json_path,
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
