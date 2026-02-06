// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmPackageVersionInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::NpmVersionResolver;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use reqwest::StatusCode;

/// This example is not an example, but is a tool to create a minimal
/// reproduction of a bug from a set of real npm package requirements
/// and a provided condition.
///
/// 1. Provide your package requirements below.
/// 2. Update the condition saying what the bug is.
/// 3. Run `cargo run --example min_repro_solver`
///
/// This will output some test code that you can use in order to have
/// a small reproduction of the bug.
///
/// Additionally, it will populate the `.bench-reg` folder which allows
/// running a custom npm registry locally via:
///
/// ```sh
/// deno run --allow-net --allow-read=. --allow-write=. jsr:@david/bench-registry@0.3.2/cli --cached-only
/// ```
///
/// Then do `deno clean ; pnpm cache delete`, create an `.npmrc` with:
///
/// ```ini
/// registry=http://localhost:8000/npm/
/// ```
///
/// ...then a package.json with the package requirements below, and you can
/// compare the reproduction in other package managers.
#[tokio::main(flavor = "current_thread")]
async fn main() {
  let mut solver =
    MinimalReproductionSolver::new(&["@aws-cdk/aws-ecs"], |snapshot| {
      snapshot.all_packages_for_every_system().any(|s| {
        s.id.as_serialized().chars().filter(|c| *c == '_').count() > 200
      })
    })
    .await;
  let mut had_change = true;
  while had_change {
    had_change = false;
    had_change |= solver.attempt_reduce_reqs().await;
    had_change |= solver.attempt_reduce_dependendencies().await;
  }

  eprintln!("===========================");
  eprintln!("Test code");
  eprintln!("===========================");
  let test_code = solver.get_test_code();
  println!("{}", test_code);
  solver.output_bench_registry_folder();
}

struct MinimalReproductionSolver {
  reqs: Vec<String>,
  condition: Box<dyn Fn(&NpmResolutionSnapshot) -> bool + 'static>,
  api: SubsetRegistryApi,
  current_snapshot: NpmResolutionSnapshot,
}

impl MinimalReproductionSolver {
  pub async fn new(
    reqs: &[&str],
    condition: impl Fn(&NpmResolutionSnapshot) -> bool + 'static,
  ) -> Self {
    let reqs = reqs.iter().map(|r| r.to_string()).collect::<Vec<_>>();
    let api = SubsetRegistryApi::default();
    let snapshot = run_resolver_and_get_snapshot(&api, &reqs).await;
    assert!(condition(&snapshot), "bug does not exist in provided setup");
    MinimalReproductionSolver {
      reqs,
      condition: Box::new(condition),
      api,
      current_snapshot: snapshot,
    }
  }

  pub async fn attempt_reduce_reqs(&mut self) -> bool {
    let mut made_reduction = false;
    for i in (0..self.reqs.len()).rev() {
      if self.reqs.len() <= 1 {
        break;
      }
      let mut new_reqs = self.reqs.clone();
      let removed_req = new_reqs.remove(i);
      eprintln!("Checking removal of package req {}", removed_req);
      let changed = self
        .resolve_and_update_state_if_matches_condition(None, Some(new_reqs))
        .await;
      if changed {
        made_reduction = true;
        eprintln!("Removed req: {}", removed_req);
      }
    }
    made_reduction
  }

  pub async fn attempt_reduce_dependendencies(&mut self) -> bool {
    let mut made_reduction = false;
    let package_nvs = self.current_nvs();

    for package_nv in package_nvs {
      let dep_names = {
        let version_info = self.api.get_version_info(&package_nv);
        version_info
          .dependencies
          .keys()
          .chain(version_info.optional_dependencies.keys())
          .chain(version_info.peer_dependencies.keys())
          .cloned()
          .collect::<BTreeSet<_>>()
      };
      for dep_name in dep_names {
        eprintln!("{}: checking removal of {}", package_nv, dep_name);
        let new_api = self.api.clone();
        let mut new_version_info =
          self.api.get_version_info(&package_nv).clone();
        new_version_info.dependencies.remove(&dep_name);
        new_version_info.optional_dependencies.remove(&dep_name);
        new_version_info.peer_dependencies.remove(&dep_name);
        new_version_info.peer_dependencies_meta.remove(&dep_name);
        new_api.set_package_version_info(&package_nv, new_version_info);
        let changed = self
          .resolve_and_update_state_if_matches_condition(Some(new_api), None)
          .await;
        if changed {
          made_reduction = true;
          eprintln!("{}: removed {}", package_nv, dep_name);
        }
      }
    }

    made_reduction
  }

  async fn resolve_and_update_state_if_matches_condition(
    &mut self,
    api: Option<SubsetRegistryApi>,
    reqs: Option<Vec<String>>,
  ) -> bool {
    let snapshot = run_resolver_and_get_snapshot(
      api.as_ref().unwrap_or(&self.api),
      reqs.as_ref().unwrap_or(&self.reqs),
    )
    .await;
    if !(self.condition)(&snapshot) {
      return false;
    }
    if let Some(api) = api {
      self.api = api;
    }
    if let Some(reqs) = reqs {
      self.reqs = reqs;
    }
    true
  }

  fn current_nvs(&self) -> BTreeSet<PackageNv> {
    self
      .current_snapshot
      .all_packages_for_every_system()
      .map(|pkg| pkg.id.nv.clone())
      .collect::<BTreeSet<_>>()
  }

  fn get_test_code(&self) -> String {
    let mut text = String::new();

    text.push_str("let api = TestNpmRegistryApi::default();\n");

    let nvs = self.current_nvs();

    for nv in &nvs {
      text.push_str(&format!(
        "api.ensure_package_version(\"{}\", \"{}\");\n",
        nv.name, nv.version
      ));
    }

    for nv in &nvs {
      if !text.ends_with("\n\n") {
        text.push('\n');
      }
      // text.push_str(&format!("// {}\n", nv));
      let version_info = self.api.get_version_info(nv);
      for (key, value) in &version_info.dependencies {
        text.push_str(&format!(
          "api.add_dependency((\"{}\", \"{}\"), (\"{}\", \"{}\"));\n",
          nv.name, nv.version, key, value
        ));
      }
      for (key, value) in &version_info.peer_dependencies {
        let is_optional = version_info
          .peer_dependencies_meta
          .get(key)
          .map(|m| m.optional)
          .unwrap_or(false);
        if is_optional {
          text.push_str(&format!(
            "api.add_optional_peer_dependency((\"{}\", \"{}\"), (\"{}\", \"{}\"));\n",
            nv.name, nv.version, key, value
          ));
        } else {
          text.push_str(&format!(
            "api.add_peer_dependency((\"{}\", \"{}\"), (\"{}\", \"{}\"));\n",
            nv.name, nv.version, key, value
          ));
        }
      }
    }

    let reqs = self
      .reqs
      .iter()
      .map(|k| format!("\"{}\"", k))
      .collect::<Vec<_>>();
    text.push_str(
      "\nlet (packages, package_reqs) = run_resolver_and_get_output(\n",
    );
    text.push_str("  api,\n");
    text.push_str("  vec![");
    text.push_str(&reqs.join(", "));
    text.push_str("],\n");
    text.push_str(").await;\n");
    text.push_str("assert_eq!(packages, vec![]);\n");
    text.push_str("assert_eq!(package_reqs, vec![]);\n");

    text
  }

  /// Output a .bench-reg folder so that the output can be compared
  /// with other package managers when using: https://jsr.io/@david/bench-registry@0.2.0
  fn output_bench_registry_folder(&self) {
    let package_infos = self.api.get_all_package_infos();
    let nvs = self.current_nvs();
    let bench_reg_folder = PathBuf::from(".bench-reg");
    std::fs::create_dir_all(&bench_reg_folder).unwrap();
    // trim down the package infos to only contain the found nvs
    for mut package_info in package_infos {
      let keys_to_remove = package_info
        .versions
        .keys()
        .filter(|&v| {
          let nv = PackageNv {
            name: package_info.name.clone(),
            version: (v).clone(),
          };
          !nvs.contains(&nv)
        })
        .cloned()
        .collect::<Vec<_>>();
      for key in &keys_to_remove {
        package_info.versions.remove(key);
      }
      let url = packument_url(&package_info.name);
      let file_path = bench_reg_folder.join(sha256_hex(&url));
      std::fs::write(
        &file_path,
        serde_json::to_string_pretty(&package_info).unwrap(),
      )
      .unwrap();
      std::fs::write(file_path.with_extension("headers"), "{}").unwrap();
    }
  }
}

async fn run_resolver_and_get_snapshot(
  api: &SubsetRegistryApi,
  reqs: &[String],
) -> NpmResolutionSnapshot {
  let snapshot = NpmResolutionSnapshot::new(Default::default());
  let reqs = reqs
    .iter()
    .map(|req| PackageReq::from_str(req).unwrap())
    .collect::<Vec<_>>();
  let version_resolver = NpmVersionResolver {
    link_packages: Default::default(),
    newest_dependency_date_options: Default::default(),
  };
  let result = snapshot
    .add_pkg_reqs(
      api,
      AddPkgReqsOptions {
        package_reqs: &reqs,
        version_resolver: &version_resolver,
        should_dedup: true,
      },
      None,
    )
    .await;
  result.dep_graph_result.unwrap()
}

#[derive(Clone)]
struct SubsetRegistryApi {
  data: RefCell<HashMap<String, Arc<NpmPackageInfo>>>,
}

impl Default for SubsetRegistryApi {
  fn default() -> Self {
    std::fs::create_dir_all("target/.deno_npm").unwrap();
    Self {
      data: Default::default(),
    }
  }
}

impl SubsetRegistryApi {
  pub fn get_all_package_infos(&self) -> Vec<NpmPackageInfo> {
    self
      .data
      .borrow()
      .values()
      .map(|i| i.as_ref().clone())
      .collect()
  }

  pub fn get_version_info(&self, nv: &PackageNv) -> NpmPackageVersionInfo {
    self
      .data
      .borrow()
      .get(nv.name.as_str())
      .unwrap()
      .versions
      .get(&nv.version)
      .unwrap()
      .clone()
  }

  pub fn set_package_version_info(
    &self,
    nv: &PackageNv,
    version_info: NpmPackageVersionInfo,
  ) {
    let mut data = self.data.borrow_mut();
    let mut package_info = data.get(nv.name.as_str()).unwrap().as_ref().clone();
    package_info
      .versions
      .insert(nv.version.clone(), version_info);
    data.insert(nv.name.to_string(), Arc::new(package_info));
  }
}

#[async_trait::async_trait(?Send)]
impl NpmRegistryApi for SubsetRegistryApi {
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    if let Some(data) = self.data.borrow_mut().get(name).cloned() {
      return Ok(data);
    }
    let file_path = PathBuf::from(packument_cache_filepath(name));
    if let Ok(data) = std::fs::read_to_string(&file_path)
      && let Ok(data) = serde_json::from_str::<Arc<NpmPackageInfo>>(&data)
    {
      self
        .data
        .borrow_mut()
        .insert(name.to_string(), data.clone());
      return Ok(data);
    }
    let url = packument_url(name);
    eprintln!("Downloading {}", url);
    let resp = reqwest::get(&url).await.unwrap();
    if resp.status() == StatusCode::NOT_FOUND {
      return Err(NpmRegistryPackageInfoLoadError::PackageNotExists {
        package_name: name.to_string(),
      });
    }
    let text = resp.text().await.unwrap();
    let temp_path = file_path.with_extension(".tmp");
    std::fs::write(&temp_path, &text).unwrap();
    std::fs::rename(&temp_path, &file_path).unwrap();
    let data = serde_json::from_str::<Arc<NpmPackageInfo>>(&text).unwrap();
    self
      .data
      .borrow_mut()
      .insert(name.to_string(), data.clone());
    Ok(data)
  }
}

fn packument_cache_filepath(name: &str) -> String {
  format!("target/.deno_npm/{}", encode_package_name(name))
}

fn packument_url(name: &str) -> String {
  format!("https://registry.npmjs.org/{}", encode_package_name(name))
}

fn encode_package_name(name: &str) -> String {
  name.replace("/", "%2f")
}

fn sha256_hex(input: &str) -> String {
  use hex;
  use sha2::Digest;
  use sha2::Sha256;

  let mut hasher = Sha256::new();
  hasher.update(input.as_bytes());
  let result = hasher.finalize();
  hex::encode(result)
}
