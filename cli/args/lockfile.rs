// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_semver::npm::NpmPackageNv;
use deno_semver::npm::NpmPackageReq;

use crate::args::config_file::LockConfig;
use crate::args::ConfigFile;
use crate::npm::CliNpmRegistryApi;
use crate::Flags;

use super::DenoSubcommand;

pub use deno_lockfile::Lockfile;
pub use deno_lockfile::LockfileError;

pub fn discover(
  flags: &Flags,
  maybe_config_file: Option<&ConfigFile>,
) -> Result<Option<Lockfile>, AnyError> {
  if flags.no_lock
    || matches!(
      flags.subcommand,
      DenoSubcommand::Install(_) | DenoSubcommand::Uninstall(_)
    )
  {
    return Ok(None);
  }

  let filename = match flags.lock {
    Some(ref lock) => PathBuf::from(lock),
    None => match maybe_config_file {
      Some(config_file) => {
        if config_file.specifier.scheme() == "file" {
          match config_file.to_lock_config()? {
            Some(LockConfig::Bool(lock)) if !lock => {
              return Ok(None);
            }
            Some(LockConfig::PathBuf(lock)) => config_file
              .specifier
              .to_file_path()
              .unwrap()
              .parent()
              .unwrap()
              .join(lock),
            _ => {
              let mut path = config_file.specifier.to_file_path().unwrap();
              path.set_file_name("deno.lock");
              path
            }
          }
        } else {
          return Ok(None);
        }
      }
      None => return Ok(None),
    },
  };

  let lockfile = Lockfile::new(filename, flags.lock_write)?;
  Ok(Some(lockfile))
}

pub async fn snapshot_from_lockfile(
  lockfile: Arc<Mutex<Lockfile>>,
  api: &CliNpmRegistryApi,
) -> Result<ValidSerializedNpmResolutionSnapshot, AnyError> {
  let (root_packages, mut packages) = {
    let lockfile = lockfile.lock();

    let mut root_packages =
      HashMap::<NpmPackageReq, NpmPackageId>::with_capacity(
        lockfile.content.npm.specifiers.len(),
      );
    // collect the specifiers to version mappings
    for (key, value) in &lockfile.content.npm.specifiers {
      let package_req = NpmPackageReq::from_str(key)
        .with_context(|| format!("Unable to parse npm specifier: {key}"))?;
      let package_id = NpmPackageId::from_serialized(value)?;
      root_packages.insert(package_req, package_id.clone());
    }

    // now fill the packages except for the dist information
    let mut packages = Vec::with_capacity(lockfile.content.npm.packages.len());
    for (key, package) in &lockfile.content.npm.packages {
      let pkg_id = NpmPackageId::from_serialized(key)?;

      // collect the dependencies
      let mut dependencies = HashMap::with_capacity(package.dependencies.len());
      for (name, specifier) in &package.dependencies {
        let dep_id = NpmPackageId::from_serialized(specifier)?;
        dependencies.insert(name.clone(), dep_id);
      }

      packages.push(SerializedNpmResolutionSnapshotPackage {
        pkg_id,
        dependencies,
        optional: false,
        // temporarily empty
        os: Default::default(),
        cpu: Default::default(),
        dist: Default::default(),
      });
    }
    (root_packages, packages)
  };

  // now that the lockfile is dropped, fetch the package version information
  let pkg_nvs = packages
    .iter()
    .map(|p| p.pkg_id.nv.clone())
    .collect::<Vec<_>>();
  let get_version_infos = || {
    FuturesOrdered::from_iter(pkg_nvs.iter().map(|nv| async move {
      let package_info = api.package_info(&nv.name).await?;
      match package_info.version_info(nv) {
        Ok(version_info) => Ok(version_info),
        Err(err) => {
          bail!("Could not find '{}' specified in the lockfile.", err.0);
        }
      }
    }))
  };
  let mut version_infos = get_version_infos();
  let mut i = 0;
  let mut had_optional = false;
  while let Some(result) = version_infos.next().await {
    match result {
      Ok(version_info) => {
        let mut package = &mut packages[i];
        package.dist = version_info.dist;
        package.cpu = version_info.cpu;
        package.os = version_info.os;

        if !version_info.optional_dependencies.is_empty() {
          had_optional = true;
        }
      }
      Err(err) => {
        if api.mark_force_reload() {
          // reset and try again
          version_infos = get_version_infos();
          i = 0;
          continue;
        } else {
          return Err(err);
        }
      }
    }

    i += 1;
  }

  // only bother to do this if there were optional dependencies
  if had_optional {
    let optional_packages =
      find_optional_packages(&root_packages, &packages, api).await?;
    // mark the packages that we know are optional
    if !optional_packages.is_empty() {
      for package in &mut packages {
        package.optional = optional_packages.contains(&package.pkg_id.nv);
      }
    }
  }

  // clear the memory cache to reduce memory usage
  api.clear_memory_cache();

  SerializedNpmResolutionSnapshot {
    packages,
    root_packages,
  }
  .into_valid()
  .context("The lockfile is corrupt. You can recreate it with --lock-write")
}

async fn find_optional_packages<TRegistryApi: NpmRegistryApi>(
  root_packages: &HashMap<NpmPackageReq, NpmPackageId>,
  packages: &[SerializedNpmResolutionSnapshotPackage],
  api: &TRegistryApi,
) -> Result<HashSet<NpmPackageNv>, AnyError> {
  // go through the tree and mark all optional subsets as optional
  let mut pending = VecDeque::with_capacity(packages.len());
  let mut traversed_ids = HashSet::with_capacity(packages.len());
  let mut optional_packages = HashSet::with_capacity(packages.len());
  let mut required_packages = HashSet::with_capacity(packages.len());
  let pkg_ids_to_index = packages
    .iter()
    .map(|pkg| pkg.pkg_id.clone())
    .enumerate()
    .map(|(i, id)| (id, i))
    .collect::<HashMap<_, _>>();
  required_packages.extend(root_packages.values().map(|id| id.nv.clone()));
  pending.extend(root_packages.values().cloned());

  while let Some(pkg_id) = pending.pop_front() {
    let is_parent_required = required_packages.contains(&pkg_id.nv);
    // at this point, this should be cached in memory and fast
    let package_info = api.package_info(&pkg_id.nv.name).await?;
    let version_info = package_info.version_info(&pkg_id.nv).unwrap();
    let package = &packages[*pkg_ids_to_index.get(&pkg_id).unwrap()];
    for (specifier, child_id) in package.dependencies.clone() {
      let is_child_optional =
        version_info.optional_dependencies.contains_key(&specifier)
          && !required_packages.contains(&child_id.nv);
      let should_mark_required = is_parent_required && !is_child_optional;
      if should_mark_required && optional_packages.contains(&child_id.nv) {
        // A previously optional package is now found to be required. Revert
        // this dependency and its required descendant dependencies back to
        // being required because this name and version is no longer found in
        // an optional tree
        let mut pending = VecDeque::with_capacity(optional_packages.len());
        pending.push_back(child_id.clone());
        while let Some(pkg_id) = pending.pop_front() {
          if optional_packages.remove(&pkg_id.nv) {
            required_packages.insert(pkg_id.nv.clone());
            // should be cached in the api at this point
            let package_info = api.package_info(&pkg_id.nv.name).await?;
            let version_info = package_info
              .versions
              .get(&pkg_id.nv.version)
              .unwrap_or_else(|| panic!("missing: {:?}", pkg_id.nv));
            let package = &packages[*pkg_ids_to_index.get(&pkg_id).unwrap()];
            for key in version_info.dependencies.keys() {
              if !version_info.optional_dependencies.contains_key(key) {
                let dep_id = package.dependencies.get(key).unwrap();
                pending.push_back(dep_id.clone());
              }
            }
          }
        }
      }
      if traversed_ids.insert(child_id.clone()) {
        pending.push_back(child_id.clone());
        if should_mark_required {
          required_packages.insert(child_id.nv.clone());
        } else {
          optional_packages.insert(child_id.nv.clone());
        }
      }
    }
  }

  Ok(optional_packages)
}

#[cfg(test)]
mod test {
  use deno_npm::registry::TestNpmRegistryApi;
  use deno_npm::resolution::NpmResolutionSnapshot;
  use deno_npm::resolution::NpmResolutionSnapshotCreateOptions;

  use super::*;

  #[tokio::test]
  async fn test_find_optional_packages() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_optional_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_optional_dependency(("package-d", "1.0.0"), ("package-e", "1"));
    let optional_packages = run_find_optional_packages(
      vec![("package-a@1", "package-a@1.0.0")],
      &api,
    )
    .await;
    assert_eq!(
      optional_packages,
      vec![
        "package-c@1.0.0".to_string(),
        "package-d@1.0.0".to_string(),
        "package-e@1.0.0".to_string()
      ]
    );
  }

  #[tokio::test]
  async fn optional_to_required() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version("package-a", "1.0.0");
    api.ensure_package_version("package-b", "1.0.0");
    api.ensure_package_version("package-b2", "1.0.0");
    api.ensure_package_version("package-b3", "1.0.0");
    api.ensure_package_version("package-c", "1.0.0");
    api.ensure_package_version("package-d", "1.0.0");
    api.ensure_package_version("package-e", "1.0.0");
    api.add_dependency(("package-a", "1.0.0"), ("package-b", "1"));
    api.add_dependency(("package-b", "1.0.0"), ("package-b2", "1"));
    api.add_dependency(("package-b2", "1.0.0"), ("package-b3", "1"));
    // deep down this is set back to being required, so it and its required
    // dependency should be marked as required
    api.add_dependency(("package-b3", "1.0.0"), ("package-c", "1"));
    api.add_optional_dependency(("package-a", "1.0.0"), ("package-c", "1"));
    api.add_dependency(("package-c", "1.0.0"), ("package-d", "1"));
    api.add_optional_dependency(("package-d", "1.0.0"), ("package-e", "1"));

    let optional_packages = run_find_optional_packages(
      vec![("package-a@1", "package-a@1.0.0")],
      &api,
    )
    .await;
    assert_eq!(optional_packages, vec!["package-e@1.0.0".to_string()],);
  }

  async fn run_find_optional_packages(
    root_packages: Vec<(&str, &str)>,
    api: &TestNpmRegistryApi,
  ) -> Vec<String> {
    let root_packages = root_packages
      .into_iter()
      .map(|(req, id)| {
        (
          NpmPackageReq::from_str(req).unwrap(),
          NpmPackageId::from_serialized(id).unwrap(),
        )
      })
      .collect::<HashMap<_, _>>();

    let snapshot =
      NpmResolutionSnapshot::new(NpmResolutionSnapshotCreateOptions {
        api: Arc::new(api.clone()),
        snapshot: Default::default(),
        types_node_version_req: None,
      });
    let snapshot = snapshot
      .resolve_pending({
        let mut reqs = root_packages.keys().cloned().collect::<Vec<_>>();
        reqs.sort();
        reqs
      })
      .await
      .unwrap();
    let mut serialized = snapshot.as_serialized();
    for package in &mut serialized.packages {
      package.optional = false;
    }

    let mut optional =
      find_optional_packages(&root_packages, &serialized.packages, api)
        .await
        .unwrap()
        .into_iter()
        .map(|nv| nv.to_string())
        .collect::<Vec<_>>();
    optional.sort();
    optional
  }
}
