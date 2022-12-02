// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;

use crate::args::Lockfile;
use crate::npm::cache::should_sync_download;
use crate::npm::cache::NpmPackageCacheFolderId;
use crate::npm::registry::NpmPackageVersionDistInfo;
use crate::npm::registry::NpmRegistryApi;
use crate::npm::registry::RealNpmRegistryApi;

use super::NpmPackageId;
use super::NpmPackageReq;
use super::NpmResolutionPackage;
use super::NpmVersionMatcher;

/// Packages partitioned by if they are "copy" packages or not.
pub struct NpmPackagesPartitioned {
  pub packages: Vec<NpmResolutionPackage>,
  /// Since peer dependency resolution occurs based on ancestors and ancestor
  /// siblings, this may sometimes cause the same package (name and version)
  /// to have different dependencies based on where it appears in the tree.
  /// For these packages, we create a "copy package" or duplicate of the package
  /// whose dependencies are that of where in the tree they've resolved to.
  pub copy_packages: Vec<NpmResolutionPackage>,
}

impl NpmPackagesPartitioned {
  pub fn into_all(self) -> Vec<NpmResolutionPackage> {
    let mut packages = self.packages;
    packages.extend(self.copy_packages);
    packages
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NpmResolutionSnapshot {
  #[serde(with = "map_to_vec")]
  pub(super) package_reqs: HashMap<NpmPackageReq, NpmPackageId>,
  pub(super) packages_by_name: HashMap<String, Vec<NpmPackageId>>,
  #[serde(with = "map_to_vec")]
  pub(super) packages: HashMap<NpmPackageId, NpmResolutionPackage>,
}

// This is done so the maps with non-string keys get serialized and deserialized as vectors.
// Adapted from: https://github.com/serde-rs/serde/issues/936#issuecomment-302281792
mod map_to_vec {
  use std::collections::HashMap;

  use serde::de::Deserialize;
  use serde::de::Deserializer;
  use serde::ser::Serializer;
  use serde::Serialize;

  pub fn serialize<S, K: Serialize, V: Serialize>(
    map: &HashMap<K, V>,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.collect_seq(map.iter())
  }

  pub fn deserialize<
    'de,
    D,
    K: Deserialize<'de> + Eq + std::hash::Hash,
    V: Deserialize<'de>,
  >(
    deserializer: D,
  ) -> Result<HashMap<K, V>, D::Error>
  where
    D: Deserializer<'de>,
  {
    let mut map = HashMap::new();
    for (key, value) in Vec::<(K, V)>::deserialize(deserializer)? {
      map.insert(key, value);
    }
    Ok(map)
  }
}

impl NpmResolutionSnapshot {
  /// Resolve a node package from a deno module.
  pub fn resolve_package_from_deno_module(
    &self,
    req: &NpmPackageReq,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    match self.package_reqs.get(req) {
      Some(id) => Ok(self.packages.get(id).unwrap()),
      None => bail!("could not find npm package directory for '{}'", req),
    }
  }

  pub fn top_level_packages(&self) -> Vec<NpmPackageId> {
    self
      .package_reqs
      .values()
      .cloned()
      .collect::<HashSet<_>>()
      .into_iter()
      .collect::<Vec<_>>()
  }

  pub fn package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<&NpmResolutionPackage> {
    self.packages.get(id)
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageCacheFolderId,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    // todo(dsherret): do we need an additional hashmap to get this quickly?
    let referrer_package = self
      .packages_by_name
      .get(&referrer.name)
      .and_then(|packages| {
        packages
          .iter()
          .filter(|p| p.version == referrer.version)
          .filter_map(|id| {
            let package = self.packages.get(id)?;
            if package.copy_index == referrer.copy_index {
              Some(package)
            } else {
              None
            }
          })
          .next()
      })
      .ok_or_else(|| {
        anyhow!("could not find referrer npm package '{}'", referrer)
      })?;

    let name = name_without_path(name);
    if let Some(id) = referrer_package.dependencies.get(name) {
      return Ok(self.packages.get(id).unwrap());
    }

    if referrer_package.id.name == name {
      return Ok(referrer_package);
    }

    // TODO(bartlomieju): this should use a reverse lookup table in the
    // snapshot instead of resolving best version again.
    let req = NpmPackageReq {
      name: name.to_string(),
      version_req: None,
    };

    if let Some(id) = self.resolve_best_package_id(name, &req) {
      if let Some(pkg) = self.packages.get(&id) {
        return Ok(pkg);
      }
    }

    bail!(
      "could not find npm package '{}' referenced by '{}'",
      name,
      referrer
    )
  }

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.packages.values().cloned().collect()
  }

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    let mut packages = self.all_packages();
    let mut copy_packages = Vec::with_capacity(packages.len() / 2); // at most 1 copy for every package

    // partition out any packages that are "copy" packages
    for i in (0..packages.len()).rev() {
      if packages[i].copy_index > 0 {
        copy_packages.push(packages.swap_remove(i));
      }
    }

    NpmPackagesPartitioned {
      packages,
      copy_packages,
    }
  }

  pub fn resolve_best_package_id(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Option<NpmPackageId> {
    // todo(dsherret): this is not exactly correct because some ids
    // will be better than others due to peer dependencies
    let mut maybe_best_id: Option<&NpmPackageId> = None;
    if let Some(ids) = self.packages_by_name.get(name) {
      for id in ids {
        if version_matcher.matches(&id.version) {
          let is_best_version = maybe_best_id
            .as_ref()
            .map(|best_id| best_id.version.cmp(&id.version).is_lt())
            .unwrap_or(true);
          if is_best_version {
            maybe_best_id = Some(id);
          }
        }
      }
    }
    maybe_best_id.cloned()
  }

  pub async fn from_lockfile(
    lockfile: Arc<Mutex<Lockfile>>,
    api: &RealNpmRegistryApi,
  ) -> Result<Self, AnyError> {
    let mut package_reqs: HashMap<NpmPackageReq, NpmPackageId>;
    let mut packages_by_name: HashMap<String, Vec<NpmPackageId>>;
    let mut packages: HashMap<NpmPackageId, NpmResolutionPackage>;
    let mut copy_index_resolver: SnapshotPackageCopyIndexResolver;

    {
      let lockfile = lockfile.lock();

      // pre-allocate collections
      package_reqs =
        HashMap::with_capacity(lockfile.content.npm.specifiers.len());
      let packages_len = lockfile.content.npm.packages.len();
      packages = HashMap::with_capacity(packages_len);
      packages_by_name = HashMap::with_capacity(packages_len); // close enough
      copy_index_resolver =
        SnapshotPackageCopyIndexResolver::with_capacity(packages_len);
      let mut verify_ids = HashSet::with_capacity(packages_len);

      // collect the specifiers to version mappings
      for (key, value) in &lockfile.content.npm.specifiers {
        let package_req = NpmPackageReq::from_str(key)
          .with_context(|| format!("Unable to parse npm specifier: {}", key))?;
        let package_id = NpmPackageId::from_serialized(value)?;
        package_reqs.insert(package_req, package_id.clone());
        verify_ids.insert(package_id.clone());
      }

      // then the packages
      for (key, value) in &lockfile.content.npm.packages {
        let package_id = NpmPackageId::from_serialized(key)?;

        // collect the dependencies
        let mut dependencies = HashMap::default();

        packages_by_name
          .entry(package_id.name.to_string())
          .or_default()
          .push(package_id.clone());

        for (name, specifier) in &value.dependencies {
          let dep_id = NpmPackageId::from_serialized(specifier)?;
          dependencies.insert(name.to_string(), dep_id.clone());
          verify_ids.insert(dep_id);
        }

        let package = NpmResolutionPackage {
          id: package_id.clone(),
          copy_index: copy_index_resolver.resolve(&package_id),
          // temporary dummy value
          dist: NpmPackageVersionDistInfo {
            tarball: "foobar".to_string(),
            shasum: "foobar".to_string(),
            integrity: Some("foobar".to_string()),
          },
          dependencies,
        };

        packages.insert(package_id, package);
      }

      // verify that all these ids exist in packages
      for id in &verify_ids {
        if !packages.contains_key(id) {
          bail!(
            "the lockfile is corrupt. You can recreate it with --lock-write"
          );
        }
      }
    }

    let mut unresolved_tasks = Vec::with_capacity(packages_by_name.len());

    // cache the package names in parallel in the registry api
    // unless synchronous download should occur
    if should_sync_download() {
      let mut package_names = packages_by_name.keys().collect::<Vec<_>>();
      package_names.sort();
      for package_name in package_names {
        api.package_info(package_name).await?;
      }
    } else {
      for package_name in packages_by_name.keys() {
        let package_name = package_name.clone();
        let api = api.clone();
        unresolved_tasks.push(tokio::task::spawn(async move {
          api.package_info(&package_name).await?;
          Result::<_, AnyError>::Ok(())
        }));
      }
    }
    for result in futures::future::join_all(unresolved_tasks).await {
      result??;
    }

    // ensure the dist is set for each package
    for package in packages.values_mut() {
      // this will read from the memory cache now
      let version_info = match api
        .package_version_info(&package.id.name, &package.id.version)
        .await?
      {
        Some(version_info) => version_info,
        None => {
          bail!("could not find '{}' specified in the lockfile. Maybe try again with --reload", package.id.display());
        }
      };
      package.dist = version_info.dist;
    }

    Ok(Self {
      package_reqs,
      packages_by_name,
      packages,
    })
  }
}

pub struct SnapshotPackageCopyIndexResolver {
  packages_to_copy_index: HashMap<NpmPackageId, usize>,
  package_name_version_to_copy_count: HashMap<(String, String), usize>,
}

impl SnapshotPackageCopyIndexResolver {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      packages_to_copy_index: HashMap::with_capacity(capacity),
      package_name_version_to_copy_count: HashMap::with_capacity(capacity), // close enough
    }
  }

  pub fn from_map_with_capacity(
    mut packages_to_copy_index: HashMap<NpmPackageId, usize>,
    capacity: usize,
  ) -> Self {
    let mut package_name_version_to_copy_count =
      HashMap::with_capacity(capacity); // close enough
    if capacity > packages_to_copy_index.len() {
      packages_to_copy_index.reserve(capacity - packages_to_copy_index.len());
    }

    for (id, index) in &packages_to_copy_index {
      let entry = package_name_version_to_copy_count
        .entry((id.name.to_string(), id.version.to_string()))
        .or_insert(0);
      if *entry < *index {
        *entry = *index;
      }
    }
    Self {
      packages_to_copy_index,
      package_name_version_to_copy_count,
    }
  }

  pub fn resolve(&mut self, id: &NpmPackageId) -> usize {
    if let Some(index) = self.packages_to_copy_index.get(id) {
      *index
    } else {
      let index = *self
        .package_name_version_to_copy_count
        .entry((id.name.to_string(), id.version.to_string()))
        .and_modify(|count| {
          *count += 1;
        })
        .or_insert(0);
      self.packages_to_copy_index.insert(id.clone(), index);
      index
    }
  }
}

fn name_without_path(name: &str) -> &str {
  let mut search_start_index = 0;
  if name.starts_with('@') {
    if let Some(slash_index) = name.find('/') {
      search_start_index = slash_index + 1;
    }
  }
  if let Some(slash_index) = &name[search_start_index..].find('/') {
    // get the name up until the path slash
    &name[0..search_start_index + slash_index]
  } else {
    name
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_name_without_path() {
    assert_eq!(name_without_path("foo"), "foo");
    assert_eq!(name_without_path("@foo/bar"), "@foo/bar");
    assert_eq!(name_without_path("@foo/bar/baz"), "@foo/bar");
    assert_eq!(name_without_path("@hello"), "@hello");
  }

  #[test]
  fn test_copy_index_resolver() {
    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::with_capacity(10);
    assert_eq!(
      copy_index_resolver
        .resolve(&NpmPackageId::from_serialized("package@1.0.0").unwrap()),
      0
    );
    assert_eq!(
      copy_index_resolver
        .resolve(&NpmPackageId::from_serialized("package@1.0.0").unwrap()),
      0
    );
    assert_eq!(
      copy_index_resolver.resolve(
        &NpmPackageId::from_serialized("package@1.0.0_package-b@1.0.0")
          .unwrap()
      ),
      1
    );
    assert_eq!(
      copy_index_resolver.resolve(
        &NpmPackageId::from_serialized(
          "package@1.0.0_package-b@1.0.0__package-c@2.0.0"
        )
        .unwrap()
      ),
      2
    );
    assert_eq!(
      copy_index_resolver.resolve(
        &NpmPackageId::from_serialized("package@1.0.0_package-b@1.0.0")
          .unwrap()
      ),
      1
    );
    assert_eq!(
      copy_index_resolver
        .resolve(&NpmPackageId::from_serialized("package-b@1.0.0").unwrap()),
      0
    );
  }
}
