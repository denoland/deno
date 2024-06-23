// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_npm::registry::parse_dep_entry_name_and_raw_version;
use deno_runtime::deno_node::PackageJson;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageReq;
use deno_semver::VersionReq;
use indexmap::IndexMap;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum PackageJsonDepValueParseError {
  #[error(transparent)]
  VersionReq(#[from] NpmVersionReqParseError),
  #[error("Not implemented scheme '{scheme}'")]
  Unsupported { scheme: String },
}

pub type PackageJsonDeps =
  IndexMap<String, Result<PackageReq, PackageJsonDepValueParseError>>;

#[derive(Debug, Default)]
pub struct PackageJsonDepsProvider(Option<PackageJsonDeps>);

impl PackageJsonDepsProvider {
  pub fn new(deps: Option<PackageJsonDeps>) -> Self {
    Self(deps)
  }

  pub fn deps(&self) -> Option<&PackageJsonDeps> {
    self.0.as_ref()
  }

  pub fn reqs(&self) -> Option<Vec<&PackageReq>> {
    match &self.0 {
      Some(deps) => {
        let mut package_reqs = deps
          .values()
          .filter_map(|r| r.as_ref().ok())
          .collect::<Vec<_>>();
        package_reqs.sort(); // deterministic resolution
        Some(package_reqs)
      }
      None => None,
    }
  }
}

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
  ) -> Result<PackageReq, PackageJsonDepValueParseError> {
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
    let (name, version_req) = parse_dep_entry_name_and_raw_version(key, value);
    let result = VersionReq::parse_from_npm(version_req);
    match result {
      Ok(version_req) => Ok(PackageReq {
        name: name.to_string(),
        version_req,
      }),
      Err(err) => Err(PackageJsonDepValueParseError::VersionReq(err)),
    }
  }

  fn insert_deps(
    deps: Option<&IndexMap<String, String>>,
    result: &mut PackageJsonDeps,
  ) {
    if let Some(deps) = deps {
      for (key, value) in deps {
        result
          .entry(key.to_string())
          .or_insert_with(|| parse_entry(key, value));
      }
    }
  }

  let deps = package_json.dependencies.as_ref();
  let dev_deps = package_json.dev_dependencies.as_ref();
  let mut result = IndexMap::new();

  // favors the deps over dev_deps
  insert_deps(deps, &mut result);
  insert_deps(dev_deps, &mut result);

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

  fn get_local_package_json_version_reqs_for_tests(
    package_json: &PackageJson,
  ) -> IndexMap<String, Result<PackageReq, String>> {
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
      .collect::<IndexMap<_, _>>()
  }

  #[test]
  fn test_get_local_package_json_version_reqs() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(IndexMap::from([
      ("test".to_string(), "^1.2".to_string()),
      ("other".to_string(), "npm:package@~1.3".to_string()),
    ]));
    package_json.dev_dependencies = Some(IndexMap::from([
      ("package_b".to_string(), "~2.2".to_string()),
      // should be ignored
      ("other".to_string(), "^3.2".to_string()),
    ]));
    let deps = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      deps,
      IndexMap::from([
        (
          "test".to_string(),
          Ok(PackageReq::from_str("test@^1.2").unwrap())
        ),
        (
          "other".to_string(),
          Ok(PackageReq::from_str("package@~1.3").unwrap())
        ),
        (
          "package_b".to_string(),
          Ok(PackageReq::from_str("package_b@~2.2").unwrap())
        )
      ])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_errors_non_npm_specifier() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(IndexMap::from([(
      "test".to_string(),
      "%*(#$%()".to_string(),
    )]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map,
      IndexMap::from([(
        "test".to_string(),
        Err(
          concat!(
            "Invalid npm version requirement. Unexpected character.\n",
            "  %*(#$%()\n",
            "  ~"
          )
          .to_string()
        )
      )])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_range() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(IndexMap::from([(
      "test".to_string(),
      "1.x - 1.3".to_string(),
    )]));
    let map = get_local_package_json_version_reqs_for_tests(&package_json);
    assert_eq!(
      map,
      IndexMap::from([(
        "test".to_string(),
        Ok(PackageReq {
          name: "test".to_string(),
          version_req: VersionReq::parse_from_npm("1.x - 1.3").unwrap()
        })
      )])
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_skips_certain_specifiers() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(IndexMap::from([
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
      IndexMap::from([
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
          Ok(PackageReq::from_str("test@1").unwrap())
        ),
        (
          "work-test".to_string(),
          Err("Not implemented scheme 'workspace'".to_string()),
        )
      ])
    );
  }
}
