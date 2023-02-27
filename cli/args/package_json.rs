// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_graph::npm::NpmPackageReq;
use deno_graph::semver::VersionReq;
use deno_runtime::deno_node::PackageJson;

/// Gets the name and raw version constraint taking into account npm
/// package aliases.
pub fn parse_dep_entry_name_and_raw_version<'a>(
  key: &'a str,
  value: &'a str,
) -> Result<(&'a str, &'a str), AnyError> {
  if let Some(package_and_version) = value.strip_prefix("npm:") {
    if let Some((name, version)) = package_and_version.rsplit_once('@') {
      Ok((name, version))
    } else {
      bail!("could not find @ symbol in npm url '{}'", value);
    }
  } else {
    Ok((key, value))
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
) -> Result<BTreeMap<String, NpmPackageReq>, AnyError> {
  fn insert_deps(
    deps: Option<&HashMap<String, String>>,
    result: &mut BTreeMap<String, NpmPackageReq>,
  ) -> Result<(), AnyError> {
    if let Some(deps) = deps {
      for (key, value) in deps {
        if value.starts_with("workspace:")
          || value.starts_with("file:")
          || value.starts_with("git:")
          || value.starts_with("http:")
          || value.starts_with("https:")
        {
          // skip these specifiers for now
          continue;
        }
        let (name, version_req) =
          parse_dep_entry_name_and_raw_version(key, value)?;

        let version_req = {
          let result = VersionReq::parse_from_specifier(version_req);
          match result {
            Ok(version_req) => version_req,
            Err(e) => {
              let err = anyhow!("{:#}", e).context(concat!(
                "Parsing version constraints in the application-level ",
                "package.json is more strict at the moment"
              ));
              return Err(err);
            }
          }
        };
        result.insert(
          key.to_string(),
          NpmPackageReq {
            name: name.to_string(),
            version_req: Some(version_req),
          },
        );
      }
    }
    Ok(())
  }

  let deps = package_json.dependencies.as_ref();
  let dev_deps = package_json.dev_dependencies.as_ref();
  let mut result = BTreeMap::new();

  // insert the dev dependencies first so the dependencies will
  // take priority and overwrite any collisions
  insert_deps(dev_deps, &mut result)?;
  insert_deps(deps, &mut result)?;

  Ok(result)
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
        Err("could not find @ symbol in npm url 'npm:package'"),
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
    let result = get_local_package_json_version_reqs(&package_json).unwrap();
    assert_eq!(
      result,
      BTreeMap::from([
        (
          "test".to_string(),
          NpmPackageReq::from_str("test@^1.2").unwrap()
        ),
        (
          "other".to_string(),
          NpmPackageReq::from_str("package@~1.3").unwrap()
        ),
        (
          "package_b".to_string(),
          NpmPackageReq::from_str("package_b@~2.2").unwrap()
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
    let err = get_local_package_json_version_reqs(&package_json)
      .err()
      .unwrap();
    assert_eq!(
      format!("{err:#}"),
      concat!(
        "Parsing version constraints in the application-level ",
        "package.json is more strict at the moment: Invalid npm specifier ",
        "version requirement. Unexpected character.\n",
        "   - 1.3\n",
        "  ~"
      )
    );
  }

  #[test]
  fn test_get_local_package_json_version_reqs_skips_certain_specifiers() {
    let mut package_json = PackageJson::empty(PathBuf::from("/package.json"));
    package_json.dependencies = Some(HashMap::from([
      ("test".to_string(), "1".to_string()),
      ("work".to_string(), "workspace:1.1.1".to_string()),
      ("file".to_string(), "file:something".to_string()),
      ("git".to_string(), "git:something".to_string()),
      ("http".to_string(), "http://something".to_string()),
      ("https".to_string(), "https://something".to_string()),
    ]));
    let result = get_local_package_json_version_reqs(&package_json).unwrap();
    assert_eq!(
      result,
      BTreeMap::from([(
        "test".to_string(),
        NpmPackageReq::from_str("test@1").unwrap()
      )])
    );
  }
}
