// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::errors;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Map;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::Resolver;
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct NodeEsmResolver;

impl NodeEsmResolver {
  pub fn as_resolver(&self) -> &dyn Resolver {
    self
  }
}

impl Resolver for NodeEsmResolver {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    // TODO(bartlomieju): this is hacky, remove
    // needed to add it here because `deno_std/node` has
    // triple-slash references and they should still resolve
    // the regular way (I think)
    if referrer.as_str().starts_with("https://deno.land/std") {
      return referrer.join(specifier).map_err(AnyError::from);
    }
    node_resolve(specifier, referrer.as_str(), &std::env::current_dir()?)
  }
}

static DEFAULT_CONDITIONS: &[&str] = &["node", "import"];

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
fn node_resolve(
  specifier: &str,
  referrer: &str,
  cwd: &std::path::Path,
) -> Result<ModuleSpecifier, AnyError> {
  // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

  if let Some(resolved) = crate::compat::try_resolve_builtin_module(specifier) {
    return Ok(resolved);
  }

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data:" {
      return Ok(url);
    }

    let protocol = url.scheme();

    if protocol == "node" {
      let mut split_specifier = url.as_str().split(':');
      split_specifier.next();
      let specifier = split_specifier.collect::<Vec<_>>().join("");
      if let Some(resolved) =
        crate::compat::try_resolve_builtin_module(&specifier)
      {
        return Ok(resolved);
      } else {
        return Err(generic_error(format!("Unknown module {}", specifier)));
      }
    }

    if protocol != "file" && protocol != "data" {
      return Err(errors::err_unsupported_esm_url_scheme(&url));
    }

    if referrer.starts_with("data:") {
      let referrer_url = Url::parse(referrer)?;
      return referrer_url.join(specifier).map_err(AnyError::from);
    }
  }

  let is_main = referrer.is_empty();
  let parent_url = if is_main {
    Url::from_directory_path(cwd).unwrap()
  } else {
    Url::parse(referrer).expect("referrer was not proper url")
  };

  let conditions = DEFAULT_CONDITIONS;
  let url = module_resolve(specifier, &parent_url, conditions)?;

  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(url)
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

fn module_resolve(
  specifier: &str,
  base: &ModuleSpecifier,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  let resolved = if should_be_treated_as_relative_or_absolute_path(specifier) {
    base.join(specifier)?
  } else if specifier.starts_with('#') {
    package_imports_resolve(specifier, base)?
  } else if let Ok(resolved) = Url::parse(specifier) {
    resolved
  } else {
    package_resolve(specifier, base, conditions)?
  };
  finalize_resolution(resolved, base)
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &ModuleSpecifier,
) -> Result<ModuleSpecifier, AnyError> {
  // TODO(bartlomieju): this is not part of Node resolution
  // (as it doesn't support http/https);
  // but I had to short circuit for remote modules to avoid errors
  if resolved.scheme().starts_with("http") {
    return Ok(resolved);
  }

  let encoded_sep_re = Regex::new(r"%2F|%2C").expect("bad regex");

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(errors::err_invalid_module_specifier(
      resolved.path(),
      "must not include encoded \"/\" or \"\\\\\" characters",
      Some(base.to_file_path().unwrap().display().to_string()),
    ));
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

  let (is_dir, is_file) = if let Ok(stats) = std::fs::metadata(&p) {
    (stats.is_dir(), stats.is_file())
  } else {
    (false, false)
  };
  if is_dir {
    return Err(errors::err_unsupported_dir_import(
      &path.display().to_string(),
      &base.to_file_path().unwrap().display().to_string(),
    ));
  } else if !is_file {
    return Err(errors::err_module_not_found(
      &path.display().to_string(),
      &base.to_file_path().unwrap().display().to_string(),
      "module",
    ));
  }

  Ok(resolved)
}

fn package_imports_resolve(
  _specifier: &str,
  _base: &ModuleSpecifier,
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
    let cur_is_conditional_sugar = key.is_empty() || !key.starts_with('.');
    if i == 0 {
      is_conditional_sugar = cur_is_conditional_sugar;
      i += 1;
    } else if is_conditional_sugar != cur_is_conditional_sugar {
      return Err(errors::err_invalid_package_config(
        &package_json_url.to_file_path().unwrap().display().to_string(),
        Some(base.as_str().to_string()),
        Some("\"exports\" cannot contains some keys starting with \'.\' and some not.
        The exports object must either be an object of package subpath keys
        or an object of main entry condition name keys only.".to_string())
      ));
    }
  }

  Ok(is_conditional_sugar)
}

#[allow(clippy::too_many_arguments)]
fn resolve_package_target_string(
  target: String,
  subpath: String,
  _match_: String,
  package_json_url: Url,
  _base: &ModuleSpecifier,
  pattern: bool,
  internal: bool,
  _conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  if !subpath.is_empty() && !pattern && !target.ends_with('/') {
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
  let pattern_re = Regex::new(r"\*").expect("bad regex");

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

  if subpath.is_empty() {
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

#[allow(clippy::too_many_arguments)]
fn resolve_package_target(
  package_json_url: Url,
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
      todo!()
    }

    todo!()
  } else if let Some(target_obj) = target.as_object() {
    for key in target_obj.keys() {
      // TODO(bartlomieju): verify that keys are not numeric
      // return Err(errors::err_invalid_package_config(
      //   package_json_url.to_file_path().unwrap().display().unwrap(),
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
    todo!()
  }

  todo!()
}

fn package_exports_resolve(
  package_json_url: Url,
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
      package_json_url,
      target,
      "".to_string(),
      package_subpath,
      base,
      false,
      false,
      conditions,
    )?;
    // TODO(bartlomieju): return error here
    if resolved.is_none() {
      todo!()
    }
    return Ok(resolved.unwrap());
  }

  todo!()
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
  let mut package_json_path = package_json_url.to_file_path().unwrap();
  let mut last_path;
  loop {
    let p_str = package_json_path.to_str().unwrap();
    let p = p_str[0..=p_str.len() - 13].to_string();
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
      package_json_path = package_json_url.to_file_path().unwrap();
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
    &base.to_file_path().unwrap().display().to_string(),
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
    specifier[0..=index].to_string()
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
      Some(base.to_file_path().unwrap().display().to_string()),
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
      format!(
        "\"{}\" from {}",
        specifier,
        base.to_file_path().unwrap().display()
      )
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

  // TODO(bartlomieju): refactor
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
      package_json_url.to_file_path().unwrap(),
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

fn file_exists(path_url: &Url) -> bool {
  if let Ok(stats) = std::fs::metadata(path_url.to_file_path().unwrap()) {
    stats.is_file()
  } else {
    false
  }
}

fn legacy_main_resolve(
  package_json_url: &Url,
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

#[cfg(test)]
mod tests {
  use super::*;

  fn testdir(name: &str) -> PathBuf {
    let c = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    c.join("compat/testdata/").join(name)
  }

  #[test]
  fn basic() {
    let cwd = testdir("basic");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let expected =
      Url::from_file_path(cwd.join("node_modules/foo/index.js")).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn deep() {
    let cwd = testdir("deep");
    let main = Url::from_file_path(cwd.join("a/b/c/d/main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let expected =
      Url::from_file_path(cwd.join("node_modules/foo/index.js")).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn basic_deps() {
    let cwd = testdir("basic_deps");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let foo_js =
      Url::from_file_path(cwd.join("node_modules/foo/foo.js")).unwrap();
    assert_eq!(actual, foo_js);

    let actual = node_resolve("bar", foo_js.as_str(), &cwd).unwrap();

    let bar_js =
      Url::from_file_path(cwd.join("node_modules/bar/bar.js")).unwrap();
    assert_eq!(actual, bar_js);
  }

  #[test]
  fn builtin_http() {
    let cwd = testdir("basic");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let expected =
      Url::parse("https://deno.land/std@0.111.0/node/http.ts").unwrap();

    let actual = node_resolve("http", main.as_str(), &cwd).unwrap();
    println!("actual {}", actual);
    assert_eq!(actual, expected);

    let actual = node_resolve("node:http", main.as_str(), &cwd).unwrap();
    println!("actual {}", actual);
    assert_eq!(actual, expected);
  }
}
