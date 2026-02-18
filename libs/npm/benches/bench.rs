// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::registry::NpmRegistryPackageInfoLoadError;
use deno_npm::registry::TestNpmRegistryApi;
use deno_npm::resolution::AddPkgReqsOptions;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::NpmVersionResolver;
use deno_semver::package::PackageReq;
use reqwest::StatusCode;

fn main() {
  divan::main();
}

mod deserialization {
  use super::*;

  #[divan::bench]
  fn packument(bencher: divan::Bencher) {
    let text = get_next_packument_text();
    bencher.bench_local(|| {
      let value = serde_json::from_str::<NpmPackageInfo>(&text).unwrap();
      value.name.len()
    });
  }

  #[divan::bench]
  fn packument_no_drop(bencher: divan::Bencher) {
    let text = get_next_packument_text();
    bencher
      .bench_local(|| serde_json::from_str::<NpmPackageInfo>(&text).unwrap());
  }

  fn get_next_packument_text() -> String {
    build_rt().block_on(async {
      // ensure the fs cache is populated
      let _ = TargetFolderCachedRegistryApi::default()
        .package_info("next")
        .await
        .unwrap();
    });

    std::fs::read_to_string(packument_cache_filepath("next")).unwrap()
  }
}

mod resolution {
  use super::*;

  #[divan::bench]
  fn test(bencher: divan::Bencher) {
    let api = TestNpmRegistryApi::default();
    let mut initial_pkgs = Vec::new();
    const VERSION_COUNT: usize = 100;
    for pkg_index in 0..26 {
      let pkg_name = format!("a{}", pkg_index);
      let next_pkg = format!("a{}", pkg_index + 1);
      for version_index in 0..VERSION_COUNT {
        let version = format!("{}.0.0", version_index);
        if pkg_index == 0 {
          initial_pkgs.push(format!(
            "{}@{}",
            pkg_name.clone(),
            version.clone()
          ));
        }
        api.ensure_package_version(&pkg_name, &version);
        if pkg_index < 25 {
          api.add_dependency(
            (pkg_name.as_str(), version.as_str()),
            (next_pkg.as_str(), version.as_str()),
          );
        }
      }
    }

    let rt = build_rt();

    bencher.bench_local(|| {
      let snapshot = rt.block_on(async {
        run_resolver_and_get_snapshot(&api, &initial_pkgs).await
      });

      assert_eq!(snapshot.top_level_packages().count(), VERSION_COUNT);
    });
  }

  #[divan::bench(sample_count = 1000)]
  fn nextjs_resolve(bencher: divan::Bencher) {
    let api = TargetFolderCachedRegistryApi::default();
    let rt = build_rt();

    // run once to fill the caches
    rt.block_on(async {
      run_resolver_and_get_snapshot(&api, &["next@15.1.2".to_string()]).await
    });

    bencher.bench_local(|| {
      let snapshot = rt.block_on(async {
        run_resolver_and_get_snapshot(&api, &["next@15.1.2".to_string()]).await
      });

      assert_eq!(snapshot.top_level_packages().count(), 1);
    });
  }
}

struct TargetFolderCachedRegistryApi {
  data: Rc<RefCell<HashMap<String, Arc<NpmPackageInfo>>>>,
}

impl Default for TargetFolderCachedRegistryApi {
  fn default() -> Self {
    std::fs::create_dir_all("target/.deno_npm").unwrap();
    Self {
      data: Default::default(),
    }
  }
}

#[async_trait::async_trait(?Send)]
impl NpmRegistryApi for TargetFolderCachedRegistryApi {
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    if let Some(data) = self.data.borrow_mut().get(name).cloned() {
      return Ok(data);
    }
    let file_path = packument_cache_filepath(name);
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
    std::fs::write(&file_path, &text).unwrap();
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
  name.replace("/", "%2F")
}

fn build_rt() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()
    .unwrap()
}

async fn run_resolver_and_get_snapshot(
  api: &impl NpmRegistryApi,
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
