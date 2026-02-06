// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;

use deno_semver::Version;
use deno_semver::package::PackageNv;
use serde_json::Value;
use thiserror::Error;

pub type JsonMap = serde_json::Map<String, Value>;

pub fn transform1_to_2(json: JsonMap) -> JsonMap {
  let mut new_map = JsonMap::new();
  new_map.insert("version".to_string(), "2".into());
  new_map.insert("remote".to_string(), json.into());
  new_map
}

pub fn transform2_to_3(mut json: JsonMap) -> JsonMap {
  json.insert("version".into(), "3".into());
  if let Some(Value::Object(mut npm_obj)) = json.remove("npm") {
    let mut new_obj = JsonMap::new();
    if let Some(packages) = npm_obj.remove("packages") {
      new_obj.insert("npm".into(), packages);
    }
    if let Some(Value::Object(specifiers)) = npm_obj.remove("specifiers") {
      let mut new_specifiers = JsonMap::new();
      for (key, value) in specifiers {
        if let Value::String(value) = value {
          new_specifiers
            .insert(format!("npm:{}", key), format!("npm:{}", value).into());
        }
      }
      if !new_specifiers.is_empty() {
        new_obj.insert("specifiers".into(), new_specifiers.into());
      }
    }
    json.insert("packages".into(), new_obj.into());
  }

  json
}

#[derive(Debug, Error)]
pub enum TransformError {
  #[error("Failed extracting npm name and version from dep '{id}'.")]
  FailedExtractingV3NpmDepNv { id: String },
  #[error("Failed getting npm package info: {source}")]
  FailedGettingNpmPackageInfo {
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
  },
}

// note: although these functions are found elsewhere in this repo,
// it is purposefully duplicated here to ensure it never changes
// for these transforms
fn extract_nv_from_id(value: &str) -> Option<(&str, &str)> {
  if value.is_empty() {
    return None;
  }
  let at_index = value[1..].find('@')? + 1;
  let name = &value[..at_index];
  let version = &value[at_index + 1..];
  Some((name, version))
}

fn split_pkg_req(value: &str) -> Option<(&str, Option<&str>)> {
  if value.len() < 5 {
    return None;
  }
  // 5 is length of `jsr:@`/`npm:@`
  let Some(at_index) = value[5..].find('@').map(|i| i + 5) else {
    // no version requirement
    // ex. `npm:jsonc-parser` or `jsr:@pkg/scope`
    return Some((value, None));
  };
  let name = &value[..at_index];
  let version = &value[at_index + 1..];
  Some((name, Some(version)))
}
pub fn transform3_to_4(mut json: JsonMap) -> Result<JsonMap, TransformError> {
  json.insert("version".into(), "4".into());
  if let Some(Value::Object(mut packages)) = json.remove("packages") {
    if let Some((npm_key, Value::Object(mut npm))) =
      packages.remove_entry("npm")
    {
      let mut pkg_had_multiple_versions: HashMap<String, bool> =
        HashMap::with_capacity(npm.len());
      for id in npm.keys() {
        let Some((name, _)) = extract_nv_from_id(id) else {
          continue; // corrupt
        };
        pkg_had_multiple_versions
          .entry(name.to_string())
          .and_modify(|v| *v = true)
          .or_default();
      }
      for value in npm.values_mut() {
        let Value::Object(value) = value else {
          continue;
        };
        let Some(Value::Object(deps)) = value.remove("dependencies") else {
          continue;
        };
        let mut new_deps = Vec::with_capacity(deps.len());
        for (key, id) in deps {
          let Value::String(id) = id else {
            continue;
          };
          let Some((name, version)) = extract_nv_from_id(&id) else {
            // corrupt
            return Err(TransformError::FailedExtractingV3NpmDepNv {
              id: id.to_string(),
            });
          };
          if name == key {
            let has_single_version = pkg_had_multiple_versions
              .get(name)
              .map(|had_multiple| !had_multiple)
              .unwrap_or(false);
            if has_single_version {
              new_deps.push(Value::String(name.to_string()));
            } else {
              new_deps.push(Value::String(format!("{}@{}", name, version)));
            }
          } else {
            new_deps
              .push(Value::String(format!("{}@npm:{}@{}", key, name, version)));
          }
        }
        value.insert("dependencies".into(), new_deps.into());
      }
      json.insert(npm_key, npm.into());
    }

    if let Some((jsr_key, Value::Object(mut jsr))) =
      packages.remove_entry("jsr")
    {
      let mut pkg_had_multiple_specifiers: HashMap<&str, bool> = HashMap::new();
      if let Some(Value::Object(specifiers)) = packages.get("specifiers") {
        pkg_had_multiple_specifiers.reserve(specifiers.len());
        for req in specifiers.keys() {
          let Some((name, _)) = split_pkg_req(req) else {
            continue; // corrupt
          };
          pkg_had_multiple_specifiers
            .entry(name)
            .and_modify(|v| *v = true)
            .or_default();
        }
      }
      for pkg in jsr.values_mut() {
        let Some(Value::Array(deps)) = pkg.get_mut("dependencies") else {
          continue;
        };
        for dep in deps.iter_mut() {
          let Value::String(dep) = dep else {
            continue;
          };
          let Some((name, _)) = split_pkg_req(dep) else {
            continue;
          };
          if let Some(false) = pkg_had_multiple_specifiers.get(name) {
            *dep = name.to_string();
          }
        }
      }
      json.insert(jsr_key, jsr.into());
    }

    if let Some(Value::Object(specifiers)) = packages.get_mut("specifiers") {
      for value in specifiers.values_mut() {
        let Value::String(value) = value else {
          continue;
        };
        let Some((_, Some(id_stripped))) = split_pkg_req(value) else {
          continue;
        };
        *value = id_stripped.to_string();
      }
    }

    // flatten packages into root
    for (key, value) in packages {
      json.insert(key, value);
    }
  }

  Ok(json)
}

#[derive(Debug, Error)]
#[error(
  "Expected a different number of results from npm package info provider."
)]
pub struct MissingNpmPackageInfo;

#[derive(Debug, PartialEq, Eq)]
struct IdParts {
  key: String,
  package_name: String,
  version: String,
}
fn split_id(
  id: &str,
  version_by_dep_name: &HashMap<String, String>,
) -> Option<IdParts> {
  let (left, right) = match extract_nv_from_id(id) {
    Some((name, version)) => (name, version),
    None => match version_by_dep_name.get(id) {
      Some(version) => (id, &**version),
      None => {
        return None;
      }
    },
  };
  let (key, package_name, version) = match right.strip_prefix("npm:") {
    Some(right) => {
      // ex. key@npm:package-a@version
      match extract_nv_from_id(right) {
        Some((package_name, version)) => (left, package_name, version),
        None => {
          return None;
        }
      }
    }
    None => (left, left, right),
  };
  let version = version.split_once('_').map(|(v, _)| v).unwrap_or(version);
  Some(IdParts {
    key: key.into(),
    package_name: package_name.into(),
    version: version.into(),
  })
}

pub async fn transform4_to_5(
  mut json: JsonMap,
  info_provider: &dyn NpmPackageInfoProvider,
) -> Result<JsonMap, TransformError> {
  json.insert("version".into(), "5".into());

  if let Some(Value::Object(mut npm)) = json.remove("npm") {
    let mut npm_packages = Vec::new();
    let mut keys = Vec::new();
    let mut has_multiple_versions = HashMap::new();
    let mut version_by_dep_name = HashMap::new();
    for (key, _) in &npm {
      let Some(id_parts) = split_id(key, &version_by_dep_name) else {
        continue;
      };
      let Ok(version) = Version::parse_standard(&id_parts.version) else {
        continue;
      };
      has_multiple_versions
        .entry(id_parts.package_name.to_string())
        .and_modify(|v| *v = true)
        .or_default();
      version_by_dep_name.insert(
        id_parts.package_name.to_string(),
        id_parts.version.to_string(),
      );
      npm_packages.push(PackageNv {
        name: id_parts.package_name.as_str().into(),
        version,
      });
      keys.push(key.clone());
    }
    let results = info_provider
      .get_npm_package_info(&npm_packages)
      .await
      .map_err(|source| TransformError::FailedGettingNpmPackageInfo {
        source,
      })?;
    if results.len() != keys.len() {
      return Err(TransformError::FailedGettingNpmPackageInfo {
        source: Box::new(MissingNpmPackageInfo),
      });
    }
    for (key, result) in keys.iter().zip(results) {
      let Some(Value::Object(value)) = npm.get_mut(key) else {
        continue;
      };

      let mut existing_deps = BTreeMap::new();
      if let Some(Value::Array(deps)) = value.remove("dependencies") {
        existing_deps.extend(deps.iter().filter_map(|v| {
          let id = v.as_str().map(|s| s.to_string())?;
          let id_parts = split_id(&id, &version_by_dep_name)?;
          Some((id_parts.key, id))
        }));
      }

      let mut new_optional_deps = Vec::new();
      if !result.optional_dependencies.is_empty() {
        for (key, _) in result.optional_dependencies {
          if let Some(id) = existing_deps.remove(&*key) {
            new_optional_deps.push(id);
          }
        }
        value.insert("optionalDependencies".into(), new_optional_deps.into());
      }
      let mut new_optional_peer_deps = Vec::new();
      if !result.optional_peers.is_empty() {
        for (key, value) in result.optional_peers {
          new_optional_peer_deps.push(format!("{}@{}", key, value));
        }
        value.insert("optionalPeers".into(), new_optional_peer_deps.into());
      }

      if existing_deps.is_empty() {
        value.remove("dependencies");
      } else {
        value.insert(
          "dependencies".into(),
          existing_deps
            .into_values()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into(),
        );
      }
      if !result.cpu.is_empty() {
        value.insert("cpu".into(), result.cpu.into());
      }
      if !result.os.is_empty() {
        value.insert("os".into(), result.os.into());
      }
      if let Some(tarball_url) = result.tarball_url {
        value.insert("tarball".into(), tarball_url.into());
      }
      if result.deprecated {
        value.insert("deprecated".into(), true.into());
      }
      if result.scripts {
        value.insert("scripts".into(), true.into());
      }
      if result.bin {
        value.insert("bin".into(), true.into());
      }
    }
    json.insert("npm".into(), npm.into());
  }

  Ok(json)
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Lockfile5NpmInfo {
  pub tarball_url: Option<String>,
  pub optional_dependencies: BTreeMap<String, String>,
  pub optional_peers: BTreeMap<String, String>,
  pub cpu: Vec<String>,
  pub os: Vec<String>,
  pub deprecated: bool,
  pub scripts: bool,
  pub bin: bool,
}

#[async_trait::async_trait(?Send)]
pub trait NpmPackageInfoProvider {
  async fn get_npm_package_info(
    &self,
    values: &[PackageNv],
  ) -> Result<Vec<Lockfile5NpmInfo>, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod test {
  use std::future::Future;

  use async_executor::LocalExecutor;
  use pretty_assertions::assert_eq;
  use serde_json::json;

  use super::*;

  #[test]
  fn test_transforms_1_to_2() {
    let data: JsonMap = serde_json::from_value(json!({
      "https://github.com/": "asdf",
      "https://github.com/mod.ts": "asdf2",
    }))
    .unwrap();
    let result = transform1_to_2(data);
    assert_eq!(
      result,
      serde_json::from_value(json!({
        "version": "2",
        "remote": {
          "https://github.com/": "asdf",
          "https://github.com/mod.ts": "asdf2",
        }
      }))
      .unwrap()
    );
  }

  #[test]
  fn test_transforms_2_to_3() {
    let data: JsonMap = serde_json::from_value(json!({
      "version": "2",
      "remote": {
        "https://github.com/": "asdf",
        "https://github.com/mod.ts": "asdf2",
      },
      "npm": {
        "specifiers": {
          "nanoid": "nanoid@3.3.4",
        },
        "packages": {
          "nanoid@3.3.4": {
            "integrity": "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==",
            "dependencies": {}
          },
          "picocolors@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          }
        }
      }
    })).unwrap();
    let result = transform2_to_3(data);
    assert_eq!(result, serde_json::from_value(json!({
      "version": "3",
      "remote": {
        "https://github.com/": "asdf",
        "https://github.com/mod.ts": "asdf2",
      },
      "packages": {
        "specifiers": {
          "npm:nanoid": "npm:nanoid@3.3.4",
        },
        "npm": {
          "nanoid@3.3.4": {
            "integrity": "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==",
            "dependencies": {}
          },
          "picocolors@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          }
        }
      }
    })).unwrap());
  }

  #[test]
  fn test_transforms_3_to_4_basic() {
    let data: JsonMap = serde_json::from_value(json!({
      "version": "3",
      "remote": {
        "https://github.com/": "asdf",
        "https://github.com/mod.ts": "asdf2",
      },
      "packages": {
        "specifiers": {
          "npm:package-a": "npm:package-a@3.3.4",
        },
        "npm": {
          "package-a@3.3.4": {
            "integrity": "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==",
            "dependencies": {
              "package-b": "package-b@1.0.0",
              "package-c": "package-c@1.0.0",
              "other": "package-d@1.0.0",
            }
          },
          "package-b@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          },
          "package-c@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          },
          "package-c@2.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {
              "package-e": "package-e@1.0.0_package-d@1.0.0",
            }
          },
          "package-d@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {}
          },
          "package-e@1.0.0_package-d@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": {
              "package-d": "package-d@1.0.0",
            }
          }
        }
      }
    })).unwrap();
    let result = transform3_to_4(data).unwrap();
    assert_eq!(result, serde_json::from_value(json!({
      "version": "4",
      "specifiers": {
        "npm:package-a": "3.3.4",
      },
      "npm": {
        "package-a@3.3.4": {
          "integrity": "sha512-MqBkQh/OHTS2egovRtLk45wEyNXwF+cokD+1YPf9u5VfJiRdAiRwB2froX5Co9Rh20xs4siNPm8naNotSD6RBw==",
          "dependencies": [
            "other@npm:package-d@1.0.0",
            "package-b",
            "package-c@1.0.0",
          ]
        },
        "package-b@1.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": []
        },
        "package-c@1.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": []
        },
        "package-c@2.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": [
            "package-e"
          ]
        },
        "package-d@1.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": []
        },
        "package-e@1.0.0_package-d@1.0.0": {
          "integrity": "sha512-foobar",
          "dependencies": [
            "package-d"
          ]
        }
      },
      "remote": {
        "https://github.com/": "asdf",
        "https://github.com/mod.ts": "asdf2",
      },
    })).unwrap());
  }

  fn run_async<T: Send + Sync>(f: impl Future<Output = T>) -> T {
    let executor = LocalExecutor::new();
    let handle = executor.run(f);
    futures_lite::future::block_on(handle)
  }

  struct TestNpmPackageInfoProvider {
    packages: HashMap<PackageNv, Lockfile5NpmInfo>,
  }

  #[async_trait::async_trait(?Send)]
  impl NpmPackageInfoProvider for TestNpmPackageInfoProvider {
    async fn get_npm_package_info(
      &self,
      values: &[PackageNv],
    ) -> Result<Vec<Lockfile5NpmInfo>, Box<dyn std::error::Error + Send + Sync>>
    {
      Ok(
        values
          .iter()
          .map(|v| {
            self
              .packages
              .get(v)
              .unwrap_or_else(|| panic!("no info for {v}"))
              .clone()
          })
          .collect(),
      )
    }
  }

  fn nv(name_and_version: &str) -> PackageNv {
    PackageNv::from_str(name_and_version).unwrap()
  }

  fn default_info(name_and_version: &str) -> (PackageNv, Lockfile5NpmInfo) {
    (nv(name_and_version), Lockfile5NpmInfo::default())
  }

  #[test]
  fn test_transforms_4_to_5() {
    let result = run_async(async move {
      let packages = [
        (
          nv("package-a@3.3.4"),
          Lockfile5NpmInfo {
            optional_dependencies: [
              ("package-b", "1.0.0"),
              ("othername", "package-c@1.0.0"),
              ("package-d", "1.0.0"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            ..Default::default()
          },
        ),
        default_info("package-b@1.0.0"),
        (
          nv("package-c@1.0.0"),
          Lockfile5NpmInfo {
            bin: true,
            ..Default::default()
          },
        ),
        (
          nv("package-d@1.0.0"),
          Lockfile5NpmInfo {
            scripts: true,
            ..Default::default()
          },
        ),
        (
          nv("package-d@2.0.0"),
          Lockfile5NpmInfo {
            optional_peers: [("package-z", "1.0.0")]
              .into_iter()
              .map(|(k, v)| (k.to_string(), v.to_string()))
              .collect(),
            ..Default::default()
          },
        ),
        (
          nv("package-e@1.0.0"),
          Lockfile5NpmInfo {
            deprecated: true,
            ..Default::default()
          },
        ),
      ];
      let data = serde_json::from_value(json!({
        "version": "4",
        "npm": {
          "package-a@3.3.4": {
            "integrity": "sha512-foobar",
            "dependencies": [
              "package-b",
              "othername@npm:package-c@1.0.0",
              "package-d@1.0.0",
              "package-e",
            ]
          },
          "package-b@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": []
          },
          "package-c@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": ["package-d@2.0.0"]
          },
          "package-d@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": []
          },
          "package-d@2.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": []
          },
          "package-e@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": []
          },
        }
      }))
      .unwrap();
      transform4_to_5(
        data,
        &TestNpmPackageInfoProvider {
          packages: HashMap::from_iter(packages),
        },
      )
      .await
      .unwrap()
    });
    assert_eq!(
      result,
      serde_json::from_value(json!({
        "version": "5",
        "npm": {
          "package-a@3.3.4": {
            "integrity": "sha512-foobar",
            "dependencies": [
              "package-e",
            ],
            "optionalDependencies": ["othername@npm:package-c@1.0.0", "package-b", "package-d@1.0.0"]
          },
          "package-b@1.0.0": {
            "integrity": "sha512-foobar",
          },
          "package-c@1.0.0": {
            "integrity": "sha512-foobar",
            "dependencies": ["package-d@2.0.0"],
            "bin": true,
          },
          "package-d@1.0.0": {
            "integrity": "sha512-foobar",
            "scripts": true,
          },
          "package-d@2.0.0": {
            "integrity": "sha512-foobar",
            "optionalPeers": ["package-z@1.0.0"],
          },
          "package-e@1.0.0": {
            "integrity": "sha512-foobar",
            "deprecated": true,
          },
        }
      }))
      .unwrap()
    );
  }

  fn parts(key: &str, package_name: &str, version: &str) -> Option<IdParts> {
    Some(IdParts {
      key: key.to_string(),
      package_name: package_name.to_string(),
      version: version.to_string(),
    })
  }

  #[test]
  fn test_split_id() {
    let mut version_by_dep_name = HashMap::default();
    let ids = [
      ("package-a@1.0.0", parts("package-a", "package-a", "1.0.0")),
      (
        "othername@npm:package-b@1.0.0",
        parts("othername", "package-b", "1.0.0"),
      ),
      (
        "package-c@1.0.0_package-d@1.0.0",
        parts("package-c", "package-c", "1.0.0"),
      ),
      ("package-d@1.0.0", parts("package-d", "package-d", "1.0.0")),
      (
        "othername@npm:package-b@1.0.0_package-d@1.0.0_package-e@1.0.0",
        parts("othername", "package-b", "1.0.0"),
      ),
    ];
    for (id, expected) in ids {
      let id_parts = split_id(id, &version_by_dep_name);
      assert_eq!(id_parts, expected);
      if let Some(id_parts) = id_parts {
        version_by_dep_name.insert(id_parts.package_name, id_parts.version);
      } else {
        panic!("failed to split {id}");
      }
    }
  }
}
