use std::path::Path;
use std::path::PathBuf;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_graph::source::ResolveResponse;
use path_clean::PathClean;
use regex::Regex;

use crate::compat;
use crate::file_fetcher::FileFetcher;
use crate::npm::NpmPackageReference;
use crate::npm::NpmPackageReq;
use crate::npm::NpmPackageResolver;

use self::package_json::PackageJson;

mod package_json;

#[derive(Clone, PartialEq, Eq)]
pub enum ResolutionMode {
  Execution,
  Types,
}

static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
pub fn node_resolve_new(
  specifier: &str,
  referrer: &ModuleSpecifier,
  npm_resolver: &dyn NpmPackageResolver,
  mode: ResolutionMode,
) -> Result<Option<ResolveResponse>, AnyError> {
  // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

  if let Some(resolved) = crate::compat::try_resolve_builtin_module(specifier) {
    return Ok(Some(ResolveResponse::Esm(resolved)));
  }

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data" {
      return Ok(Some(ResolveResponse::Specifier(url)));
    }

    let protocol = url.scheme();

    if protocol == "node" {
      let split_specifier = url.as_str().split(':');
      let specifier = split_specifier.skip(1).collect::<String>();
      if let Some(resolved) = compat::try_resolve_builtin_module(&specifier) {
        return Ok(Some(ResolveResponse::Esm(resolved)));
      } else {
        return Err(generic_error(format!("Unknown module {}", specifier)));
      }
    }

    if protocol != "file" && protocol != "data" {
      return Err(compat::errors::err_unsupported_esm_url_scheme(&url));
    }

    // todo(THIS PR): I think this is handled upstream so can be removed?
    if referrer.scheme() == "data" {
      let url = referrer.join(specifier).map_err(AnyError::from)?;
      return Ok(Some(ResolveResponse::Specifier(url)));
    }
  }

  let conditions = DEFAULT_CONDITIONS;
  let url =
    module_resolve_new(specifier, referrer, conditions, npm_resolver, mode)?;
  let url = match url {
    Some(url) => url,
    None => return Ok(None),
  };

  let resolve_response = url_to_resolve_response_new(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

pub fn node_resolve_binary_export(
  pkg_req: &NpmPackageReq,
  bin_name: Option<&str>,
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ResolveResponse, AnyError> {
  let pkg = npm_resolver.resolve_package_from_deno_module(&pkg_req)?;
  let package_folder = pkg.folder_path;
  let package_json_path = package_folder.join("package.json");
  let package_json = PackageJson::load(package_json_path.clone())?;
  let bin = match &package_json.bin {
    Some(bin) => bin,
    None => bail!(
      "package {} did not have a 'bin' property in its package.json",
      pkg.id
    ),
  };
  let bin_name = bin_name.unwrap_or(&pkg_req.name);
  let bin_entry = match bin {
    Value::String(_) => {
      if bin_name != pkg_req.name {
        None
      } else {
        Some(bin)
      }
    }
    Value::Object(o) => o.get(bin_name),
    _ => bail!("package {} did not have a 'bin' property with a string or object value in its package.json", pkg.id),
  };
  let bin_entry = match bin_entry {
    Some(e) => e,
    None => bail!(
      "package {} did not have a 'bin' entry for {} in its package.json",
      pkg.id,
      bin_name,
    ),
  };
  let bin_entry = match bin_entry {
    Value::String(s) => s,
    _ => bail!("package {} had non-implemented non-string property 'bin' -> '{}' in its package.json", pkg.id, bin_name),
  };

  let url =
    ModuleSpecifier::from_file_path(package_folder.join(bin_entry)).unwrap();

  let resolve_response = url_to_resolve_response_new(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(resolve_response)
}

fn package_config_types_resolve(
  package_folder: &Path,
  path: Option<&str>,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if path.is_some() {
    todo!("npm paths are not currently implemented for type checking");
  }

  let package_json_path = package_folder.join("package.json");
  let package_json = PackageJson::load(package_json_path.clone())?;
  let types_entry = match package_json.types {
    Some(t) => t,
    // todo: handle typescript resolution when this isn't set
    None => return Ok(None),
  };
  let url =
    ModuleSpecifier::from_file_path(package_folder.join(&types_entry)).unwrap();
  Ok(Some(url))
}

pub fn node_resolve_npm_reference_new(
  reference: &NpmPackageReference,
  npm_resolver: &dyn NpmPackageResolver,
  mode: ResolutionMode,
) -> Result<Option<ResolveResponse>, AnyError> {
  let package_folder = npm_resolver
    .resolve_package_from_deno_module(&reference.req)?
    .folder_path;
  let maybe_url = match mode {
    ResolutionMode::Execution => package_config_resolve_new(
      reference.sub_path.as_deref().unwrap_or("."),
      &package_folder,
      npm_resolver,
    )
    .map(Some)
    .with_context(|| {
      format!("Error resolving package config for '{}'.", reference)
    })?,
    ResolutionMode::Types => package_config_types_resolve(
      &package_folder,
      reference.sub_path.as_deref(),
    )?,
  };
  let url = match maybe_url {
    Some(url) => url,
    None => return Ok(None),
  };

  let resolve_response = url_to_resolve_response_new(url, npm_resolver)?;
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(Some(resolve_response))
}

fn package_config_resolve_new(
  package_subpath: &str,
  package_dir: &PathBuf,
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ModuleSpecifier, AnyError> {
  let package_json_path = package_dir.join("package.json");
  // todo(dsherret): remove base from this code
  let base =
    ModuleSpecifier::from_directory_path(package_json_path.parent().unwrap())
      .unwrap();
  let package_config = PackageJson::load(package_json_path.clone())?;
  let package_json_url =
    ModuleSpecifier::from_file_path(&package_json_path).unwrap();
  if let Some(exports) = &package_config.exports {
    return package_exports_resolve_new(
      package_json_url,
      package_subpath.to_string(),
      exports,
      &base,
      DEFAULT_CONDITIONS,
      npm_resolver,
    );
  }
  if package_subpath == "." {
    return legacy_main_resolve(&package_json_url, &package_config, &base);
  }

  return package_json_url
    .join(&package_subpath)
    .map_err(AnyError::from);
}

fn url_to_resolve_response_new(
  url: ModuleSpecifier,
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ResolveResponse, AnyError> {
  Ok(if url.as_str().starts_with("http") {
    ResolveResponse::Esm(url)
  } else if url.as_str().ends_with(".js") {
    let package_config = get_package_scope_config_new(&url, npm_resolver)?;
    if package_config.typ == "module" {
      ResolveResponse::Esm(url)
    } else {
      ResolveResponse::CommonJs(url)
    }
  } else if url.as_str().ends_with(".cjs") {
    ResolveResponse::CommonJs(url)
  } else {
    ResolveResponse::Esm(url)
  })
}

const KNOWN_EXTENSIONS: [&str; 7] =
  ["js", "mjs", "cjs", "ts", "d.ts", "cts", "mts"];
const TYPES_EXTENSIONS: [&str; 2] = ["ts", "d.ts"];

fn types_extension_probe(mut p: PathBuf) -> Result<PathBuf, AnyError> {
  if p.exists() {
    Ok(p.clean())
  } else {
    if let Some(ext) = p.extension() {
      if !KNOWN_EXTENSIONS.contains(&ext.to_string_lossy().as_ref()) {
        // give the file a known extension to replace
        p.set_file_name(format!(
          "{}.js",
          p.file_name().unwrap().to_string_lossy()
        ));
      }
    }
    for ext in TYPES_EXTENSIONS {
      let p = p.with_extension(ext);
      if p.exists() {
        return Ok(p.clean());
      }
    }
    bail!("Did not find '{}'.", p.display())
  }
}

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

  // todo(dsherret): cache
  let encoded_sep_re = Regex::new(r"%2F|%2C").unwrap();

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(compat::errors::err_invalid_module_specifier(
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
    return Err(compat::errors::err_unsupported_dir_import(
      resolved.as_str(),
      base.as_str(),
    ));
  } else if !is_file {
    return Err(compat::errors::err_module_not_found(
      resolved.as_str(),
      base.as_str(),
      "module",
    ));
  }

  Ok(resolved)
}

fn module_resolve_new(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
  mode: ResolutionMode,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  let url = if should_be_treated_as_relative_or_absolute_path(specifier) {
    let resolved_specifier = referrer.join(specifier)?;
    match mode {
      ResolutionMode::Execution => Some(resolved_specifier),
      ResolutionMode::Types => {
        let path =
          types_extension_probe(resolved_specifier.to_file_path().unwrap())?;
        Some(ModuleSpecifier::from_file_path(path).unwrap())
      }
    }
  } else if specifier.starts_with('#') {
    Some(package_imports_resolve_new(
      specifier,
      referrer,
      conditions,
      npm_resolver,
    )?)
  } else if let Ok(resolved) = Url::parse(specifier) {
    Some(resolved)
  } else {
    match mode {
      ResolutionMode::Execution => Some(package_resolve_new(
        specifier,
        referrer,
        conditions,
        npm_resolver,
      )?),
      ResolutionMode::Types => {
        // todo(dsherret): handle path here
        let package_dir_path = npm_resolver
          .resolve_package_from_package(&specifier, &referrer)?
          .folder_path;
        package_config_types_resolve(&package_dir_path, None)?
      }
    }
  };
  Ok(match url {
    Some(url) => Some(finalize_resolution(url, referrer)?),
    None => None,
  })
}

fn throw_import_not_defined(
  specifier: &str,
  package_json_url: Option<ModuleSpecifier>,
  base: &ModuleSpecifier,
) -> AnyError {
  compat::errors::err_package_import_not_defined(
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

fn package_imports_resolve_new(
  name: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ModuleSpecifier, AnyError> {
  if name == "#" || name.starts_with("#/") || name.ends_with('/') {
    let reason = "is not a valid internal imports specifier name";
    return Err(compat::errors::err_invalid_module_specifier(
      name,
      reason,
      Some(to_file_path_string(referrer)),
    ));
  }

  let mut package_json_url = None;

  let package_config = get_package_scope_config_new(referrer, npm_resolver)?;
  package_json_url = Some(Url::from_file_path(package_config.path).unwrap());
  if let Some(imports) = &package_config.imports {
    if imports.contains_key(name) && !name.contains('*') {
      let maybe_resolved = resolve_package_target_new(
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
        let maybe_resolved = resolve_package_target_new(
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

  Err(throw_import_not_defined(name, package_json_url, referrer))
}

fn throw_invalid_package_target(
  subpath: String,
  target: String,
  package_json_url: &ModuleSpecifier,
  internal: bool,
  base: &ModuleSpecifier,
) -> AnyError {
  compat::errors::err_invalid_package_target(
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
  compat::errors::err_invalid_module_specifier(
    &subpath,
    &reason,
    Some(to_file_path_string(base)),
  )
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target_string_new(
  target: String,
  subpath: String,
  match_: String,
  package_json_url: ModuleSpecifier,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
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
        return package_resolve_new(
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
fn resolve_package_target_new(
  package_json_url: ModuleSpecifier,
  target: Value,
  subpath: String,
  package_subpath: String,
  base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<Option<ModuleSpecifier>, AnyError> {
  if let Some(target) = target.as_str() {
    return Ok(Some(resolve_package_target_string_new(
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
      let resolved_result = resolve_package_target_new(
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
        let resolved = resolve_package_target_new(
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
  compat::errors::err_package_path_not_exported(
    to_file_path_string(&package_json_url.join(".").unwrap()),
    subpath,
    Some(to_file_path_string(base)),
  )
}

fn package_exports_resolve_new(
  package_json_url: ModuleSpecifier,
  package_subpath: String,
  package_exports: &Map<String, Value>,
  base: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ModuleSpecifier, AnyError> {
  if package_exports.contains_key(&package_subpath)
    && package_subpath.find('*').is_none()
    && !package_subpath.ends_with('/')
  {
    let target = package_exports.get(&package_subpath).unwrap().to_owned();
    let resolved = resolve_package_target_new(
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
    let maybe_resolved = resolve_package_target_new(
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
    return Err(compat::errors::err_invalid_module_specifier(
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

fn package_resolve_new(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, _is_scoped) =
    parse_package_name(specifier, referrer)?;

  // ResolveSelf
  let package_config = get_package_scope_config_new(referrer, npm_resolver)?;
  let package_json_url = Url::from_file_path(&package_config.path).unwrap();
  if package_config.name.as_ref() == Some(&package_name) {
    if let Some(exports) = &package_config.exports {
      return package_exports_resolve_new(
        package_json_url,
        package_subpath,
        exports,
        referrer,
        conditions,
        npm_resolver,
      );
    }
  }

  let package_dir_path = npm_resolver
    .resolve_package_from_package(&package_name, &referrer)?
    .folder_path;
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
  let package_json = PackageJson::load(package_json_path.clone())?;
  if let Some(exports) = &package_json.exports {
    return package_exports_resolve_new(
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

  return package_json_url
    .join(&package_subpath)
    .map_err(AnyError::from);
}

fn get_package_scope_config_new(
  referrer: &ModuleSpecifier,
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<PackageJson, AnyError> {
  let root_folder = npm_resolver
    .resolve_package_from_specifier(&referrer)?
    .folder_path;
  let package_json_path = root_folder.join("./package.json");

  PackageJson::load(package_json_path.clone())
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

/// Translates given CJS module into ESM. This function will perform static
/// analysis on the file to find defined exports and reexports.
///
/// For all discovered reexports the analysis will be performed recursively.
///
/// If successful a source code for equivalent ES module is returned.
pub fn translate_cjs_to_esm_new(
  file_fetcher: &FileFetcher,
  specifier: &ModuleSpecifier,
  code: String,
  media_type: MediaType,
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<String, AnyError> {
  let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info: deno_ast::SourceTextInfo::new(code.into()),
    media_type,
    capture_tokens: true,
    scope_analysis: false,
    maybe_syntax: None,
  })?;
  let analysis = parsed_source.analyze_cjs();

  let mut source = vec![
    r#"const require = Deno[Deno.internal].require.Module.createRequire(import.meta.url);"#.to_string(),
  ];

  // if there are reexports, handle them first
  for (idx, reexport) in analysis.reexports.iter().enumerate() {
    // Firstly, resolve relate reexport specifier
    // todo(dsherret): call module_resolve_new instead?
    let resolved_reexport = resolve_new(
      reexport,
      &specifier,
      // FIXME(bartlomieju): check if these conditions are okay, probably
      // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
      &["deno", "require", "default"],
      npm_resolver,
    )?;
    let reexport_specifier =
      ModuleSpecifier::from_file_path(&resolved_reexport).unwrap();
    // Secondly, read the source code from disk
    let reexport_file = file_fetcher.get_source(&reexport_specifier).unwrap();
    // Now perform analysis again
    {
      let parsed_source = deno_ast::parse_script(deno_ast::ParseParams {
        specifier: reexport_specifier.to_string(),
        text_info: deno_ast::SourceTextInfo::new(reexport_file.source),
        media_type: reexport_file.media_type,
        capture_tokens: true,
        scope_analysis: false,
        maybe_syntax: None,
      })?;
      let analysis = parsed_source.analyze_cjs();

      source.push(format!(
        "const reexport{} = require(\"{}\");",
        idx, reexport
      ));

      for export in analysis.exports.iter().filter(|e| e.as_str() != "default")
      {
        // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
        // but it might not be necessary here since our analysis is more detailed?
        source.push(format!(
          "export const {} = reexport{}.{};",
          export, idx, export
        ));
      }
    }
  }

  source.push(format!(
    "const mod = require(\"{}\");",
    specifier
      .to_file_path()
      .unwrap()
      .to_str()
      .unwrap()
      .replace('\\', "\\\\")
      .replace('\'', "\\\'")
      .replace('\"', "\\\"")
  ));

  let mut had_default = false;
  for export in analysis.exports.iter() {
    if export.as_str() == "default" {
      // todo(dsherret): we should only do this if there was a `_esModule: true` instead
      source.push(format!("export default mod.{};", export,));
      had_default = true;
    } else {
      // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
      // but it might not be necessary here since our analysis is more detailed?
      source.push(format!("export const {0} = mod.{0};", export));
    }
  }

  if !had_default {
    source.push("export default mod;".to_string());
  }

  let translated_source = source.join("\n");
  Ok(translated_source)
}

fn resolve_new(
  specifier: &str,
  referrer: &ModuleSpecifier,
  conditions: &[&str],
  npm_resolver: &dyn NpmPackageResolver,
) -> Result<PathBuf, AnyError> {
  if specifier.starts_with('/') {
    todo!();
  }

  let referrer_path = referrer.to_file_path().unwrap();
  if specifier.starts_with("./") || specifier.starts_with("../") {
    if let Some(parent) = referrer_path.parent() {
      return file_extension_probe(parent.join(specifier), &referrer_path);
    } else {
      todo!();
    }
  }

  // We've got a bare specifier or maybe bare_specifier/blah.js"

  let (package_name, package_subpath) = parse_specifier(specifier).unwrap();

  // todo(dsherret): use not_found error on not found here
  let module_dir = npm_resolver
    .resolve_package_from_specifier(referrer)?
    .folder_path;

  let package_json_path = module_dir.join("package.json");
  if package_json_path.exists() {
    let package_json = PackageJson::load(package_json_path)?;

    if let Some(map) = package_json.exports {
      if let Some((key, subpath)) = exports_resolve(&map, &package_subpath) {
        let value = map.get(&key).unwrap();
        let s = conditions_resolve(value, conditions);

        let t = resolve_package_target_string(&s, subpath);
        return Ok(module_dir.join(t).clean());
      } else {
        todo!()
      }
    }

    // old school
    if package_subpath != "." {
      let d = module_dir.join(package_subpath);
      if let Ok(m) = d.metadata() {
        if m.is_dir() {
          return Ok(d.join("index.js").clean());
        }
      }
      return file_extension_probe(d, &referrer_path);
    } else if let Some(main) = package_json.main {
      return Ok(module_dir.join(main).clean());
    } else {
      return Ok(module_dir.join("index.js").clean());
    }
  }

  Err(not_found(specifier, &referrer_path))
}

fn resolve_package_target_string(
  target: &str,
  subpath: Option<String>,
) -> String {
  if let Some(subpath) = subpath {
    target.replace('*', &subpath)
  } else {
    target.to_string()
  }
}

fn conditions_resolve(value: &Value, conditions: &[&str]) -> String {
  match value {
    Value::String(s) => s.to_string(),
    Value::Object(map) => {
      for condition in conditions {
        if let Some(x) = map.get(&condition.to_string()) {
          if let Value::String(s) = x {
            return s.to_string();
          } else {
            todo!()
          }
        }
      }
      todo!()
    }
    _ => todo!(),
  }
}

fn parse_specifier(specifier: &str) -> Option<(String, String)> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  // let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    // is_scoped = true;
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
    return None;
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Some((package_name, package_subpath))
}

fn exports_resolve(
  map: &Map<String, Value>,
  subpath: &str,
) -> Option<(String, Option<String>)> {
  if map.contains_key(subpath) {
    return Some((subpath.to_string(), None));
  }

  // best match
  let mut best_match = None;
  for key in map.keys() {
    if let Some(pattern_index) = key.find('*') {
      let key_sub = &key[0..pattern_index];
      if subpath.starts_with(key_sub) {
        if subpath.ends_with('/') {
          todo!()
        }
        let pattern_trailer = &key[pattern_index + 1..];

        if subpath.len() > key.len()
          && subpath.ends_with(pattern_trailer)
          // && pattern_key_compare(best_match, key) == 1
          && key.rfind('*') == Some(pattern_index)
        {
          let rest = subpath
            [pattern_index..(subpath.len() - pattern_trailer.len())]
            .to_string();
          best_match = Some((key, rest));
        }
      }
    }
  }

  if let Some((key, subpath_)) = best_match {
    return Some((key.to_string(), Some(subpath_)));
  }

  None
}

fn file_extension_probe(
  mut p: PathBuf,
  referrer: &Path,
) -> Result<PathBuf, AnyError> {
  if p.exists() {
    Ok(p.clean())
  } else {
    p.set_extension("js");
    if p.exists() {
      Ok(p)
    } else {
      Err(not_found(&p.clean().to_string_lossy(), referrer))
    }
  }
}

fn not_found(path: &str, referrer: &Path) -> AnyError {
  let msg = format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find module \"{}\" imported from \"{}\"",
    path,
    referrer.to_string_lossy()
  );
  std::io::Error::new(std::io::ErrorKind::NotFound, msg).into()
}
