// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::errors;
use deno_core::error::AnyError;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use path_clean::PathClean;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

use super::package_json::PackageConfig;

fn throw_import_not_defined(
  specifier: &str,
  path: Option<PathBuf>,
  base: &Path,
) -> AnyError {
  errors::err_package_import_not_defined(
    specifier,
    path.map(|u| u.join(".").to_string_lossy().to_string()),
    &base.to_string_lossy().to_string(),
  )
}

fn is_conditional_exports_main_sugar(
  exports: &Value,
  package_json_path: &Path,
  base: &Path,
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
          &package_json_path.to_string_lossy().to_string(),
          Some(base.to_string_lossy().to_string()),
          Some("\"exports\" cannot contains some keys starting with \'.\' and some not.
          The exports object must either be an object of package subpath keys
          or an object of main entry condition name keys only.".to_string())
        ));
    }
  }

  Ok(is_conditional_sugar)
}

fn throw_exports_not_found(
  subpath: String,
  package_json_path: &Path,
  base: &Path,
) -> AnyError {
  errors::err_package_path_not_exported(
    package_json_path
      .parent()
      .unwrap()
      .to_string_lossy()
      .to_string(),
    subpath,
    Some(base.to_string_lossy().to_string()),
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

pub(crate) fn package_imports_resolve(
  name: &str,
  base: &Path,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  if name == "#" || name.starts_with("#/") || name.ends_with('/') {
    let reason = "is not a valid internal imports specifier name";
    return Err(errors::err_invalid_module_specifier(
      name,
      reason,
      Some(base.to_string_lossy().to_string()),
    ));
  }

  let mut package_json_path = None;

  let package_config = super::package_json::get_package_scope_config(base)?;
  if package_config.exists {
    package_json_path = Some(package_config.pjsonpath.clone());
    if let Some(imports) = &package_config.imports {
      if imports.contains_key(name) && !name.contains('*') {
        let maybe_resolved = resolve_package_target(
          package_json_path.as_ref().unwrap(),
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
            package_json_path.as_ref().unwrap(),
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

  Err(throw_import_not_defined(name, package_json_path, base))
}

pub(crate) fn package_exports_resolve(
  package_json_path: &Path,
  package_subpath: String,
  package_config: PackageConfig,
  base: &Path,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  let exports = &package_config.exports.unwrap();

  let exports_map =
    if is_conditional_exports_main_sugar(exports, package_json_path, base)? {
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
      package_json_path,
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
        package_json_path,
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
      package_json_path,
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
        package_json_path,
        base,
      ));
    }
  }
  Err(throw_exports_not_found(
    package_subpath,
    package_json_path,
    base,
  ))
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target(
  package_json_path: &Path,
  target: Value,
  subpath: String,
  package_subpath: String,
  base: &Path,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if let Some(target) = target.as_str() {
    return Ok(Some(resolve_package_target_string(
      target.to_string(),
      subpath,
      package_subpath,
      package_json_path,
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
        package_json_path,
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
      //   to_file_path_string(package_json_path),
      //   Some(base.as_str().to_string()),
      //   Some("\"exports\" cannot contain numeric property keys.".to_string()),
      // ));

      if key == "default" || conditions.contains(&key.as_str()) {
        let condition_target = target_obj.get(key).unwrap().to_owned();

        let resolved = resolve_package_target(
          package_json_path,
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
    package_json_path,
    internal,
    base,
  ))
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target_string(
  target: String,
  subpath: String,
  match_: String,
  package_json_path: &Path,
  base: &Path,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  if !subpath.is_empty() && !pattern && !target.ends_with('/') {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
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

        return super::esm_resolver::package_resolve(
          &export_target,
          package_json_path,
          conditions,
        );
      }
    }
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      base,
    ));
  }

  if invalid_segment_re.is_match(&target[2..]) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      base,
    ));
  }

  let mut resolved = package_json_path.to_path_buf();
  resolved.set_file_name(&target);
  let resolved = resolved.clean();
  let package_path = package_json_path.join("..").clean();

  if !resolved.starts_with(&package_path) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      base,
    ));
  }
  if subpath.is_empty() {
    return Ok(Url::from_file_path(resolved).unwrap());
  }

  if invalid_segment_re.is_match(&subpath) {
    let request = if pattern {
      match_.replace("*", &subpath)
    } else {
      format!("{}{}", match_, subpath)
    };
    return Err(throw_invalid_subpath(
      request,
      package_json_path,
      internal,
      base,
    ));
  }
  if pattern {
    let resolved_str = resolved.to_string_lossy().to_string();
    let replaced = pattern_re
      .replace(&resolved_str, |_caps: &regex::Captures| subpath.clone());
    let url = Url::parse(&replaced)?;
    return Ok(url);
  }

  Ok(Url::from_file_path(resolved.join(&subpath)).unwrap())
}

fn throw_invalid_package_target(
  subpath: String,
  target: String,
  package_json_path: &Path,
  internal: bool,
  base: &Path,
) -> AnyError {
  errors::err_invalid_package_target(
    package_json_path
      .parent()
      .unwrap()
      .to_string_lossy()
      .to_string(),
    subpath,
    target,
    internal,
    Some(base.to_string_lossy().to_string()),
  )
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_path: &Path,
  internal: bool,
  base: &Path,
) -> AnyError {
  let ie = if internal { "imports" } else { "exports" };
  let reason = format!(
    "request is not a valid subpath for the \"{}\" resolution of {}",
    ie,
    package_json_path.to_string_lossy()
  );
  errors::err_invalid_module_specifier(
    &subpath,
    &reason,
    Some(base.to_string_lossy().to_string()),
  )
}
