// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::errors;
use crate::resolver::ImportMapResolver;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolveResponse;
use deno_graph::source::Resolver;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

use super::package_json::get_package_config;
use super::package_json::get_package_scope_config;
use super::package_json::PackageConfig;
#[derive(Debug, Default)]
pub(crate) struct NodeEsmResolver {
  maybe_import_map_resolver: Option<ImportMapResolver>,
}

impl NodeEsmResolver {
  pub fn new(maybe_import_map_resolver: Option<ImportMapResolver>) -> Self {
    Self {
      maybe_import_map_resolver,
    }
  }
}

impl Resolver for NodeEsmResolver {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> ResolveResponse {
    // First try to resolve using import map, ignoring any errors
    if !specifier.starts_with("node:") {
      if let Some(import_map_resolver) = &self.maybe_import_map_resolver {
        let response = import_map_resolver.resolve(specifier, referrer);
        if !matches!(response, ResolveResponse::Err(_)) {
          return response;
        }
      }
    }

    // TODO(bartlomieju): this is ugly, but allows us to assert that all
    // referrers are file paths
    if specifier == super::GLOBAL_URL.as_str() {
      return ResolveResponse::Esm(super::GLOBAL_URL.clone());
    }

    if referrer.scheme().starts_with("http") {
      return match deno_core::resolve_import(specifier, referrer.as_str()) {
        Ok(specifier) => ResolveResponse::Esm(specifier),
        Err(err) => ResolveResponse::Err(err.into()),
      };
    } else if let Ok(url) = Url::parse(specifier) {
      if url.scheme().starts_with("http") {
        return ResolveResponse::Esm(url);
      }
    }

    let current_dir = match std::env::current_dir() {
      Ok(path) => path,
      Err(err) => return ResolveResponse::Err(err.into()),
    };
    let node_resolution =
      node_resolve(specifier, referrer.as_str(), &current_dir);

    match node_resolution {
      Ok(resolve_response) => {
        // If node resolution succeeded, return the specifier
        resolve_response
      }
      Err(err) => {
        // If node resolution failed, check if it's because of unsupported
        // URL scheme, and if so try to resolve using regular resolution algorithm
        if err
          .to_string()
          .starts_with("[ERR_UNSUPPORTED_ESM_URL_SCHEME]")
        {
          return match deno_core::resolve_import(specifier, referrer.as_str()) {
            Ok(specifier) => ResolveResponse::Esm(specifier),
            Err(err) => ResolveResponse::Err(err.into()),
          };
        }

        ResolveResponse::Err(err)
      }
    }
  }
}

static DEFAULT_CONDITIONS: &[&str] = &["deno", "node", "import"];

/// This function is an implementation of `defaultResolve` in
/// `lib/internal/modules/esm/resolve.js` from Node.
fn node_resolve(
  specifier: &str,
  referrer: &str,
  cwd: &std::path::Path,
) -> Result<ResolveResponse, AnyError> {
  // TODO(bartlomieju): skipped "policy" part as we don't plan to support it

  if let Some(resolved) = crate::compat::try_resolve_builtin_module(specifier) {
    return Ok(ResolveResponse::Esm(resolved));
  }

  if let Ok(url) = Url::parse(specifier) {
    if url.scheme() == "data" {
      return Ok(ResolveResponse::Specifier(url));
    }

    let protocol = url.scheme();

    if protocol == "node" {
      let split_specifier = url.as_str().split(':');
      let specifier = split_specifier.skip(1).collect::<Vec<_>>().join("");
      if let Some(resolved) =
        crate::compat::try_resolve_builtin_module(&specifier)
      {
        return Ok(ResolveResponse::Esm(resolved));
      } else {
        return Err(generic_error(format!("Unknown module {}", specifier)));
      }
    }

    if protocol != "file" && protocol != "data" {
      return Err(errors::err_unsupported_esm_url_scheme(&url));
    }

    if referrer.starts_with("data:") {
      let referrer_url = Url::parse(referrer)?;
      let url = referrer_url.join(specifier).map_err(AnyError::from)?;
      return Ok(ResolveResponse::Specifier(url));
    }
  }

  let is_main = referrer.is_empty();
  let parent_url = if is_main {
    Url::from_directory_path(cwd).unwrap()
  } else {
    Url::parse(referrer).expect("referrer was not proper url")
  };
  assert_eq!(parent_url.scheme(), "file");
  let parent_path = parent_url.to_file_path().unwrap();

  let conditions = DEFAULT_CONDITIONS;
  let url = module_resolve(specifier, &parent_path, conditions)?;
  let url_path = url.to_file_path().unwrap();

  let resolve_response = if url.as_str().starts_with("http") {
    ResolveResponse::Esm(url)
  } else if url.as_str().ends_with(".js") {
    let package_config = get_package_scope_config(&url_path)?;
    if package_config.typ == "module" {
      ResolveResponse::Esm(url)
    } else {
      ResolveResponse::CommonJs(url)
    }
  } else if url.as_str().ends_with(".cjs") {
    ResolveResponse::CommonJs(url)
  } else {
    ResolveResponse::Esm(url)
  };
  // TODO(bartlomieju): skipped checking errors for commonJS resolution and
  // "preserveSymlinksMain"/"preserveSymlinks" options.
  Ok(resolve_response)
}

fn to_file_path(url: &ModuleSpecifier) -> PathBuf {
  url
    .to_file_path()
    .unwrap_or_else(|_| panic!("Provided URL was not file:// URL: {}", url))
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

fn module_resolve(
  specifier: &str,
  base: &Path,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  let resolved = if should_be_treated_as_relative_or_absolute_path(specifier) {
    Url::from_file_path(base.join(specifier)).unwrap()
  } else if specifier.starts_with('#') {
    super::conditional_exports::package_imports_resolve(
      specifier, base, conditions,
    )?
  } else if let Ok(resolved) = Url::parse(specifier) {
    resolved
  } else {
    package_resolve(specifier, base, conditions)?
  };
  finalize_resolution(resolved, base)
}

fn finalize_resolution(
  resolved: ModuleSpecifier,
  base: &Path,
) -> Result<ModuleSpecifier, AnyError> {
  let encoded_sep_re = Regex::new(r"%2F|%2C").expect("bad regex");

  if encoded_sep_re.is_match(resolved.path()) {
    return Err(errors::err_invalid_module_specifier(
      resolved.path(),
      "must not include encoded \"/\" or \"\\\\\" characters",
      Some(base.to_string_lossy().to_string()),
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
    return Err(errors::err_unsupported_dir_import(resolved.as_str(), base));
  } else if !is_file {
    return Err(errors::err_module_not_found(
      resolved.as_str(),
      base,
      "module",
    ));
  }

  Ok(resolved)
}

pub(crate) fn package_resolve(
  specifier: &str,
  base: &Path,
  conditions: &[&str],
) -> Result<ModuleSpecifier, AnyError> {
  let (package_name, package_subpath, is_scoped) =
    parse_package_name(specifier, base)?;

  // ResolveSelf
  let package_config = get_package_scope_config(base)?;
  if package_config.exists {
    let package_json_path = package_config.pjsonpath.clone();
    if package_config.name.as_ref() == Some(&package_name) {
      if let Some(exports) = &package_config.exports {
        if !exports.is_null() {
          return super::conditional_exports::package_exports_resolve(
            &package_json_path,
            package_subpath,
            package_config,
            base,
            conditions,
          );
        }
      }
    }
  }

  let mut package_json_path = base.to_path_buf();
  package_json_path.pop();
  package_json_path.push("node_modules");
  package_json_path.push(package_name.clone());
  package_json_path.push("package.json");
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
      last_path = package_json_path.clone();

      if is_scoped {
        // "../../../../node_modules/"
        package_json_path.pop();
        package_json_path.pop();
        package_json_path.pop();
        package_json_path.pop();
      } else {
        // "../../../node_modules/"
        package_json_path.pop();
        package_json_path.pop();
        package_json_path.pop();
      };
      package_json_path.push("node_modules");
      package_json_path.push(package_name.clone());
      package_json_path.push("package_json");
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
      get_package_config(&package_json_path, specifier, Some(base))?;
    if package_config.exports.is_some() {
      return super::conditional_exports::package_exports_resolve(
        &package_json_path,
        package_subpath,
        package_config,
        base,
        conditions,
      );
    }
    if package_subpath == "." {
      let p = legacy_main_resolve(&package_json_path, &package_config, base)?;
      return Ok(Url::from_file_path(p).unwrap());
    }

    return Ok(
      Url::from_file_path(package_json_path.join(&package_subpath)).unwrap(),
    );
  }

  Err(errors::err_module_not_found(
    &package_json_path.join(".").to_string_lossy().to_string(),
    base,
    "package",
  ))
}

fn parse_package_name(
  specifier: &str,
  base: &Path,
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
      Some(base.to_string_lossy().to_string()),
    ));
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Ok((package_name, package_subpath, is_scoped))
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
  assert_eq!(main_module.scheme(), "file");
  let main_module = main_module.to_file_path().unwrap();
  let package_config = get_package_scope_config(&main_module)?;
  Ok(package_config.typ == "module")
}

fn file_exists(path_url: &Path) -> bool {
  if let Ok(stats) = std::fs::metadata(path_url) {
    stats.is_file()
  } else {
    false
  }
}

fn legacy_main_resolve(
  package_json_url: &Path,
  package_config: &PackageConfig,
  _base: &Path,
) -> Result<PathBuf, AnyError> {
  let mut guess;

  if let Some(main) = &package_config.main {
    guess = package_json_url.join(&format!("./{}", main));
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
      guess = package_json_url.join(&format!("./{}{}", main, ext));
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
    guess = package_json_url.join(p);
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
    assert!(matches!(actual, ResolveResponse::Esm(_)));
    assert_eq!(actual.to_result().unwrap(), expected);

    let actual = node_resolve(
      "data:application/javascript,console.log(\"Hello%20Deno\");",
      main.as_str(),
      &cwd,
    )
    .unwrap();
    let expected =
      Url::parse("data:application/javascript,console.log(\"Hello%20Deno\");")
        .unwrap();
    assert!(matches!(actual, ResolveResponse::Specifier(_)));
    assert_eq!(actual.to_result().unwrap(), expected);
  }

  #[test]
  fn deep() {
    let cwd = testdir("deep");
    let main = Url::from_file_path(cwd.join("a/b/c/d/main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let expected =
      Url::from_file_path(cwd.join("node_modules/foo/index.js")).unwrap();
    matches!(actual, ResolveResponse::Esm(_));
    assert_eq!(actual.to_result().unwrap(), expected);
  }

  #[test]
  fn package_subpath() {
    let cwd = testdir("subpath");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let expected =
      Url::from_file_path(cwd.join("node_modules/foo/index.js")).unwrap();
    matches!(actual, ResolveResponse::CommonJs(_));
    assert_eq!(actual.to_result().unwrap(), expected);
    let actual = node_resolve("foo/server.js", main.as_str(), &cwd).unwrap();
    let expected =
      Url::from_file_path(cwd.join("node_modules/foo/server.js")).unwrap();
    matches!(actual, ResolveResponse::CommonJs(_));
    assert_eq!(actual.to_result().unwrap(), expected);
  }

  #[test]
  fn basic_deps() {
    let cwd = testdir("basic_deps");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let actual = node_resolve("foo", main.as_str(), &cwd).unwrap();
    let foo_js =
      Url::from_file_path(cwd.join("node_modules/foo/foo.js")).unwrap();
    assert!(matches!(actual, ResolveResponse::Esm(_)));
    assert_eq!(actual.to_result().unwrap(), foo_js);

    let actual = node_resolve("bar", foo_js.as_str(), &cwd).unwrap();

    let bar_js =
      Url::from_file_path(cwd.join("node_modules/bar/bar.js")).unwrap();
    assert!(matches!(actual, ResolveResponse::Esm(_)));
    assert_eq!(actual.to_result().unwrap(), bar_js);
  }

  #[test]
  fn builtin_http() {
    let cwd = testdir("basic");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let expected =
      Url::parse("https://deno.land/std@0.126.0/node/http.ts").unwrap();

    let actual = node_resolve("http", main.as_str(), &cwd).unwrap();
    assert!(matches!(actual, ResolveResponse::Esm(_)));
    assert_eq!(actual.to_result().unwrap(), expected);

    let actual = node_resolve("node:http", main.as_str(), &cwd).unwrap();
    assert!(matches!(actual, ResolveResponse::Esm(_)));
    assert_eq!(actual.to_result().unwrap(), expected);
  }

  #[test]
  fn conditional_exports() {
    // check that `exports` mapping works correctly
    let cwd = testdir("conditions");
    let main = Url::from_file_path(cwd.join("main.js")).unwrap();
    let actual = node_resolve("imports_exports", main.as_str(), &cwd).unwrap();
    let expected = Url::from_file_path(
      cwd.join("node_modules/imports_exports/import_export.js"),
    )
    .unwrap();
    assert!(matches!(actual, ResolveResponse::CommonJs(_)));
    assert_eq!(actual.to_result().unwrap(), expected);

    // check that `imports` mapping works correctly
    let cwd = testdir("conditions/node_modules/imports_exports");
    let main = Url::from_file_path(cwd.join("import_export.js")).unwrap();
    let actual = node_resolve("#dep", main.as_str(), &cwd).unwrap();
    let expected = Url::from_file_path(cwd.join("import_polyfill.js")).unwrap();
    assert!(matches!(actual, ResolveResponse::CommonJs(_)));
    assert_eq!(actual.to_result().unwrap(), expected);
  }

  #[test]
  fn test_is_relative_specifier() {
    assert!(is_relative_specifier("./foo.js"));
    assert!(!is_relative_specifier("https://deno.land/std/node/http.ts"));
  }

  #[test]
  fn test_check_if_should_use_esm_loader() {
    let basic = testdir("basic");
    let main = Url::from_file_path(basic.join("main.js")).unwrap();
    assert!(check_if_should_use_esm_loader(&main).unwrap());

    let cjs = Url::from_file_path(basic.join("main.cjs")).unwrap();
    assert!(!check_if_should_use_esm_loader(&cjs).unwrap());

    let not_esm = testdir("not_esm");
    let main = Url::from_file_path(not_esm.join("main.js")).unwrap();
    assert!(!check_if_should_use_esm_loader(&main).unwrap());
  }
}
