// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use regex::Regex;

use crate::errors;
use crate::package_json::PackageJson;
use crate::path::PathClean;
use crate::NodePermissions;
use crate::RequireNpmResolver;

pub static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];
pub static REQUIRE_CONDITIONS: &[&str] = &["require", "node"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeModuleKind {
  Esm,
  Cjs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeResolutionMode {
  Execution,
  Types,
}

impl NodeResolutionMode {
  pub fn is_types(&self) -> bool {
    matches!(self, NodeResolutionMode::Types)
  }
}

/// Checks if the resolved file has a corresponding declaration file.
pub fn path_to_declaration_path(
  path: PathBuf,
  referrer_kind: NodeModuleKind,
) -> Option<PathBuf> {
  fn probe_extensions(
    path: &Path,
    referrer_kind: NodeModuleKind,
  ) -> Option<PathBuf> {
    let specific_dts_path = match referrer_kind {
      NodeModuleKind::Cjs => with_known_extension(path, "d.cts"),
      NodeModuleKind::Esm => with_known_extension(path, "d.mts"),
    };
    if specific_dts_path.exists() {
      return Some(specific_dts_path);
    }
    let dts_path = with_known_extension(path, "d.ts");
    if dts_path.exists() {
      Some(dts_path)
    } else {
      None
    }
  }

  let lowercase_path = path.to_string_lossy().to_lowercase();
  if lowercase_path.ends_with(".d.ts")
    || lowercase_path.ends_with(".d.cts")
    || lowercase_path.ends_with(".d.ts")
  {
    return Some(path);
  }
  if let Some(path) = probe_extensions(&path, referrer_kind) {
    return Some(path);
  }
  if path.is_dir() {
    if let Some(path) = probe_extensions(&path.join("index"), referrer_kind) {
      return Some(path);
    }
  }
  None
}

/// Alternate `PathBuf::with_extension` that will handle known extensions
/// more intelligently.
pub fn with_known_extension(path: &Path, ext: &str) -> PathBuf {
  const NON_DECL_EXTS: &[&str] = &["cjs", "js", "json", "jsx", "mjs", "tsx"];
  const DECL_EXTS: &[&str] = &["cts", "mts", "ts"];

  let file_name = match path.file_name() {
    Some(value) => value.to_string_lossy(),
    None => return path.to_path_buf(),
  };
  let lowercase_file_name = file_name.to_lowercase();
  let period_index = lowercase_file_name.rfind('.').and_then(|period_index| {
    let ext = &lowercase_file_name[period_index + 1..];
    if DECL_EXTS.contains(&ext) {
      if let Some(next_period_index) =
        lowercase_file_name[..period_index].rfind('.')
      {
        if &lowercase_file_name[next_period_index + 1..period_index] == "d" {
          Some(next_period_index)
        } else {
          Some(period_index)
        }
      } else {
        Some(period_index)
      }
    } else if NON_DECL_EXTS.contains(&ext) {
      Some(period_index)
    } else {
      None
    }
  });

  let file_name = match period_index {
    Some(period_index) => &file_name[..period_index],
    None => &file_name,
  };
  path.with_file_name(format!("{}.{}", file_name, ext))
}

fn to_specifier_display_string(url: &ModuleSpecifier) -> String {
  if let Ok(path) = url.to_file_path() {
    path.display().to_string()
  } else {
    url.to_string()
  }
}

fn throw_import_not_defined(
  specifier: &str,
  package_json_path: Option<&Path>,
  base: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_import_not_defined(
    specifier,
    package_json_path.map(|p| p.parent().unwrap().display().to_string()),
    &to_specifier_display_string(base),
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
  referrer_kind: NodeModuleKind,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<PathBuf, AnyError> {
  if name == "#" || name.starts_with("#/") || name.ends_with('/') {
    let reason = "is not a valid internal imports specifier name";
    return Err(errors::err_invalid_module_specifier(
      name,
      reason,
      Some(to_specifier_display_string(referrer)),
    ));
  }

  let package_config =
    get_package_scope_config(referrer, npm_resolver, permissions)?;
  let mut package_json_path = None;
  if package_config.exists {
    package_json_path = Some(package_config.path.clone());
    if let Some(imports) = &package_config.imports {
      if imports.contains_key(name) && !name.contains('*') {
        let maybe_resolved = resolve_package_target(
          package_json_path.as_ref().unwrap(),
          imports.get(name).unwrap().to_owned(),
          "".to_string(),
          name.to_string(),
          referrer,
          referrer_kind,
          false,
          true,
          conditions,
          mode,
          npm_resolver,
          permissions,
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
            referrer,
            referrer_kind,
            true,
            true,
            conditions,
            mode,
            npm_resolver,
            permissions,
          )?;
          if let Some(resolved) = maybe_resolved {
            return Ok(resolved);
          }
        }
      }
    }
  }

  Err(throw_import_not_defined(
    name,
    package_json_path.as_deref(),
    referrer,
  ))
}

fn throw_invalid_package_target(
  subpath: String,
  target: String,
  package_json_path: &Path,
  internal: bool,
  referrer: &ModuleSpecifier,
) -> AnyError {
  errors::err_invalid_package_target(
    package_json_path.parent().unwrap().display().to_string(),
    subpath,
    target,
    internal,
    Some(referrer.as_str().to_string()),
  )
}

fn throw_invalid_subpath(
  subpath: String,
  package_json_path: &Path,
  internal: bool,
  referrer: &ModuleSpecifier,
) -> AnyError {
  let ie = if internal { "imports" } else { "exports" };
  let reason = format!(
    "request is not a valid subpath for the \"{}\" resolution of {}",
    ie,
    package_json_path.display(),
  );
  errors::err_invalid_module_specifier(
    &subpath,
    &reason,
    Some(to_specifier_display_string(referrer)),
  )
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target_string(
  target: String,
  subpath: String,
  match_: String,
  package_json_path: &Path,
  referrer: &ModuleSpecifier,
  referrer_kind: NodeModuleKind,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<PathBuf, AnyError> {
  if !subpath.is_empty() && !pattern && !target.ends_with('/') {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      referrer,
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
        let package_json_url =
          ModuleSpecifier::from_file_path(package_json_path).unwrap();
        return match package_resolve(
          &export_target,
          &package_json_url,
          referrer_kind,
          conditions,
          mode,
          npm_resolver,
          permissions,
        ) {
          Ok(Some(path)) => Ok(path),
          Ok(None) => Err(generic_error("not found")),
          Err(err) => Err(err),
        };
      }
    }
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      referrer,
    ));
  }
  if invalid_segment_re.is_match(&target[2..]) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      referrer,
    ));
  }
  let package_path = package_json_path.parent().unwrap();
  let resolved_path = package_path.join(&target).clean();
  if !resolved_path.starts_with(package_path) {
    return Err(throw_invalid_package_target(
      match_,
      target,
      package_json_path,
      internal,
      referrer,
    ));
  }
  if subpath.is_empty() {
    return Ok(resolved_path);
  }
  if invalid_segment_re.is_match(&subpath) {
    let request = if pattern {
      match_.replace('*', &subpath)
    } else {
      format!("{}{}", match_, subpath)
    };
    return Err(throw_invalid_subpath(
      request,
      package_json_path,
      internal,
      referrer,
    ));
  }
  if pattern {
    let resolved_path_str = resolved_path.to_string_lossy();
    let replaced = pattern_re
      .replace(&resolved_path_str, |_caps: &regex::Captures| {
        subpath.clone()
      });
    return Ok(PathBuf::from(replaced.to_string()));
  }
  Ok(resolved_path.join(&subpath).clean())
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target(
  package_json_path: &Path,
  target: Value,
  subpath: String,
  package_subpath: String,
  referrer: &ModuleSpecifier,
  referrer_kind: NodeModuleKind,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<PathBuf>, AnyError> {
  if let Some(target) = target.as_str() {
    return resolve_package_target_string(
      target.to_string(),
      subpath,
      package_subpath,
      package_json_path,
      referrer,
      referrer_kind,
      pattern,
      internal,
      conditions,
      mode,
      npm_resolver,
      permissions,
    )
    .map(|path| {
      if mode.is_types() {
        path_to_declaration_path(path, referrer_kind)
      } else {
        Some(path)
      }
    });
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
        referrer,
        referrer_kind,
        pattern,
        internal,
        conditions,
        mode,
        npm_resolver,
        permissions,
      );

      match resolved_result {
        Ok(Some(resolved)) => return Ok(Some(resolved)),
        Ok(None) => {
          last_error = None;
          continue;
        }
        Err(e) => {
          let err_string = e.to_string();
          last_error = Some(e);
          if err_string.starts_with("[ERR_INVALID_PACKAGE_TARGET]") {
            continue;
          }
          return Err(last_error.unwrap());
        }
      }
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

      if key == "default"
        || conditions.contains(&key.as_str())
        || mode.is_types() && key.as_str() == "types"
      {
        let condition_target = target_obj.get(key).unwrap().to_owned();

        let resolved = resolve_package_target(
          package_json_path,
          condition_target,
          subpath.clone(),
          package_subpath.clone(),
          referrer,
          referrer_kind,
          pattern,
          internal,
          conditions,
          mode,
          npm_resolver,
          permissions,
        )?;
        match resolved {
          Some(resolved) => return Ok(Some(resolved)),
          None => {
            continue;
          }
        }
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
    referrer,
  ))
}

fn throw_exports_not_found(
  subpath: String,
  package_json_path: &Path,
  referrer: &ModuleSpecifier,
) -> AnyError {
  errors::err_package_path_not_exported(
    package_json_path.parent().unwrap().display().to_string(),
    subpath,
    Some(to_specifier_display_string(referrer)),
  )
}

#[allow(clippy::too_many_arguments)]
pub fn package_exports_resolve(
  package_json_path: &Path,
  package_subpath: String,
  package_exports: &Map<String, Value>,
  referrer: &ModuleSpecifier,
  referrer_kind: NodeModuleKind,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<PathBuf, AnyError> {
  if package_exports.contains_key(&package_subpath)
    && package_subpath.find('*').is_none()
    && !package_subpath.ends_with('/')
  {
    let target = package_exports.get(&package_subpath).unwrap().to_owned();
    let resolved = resolve_package_target(
      package_json_path,
      target,
      "".to_string(),
      package_subpath.to_string(),
      referrer,
      referrer_kind,
      false,
      false,
      conditions,
      mode,
      npm_resolver,
      permissions,
    )?;
    if resolved.is_none() {
      return Err(throw_exports_not_found(
        package_subpath,
        package_json_path,
        referrer,
      ));
    }
    return Ok(resolved.unwrap());
  }

  let mut best_match = "";
  let mut best_match_subpath = None;
  for key in package_exports.keys() {
    let pattern_index = key.find('*');
    if let Some(pattern_index) = pattern_index {
      let key_sub = &key[0..pattern_index];
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
              [pattern_index..(package_subpath.len() - pattern_trailer.len())]
              .to_string(),
          );
        }
      }
    }
  }

  if !best_match.is_empty() {
    let target = package_exports.get(best_match).unwrap().to_owned();
    let maybe_resolved = resolve_package_target(
      package_json_path,
      target,
      best_match_subpath.unwrap(),
      best_match.to_string(),
      referrer,
      referrer_kind,
      true,
      false,
      conditions,
      mode,
      npm_resolver,
      permissions,
    )?;
    if let Some(resolved) = maybe_resolved {
      return Ok(resolved);
    } else {
      return Err(throw_exports_not_found(
        package_subpath,
        package_json_path,
        referrer,
      ));
    }
  }

  Err(throw_exports_not_found(
    package_subpath,
    package_json_path,
    referrer,
  ))
}

fn parse_package_name(
  specifier: &str,
  referrer: &ModuleSpecifier,
) -> Result<(String, String, bool), AnyError> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..]
        .find('/')
        .map(|new_index| index + 1 + new_index);
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
      Some(to_specifier_display_string(referrer)),
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
  referrer_kind: NodeModuleKind,
  conditions: &[&str],
  mode: NodeResolutionMode,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<Option<PathBuf>, AnyError> {
  let (package_name, package_subpath, _is_scoped) =
    parse_package_name(specifier, referrer)?;

  // ResolveSelf
  let package_config =
    get_package_scope_config(referrer, npm_resolver, permissions)?;
  if package_config.exists
    && package_config.name.as_ref() == Some(&package_name)
  {
    if let Some(exports) = &package_config.exports {
      return package_exports_resolve(
        &package_config.path,
        package_subpath,
        exports,
        referrer,
        referrer_kind,
        conditions,
        mode,
        npm_resolver,
        permissions,
      )
      .map(Some);
    }
  }

  let package_dir_path = npm_resolver.resolve_package_folder_from_package(
    &package_name,
    &referrer.to_file_path().unwrap(),
    mode,
  )?;
  let package_json_path = package_dir_path.join("package.json");

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
  let package_json =
    PackageJson::load(npm_resolver, permissions, package_json_path)?;
  if let Some(exports) = &package_json.exports {
    return package_exports_resolve(
      &package_json.path,
      package_subpath,
      exports,
      referrer,
      referrer_kind,
      conditions,
      mode,
      npm_resolver,
      permissions,
    )
    .map(Some);
  }
  if package_subpath == "." {
    return legacy_main_resolve(&package_json, referrer_kind, mode);
  }

  let file_path = package_json.path.parent().unwrap().join(&package_subpath);

  if mode.is_types() {
    let maybe_declaration_path =
      path_to_declaration_path(file_path, referrer_kind);
    Ok(maybe_declaration_path)
  } else {
    Ok(Some(file_path))
  }
}

pub fn get_package_scope_config(
  referrer: &ModuleSpecifier,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<PackageJson, AnyError> {
  let root_folder = npm_resolver
    .resolve_package_folder_from_path(&referrer.to_file_path().unwrap())?;
  let package_json_path = root_folder.join("package.json");
  PackageJson::load(npm_resolver, permissions, package_json_path)
}

pub fn get_closest_package_json(
  url: &ModuleSpecifier,
  npm_resolver: &dyn RequireNpmResolver,
  permissions: &mut dyn NodePermissions,
) -> Result<PackageJson, AnyError> {
  let package_json_path = get_closest_package_json_path(url, npm_resolver)?;
  PackageJson::load(npm_resolver, permissions, package_json_path)
}

fn get_closest_package_json_path(
  url: &ModuleSpecifier,
  npm_resolver: &dyn RequireNpmResolver,
) -> Result<PathBuf, AnyError> {
  let file_path = url.to_file_path().unwrap();
  let mut current_dir = file_path.parent().unwrap();
  let package_json_path = current_dir.join("package.json");
  if package_json_path.exists() {
    return Ok(package_json_path);
  }
  let root_pkg_folder = npm_resolver
    .resolve_package_folder_from_path(&url.to_file_path().unwrap())?;
  while current_dir.starts_with(&root_pkg_folder) {
    current_dir = current_dir.parent().unwrap();
    let package_json_path = current_dir.join("package.json");
    if package_json_path.exists() {
      return Ok(package_json_path);
    }
  }

  bail!("did not find package.json in {}", root_pkg_folder.display())
}

fn file_exists(path: &Path) -> bool {
  if let Ok(stats) = std::fs::metadata(path) {
    stats.is_file()
  } else {
    false
  }
}

pub fn legacy_main_resolve(
  package_json: &PackageJson,
  referrer_kind: NodeModuleKind,
  mode: NodeResolutionMode,
) -> Result<Option<PathBuf>, AnyError> {
  let maybe_main = if mode.is_types() {
    match package_json.types.as_ref() {
      Some(types) => Some(types),
      None => {
        // fallback to checking the main entrypoint for
        // a corresponding declaration file
        if let Some(main) = package_json.main(referrer_kind) {
          let main = package_json.path.parent().unwrap().join(main).clean();
          if let Some(path) = path_to_declaration_path(main, referrer_kind) {
            return Ok(Some(path));
          }
        }
        None
      }
    }
  } else {
    package_json.main(referrer_kind)
  };

  if let Some(main) = maybe_main {
    let guess = package_json.path.parent().unwrap().join(main).clean();
    if file_exists(&guess) {
      return Ok(Some(guess));
    }

    // todo(dsherret): investigate exactly how node and typescript handles this
    let endings = if mode.is_types() {
      match referrer_kind {
        NodeModuleKind::Cjs => {
          vec![".d.ts", ".d.cts", "/index.d.ts", "/index.d.cts"]
        }
        NodeModuleKind::Esm => vec![
          ".d.ts",
          ".d.mts",
          "/index.d.ts",
          "/index.d.mts",
          ".d.cts",
          "/index.d.cts",
        ],
      }
    } else {
      vec![".js", "/index.js"]
    };
    for ending in endings {
      let guess = package_json
        .path
        .parent()
        .unwrap()
        .join(format!("{}{}", main, ending))
        .clean();
      if file_exists(&guess) {
        // TODO(bartlomieju): emitLegacyIndexDeprecation()
        return Ok(Some(guess));
      }
    }
  }

  let index_file_names = if mode.is_types() {
    // todo(dsherret): investigate exactly how typescript does this
    match referrer_kind {
      NodeModuleKind::Cjs => vec!["index.d.ts", "index.d.cts"],
      NodeModuleKind::Esm => vec!["index.d.ts", "index.d.mts", "index.d.cts"],
    }
  } else {
    vec!["index.js"]
  };
  for index_file_name in index_file_names {
    let guess = package_json
      .path
      .parent()
      .unwrap()
      .join(index_file_name)
      .clean();
    if file_exists(&guess) {
      // TODO(bartlomieju): emitLegacyIndexDeprecation()
      return Ok(Some(guess));
    }
  }

  Ok(None)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_package_name() {
    let dummy_referrer = Url::parse("http://example.com").unwrap();

    assert_eq!(
      parse_package_name("fetch-blob", &dummy_referrer).unwrap(),
      ("fetch-blob".to_string(), ".".to_string(), false)
    );
    assert_eq!(
      parse_package_name("@vue/plugin-vue", &dummy_referrer).unwrap(),
      ("@vue/plugin-vue".to_string(), ".".to_string(), true)
    );
    assert_eq!(
      parse_package_name("@astrojs/prism/dist/highlighter", &dummy_referrer)
        .unwrap(),
      (
        "@astrojs/prism".to_string(),
        "./dist/highlighter".to_string(),
        true
      )
    );
  }

  #[test]
  fn test_with_known_extension() {
    let cases = &[
      ("test", "d.ts", "test.d.ts"),
      ("test.d.ts", "ts", "test.ts"),
      ("test.worker", "d.ts", "test.worker.d.ts"),
      ("test.d.mts", "js", "test.js"),
    ];
    for (path, ext, expected) in cases {
      let actual = with_known_extension(&PathBuf::from(path), ext);
      assert_eq!(actual.to_string_lossy(), *expected);
    }
  }
}
