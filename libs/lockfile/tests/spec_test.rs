// Copyright 2018-2024 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::panic::UnwindSafe;
use std::path::PathBuf;

use deno_lockfile::Lockfile;
use deno_lockfile::Lockfile5NpmInfo;
use deno_lockfile::LockfileLinkContent;
use deno_lockfile::NewLockfileOptions;
use deno_lockfile::NpmPackageInfoProvider;
use deno_lockfile::PackagesContent;
use deno_lockfile::SetWorkspaceConfigOptions;
use deno_lockfile::WorkspaceConfig;
use deno_lockfile::WorkspaceMemberConfig;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use file_test_runner::RunOptions;
use file_test_runner::SubTestResult;
use file_test_runner::TestResult;
use file_test_runner::collect_and_run_tests;
use file_test_runner::collection::CollectOptions;
use file_test_runner::collection::CollectedTest;
use file_test_runner::collection::strategies::TestPerFileCollectionStrategy;
use helpers::ConfigChangeSpec;
use helpers::SpecSection;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;

mod helpers;

fn main() {
  collect_and_run_tests(
    CollectOptions {
      base: "tests/specs".into(),
      strategy: Box::<TestPerFileCollectionStrategy>::default(),
      filter_override: None,
    },
    RunOptions { parallel: true },
    run_test,
  )
}

fn run_test(test: &CollectedTest) -> TestResult {
  TestResult::from_maybe_panic_or_result(AssertUnwindSafe(|| {
    futures_lite::future::block_on(async_executor::Executor::new().run(
      async move {
        if test.name.starts_with("specs::config_changes::") {
          config_changes_test(test).await;
          TestResult::Passed
        } else if test.name.starts_with("specs::transforms::") {
          transforms_test(test).await
        } else {
          panic!("Unknown test: {}", test.name);
        }
      },
    ))
  }))
}

fn from_maybe_panic_async<T>(
  f: impl Future<Output = T> + UnwindSafe,
) -> TestResult {
  TestResult::from_maybe_panic(|| {
    futures_lite::future::block_on(async_executor::Executor::new().run(f));
  })
}

async fn config_changes_test(test: &CollectedTest) {
  #[derive(Debug, Default, Clone, Serialize, Deserialize, Hash)]
  #[serde(rename_all = "camelCase")]
  struct LockfilePackageJsonContent {
    #[serde(default)]
    dependencies: BTreeSet<JsrDepPackageReq>,
  }

  #[derive(Debug, Default, Clone, Deserialize, Hash)]
  #[serde(rename_all = "camelCase")]
  struct WorkspaceMemberConfigContent {
    #[serde(default)]
    dependencies: BTreeSet<JsrDepPackageReq>,
    #[serde(default)]
    package_json: LockfilePackageJsonContent,
  }

  #[derive(Debug, Default, Clone, Deserialize, Hash)]
  #[serde(rename_all = "camelCase")]
  struct LinkConfigContent {
    #[serde(default)]
    dependencies: BTreeSet<JsrDepPackageReq>,
    #[serde(default)]
    optional_dependencies: BTreeSet<JsrDepPackageReq>,
    #[serde(default)]
    peer_dependencies: BTreeSet<JsrDepPackageReq>,
    #[serde(default)]
    peer_dependencies_meta: BTreeMap<String, PeerDependenciesMetaValue>,
  }

  #[derive(Debug, Default, Clone, Serialize, Deserialize, Hash)]
  #[serde(rename_all = "camelCase")]
  struct PeerDependenciesMetaValue {
    optional: bool,
  }

  #[derive(Debug, Default, Clone, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct WorkspaceConfigContent {
    #[serde(default, flatten)]
    root: WorkspaceMemberConfigContent,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    members: BTreeMap<String, WorkspaceMemberConfigContent>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(default)]
    links: BTreeMap<String, LinkConfigContent>,
    #[serde(default)]
    #[serde(alias = "overrides")]
    npm_overrides: Option<serde_json::Value>,
  }

  impl WorkspaceConfigContent {
    fn into_workspace_config(self) -> WorkspaceConfig {
      WorkspaceConfig {
        root: WorkspaceMemberConfig {
          dependencies: self.root.dependencies.into_iter().collect(),
          package_json_deps: self
            .root
            .package_json
            .dependencies
            .into_iter()
            .collect(),
        },
        members: self
          .members
          .into_iter()
          .map(|(k, v)| {
            (
              k,
              WorkspaceMemberConfig {
                dependencies: v.dependencies.into_iter().collect(),
                package_json_deps: v
                  .package_json
                  .dependencies
                  .into_iter()
                  .collect(),
              },
            )
          })
          .collect(),
        links: self
          .links
          .into_iter()
          .map(|(k, v)| {
            (
              k,
              LockfileLinkContent {
                dependencies: v.dependencies.into_iter().collect(),
                optional_dependencies: v
                  .optional_dependencies
                  .into_iter()
                  .collect(),
                peer_dependencies: v.peer_dependencies.into_iter().collect(),
                peer_dependencies_meta: v
                  .peer_dependencies_meta
                  .into_iter()
                  .map(|(k, v)| (k, serde_json::to_value(v).unwrap()))
                  .collect(),
              },
            )
          })
          .collect(),
        npm_overrides: self.npm_overrides,
      }
    }
  }

  let is_update = std::env::var("UPDATE") == Ok("1".to_string());
  let mut spec = ConfigChangeSpec::parse(&test.read_to_string().unwrap());
  let mut lockfile = Lockfile::new(
    NewLockfileOptions {
      file_path: test.path.with_extension("lock"),
      content: &spec.original_text.text,
      overwrite: false,
    },
    &TestNpmPackageInfoProvider::default(),
  )
  .await
  .unwrap();
  for change_and_output in &mut spec.change_and_outputs {
    // setting the new workspace config should change the has_content_changed flag
    lockfile.has_content_changed = false;
    let config = serde_json::from_str::<WorkspaceConfigContent>(
      &change_and_output.change.text,
    )
    .unwrap()
    .into_workspace_config();
    let no_npm = change_and_output.change.title.contains("--no-npm");
    let no_config = change_and_output.change.title.contains("--no-config");
    lockfile.set_workspace_config(SetWorkspaceConfigOptions {
      no_config,
      no_npm,
      config: config.clone(),
    });
    assert_eq!(
      lockfile.has_content_changed,
      !change_and_output.change.title.contains("no change"),
      "Failed for {}",
      change_and_output.change.title,
    );

    // now try resetting it and the flag should remain the same
    lockfile.has_content_changed = false;
    lockfile.set_workspace_config(SetWorkspaceConfigOptions {
      no_config,
      no_npm,
      config: config.clone(),
    });
    assert!(!lockfile.has_content_changed);

    let expected_text = change_and_output.output.text.clone();
    let actual_text = lockfile.as_json_string();
    if is_update {
      change_and_output.output.text = actual_text;
    } else {
      assert_eq!(
        actual_text.trim(),
        expected_text.trim(),
        "Failed for: {}",
        change_and_output.change.title,
      );
    }
    verify_packages_content(&lockfile.content.packages);
  }
  if is_update {
    std::fs::write(&test.path, spec.emit()).unwrap();
  }
}

struct TestNpmPackageInfoProvider {
  cache: RefCell<HashMap<PackageNv, Lockfile5NpmInfo>>,
}

impl Default for TestNpmPackageInfoProvider {
  fn default() -> Self {
    Self {
      cache: RefCell::new(HashMap::new()),
    }
  }
}

#[async_trait::async_trait(?Send)]
impl NpmPackageInfoProvider for TestNpmPackageInfoProvider {
  async fn get_npm_package_info(
    &self,
    packages: &[PackageNv],
  ) -> Result<Vec<Lockfile5NpmInfo>, Box<dyn std::error::Error + Send + Sync>>
  {
    let mut infos = Vec::with_capacity(packages.len());
    for package in packages {
      let info = { self.cache.borrow().get(package).cloned() };
      if let Some(info) = info {
        infos.push(info);
      } else {
        let path = package_file_path(package);
        if path.exists() {
          let text = std::fs::read_to_string(path).unwrap();
          let info: Lockfile5NpmInfo = serde_json::from_str(&text).unwrap();
          self
            .cache
            .borrow_mut()
            .insert(package.clone(), info.clone());
          infos.push(info);
        } else {
          infos.push(Default::default());
        }
      }
    }
    Ok(infos)
  }
}

fn package_file_path(package: &PackageNv) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/registry_data/")
    .join(package_file_name(package))
}

fn package_file_name(package: &PackageNv) -> String {
  format!(
    "{}@{}.json",
    package.name.replace("/", "__"),
    package.version
  )
}

async fn transforms_test(test: &CollectedTest) -> TestResult {
  let text = test.read_to_string().unwrap();
  let mut sections = SpecSection::parse_many(&text);
  assert_eq!(sections.len(), 2);
  let original_section = sections.remove(0);
  let mut expected_section = sections.remove(0);
  let lockfile = Lockfile::new(
    NewLockfileOptions {
      file_path: test.path.with_extension("lock"),
      content: &original_section.text,
      overwrite: false,
    },
    &TestNpmPackageInfoProvider::default(),
  )
  .await
  .unwrap();
  let actual_text = lockfile.as_json_string();
  let is_update = std::env::var("UPDATE") == Ok("1".to_string());
  if is_update {
    expected_section.text = actual_text;
    std::fs::write(
      &test.path,
      format!("{}{}", original_section.emit(), expected_section.emit()),
    )
    .unwrap();
    TestResult::Passed
  } else {
    let mut sub_tests = Vec::with_capacity(2);
    sub_tests.push(SubTestResult {
      name: "v4_upgrade".to_string(),
      result: TestResult::from_maybe_panic(|| {
        assert_eq!(actual_text.trim(), expected_section.text.trim());
      }),
    });
    // now try parsing the lockfile v4 output, then reserialize it and ensure it matches
    sub_tests.push(SubTestResult {
      name: "v4_reparse_and_emit".to_string(),
      result: from_maybe_panic_async(AssertUnwindSafe(async {
        let lockfile: Lockfile = Lockfile::new(
          NewLockfileOptions {
            file_path: test.path.with_extension("lock"),
            content: &actual_text,
            overwrite: false,
          },
          &TestNpmPackageInfoProvider::default(),
        )
        .await
        .unwrap();
        assert_eq!(lockfile.as_json_string().trim(), actual_text.trim());
      })),
    });
    TestResult::SubTests(sub_tests)
  }
}

fn verify_packages_content(packages: &PackagesContent) {
  // verify the specifiers
  for (req, id_suffix_or_nv) in &packages.specifiers {
    let id = format!(
      "{}{}@{}",
      req.kind.scheme_with_colon(),
      req.req.name,
      id_suffix_or_nv
    );
    if let Some(npm_id) = id.strip_prefix("npm:") {
      assert!(packages.npm.contains_key(npm_id), "Missing: {}", id);
    } else if let Some(jsr_nv) = id.strip_prefix("jsr:") {
      let nv = PackageNv::from_str(jsr_nv).unwrap();
      assert!(packages.jsr.contains_key(&nv), "Missing: {}", id);
    } else {
      panic!("Invalid package id: {}", id);
    }
  }
  for (pkg_id, package) in &packages.npm {
    for dep_id in package.dependencies.values() {
      assert!(
        packages.npm.contains_key(dep_id),
        "Missing '{}' dep in '{}'",
        dep_id,
        pkg_id,
      );
    }
  }
  for (pkg_id, package) in &packages.jsr {
    for req in &package.dependencies {
      let Some((req, id_suffix_or_nv)) = packages.specifiers.get_key_value(req)
      else {
        panic!("Missing specifier for '{}' in '{}'", req, pkg_id);
      };
      let dep_id = format!(
        "{}{}@{}",
        req.kind.scheme_with_colon(),
        req.req.name,
        id_suffix_or_nv
      );
      if let Some(npm_id) = dep_id.strip_prefix("npm:") {
        assert!(
          packages.npm.contains_key(npm_id),
          "Missing: '{}' dep in '{}'",
          dep_id,
          pkg_id,
        );
      } else if let Some(jsr_nv) = dep_id.strip_prefix("jsr:") {
        let nv = PackageNv::from_str(jsr_nv).unwrap();
        assert!(
          packages.jsr.contains_key(&nv),
          "Missing: '{}' dep in '{}'",
          dep_id,
          pkg_id,
        );
      } else {
        panic!("Invalid package id: {}", dep_id);
      }
    }
  }
}
