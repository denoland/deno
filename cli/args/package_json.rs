// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_npm::registry::parse_dep_entry_name_and_raw_version;
use deno_npm::registry::PackageDepNpmSchemeValueParseError;
use deno_runtime::deno_node::PackageJson;
use deno_semver::npm::NpmPackageReq;
use deno_semver::npm::NpmVersionReqSpecifierParseError;
use deno_semver::VersionReq;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum PackageJsonDepValueParseError {
  #[error(transparent)]
  SchemeValue(#[from] PackageDepNpmSchemeValueParseError),
  #[error(transparent)]
  Specifier(#[from] NpmVersionReqSpecifierParseError),
  #[error("Not implemented scheme '{scheme}'")]
  Unsupported { scheme: String },
}

pub type PackageJsonDeps =
  BTreeMap<String, Result<NpmPackageReq, PackageJsonDepValueParseError>>;

/// Gets an application level package.json's npm package requirements.
///
/// Note that this function is not general purpose. It is specifically for
/// parsing the application level package.json that the user has control
/// over. This is a design limitation to allow mapping these dependency
/// entries to npm specifiers which can then be used in the resolver.
pub fn get_local_package_json_version_reqs(
  package_json: &PackageJson,
) -> PackageJsonDeps {
  fn parse_entry(
    key: &str,
    value: &str,
  ) -> Result<NpmPackageReq, PackageJsonDepValueParseError> {
    if value.starts_with("workspace:")
      || value.starts_with("file:")
      || value.starts_with("git:")
      || value.starts_with("http:")
      || value.starts_with("https:")
    {
      return Err(PackageJsonDepValueParseError::Unsupported {
        scheme: value.split(':').next().unwrap().to_string(),
      });
    }
    let (name, version_req) = parse_dep_entry_name_and_raw_version(key, value)
      .map_err(PackageJsonDepValueParseError::SchemeValue)?;

    let result = VersionReq::parse_from_specifier(version_req);
    match result {
      Ok(version_req) => Ok(NpmPackageReq {
        name: name.to_string(),
        version_req: Some(version_req),
      }),
      Err(err) => Err(PackageJsonDepValueParseError::Specifier(err)),
    }
  }

  fn insert_deps(
    deps: Option<&HashMap<String, String>>,
    result: &mut PackageJsonDeps,
  ) {
    if let Some(deps) = deps {
      for (key, value) in deps {
        result.insert(key.to_string(), parse_entry(key, value));
      }
    }
  }

  let deps = package_json.dependencies.as_ref();
  let dev_deps = package_json.dev_dependencies.as_ref();
  let mut result = BTreeMap::new();

  // insert the dev dependencies first so the dependencies will
  // take priority and overwrite any collisions
  insert_deps(dev_deps, &mut result);
  insert_deps(deps, &mut result);

  result
}

/// Attempts to discover the package.json file, maybe stopping when it
/// reaches the specified `maybe_stop_at` directory.
pub fn discover_from(
  start: &Path,
  maybe_stop_at: Option<PathBuf>,
) -> Result<Option<PackageJson>, AnyError> {
  const PACKAGE_JSON_NAME: &str = "package.json";

  // note: ancestors() includes the `start` path
  for ancestor in start.ancestors() {
    let path = ancestor.join(PACKAGE_JSON_NAME);

    let source = match std::fs::read_to_string(&path) {
      Ok(source) => source,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        if let Some(stop_at) = maybe_stop_at.as_ref() {
          if ancestor == stop_at {
            break;
          }
        }
        continue;
      }
      Err(err) => bail!(
        "Error loading package.json at {}. {:#}",
        path.display(),
        err
      ),
    };

    let package_json = PackageJson::load_from_string(path.clone(), source)?;
    log::debug!("package.json file found at '{}'", path.display());
    return Ok(Some(package_json));
  }

  log::debug!("No package.json file found");
  Ok(None)
}

#[cfg(test)]
mod test {
  use pretty_assertions::assert_eq;
  use std::path::PathBuf;

  use super::*;

  #[test]
  fn test_parse_dep_entry_name_and_raw_version() {
    let cases = [
      ("test", "^1.2", Ok(("test", "^1.2"))),
      ("test", "1.x - 2.6", Ok(("test", "1.x - 2.6"))),
      ("test", "npm:package@^1.2", Ok(("package", "^1.2"))),
      (
        "test",
        "npm:package",
        Err("Could not find @ symbol in npm url 'npm:package'"),
      ),
    ];
    for (key, value, expected_result) in cases {
      let result = parse_dep_entry_name_and_raw_version(key, value);
      match result {
        Ok(result) => assert_eq!(result, expected_result.unwrap()),
        Err(err) => assert_eq!(err.to_string(), expected_result.err().unwrap()),
      }
    }
  }

  fn get_local_package_json_version_reqs_for_tests(
    package_json: &PackageJson,
  ) -> BTreeMap<String, Result<NpmPackageReq, String>> {
    get_local_package_json_version_reqs(package_json)
      .into_iter()
      .map(|(k, v)| {
        (
          k,
          match v {
            Ok(v) => Ok(v),
            Err(err) => Err(err.to_string()),
          },
        )
      })
      .collect::<BTreeMap<_, _>>()
  }

  #[test]
  fn test_get_local_package_json_version_reqs() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(HashMap::from([
      ("test".to_string(), "^1.2".to_string()),
      ("other".to_string(), "npm:package@~1.3".to_string()),
    ]));
    package_json.dev_dependencies = Some(HashMap::from([
      ("package_b".to_string(), "~2.2".to_string()),
      // should be ignored
      ("other".to_string(), "^3.2".to_string()),
    ]));
    let deps = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      deps,
      BTreeMap::from([
        (
          "test".to_string(),
          Ok(NpmPackageReq::from_str("test@^1.2").unwrap())
        ),
        (
          "other".to_string(),
          Ok(NpmPackageReq::from_str("package@~1.3").unwrap())
        ),
        (
          "package_b".to_string(),
          Ok(NpmPackageReq::from_str("package_b@~2.2").unwrap())
        )
      ])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_errors_non_npm_specifier() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(HashMap::from([(
      "test".to_string(),
      "1.x - 1.3".to_string(),
    )]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map,
      BTreeMap::from([(
        "test".to_string(),
        Err(
          concat!(
            "Invalid npm specifier version requirement. Unexpected character.\n",
            "   - 1.3\n",
            "  ~"
          )
          .to_string()
        )
      )])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_skips_certain_specifiers() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(HashMap::from([
      ("test".to_string(), "1".to_string()),
      ("work-test".to_string(), "workspace:1.1.1".to_string()),
      ("file-test".to_string(), "file:something".to_string()),
      ("git-test".to_string(), "git:something".to_string()),
      ("http-test".to_string(), "http://something".to_string()),
      ("https-test".to_string(), "https://something".to_string()),
    ]));
    let result = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      result,
      BTreeMap::from([
        (
          "file-test".to_string(),
          Err("Not implemented scheme 'file'".to_string()),
        ),
        (
          "git-test".to_string(),
          Err("Not implemented scheme 'git'".to_string()),
        ),
        (
          "http-test".to_string(),
          Err("Not implemented scheme 'http'".to_string()),
        ),
        (
          "https-test".to_string(),
          Err("Not implemented scheme 'https'".to_string()),
        ),
        (
          "test".to_string(),
          Ok(NpmPackageReq::from_str("test@1").unwrap())
        ),
        (
          "work-test".to_string(),
          Err("Not implemented scheme 'workspace'".to_string()),
        )
      ])
    );
  }
}
