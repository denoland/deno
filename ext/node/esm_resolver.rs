// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::errors;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use regex::Regex;
use std::path::PathBuf;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];

fn to_file_path(url: &ModuleSpecifier) -> PathBuf {
  url
    .to_file_path()
    .unwrap_or_else(|_| panic!("Provided URL was not file:// URL: {}", url))
}

fn to_file_path_string(url: &ModuleSpecifier) -> String {
  to_file_path(url).display().to_string()
}

fn should_be_treated_as_relative_or_absolute_path(specifier: &str) -> bool {
  if specifier.is_empty() {
    return false;
  }

  if specifier.starts_with('/') {
    return true;
  }

  is_relative_specifier(specifier)
}

// TODO(ry) We very likely have this utility function elsewhere in Deno.
fn is_relative_specifier(specifier: &str) -> bool {
  let specifier_len = specifier.len();
  let specifier_chars: Vec<_> = specifier.chars().collect();

  if !specifier_chars.is_empty() && specifier_chars[0] == '.' {
    if specifier_len == 1 || specifier_chars[1] == '/' {
      return true;
    }
    if specifier_chars[1] == '.'
      && (specifier_len == 2 || specifier_chars[2] == '/')
    {
      return true;
    }
  }
  false
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  // TODO(bartlomieju): this is not part of Node resolution algorithm
  // (as it doesn't support http/https); but I had to short circuit here
  // for remote modules because they are mainly used to polyfill `node` built
  // in modules. Another option would be to leave the resolved URLs
  // as `node:<module_name>` and do the actual remapping to std's polyfill
  // in module loader. I'm not sure which approach is better.
  if resolved.scheme().starts_with("http") {
    return Ok(resolved);
  }

  let encoded_sep_re = Regex::new(r"%2F|%2C").expect("bad regex");

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(errors::err_invalid_module_specifier(
      resolved.path(),
      "must not include encoded \"/\" or \"\\\\\" characters",
      Some(to_file_path_string(base)),
    ));
  }

  let path = to_file_path(&resolved);

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

  let (is_dir, is_file) = if let Ok(stats) = std::fs::metadata(&p) {
    (stats.is_dir(), stats.is_file())
  } else {
    (false, false)
  };
  if is_dir {
    return Err(errors::err_unsupported_dir_import(
      resolved.as_str(),
      base.as_str(),
    ));
  } else if !is_file {
    return Err(errors::err_module_not_found(
      resolved.as_str(),
      base.as_str(),
      "module",
    ));
  }

  Ok(resolved)
}

fn throw_import_not_defined(
  specifier: &str,
  package_json_url: Option<ModuleSpecifier>,
  base: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_import_not_defined(
    specifier,
    package_json_url.map(|u| to_file_path_string(&u.join(".").unwrap())),
    &to_file_path_string(base),
  )
}

fn pattern_key_compare(a: &str, b: &str) -> i32 {
  let a_pattern_index = a.find('*');
  let b_pattern_index = b.find('*');

  let base_len_a = if let Some(index) = a_pattern_index {
    index + 1
  } else {
    a.len()
  };
  let base_len_b = if let Some(index) = b_pattern_index {
    index + 1
  } else {
    b.len()
  };

  if base_len_a > base_len_b {
    return -1;
  }

  if base_len_b > base_len_a {
    return 1;
  }

  if a_pattern_index.is_none() {
    return 1;
  }

  if b_pattern_index.is_none() {
    return -1;
  }

  if a.len() > b.len() {
    return -1;
  }

  if b.len() > a.len() {
    return 1;
  }

  0
}

fn package_imports_resolve(
  name: &str,
  base: &ModuleSpecifier,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  if name == "#" || name.starts_with("#/") || name.ends_with('/') {
    let reason = "is not a valid internal imports specifier name";
    return Err(errors::err_invalid_module_specifier(
      name,
      reason,
      Some(to_file_path_string(base)),
    ));
  }

  let mut package_json_url = None;

  let package_config = get_package_scope_config(base)?;
  if package_config.exists {
    package_json_url =
      Some(Url::from_file_path(package_config.pjsonpath).unwrap());
    if let Some(imports) = &package_config.imports {
      if imports.contains_key(name) && !name.contains('*') {
        let maybe_resolved = resolve_package_target(
          package_json_url.clone().unwrap(),
          imports.get(name).unwrap().to_owned(),
          "".to_string(),
          name.to_string(),
          base,
          false,
          true,
          conditions,
        )?;
        if let Some(resolved) = maybe_resolved {
          return Ok(resolved);
        }
      } else {
        let mut best_match = "";
        let mut best_match_subpath = None;
        for key in imports.keys() {
          let pattern_index = key.find('*');
          if let Some(pattern_index) = pattern_index {
            let key_sub = &key[0..=pattern_index];
            if name.starts_with(key_sub) {
              let pattern_trailer = &key[pattern_index + 1..];
              if name.len() > key.len()
                && name.ends_with(&pattern_trailer)
                && pattern_key_compare(best_match, key) == 1
                && key.rfind('*') == Some(pattern_index)
              {
                best_match = key;
                best_match_subpath = Some(
                  name[pattern_index..=(name.len() - pattern_trailer.len())]
                    .to_string(),
                );
              }
            }
          }
        }

        if !best_match.is_empty() {
          let target = imports.get(best_match).unwrap().to_owned();
          let maybe_resolved = resolve_package_target(
            package_json_url.clone().unwrap(),
            target,
            best_match_subpath.unwrap(),
            best_match.to_string(),
            base,
            true,
            true,
            conditions,
          )?;
          if let Some(resolved) = maybe_resolved {
            return Ok(resolved);
          }
        }
      }
    }
  }

  Err(throw_import_not_defined(name, package_json_url, base))
}

fn is_conditional_exports_main_sugar(
  exports: &Value,
  package_json_url: &ModuleSpecifier,
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
    let cur_is_conditional_sugar = key.is_empty() || !key.starts_with('.');
    if i == 0 {
      is_conditional_sugar = cur_is_conditional_sugar;
      i += 1;
    } else if is_conditional_sugar != cur_is_conditional_sugar {
      return Err(errors::err_invalid_package_config(
        &to_file_path_string(package_json_url),
        Some(base.as_str().to_string()),
        Some("\"exports\" cannot contains some keys starting with \'.\' and some not.
        The exports object must either be an object of package subpath keys
        or an object of main entry condition name keys only.".to_string())
      ));
    }
  }

  Ok(is_conditional_sugar)
}

fn throw_invalid_package_target(
  subpath: String,
  target: String,
  package_json_url: &ModuleSpecifier,
  internal: bool,
  base: &ModuleSpecifier,
) -> AnyError {
  errors::err_invalid_package_target(
    to_file_path_string(&package_json_url.join(".").unwrap()),
    subpath,
    target,
    internal,
    Some(base.as_str().to_string()),
  )
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_url: &ModuleSpecifier,
  internal: bool,
  base: &ModuleSpecifier,
) -> AnyError {
  let ie = if internal { "imports" } else { "exports" };
  let reason = format!(
    "request is not a valid subpath for the \"{}\" resolution of {}",
    ie,
    to_file_path_string(package_json_url)
  );
  errors::err_invalid_module_specifier(
    &subpath,
    &reason,
    Some(to_file_path_string(base)),
  )
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target_string(
  target: String,
  subpath: String,
  match_: String,
  package_json_url: ModuleSpecifier,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  if !subpath.is_empty() && !pattern && !target.ends_with('/') {
    return Err(throw_invalid_package_target(
      match_,
      target,
      &package_json_url,
      internal,
      base,
    ));
  }

  let invalid_segment_re =
    Regex::new(r"(^|\|/)(..?|node_modules)(\|/|$)").expect("bad regex");
  let pattern_re = Regex::new(r"\*").expect("bad regex");

  if !target.starts_with("./") {
    if internal && !target.starts_with("../") && !target.starts_with('/') {
      let is_url = Url::parse(&target).is_ok();
      if !is_url {
        let export_target = if pattern {
          pattern_re
            .replace(&target, |_caps: &regex::Captures| subpath.clone())
            .to_string()
        } else {
          format!("{}{}", target, subpath)
        };
        return package_resolve(&export_target, &package_json_url, conditions);
      }
    }
    return Err(throw_invalid_package_target(
      match_,
      target,
      &package_json_url,
      internal,
      base,
    ));
  }

  if invalid_segment_re.is_match(&target[2..]) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      &package_json_url,
      internal,
      base,
    ));
  }

  let resolved = package_json_url.join(&target)?;
  let resolved_path = resolved.path();
  let package_url = package_json_url.join(".").unwrap();
  let package_path = package_url.path();

  if !resolved_path.starts_with(package_path) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      &package_json_url,
      internal,
      base,
    ));
  }

  if subpath.is_empty() {
    return Ok(resolved);
  }

  if invalid_segment_re.is_match(&subpath) {
    let request = if pattern {
      match_.replace('*', &subpath)
    } else {
      format!("{}{}", match_, subpath)
    };
    return Err(throw_invalid_subpath(
      request,
      &package_json_url,
      internal,
      base,
    ));
  }

  if pattern {
    let replaced = pattern_re
      .replace(resolved.as_str(), |_caps: &regex::Captures| subpath.clone());
    let url = Url::parse(&replaced)?;
    return Ok(url);
  }

  Ok(resolved.join(&subpath)?)
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target(
  package_json_url: ModuleSpecifier,
  target: Value,
  subpath: String,
  package_subpath: String,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if let Some(target) = target.as_str() {
    return Ok(Some(resolve_package_target_string(
      target.to_string(),
      subpath,
      package_subpath,
      package_json_url,
      base,
      pattern,
      internal,
      conditions,
    )?));
  } else if let Some(target_arr) = target.as_array() {
    if target_arr.is_empty() {
      return Ok(None);
    }

    let mut last_error = None;
    for target_item in target_arr {
      let resolved_result = resolve_package_target(
        package_json_url.clone(),
        target_item.to_owned(),
        subpath.clone(),
        package_subpath.clone(),
        base,
        pattern,
        internal,
        conditions,
      );

      if let Err(e) = resolved_result {
        let err_string = e.to_string();
        last_error = Some(e);
        if err_string.starts_with("[ERR_INVALID_PACKAGE_TARGET]") {
          continue;
        }
        return Err(last_error.unwrap());
      }
      let resolved = resolved_result.unwrap();
      if resolved.is_none() {
        last_error = None;
        continue;
      }
      return Ok(resolved);
    }
    if last_error.is_none() {
      return Ok(None);
    }
    return Err(last_error.unwrap());
  } else if let Some(target_obj) = target.as_object() {
    for key in target_obj.keys() {
      // TODO(bartlomieju): verify that keys are not numeric
      // return Err(errors::err_invalid_package_config(
      //   to_file_path_string(package_json_url),
      //   Some(base.as_str().to_string()),
      //   Some("\"exports\" cannot contain numeric property keys.".to_string()),
      // ));

      if key == "default" || conditions.contains(&key.as_str()) {
        let condition_target = target_obj.get(key).unwrap().to_owned();
        let resolved = resolve_package_target(
          package_json_url.clone(),
          condition_target,
          subpath.clone(),
          package_subpath.clone(),
          base,
          pattern,
          internal,
          conditions,
        )?;
        if resolved.is_none() {
          continue;
        }
        return Ok(resolved);
      }
    }
  } else if target.is_null() {
    return Ok(None);
  }

  Err(throw_invalid_package_target(
    package_subpath,
    target.to_string(),
    &package_json_url,
    internal,
    base,
  ))
}

fn throw_exports_not_found(
  subpath: String,
  package_json_url: &ModuleSpecifier,
  base: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_path_not_exported(
    to_file_path_string(&package_json_url.join(".").unwrap()),
    subpath,
    Some(to_file_path_string(base)),
  )
}

pub fn package_exports_resolve(
  package_json_url: ModuleSpecifier,
  package_subpath: String,
  package_config: PackageConfig,
  base: &ModuleSpecifier,
  conditions: &[&str],
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
    let resolved = resolve_package_target(
      package_json_url.clone(),
      target,
      "".to_string(),
      package_subpath.to_string(),
      base,
      false,
      false,
      conditions,
    )?;
    if resolved.is_none() {
      return Err(throw_exports_not_found(
        package_subpath,
        &package_json_url,
        base,
      ));
    }
    return Ok(resolved.unwrap());
  }

  let mut best_match = "";
  let mut best_match_subpath = None;
  for key in exports_map.keys() {
    let pattern_index = key.find('*');
    if let Some(pattern_index) = pattern_index {
      let key_sub = &key[0..=pattern_index];
      if package_subpath.starts_with(key_sub) {
        // When this reaches EOL, this can throw at the top of the whole function:
        //
        // if (StringPrototypeEndsWith(packageSubpath, '/'))
        //   throwInvalidSubpath(packageSubpath)
        //
        // To match "imports" and the spec.
        if package_subpath.ends_with('/') {
          // TODO(bartlomieju):
          // emitTrailingSlashPatternDeprecation();
        }
        let pattern_trailer = &key[pattern_index + 1..];
        if package_subpath.len() > key.len()
          && package_subpath.ends_with(&pattern_trailer)
          && pattern_key_compare(best_match, key) == 1
          && key.rfind('*') == Some(pattern_index)
        {
          best_match = key;
          best_match_subpath = Some(
            package_subpath
              [pattern_index..=(package_subpath.len() - pattern_trailer.len())]
              .to_string(),
          );
        }
      }
    }
  }

  if !best_match.is_empty() {
    let target = exports.get(best_match).unwrap().to_owned();
    let maybe_resolved = resolve_package_target(
      package_json_url.clone(),
      target,
      best_match_subpath.unwrap(),
      best_match.to_string(),
      base,
      true,
      false,
      conditions,
    )?;
    if let Some(resolved) = maybe_resolved {
      return Ok(resolved);
    } else {
      return Err(throw_exports_not_found(
        package_subpath,
        &package_json_url,
        base,
      ));
    }
  }

  Err(throw_exports_not_found(
    package_subpath,
    &package_json_url,
    base,
  ))
}

fn package_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, is_scoped) =
    parse_package_name(specifier, base)?;

  // ResolveSelf
  let package_config = get_package_scope_config(base)?;
  if package_config.exists {
    let package_json_url =
      Url::from_file_path(&package_config.pjsonpath).unwrap();
    if package_config.name.as_ref() == Some(&package_name) {
      if let Some(exports) = &package_config.exports {
        if !exports.is_null() {
          return package_exports_resolve(
            package_json_url,
            package_subpath,
            package_config,
            base,
            conditions,
          );
        }
      }
    }
  }

  let mut package_json_url =
    base.join(&format!("./node_modules/{}/package.json", package_name))?;
  let mut package_json_path = to_file_path(&package_json_url);
  let mut last_path;
  loop {
    let p_str = package_json_path.to_str().unwrap();
    let package_str_len = "/package.json".len();
    let p = p_str[0..=p_str.len() - package_str_len].to_string();
    let is_dir = if let Ok(stats) = std::fs::metadata(&p) {
      stats.is_dir()
    } else {
      false
    };
    if !is_dir {
      last_path = package_json_path;

      let prefix = if is_scoped {
        "../../../../node_modules/"
      } else {
        "../../../node_modules/"
      };
      package_json_url = package_json_url
        .join(&format!("{}{}/package.json", prefix, package_name))?;
      package_json_path = to_file_path(&package_json_url);
      if package_json_path.to_str().unwrap().len()
        == last_path.to_str().unwrap().len()
      {
        break;
      } else {
        continue;
      }
    }

    // Package match.
    let package_config =
      get_package_config(package_json_path.clone(), specifier, Some(base))?;
    if package_config.exports.is_some() {
      return package_exports_resolve(
        package_json_url,
        package_subpath,
        package_config,
        base,
        conditions,
      );
    }
    if package_subpath == "." {
      return legacy_main_resolve(&package_json_url, &package_config, base);
    }

    return package_json_url
      .join(&package_subpath)
      .map_err(AnyError::from);
  }

  Err(errors::err_module_not_found(
    &package_json_url
      .join(".")
      .unwrap()
      .to_file_path()
      .unwrap()
      .display()
      .to_string(),
    &to_file_path_string(base),
    "package",
  ))
}

fn parse_package_name(
  specifier: &str,
  base: &ModuleSpecifier,
) -> Result<(String, String, bool), AnyError> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..].find('/');
    } else {
      valid_package_name = false;
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
    return Err(errors::err_invalid_module_specifier(
      specifier,
      "is not a valid package name",
      Some(to_file_path_string(base)),
    ));
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Ok((package_name, package_subpath, is_scoped))
}

#[derive(Clone, Debug)]
pub struct PackageConfig {
  pub exists: bool,
  pub exports: Option<Value>,
  pub imports: Option<Map<String, Value>>,
  pub main: Option<String>,
  pub name: Option<String>,
  pub pjsonpath: PathBuf,
  pub typ: String,
}

pub fn check_if_should_use_esm_loader(
  main_module: &ModuleSpecifier,
) -> Result<bool, AnyError> {
  let s = main_module.as_str();
  if s.ends_with(".mjs") {
    return Ok(true);
  }
  if s.ends_with(".cjs") {
    return Ok(false);
  }
  let package_config = get_package_scope_config(main_module)?;
  Ok(package_config.typ == "module")
}

pub fn get_package_config(
  path: PathBuf,
  specifier: &str,
  maybe_base: Option<&ModuleSpecifier>,
) -> Result<PackageConfig, AnyError> {
  // TODO(bartlomieju):
  // if let Some(existing) = package_json_cache.get(path) {
  //   return Ok(existing.clone());
  // }

  let result = std::fs::read_to_string(&path);

  let source = result.unwrap_or_else(|_| "".to_string());
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

  let package_json: Value = serde_json::from_str(&source).map_err(|err| {
    let base_msg = maybe_base.map(|base| {
      format!("\"{}\" from {}", specifier, to_file_path(base).display())
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
    pjsonpath: path,
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
      to_file_path(&package_json_url),
      resolved.as_str(),
      None,
    )?;

    if package_config.exists {
      return Ok(package_config);
    }

    let last_package_json_url = package_json_url.clone();
    package_json_url = package_json_url.join("../package.json")?;

    // TODO(bartlomieju): I'm not sure this will work properly
    // Terminates at root where ../package.json equals ../../package.json
    // (can't just check "/package.json" for Windows support)
    if package_json_url.path() == last_package_json_url.path() {
      break;
    }
  }

  let package_json_path = to_file_path(&package_json_url);
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

fn file_exists(path_url: &ModuleSpecifier) -> bool {
  if let Ok(stats) = std::fs::metadata(to_file_path(path_url)) {
    stats.is_file()
  } else {
    false
  }
}

fn legacy_main_resolve(
  package_json_url: &ModuleSpecifier,
  package_config: &PackageConfig,
  _base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let mut guess;

  if let Some(main) = &package_config.main {
    guess = package_json_url.join(&format!("./{}", main))?;
    if file_exists(&guess) {
      return Ok(guess);
    }

    let mut found = false;
    for ext in [
      ".js",
      ".json",
      ".node",
      "/index.js",
      "/index.json",
      "/index.node",
    ] {
      guess = package_json_url.join(&format!("./{}{}", main, ext))?;
      if file_exists(&guess) {
        found = true;
        break;
      }
    }

    if found {
      // TODO(bartlomieju): emitLegacyIndexDeprecation()
      return Ok(guess);
    }
  }

  for p in ["./index.js", "./index.json", "./index.node"] {
    guess = package_json_url.join(p)?;
    if file_exists(&guess) {
      // TODO(bartlomieju): emitLegacyIndexDeprecation()
      return Ok(guess);
    }
  }

  Err(generic_error("not found"))
}
