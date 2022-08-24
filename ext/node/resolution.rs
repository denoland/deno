// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use regex::Regex;

use crate::errors;
use crate::package_json::PackageJson;
use crate::DenoDirNpmResolver;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];
pub static REQUIRE_CONDITIONS: &[&str] = &["require", "node"];

fn to_file_path(url: &ModuleSpecifier) -> PathBuf {
  url
    .to_file_path()
    .unwrap_or_else(|_| panic!("Provided URL was not file:// URL: {}", url))
}

fn to_file_path_string(url: &ModuleSpecifier) -> String {
  to_file_path(url).display().to_string()
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

pub fn package_imports_resolve(
  name: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<ModuleSpecifier, AnyError> {
  if name == "#" || name.starts_with("#/") || name.ends_with('/') {
    let reason = "is not a valid internal imports specifier name";
    return Err(errors::err_invalid_module_specifier(
      name,
      reason,
      Some(to_file_path_string(referrer)),
    ));
  }

  let package_config = get_package_scope_config(referrer, npm_resolver)?;
  let mut package_json_url = None;
  if package_config.exists {
    package_json_url = Some(Url::from_file_path(package_config.path).unwrap());
    if let Some(imports) = &package_config.imports {
      if imports.contains_key(name) && !name.contains('*') {
        let maybe_resolved = resolve_package_target(
          package_json_url.clone().unwrap(),
          imports.get(name).unwrap().to_owned(),
          "".to_string(),
          name.to_string(),
          referrer,
          false,
          true,
          conditions,
          npm_resolver,
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
            referrer,
            true,
            true,
            conditions,
            npm_resolver,
          )?;
          if let Some(resolved) = maybe_resolved {
            return Ok(resolved);
          }
        }
      }
    }
  }

  Err(throw_import_not_defined(name, package_json_url, referrer))
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
  npm_resolver: &dyn DenoDirNpmResolver,
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
        return package_resolve(
          &export_target,
          &package_json_url,
          conditions,
          npm_resolver,
        );
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
  npm_resolver: &dyn DenoDirNpmResolver,
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
      npm_resolver,
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
        npm_resolver,
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
          npm_resolver,
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
  package_exports: &Map<String, Value>,
  base: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<ModuleSpecifier, AnyError> {
  if package_exports.contains_key(&package_subpath)
    && package_subpath.find('*').is_none()
    && !package_subpath.ends_with('/')
  {
    let target = package_exports.get(&package_subpath).unwrap().to_owned();
    let resolved = resolve_package_target(
      package_json_url.clone(),
      target,
      "".to_string(),
      package_subpath.to_string(),
      base,
      false,
      false,
      conditions,
      npm_resolver,
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
  for key in package_exports.keys() {
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
    let target = package_exports.get(best_match).unwrap().to_owned();
    let maybe_resolved = resolve_package_target(
      package_json_url.clone(),
      target,
      best_match_subpath.unwrap(),
      best_match.to_string(),
      base,
      true,
      false,
      conditions,
      npm_resolver,
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

pub fn package_resolve(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, _is_scoped) =
    parse_package_name(specifier, referrer)?;

  // ResolveSelf
  let package_config = get_package_scope_config(referrer, npm_resolver)?;
  if package_config.exists {
    let package_json_url = Url::from_file_path(&package_config.path).unwrap();
    if package_config.name.as_ref() == Some(&package_name) {
      if let Some(exports) = &package_config.exports {
        return package_exports_resolve(
          package_json_url,
          package_subpath,
          exports,
          referrer,
          conditions,
          npm_resolver,
        );
      }
    }
  }

  let package_dir_path = npm_resolver.resolve_package_folder_from_package(
    &package_name,
    &referrer.to_file_path().unwrap(),
  )?;
  let package_json_path = package_dir_path.join("package.json");
  let package_json_url =
    ModuleSpecifier::from_file_path(&package_json_path).unwrap();

  // todo: error with this instead when can't find package
  // Err(errors::err_module_not_found(
  //   &package_json_url
  //     .join(".")
  //     .unwrap()
  //     .to_file_path()
  //     .unwrap()
  //     .display()
  //     .to_string(),
  //   &to_file_path_string(referrer),
  //   "package",
  // ))

  // Package match.
  let package_json = PackageJson::load(npm_resolver, package_json_path)?;
  if let Some(exports) = &package_json.exports {
    return package_exports_resolve(
      package_json_url,
      package_subpath,
      exports,
      referrer,
      conditions,
      npm_resolver,
    );
  }
  if package_subpath == "." {
    return legacy_main_resolve(&package_json_url, &package_json, referrer);
  }

  package_json_url
    .join(&package_subpath)
    .map_err(AnyError::from)
}

pub fn get_package_scope_config(
  referrer: &ModuleSpecifier,
  npm_resolver: &dyn DenoDirNpmResolver,
) -> Result<PackageJson, AnyError> {
  let root_folder = npm_resolver
    .resolve_package_folder_from_path(&referrer.to_file_path().unwrap())?;
  let package_json_path = root_folder.join("./package.json");
  PackageJson::load(npm_resolver, package_json_path)
}

fn file_exists(path_url: &ModuleSpecifier) -> bool {
  if let Ok(stats) = std::fs::metadata(to_file_path(path_url)) {
    stats.is_file()
  } else {
    false
  }
}

pub fn legacy_main_resolve(
  package_json_url: &ModuleSpecifier,
  package_json: &PackageJson,
  _base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  let mut guess;

  if let Some(main) = &package_json.main {
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
