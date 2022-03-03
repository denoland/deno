// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::errors;
use crate::resolver::ImportMapResolver;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolveResponse;
use deno_graph::source::Resolver;
use std::path::PathBuf;

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

    if let Ok(url) = Url::parse(specifier) {
      if url.scheme() == "file" {
        return ResolveResponse::Esm(url);
      }
    }

    if referrer.scheme().starts_with("http") {
      let response =
        match deno_core::resolve_import(specifier, referrer.as_str()) {
          Ok(specifier) => ResolveResponse::Esm(specifier),
          Err(err) => ResolveResponse::Err(err.into()),
        };
      return response;
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

  let conditions = DEFAULT_CONDITIONS;
  // eprintln!("parent_url {} {}", specifier, parent_url.as_str());

  // let url = module_resolve(specifier, &parent_url, conditions)?;
  let result = node_resolver::resolve(
    specifier,
    &parent_url.to_file_path().unwrap(),
    conditions,
  );

  // eprintln!("{} {:?} {:?}", specifier, parent_url, result);
  let p = result?;

  let url = Url::from_file_path(p).unwrap();

  let resolve_response = if url.as_str().starts_with("http") {
    ResolveResponse::Esm(url)
  } else if url.as_str().ends_with(".js") {
    let package_config = get_package_scope_config(&url)?;
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

fn get_package_scope_config(
  resolved: &ModuleSpecifier,
) -> Result<node_resolver::PackageJson, AnyError> {
  let mut package_json_url = resolved.join("./package.json")?;

  loop {
    let package_json_path = package_json_url.path();

    if package_json_path.ends_with("node_modules/package.json") {
      break;
    }

    let result =
      node_resolver::PackageJson::load(to_file_path(&package_json_url));
    // TODO(bartlomieju): ignores all errors, instead of checking for "No such file or directory"
    if let Ok(package_config) = result {
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

  let package_config = node_resolver::PackageJson {
    // exists: false,
    typ: "none".to_string(),
    exports_map: None,
    imports: None,
    main: None,
    name: None,
    path: PathBuf::new(),
  };

  // TODO(bartlomieju):
  // package_json_cache.set(package_json_path, package_config.clone());

  Ok(package_config)
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
      Url::parse("https://deno.land/std@0.127.0/node/http.ts").unwrap();

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
