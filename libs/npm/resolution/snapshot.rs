// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map;
use std::sync::Arc;

use deno_error::JsError;
use deno_lockfile::Lockfile;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use futures::StreamExt;
use futures::stream::FuturesOrdered;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use super::NpmPackageVersionNotFound;
use super::UnmetPeerDepDiagnostic;
use super::common::NpmVersionResolver;
use super::graph::Graph;
use super::graph::GraphDependencyResolver;
use super::graph::NpmResolutionError;
use crate::NpmPackageCacheFolderId;
use crate::NpmPackageExtraInfo;
use crate::NpmPackageId;
use crate::NpmPackageIdDeserializationError;
use crate::NpmResolutionPackage;
use crate::NpmResolutionPackageSystemInfo;
use crate::NpmSystemInfo;
use crate::registry::NpmPackageInfo;
use crate::registry::NpmPackageVersionDistInfo;
use crate::registry::NpmPackageVersionInfo;
use crate::registry::NpmRegistryApi;
use crate::registry::NpmRegistryPackageInfoLoadError;
use crate::resolution::Reporter;
use crate::resolution::graph::GraphDependencyResolverOptions;

#[derive(Debug, Error, Clone, JsError)]
#[class(type)]
#[error("Could not find '{}' in the list of packages.", self.0.as_serialized())]
pub struct PackageIdNotFoundError(pub NpmPackageId);

#[derive(Debug, Error, Clone, JsError)]
#[class(type)]
#[error("Could not find constraint '{0}' in the list of packages.")]
pub struct PackageReqNotFoundError(pub PackageReq);

#[derive(Debug, Error, Clone, JsError)]
#[class(type)]
#[error("Could not find '{0}' in the list of packages.")]
pub struct PackageNvNotFoundError(pub PackageNv);

#[derive(Debug, Error, Clone, JsError)]
#[class(type)]
#[error("Could not find package folder id '{0}' in the list of packages.")]
pub struct PackageCacheFolderIdNotFoundError(pub NpmPackageCacheFolderId);

#[derive(Debug, Error, Clone, JsError)]
#[class(type)]
pub enum PackageNotFoundFromReferrerError {
  #[error("Could not find referrer npm package '{0}'.")]
  Referrer(NpmPackageCacheFolderId),
  #[error("Could not find npm package '{name}' referenced by '{referrer}'.")]
  Package {
    name: String,
    referrer: NpmPackageCacheFolderId,
  },
}

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
  pub fn iter_all(&self) -> impl Iterator<Item = &NpmResolutionPackage> {
    self.packages.iter().chain(self.copy_packages.iter())
  }
}

/// A serialized snapshot that has been verified to be non-corrupt
/// and valid.
#[derive(Debug, Default, Clone)]
pub struct ValidSerializedNpmResolutionSnapshot(
  // keep private -- once verified the caller
  // shouldn't be able to modify it
  SerializedNpmResolutionSnapshot,
);

impl ValidSerializedNpmResolutionSnapshot {
  pub fn as_serialized(&self) -> &SerializedNpmResolutionSnapshot {
    &self.0
  }

  pub fn into_serialized(self) -> SerializedNpmResolutionSnapshot {
    self.0
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializedNpmResolutionSnapshotPackage {
  pub id: NpmPackageId,
  #[serde(flatten)]
  pub system: NpmResolutionPackageSystemInfo,
  pub dist: Option<NpmPackageVersionDistInfo>,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<StackString, NpmPackageId>,
  pub optional_dependencies: HashSet<StackString>,
  pub optional_peer_dependencies: HashSet<StackString>,
  #[serde(flatten)]
  pub extra: Option<NpmPackageExtraInfo>,
  pub is_deprecated: bool,
  pub has_bin: bool,
  pub has_scripts: bool,
}

#[derive(Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializedNpmResolutionSnapshot {
  /// Resolved npm specifiers to package id mappings.
  pub root_packages: HashMap<PackageReq, NpmPackageId>,
  /// Collection of resolved packages in the dependency graph.
  pub packages: Vec<SerializedNpmResolutionSnapshotPackage>,
}

impl SerializedNpmResolutionSnapshot {
  /// Marks the serialized snapshot as valid, if able.
  ///
  /// Snapshots from serialized sources might be invalid due to tampering
  /// by the user. For example, this could be populated from a lockfile
  /// that the user modified.
  pub fn into_valid(
    self,
  ) -> Result<ValidSerializedNpmResolutionSnapshot, PackageIdNotFoundError> {
    let mut verify_ids = HashSet::with_capacity(self.packages.len());

    // collect the specifiers to version mappings
    verify_ids.extend(self.root_packages.values());

    // then the packages
    let mut package_ids = HashSet::with_capacity(self.packages.len());
    for package in &self.packages {
      package_ids.insert(&package.id);
      verify_ids.extend(package.dependencies.values());
    }

    // verify that all these ids exist in packages
    for id in verify_ids {
      if !package_ids.contains(&id) {
        return Err(PackageIdNotFoundError(id.clone()));
      }
    }

    Ok(ValidSerializedNpmResolutionSnapshot(self))
  }

  /// Trusts that the serialized snapshot is valid and skips runtime verification
  /// that is done in `into_valid`.
  ///
  /// Note: It will still do the verification in debug.
  pub fn into_valid_unsafe(self) -> ValidSerializedNpmResolutionSnapshot {
    if cfg!(debug_assertions) {
      self.into_valid().unwrap()
    } else {
      ValidSerializedNpmResolutionSnapshot(self)
    }
  }
}

impl std::fmt::Debug for SerializedNpmResolutionSnapshot {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // do a custom debug implementation that creates deterministic output for the tests
    f.debug_struct("SerializedNpmResolutionSnapshot")
      .field(
        "root_packages",
        &self.root_packages.iter().collect::<BTreeMap<_, _>>(),
      )
      .field("packages", &self.packages)
      .finish()
  }
}

#[derive(Debug, Clone)]
pub struct AddPkgReqsOptions<'a> {
  pub package_reqs: &'a [PackageReq],
  pub version_resolver: &'a NpmVersionResolver,
  /// If a deduplication pass should be done on the final graph.
  ///
  /// This should never be done after code execution occurs (ex. this
  /// should NOT be done when resolving a dynamically resolved npm dependency),
  /// but is recommended for most other actions as it will create a smaller npm
  /// dependency graph.
  pub should_dedup: bool,
}

#[derive(Debug)]
pub struct AddPkgReqsResult {
  /// Results from adding the individual packages.
  ///
  /// The indexes of the results correspond to the indexes of the provided
  /// package requirements.
  pub results: Vec<Result<PackageNv, NpmResolutionError>>,
  /// The result of resolving the entire dependency graph after the initial
  /// reqs were resolved to nvs.
  ///
  /// If a resolution error occurs, this will contain the first error.
  pub dep_graph_result: Result<NpmResolutionSnapshot, NpmResolutionError>,
  /// Diagnostics that were found during resolution. These should be
  /// displayed as warnings.
  pub unmet_peer_diagnostics: Vec<UnmetPeerDepDiagnostic>,
}

impl AddPkgReqsResult {
  pub fn into_result(
    self,
  ) -> Result<NpmResolutionSnapshot, NpmResolutionError> {
    self.dep_graph_result
  }
}

#[derive(Debug, Default, Clone)]
pub struct NpmResolutionSnapshot {
  /// The unique package requirements map to a single npm package name and version.
  pub(super) package_reqs: HashMap<PackageReq, PackageNv>,
  // Each root level npm package name and version maps to an exact npm package node id.
  pub(super) root_packages: HashMap<PackageNv, NpmPackageId>,
  pub(super) packages_by_name: HashMap<StackString, Vec<NpmPackageId>>,
  pub(super) packages: HashMap<NpmPackageId, NpmResolutionPackage>,
}

impl NpmResolutionSnapshot {
  pub fn new(snapshot: ValidSerializedNpmResolutionSnapshot) -> Self {
    let snapshot = snapshot.0;
    let mut package_reqs = HashMap::<PackageReq, PackageNv>::with_capacity(
      snapshot.root_packages.len(),
    );
    let mut root_packages = HashMap::<PackageNv, NpmPackageId>::with_capacity(
      snapshot.root_packages.len(),
    );
    let mut packages_by_name =
      HashMap::<StackString, Vec<NpmPackageId>>::with_capacity(
        snapshot.packages.len(),
      ); // close enough
    let mut packages =
      HashMap::<NpmPackageId, NpmResolutionPackage>::with_capacity(
        snapshot.packages.len(),
      );
    let mut copy_index_resolver =
      SnapshotPackageCopyIndexResolver::with_capacity(snapshot.packages.len());

    // collect the specifiers to version mappings
    for (req, id) in snapshot.root_packages {
      package_reqs.insert(req, id.nv.clone());
      root_packages.insert(id.nv.clone(), id.clone());
    }

    // then the packages
    for package in snapshot.packages {
      packages_by_name
        .entry(package.id.nv.name.clone())
        .or_default()
        .push(package.id.clone());

      let copy_index = copy_index_resolver.resolve(&package.id);
      packages.insert(
        package.id.clone(),
        NpmResolutionPackage {
          id: package.id,
          copy_index,
          system: package.system,
          dependencies: package.dependencies,
          optional_dependencies: package.optional_dependencies,
          optional_peer_dependencies: package.optional_peer_dependencies,
          dist: package.dist,
          extra: package.extra,
          is_deprecated: package.is_deprecated,
          has_bin: package.has_bin,
          has_scripts: package.has_scripts,
        },
      );
    }

    Self {
      package_reqs,
      root_packages,
      packages_by_name,
      packages,
    }
  }

  /// Resolves the provided package requirements adding them to the snapshot.
  pub async fn add_pkg_reqs(
    self,
    api: &impl NpmRegistryApi,
    options: AddPkgReqsOptions<'_>,
    reporter: Option<&dyn Reporter>,
  ) -> AddPkgReqsResult {
    enum InfoOrNv {
      InfoResult(Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError>),
      Nv(PackageNv),
    }
    // convert the snapshot to a traversable graph
    let mut graph = Graph::from_snapshot(self);

    let reqs_with_in_graph = options
      .package_reqs
      .iter()
      .map(|req| (req, graph.get_req_nv(req).map(|r| r.as_ref().clone())));
    let mut top_level_packages = FuturesOrdered::from_iter({
      reqs_with_in_graph.map(|(req, maybe_nv)| async move {
        let maybe_info = if let Some(nv) = maybe_nv {
          InfoOrNv::Nv(nv)
        } else {
          InfoOrNv::InfoResult(api.package_info(&req.name).await)
        };
        (req, maybe_info)
      })
    });

    // go over the top level package names first (npm package reqs and pending unresolved),
    // then down the tree one level at a time through all the branches
    let mut resolver = GraphDependencyResolver::new(
      &mut graph,
      api,
      options.version_resolver,
      reporter,
      GraphDependencyResolverOptions {
        should_dedup: options.should_dedup,
      },
    );

    // The package reqs and ids should already be sorted
    // in the order they should be resolved in.
    let mut results = Vec::with_capacity(options.package_reqs.len());
    let mut first_resolution_error = None;
    while let Some(result) = top_level_packages.next().await {
      let (req, info_or_nv) = result;
      match info_or_nv {
        InfoOrNv::InfoResult(info_result) => {
          match info_result
            .map_err(|err| err.into())
            .and_then(|info| resolver.add_package_req(req, &info))
          {
            Ok(nv) => {
              results.push(Ok(nv.as_ref().clone()));
            }
            Err(err) => {
              if first_resolution_error.is_none() {
                first_resolution_error = Some(err.clone());
              }
              results.push(Err(err));
            }
          }
        }
        InfoOrNv::Nv(nv) => {
          results.push(Ok(nv));
        }
      }
    }
    drop(top_level_packages); // stop borrow of api
    let mut unmet_peer_diagnostics = Vec::new();

    let dep_graph_result = match first_resolution_error {
      Some(err) => Err(err),
      None => match resolver.resolve_pending().await {
        Ok(()) => {
          unmet_peer_diagnostics = resolver.take_unmet_peer_diagnostics();
          graph
            .into_snapshot(api, &options.version_resolver.link_packages)
            .await
        }
        Err(err) => Err(err),
      },
    };

    AddPkgReqsResult {
      results,
      dep_graph_result,
      unmet_peer_diagnostics,
    }
  }

  /// Returns a new snapshot made from a subset of this snapshot's package reqs.
  /// Requirements not present in this snapshot will be ignored.
  pub fn subset(&self, package_reqs: &[PackageReq]) -> Self {
    let mut new_package_reqs = HashMap::with_capacity(package_reqs.len());
    let mut packages = HashMap::with_capacity(package_reqs.len() * 2);
    let mut packages_by_name: HashMap<StackString, Vec<NpmPackageId>> =
      HashMap::with_capacity(package_reqs.len());
    let mut root_packages = HashMap::with_capacity(package_reqs.len());

    let mut visited = HashSet::with_capacity(packages.len());

    let mut stack = Vec::new();
    for req in package_reqs {
      let Some(nv) = self.package_reqs.get(req) else {
        continue;
      };
      let Some(id) = self.root_packages.get(nv) else {
        continue;
      };
      new_package_reqs.insert(req.clone(), nv.clone());
      root_packages.insert(nv.clone(), id.clone());
      visited.insert(id);
      stack.push(id);
    }

    while let Some(id) = stack.pop() {
      let Some(package) = self.package_from_id(id) else {
        continue;
      };
      packages_by_name
        .entry(package.id.nv.name.clone())
        .or_default()
        .push(package.id.clone());
      let Some(package) = self.package_from_id(id) else {
        continue;
      };
      packages.insert(id.clone(), package.clone());
      for dep in package.dependencies.values() {
        if visited.insert(dep) {
          stack.push(dep);
        }
      }
    }

    Self {
      package_reqs: new_package_reqs,
      packages,
      packages_by_name,
      root_packages,
    }
  }

  /// Gets the snapshot as a valid serialized snapshot.
  pub fn as_valid_serialized(&self) -> ValidSerializedNpmResolutionSnapshot {
    ValidSerializedNpmResolutionSnapshot(SerializedNpmResolutionSnapshot {
      root_packages: self
        .package_reqs
        .iter()
        .map(|(req, nv)| {
          let id = self.root_packages.get(nv).unwrap();
          (req.clone(), id.clone())
        })
        .collect(),
      packages: self
        .packages
        .values()
        .map(|package| package.as_serialized())
        .collect(),
    })
  }

  /// Filters out any optional dependencies that don't match for the
  /// given system. The resulting valid serialized snapshot will then not
  /// have any optional dependencies that don't match the given system.
  pub fn as_valid_serialized_for_system(
    &self,
    system_info: &NpmSystemInfo,
  ) -> ValidSerializedNpmResolutionSnapshot {
    let mut final_packages = Vec::with_capacity(self.packages.len());
    let mut pending = VecDeque::with_capacity(self.packages.len());
    let mut visited_nvs = HashSet::with_capacity(self.packages.len());

    // add the root packages
    for pkg_id in self.root_packages.values() {
      if visited_nvs.insert(&pkg_id.nv) {
        pending.push_back(&pkg_id.nv);
      }
    }

    while let Some(nv) = pending.pop_front() {
      for id in self.package_ids_for_nv(nv) {
        let pkg = self.packages.get(id).unwrap();
        let mut new_pkg = SerializedNpmResolutionSnapshotPackage {
          id: pkg.id.clone(),
          dependencies: HashMap::with_capacity(pkg.dependencies.len()),
          optional_peer_dependencies: pkg.optional_peer_dependencies.clone(),
          // the fields below are stripped from the output
          system: Default::default(),
          optional_dependencies: Default::default(),
          extra: pkg.extra.clone(),
          dist: pkg.dist.clone(),
          is_deprecated: pkg.is_deprecated,
          has_bin: pkg.has_bin,
          has_scripts: pkg.has_scripts,
        };
        for (key, dep_id) in &pkg.dependencies {
          let dep = self.packages.get(dep_id).unwrap();

          let matches_system = !pkg.optional_dependencies.contains(key)
            || dep.system.matches_system(system_info);
          if matches_system {
            new_pkg.dependencies.insert(key.clone(), dep_id.clone());
            if visited_nvs.insert(&dep_id.nv) {
              pending.push_back(&dep_id.nv);
            }
          }
        }
        final_packages.push(new_pkg);
      }
    }

    ValidSerializedNpmResolutionSnapshot(SerializedNpmResolutionSnapshot {
      packages: final_packages,
      // the root packages are always included since they're
      // what the user imports
      root_packages: self
        .package_reqs
        .iter()
        .map(|(req, nv)| {
          let id = self.root_packages.get(nv).unwrap();
          (req.clone(), id.clone())
        })
        .collect(),
    })
  }

  /// Gets if this snapshot is empty.
  pub fn is_empty(&self) -> bool {
    self.packages.is_empty()
  }

  /// Converts the snapshot into an empty snapshot.
  pub fn into_empty(self) -> Self {
    // this is `into_empty()` instead of something like `clear()` in order
    // to reduce the chance of a mistake forgetting to clear a collection
    Self {
      package_reqs: Default::default(),
      root_packages: Default::default(),
      packages_by_name: Default::default(),
      packages: Default::default(),
    }
  }

  /// Resolve a package from a package requirement.
  pub fn resolve_pkg_from_pkg_req(
    &self,
    req: &PackageReq,
  ) -> Result<&NpmResolutionPackage, PackageReqNotFoundError> {
    let package_nv = self.package_reqs.get(req).or_else(|| {
      // fallback to iterating over the versions
      req
        .version_req
        .tag()
        .is_none()
        .then(|| self.packages_by_name.get(&req.name))
        .flatten()
        .and_then(|ids| {
          ids
            .iter()
            .filter(|id| req.version_req.matches(&id.nv.version))
            .map(|id| &id.nv)
            .max_by_key(|nv| &nv.version)
        })
    });
    match package_nv {
      Some(nv) => self
        .resolve_package_from_deno_module(nv)
        // ignore the nv not found error and return a req not found
        .map_err(|_| PackageReqNotFoundError(req.clone())),
      None => Err(PackageReqNotFoundError(req.clone())),
    }
  }

  /// Resolve a package from a package cache folder id.
  pub fn resolve_pkg_from_pkg_cache_folder_id(
    &self,
    pkg_cache_folder_id: &NpmPackageCacheFolderId,
  ) -> Result<&NpmResolutionPackage, PackageCacheFolderIdNotFoundError> {
    self
      .packages_by_name
      .get(&pkg_cache_folder_id.nv.name)
      .and_then(|ids| {
        for id in ids {
          if id.nv == pkg_cache_folder_id.nv
            && let Some(pkg) = self.packages.get(id)
            && pkg.copy_index == pkg_cache_folder_id.copy_index
          {
            return Some(pkg);
          }
        }
        None
      })
      .map(Ok)
      .unwrap_or_else(|| {
        Err(PackageCacheFolderIdNotFoundError(
          pkg_cache_folder_id.clone(),
        ))
      })
  }

  /// Resolve a package id from a deno module.
  pub fn resolve_package_id_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<&NpmPackageId, PackageNvNotFoundError> {
    match self.root_packages.get(nv) {
      Some(id) => Ok(id),
      None => Err(PackageNvNotFoundError(nv.clone())),
    }
  }

  /// Resolve a package from a deno module.
  pub fn resolve_package_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<&NpmResolutionPackage, PackageNvNotFoundError> {
    self
      .resolve_package_id_from_deno_module(nv)
      .map(|id| self.packages.get(id).unwrap())
  }

  pub fn top_level_packages(
    &self,
  ) -> hash_map::Values<'_, PackageNv, NpmPackageId> {
    self.root_packages.values()
  }

  pub fn package_reqs(&self) -> &HashMap<PackageReq, PackageNv> {
    &self.package_reqs
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
  ) -> Result<&NpmResolutionPackage, Box<PackageNotFoundFromReferrerError>> {
    // todo(dsherret): do we need an additional hashmap to get this quickly?
    let referrer_package = self
      .packages_by_name
      .get(&referrer.nv.name)
      .and_then(|packages| {
        packages
          .iter()
          .filter(|p| p.nv.version == referrer.nv.version)
          .filter_map(|node_id| {
            let package = self.packages.get(node_id)?;
            if package.copy_index == referrer.copy_index {
              Some(package)
            } else {
              None
            }
          })
          .next()
      })
      .ok_or_else(|| {
        Box::new(PackageNotFoundFromReferrerError::Referrer(referrer.clone()))
      })?;

    let name = name_without_path(name);
    if let Some(id) = referrer_package.dependencies.get(name) {
      return Ok(self.packages.get(id).unwrap());
    }

    if referrer_package.id.nv.name == name {
      return Ok(referrer_package);
    }

    // TODO(bartlomieju): this should use a reverse lookup table in the
    // snapshot instead of resolving best version again.
    let any_version_req = VersionReq::parse_from_npm("*").unwrap();
    if let Some(id) = self.resolve_best_package_id(name, &any_version_req)
      && let Some(pkg) = self.packages.get(&id)
    {
      return Ok(pkg);
    }

    Err(Box::new(PackageNotFoundFromReferrerError::Package {
      name: name.to_string(),
      referrer: referrer.clone(),
    }))
  }

  /// Gets all the packages found in the snapshot regardless of
  /// whether they are supported on the current system.
  pub fn all_packages_for_every_system(
    &self,
  ) -> impl Iterator<Item = &NpmResolutionPackage> {
    // NOTE: This method intentionally has a verbose name
    // to discourage its use.
    self.packages.values()
  }

  pub fn all_system_packages(
    &self,
    system_info: &NpmSystemInfo,
  ) -> Vec<NpmResolutionPackage> {
    let mut packages = Vec::with_capacity(self.packages.len());
    let mut pending = VecDeque::with_capacity(self.packages.len());
    let mut visited_nvs = HashSet::with_capacity(self.packages.len());

    for pkg_id in self.root_packages.values() {
      if visited_nvs.insert(&pkg_id.nv) {
        pending.push_back(&pkg_id.nv);
      }
    }

    while let Some(nv) = pending.pop_front() {
      for pkg_id in self.package_ids_for_nv(nv) {
        let pkg = self.packages.get(pkg_id).unwrap();
        packages.push(pkg.clone());

        for (key, dep_id) in &pkg.dependencies {
          let dep = self.packages.get(dep_id).unwrap();

          let matches_system = !pkg.optional_dependencies.contains(key)
            || dep.system.matches_system(system_info);
          if matches_system && visited_nvs.insert(&dep_id.nv) {
            pending.push_back(&dep.id.nv);
          }
        }
      }
    }

    packages
  }

  pub fn all_system_packages_partitioned(
    &self,
    system_info: &NpmSystemInfo,
  ) -> NpmPackagesPartitioned {
    let mut packages = self.all_system_packages(system_info);

    // in most scenarios, there won't ever be any copy packages so skip
    // the extra allocations if so
    let copy_packages = if packages.iter().any(|p| p.copy_index > 0) {
      let mut copy_packages = Vec::with_capacity(packages.len() / 2); // at most 1 copy for every package
      let copy_index_zero_nvs = packages
        .iter()
        .filter(|p| p.copy_index == 0)
        .map(|p| p.id.nv.clone())
        .collect::<HashSet<_>>();

      // partition out any packages that are "copy" packages
      for i in (0..packages.len()).rev() {
        if packages[i].copy_index > 0
        // the system might not have resolved the package with a
        // copy_index of 0, so we also need to check that
        && copy_index_zero_nvs.contains(&packages[i].id.nv)
        {
          copy_packages.push(packages.swap_remove(i));
        }
      }
      copy_packages
    } else {
      Vec::new()
    };

    NpmPackagesPartitioned {
      packages,
      copy_packages,
    }
  }

  pub fn resolve_best_package_id(
    &self,
    name: &str,
    version_req: &VersionReq,
  ) -> Option<NpmPackageId> {
    // todo(dsherret): this is not exactly correct because some ids
    // will be better than others due to peer dependencies
    let mut maybe_best_id: Option<&NpmPackageId> = None;
    if let Some(node_ids) = self.packages_by_name.get(name) {
      for node_id in node_ids.iter() {
        if version_req.matches(&node_id.nv.version) {
          let is_best_version = maybe_best_id
            .as_ref()
            .map(|best_id| best_id.nv.version.cmp(&node_id.nv.version).is_lt())
            .unwrap_or(true);
          if is_best_version {
            maybe_best_id = Some(node_id);
          }
        }
      }
    }
    maybe_best_id.cloned()
  }

  pub fn package_ids_for_nv<'a>(
    &'a self,
    nv: &'a PackageNv,
  ) -> impl Iterator<Item = &'a NpmPackageId> {
    self
      .packages_by_name
      .get(&nv.name)
      .map(|p| p.iter().filter(|p| p.nv == *nv))
      .into_iter()
      .flatten()
  }
}

pub struct SnapshotPackageCopyIndexResolver {
  packages_to_copy_index: HashMap<NpmPackageId, u8>,
  package_name_version_to_copy_count: HashMap<PackageNv, u8>,
}

impl SnapshotPackageCopyIndexResolver {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      packages_to_copy_index: HashMap::with_capacity(capacity),
      package_name_version_to_copy_count: HashMap::with_capacity(capacity), // close enough
    }
  }

  pub fn from_map_with_capacity(
    mut packages_to_copy_index: HashMap<NpmPackageId, u8>,
    capacity: usize,
  ) -> Self {
    let mut package_name_version_to_copy_count =
      HashMap::with_capacity(capacity); // close enough
    if capacity > packages_to_copy_index.len() {
      packages_to_copy_index.reserve(capacity - packages_to_copy_index.len());
    }

    for (node_id, index) in &packages_to_copy_index {
      let entry = package_name_version_to_copy_count
        .entry(node_id.nv.clone())
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

  pub fn resolve(&mut self, node_id: &NpmPackageId) -> u8 {
    if let Some(index) = self.packages_to_copy_index.get(node_id) {
      *index
    } else {
      let index = *self
        .package_name_version_to_copy_count
        .entry(node_id.nv.clone())
        .and_modify(|count| {
          *count += 1;
        })
        .or_insert(0);
      self.packages_to_copy_index.insert(node_id.clone(), index);
      index
    }
  }
}

fn name_without_path(name: &str) -> &str {
  let mut search_start_index = 0;
  if name.starts_with('@')
    && let Some(slash_index) = name.find('/')
  {
    search_start_index = slash_index + 1;
  }
  if let Some(slash_index) = &name[search_start_index..].find('/') {
    // get the name up until the path slash
    &name[0..search_start_index + slash_index]
  } else {
    name
  }
}

#[derive(Debug, Error, Clone, JsError)]
pub enum SnapshotFromLockfileError {
  #[error("Could not find '{}' specified in the lockfile.", .source.0)]
  #[class(inherit)]
  VersionNotFound {
    #[from]
    source: NpmPackageVersionNotFound,
  },
  #[error("The lockfile is corrupt. Remove the lockfile to regenerate it.")]
  #[class(inherit)]
  PackageIdNotFound(#[from] PackageIdNotFoundError),
  #[error(transparent)]
  #[class(inherit)]
  PackageIdDeserialization(#[from] NpmPackageIdDeserializationError),
}

pub struct SnapshotFromLockfileParams<'a> {
  pub link_packages: &'a HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
  pub lockfile: &'a Lockfile,
  pub default_tarball_url: &'a dyn DefaultTarballUrlProvider,
}

pub trait DefaultTarballUrlProvider {
  fn default_tarball_url(&self, nv: &PackageNv) -> String;
}

impl Default for &dyn DefaultTarballUrlProvider {
  fn default() -> Self {
    &NpmRegistryDefaultTarballUrlProvider
  }
}

/// `DefaultTarballUrlProvider` that uses the url of the real npm registry.
#[derive(Debug, Default, Clone)]
pub struct NpmRegistryDefaultTarballUrlProvider;

impl DefaultTarballUrlProvider for NpmRegistryDefaultTarballUrlProvider {
  fn default_tarball_url(
    &self,
    nv: &deno_semver::package::PackageNv,
  ) -> String {
    let scope = nv.scope();
    let package_name = if let Some(scope) = scope {
      nv.name
        .strip_prefix(scope)
        .unwrap_or(&nv.name)
        .trim_start_matches('/')
    } else {
      &nv.name
    };
    format!(
      "https://registry.npmjs.org/{}/-/{}-{}.tgz",
      nv.name, package_name, nv.version
    )
  }
}

fn dist_from_incomplete_package_info(
  id: &PackageNv,
  integrity: Option<&str>,
  tarball: Option<&str>,
  default_tarball_url: &dyn DefaultTarballUrlProvider,
) -> NpmPackageVersionDistInfo {
  let (shasum, integrity) = if let Some(integrity) = integrity {
    if integrity.contains('-') {
      (None, Some(integrity.to_string()))
    } else {
      (Some(integrity.to_string()), None)
    }
  } else {
    (None, None)
  };
  NpmPackageVersionDistInfo {
    tarball: tarball
      .map(|t| t.to_string())
      .unwrap_or_else(|| default_tarball_url.default_tarball_url(id)),
    shasum,
    integrity,
  }
}

#[derive(Debug, Error, Clone, JsError)]
pub enum IncompleteSnapshotFromLockfileError {
  #[error(transparent)]
  #[class(inherit)]
  PackageIdDeserialization(#[from] NpmPackageIdDeserializationError),
}

/// Constructs [`ValidSerializedNpmResolutionSnapshot`] from the given [`Lockfile`].
#[allow(clippy::needless_lifetimes)] // clippy bug
pub fn snapshot_from_lockfile(
  params: SnapshotFromLockfileParams<'_>,
) -> Result<ValidSerializedNpmResolutionSnapshot, SnapshotFromLockfileError> {
  let default_tarball_url = params.default_tarball_url;
  let lockfile = params.lockfile;
  let mut root_packages = HashMap::<PackageReq, NpmPackageId>::with_capacity(
    lockfile.content.packages.specifiers.len(),
  );
  let link_package_ids = params
    .link_packages
    .iter()
    .flat_map(|(name, info_vec)| {
      info_vec.iter().map(move |info| {
        StackString::from_string(format!("{}@{}", name, info.version))
      })
    })
    .collect::<HashSet<_>>();
  // collect the specifiers to version mappings
  for (key, value) in &lockfile.content.packages.specifiers {
    match key.kind {
      deno_semver::package::PackageKind::Npm => {
        let package_id = NpmPackageId::from_serialized(&format!(
          "{}@{}",
          key.req.name, value
        ))?;
        root_packages.insert(key.req.clone(), package_id);
      }
      deno_semver::package::PackageKind::Jsr => {}
    }
  }

  // now fill the packages except for the dist information
  let mut packages = Vec::with_capacity(lockfile.content.packages.npm.len());
  for (key, package) in &lockfile.content.packages.npm {
    let id = NpmPackageId::from_serialized(key)?;

    // collect the dependencies
    let mut dependencies = HashMap::with_capacity(package.dependencies.len());
    for (name, specifier) in &package.dependencies {
      let dep_id = NpmPackageId::from_serialized(specifier)?;
      dependencies.insert(name.clone(), dep_id);
    }

    let mut optional_dependencies =
      HashMap::with_capacity(package.optional_dependencies.len());
    for (name, specifier) in &package.optional_dependencies {
      let dep_id = NpmPackageId::from_serialized(specifier)?;
      optional_dependencies.insert(name.clone(), dep_id);
    }

    packages.push(SerializedNpmResolutionSnapshotPackage {
      dist: if !link_package_ids.contains(key) {
        Some(dist_from_incomplete_package_info(
          &id.nv,
          package.integrity.as_deref(),
          package.tarball.as_deref(),
          default_tarball_url,
        ))
      } else {
        None
      },
      id,
      dependencies: dependencies
        .into_iter()
        .chain(optional_dependencies.clone().into_iter())
        .collect(),
      optional_dependencies: optional_dependencies.into_keys().collect(),
      system: NpmResolutionPackageSystemInfo {
        cpu: package.cpu.clone(),
        os: package.os.clone(),
      },
      is_deprecated: package.deprecated,
      has_bin: package.bin,
      has_scripts: package.scripts,
      optional_peer_dependencies: package
        .optional_peers
        .clone()
        .into_keys()
        .collect(),
      extra: None,
    });
  }

  let snapshot = SerializedNpmResolutionSnapshot {
    packages,
    root_packages,
  }
  .into_valid()?;
  Ok(snapshot)
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use deno_lockfile::NewLockfileOptions;
  use deno_semver::Version;
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::registry::TestNpmRegistryApi;

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

  #[test]
  fn test_as_valid_serialized_for_system() {
    let original_serialized = SerializedNpmResolutionSnapshot {
      root_packages: root_pkgs(&[("a@1", "a@1.0.0")]),
      packages: vec![
        SerializedNpmResolutionSnapshotPackage {
          id: NpmPackageId::from_serialized("a@1.0.0").unwrap(),
          dependencies: deps(&[("b", "b@1.0.0"), ("c", "c@1.0.0")]),
          optional_peer_dependencies: HashSet::from(["b".into()]),
          system: Default::default(),
          optional_dependencies: HashSet::from(["c".into()]),
          dist: Some(crate::registry::NpmPackageVersionDistInfo {
            tarball: "https://example.com/a@1.0.0.tgz".to_string(),
            shasum: None,
            integrity: None,
          }),
          extra: None,
          is_deprecated: false,
          has_bin: false,
          has_scripts: false,
        },
        SerializedNpmResolutionSnapshotPackage {
          id: NpmPackageId::from_serialized("b@1.0.0").unwrap(),
          dependencies: Default::default(),
          optional_peer_dependencies: Default::default(),
          system: Default::default(),
          optional_dependencies: Default::default(),
          dist: Some(crate::registry::NpmPackageVersionDistInfo {
            tarball: "https://example.com/b@1.0.0.tgz".to_string(),
            shasum: None,
            integrity: None,
          }),
          extra: None,
          is_deprecated: false,
          has_bin: false,
          has_scripts: false,
        },
        SerializedNpmResolutionSnapshotPackage {
          id: NpmPackageId::from_serialized("c@1.0.0").unwrap(),
          dependencies: deps(&[("b", "b@1.0.0"), ("d", "d@1.0.0")]),
          optional_peer_dependencies: Default::default(),
          system: NpmResolutionPackageSystemInfo {
            os: vec!["win32".into()],
            cpu: vec!["x64".into()],
          },
          optional_dependencies: Default::default(),
          dist: Some(crate::registry::NpmPackageVersionDistInfo {
            tarball: "https://example.com/c@1.0.0.tgz".to_string(),
            shasum: None,
            integrity: None,
          }),
          extra: None,
          is_deprecated: false,
          has_bin: false,
          has_scripts: false,
        },
        SerializedNpmResolutionSnapshotPackage {
          id: NpmPackageId::from_serialized("d@1.0.0").unwrap(),
          dependencies: Default::default(),
          optional_peer_dependencies: Default::default(),
          system: Default::default(),
          optional_dependencies: Default::default(),
          dist: Some(crate::registry::NpmPackageVersionDistInfo {
            tarball: "https://example.com/d@1.0.0.tgz".to_string(),
            shasum: None,
            integrity: None,
          }),
          extra: None,
          is_deprecated: false,
          has_bin: false,
          has_scripts: false,
        },
      ],
    }
    .into_valid()
    .unwrap();
    let snapshot = NpmResolutionSnapshot::new(original_serialized.clone());
    // test providing a matching system
    {
      let mut actual = snapshot
        .as_valid_serialized_for_system(&NpmSystemInfo {
          os: "win32".into(),
          cpu: "x64".into(),
        })
        .into_serialized();
      actual.packages.sort_by(|a, b| a.id.cmp(&b.id));
      let mut expected = original_serialized.clone().into_serialized();
      for pkg in expected.packages.iter_mut() {
        pkg.system = Default::default();
        pkg.optional_dependencies.clear();
      }
      expected.packages.sort_by(|a, b| a.id.cmp(&b.id));
      assert_eq!(actual, expected);
    }
    // test providing a non-matching system
    {
      let mut actual = snapshot
        .as_valid_serialized_for_system(&NpmSystemInfo {
          os: "darwin".into(),
          cpu: "x64".into(),
        })
        .into_serialized();
      actual.packages.sort_by(|a, b| a.id.cmp(&b.id));
      let mut expected = original_serialized.into_serialized();
      for pkg in expected.packages.iter_mut() {
        pkg.system = Default::default();
        pkg.optional_dependencies.clear();
      }
      expected.packages.sort_by(|a, b| a.id.cmp(&b.id));
      // these are sorted, so remove the c and d packages
      expected.packages.remove(3);
      expected.packages.remove(2);
      // remove c as a dependency from a
      assert!(expected.packages[0].dependencies.remove("c").is_some());
      assert_eq!(actual, expected);
    }
  }

  #[test]
  fn resolve_pkg_from_pkg_cache_folder_id() {
    let original_serialized = SerializedNpmResolutionSnapshot {
      root_packages: root_pkgs(&[("a@1", "a@1.0.0")]),
      packages: vec![
        pkg_with_id("a@1.0.0"),
        pkg_with_id("a@1.0.0_b@1.0.0"),
        pkg_with_id("a@1.1.0"),
        pkg_with_id("b@1.0.0"),
      ],
    }
    .into_valid()
    .unwrap();
    let snapshot = NpmResolutionSnapshot::new(original_serialized);

    let pkg = snapshot
      .resolve_pkg_from_pkg_cache_folder_id(&npm_cache_folder_id(
        "a", "1.0.0", 0,
      ))
      .unwrap();
    assert_eq!(pkg.id.as_serialized(), "a@1.0.0");
    assert_eq!(pkg.copy_index, 0);

    let pkg = snapshot
      .resolve_pkg_from_pkg_cache_folder_id(&npm_cache_folder_id(
        "a", "1.0.0", 1,
      ))
      .unwrap();
    assert_eq!(pkg.id.as_serialized(), "a@1.0.0_b@1.0.0");
    assert_eq!(pkg.copy_index, 1);
    assert!(
      snapshot
        .resolve_pkg_from_pkg_cache_folder_id(&npm_cache_folder_id(
          "a", "1.0.0", 2,
        ))
        .is_err()
    );
    assert!(
      snapshot
        .resolve_pkg_from_pkg_cache_folder_id(&npm_cache_folder_id(
          "b", "1.0.0", 2,
        ))
        .is_err()
    );
  }

  fn npm_cache_folder_id(
    name: &str,
    version: &str,
    copy_index: u8,
  ) -> NpmPackageCacheFolderId {
    NpmPackageCacheFolderId {
      nv: PackageNv {
        name: name.into(),
        version: Version::parse_standard(version).unwrap(),
      },
      copy_index,
    }
  }

  fn pkg_with_id(id: &str) -> SerializedNpmResolutionSnapshotPackage {
    SerializedNpmResolutionSnapshotPackage {
      id: NpmPackageId::from_serialized(id).unwrap(),
      dependencies: Default::default(),
      optional_peer_dependencies: Default::default(),
      system: Default::default(),
      optional_dependencies: Default::default(),
      dist: Some(crate::registry::NpmPackageVersionDistInfo {
        tarball: format!("https://example.com/{id}.tar.gz", id = id),
        shasum: None,
        integrity: None,
      }),
      extra: None,
      is_deprecated: false,
      has_bin: false,
      has_scripts: false,
    }
  }

  fn deps(deps: &[(&str, &str)]) -> HashMap<StackString, NpmPackageId> {
    deps
      .iter()
      .map(|(key, value)| {
        (
          StackString::from(*key),
          NpmPackageId::from_serialized(value).unwrap(),
        )
      })
      .collect()
  }

  fn root_pkgs(pkgs: &[(&str, &str)]) -> HashMap<PackageReq, NpmPackageId> {
    pkgs
      .iter()
      .map(|(key, value)| {
        (
          PackageReq::from_str(key).unwrap(),
          NpmPackageId::from_serialized(value).unwrap(),
        )
      })
      .collect()
  }

  struct TestDefaultTarballUrlProvider;

  impl DefaultTarballUrlProvider for TestDefaultTarballUrlProvider {
    fn default_tarball_url(&self, nv: &PackageNv) -> String {
      format!("https://example.com/{nv}.tar.gz", nv = nv)
    }
  }

  #[tokio::test]
  async fn test_snapshot_from_lockfile_v2() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version_with_integrity(
      "chalk",
      "5.3.0",
      Some("sha512-integrity1"),
    );
    api.ensure_package_version_with_integrity(
      "emoji-regex",
      "10.2.1",
      Some("sha512-integrity2"),
    );

    let lockfile = Lockfile::new(
      NewLockfileOptions {
        file_path: PathBuf::from("/deno.lock"),
        content: r#"{
        "version": "2",
        "remote": {},
        "npm": {
          "specifiers": {
            "chalk@5": "chalk@5.3.0",
            "emoji-regex": "emoji-regex@10.2.1"
          },
          "packages": {
            "chalk@5.3.0": {
              "integrity": "sha512-integrity1",
              "dependencies": {}
            },
            "emoji-regex@10.2.1": {
              "integrity": "sha512-integrity2",
              "dependencies": {}
            }
          }
        }
      }"#,
        overwrite: false,
      },
      &api,
    )
    .await
    .unwrap();

    assert!(
      snapshot_from_lockfile(SnapshotFromLockfileParams {
        lockfile: &lockfile,
        link_packages: &Default::default(),
        default_tarball_url: &TestDefaultTarballUrlProvider,
      })
      .is_ok()
    );
  }

  #[tokio::test]
  async fn test_snapshot_from_lockfile_v4() {
    let api = TestNpmRegistryApi::default();
    api.ensure_package_version_with_integrity(
      "chalk",
      "5.3.0",
      Some("sha512-integrity1"),
    );
    api.ensure_package_version_with_integrity(
      "emoji-regex",
      "10.2.1",
      Some("sha512-integrity2"),
    );

    let lockfile = Lockfile::new(
      NewLockfileOptions {
        file_path: PathBuf::from("/deno.lock"),
        content: r#"{
        "version": "4",
        "specifiers": {
          "npm:chalk@5": "5.3.0",
          "npm:emoji-regex": "10.2.1",
          "jsr:@std/path": "1.0.0"
        },
        "npm": {
          "chalk@5.3.0": {
            "integrity": "sha512-integrity1",
            "dependencies": []
          },
          "emoji-regex@10.2.1": {
            "integrity": "sha512-integrity2",
            "dependencies": []
          }
        }
      }"#,
        overwrite: false,
      },
      &api,
    )
    .await
    .unwrap();

    let snapshot = snapshot_from_lockfile(SnapshotFromLockfileParams {
      lockfile: &lockfile,
      link_packages: &Default::default(),
      default_tarball_url: &TestDefaultTarballUrlProvider,
    })
    .unwrap();
    assert_eq!(
      snapshot.as_serialized().root_packages,
      HashMap::from([
        (
          PackageReq::from_str("chalk@5").unwrap(),
          NpmPackageId::from_serialized("chalk@5.3.0").unwrap()
        ),
        (
          PackageReq::from_str("emoji-regex").unwrap(),
          NpmPackageId::from_serialized("emoji-regex@10.2.1").unwrap()
        )
      ])
    );
  }

  fn package(
    id: &str,
    dependencies: &[(&str, &str)],
  ) -> SerializedNpmResolutionSnapshotPackage {
    SerializedNpmResolutionSnapshotPackage {
      id: NpmPackageId::from_serialized(id).unwrap(),
      dependencies: deps(dependencies),
      optional_peer_dependencies: Default::default(),
      system: Default::default(),
      optional_dependencies: Default::default(),
      dist: Some(crate::registry::NpmPackageVersionDistInfo {
        tarball: format!("https://example.com/{id}.tar.gz",),
        shasum: None,
        integrity: None,
      }),
      extra: None,
      is_deprecated: false,
      has_bin: false,
      has_scripts: false,
    }
  }

  fn reqs<'a>(reqs: impl IntoIterator<Item = &'a str>) -> Vec<PackageReq> {
    reqs
      .into_iter()
      .map(|s| PackageReq::from_str_loose(s).unwrap())
      .collect()
  }

  #[track_caller]
  fn assert_snapshot_eq(
    a: &SerializedNpmResolutionSnapshot,
    b: &SerializedNpmResolutionSnapshot,
  ) {
    let mut a_root_packages = a.root_packages.iter().collect::<Vec<_>>();
    a_root_packages.sort();
    let mut b_root_packages = b.root_packages.iter().collect::<Vec<_>>();
    b_root_packages.sort();
    let mut a_packages = a.packages.clone();
    a_packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut b_packages = b.packages.clone();
    b_packages.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(a_root_packages, b_root_packages);
    assert_eq!(a_packages, b_packages);
  }

  #[test]
  fn snapshot_subset() {
    let a = package("a@1.0.0", &[("b", "b@1.0.0"), ("c", "c@1.0.0")]);
    let b = package("b@1.0.0", &[("d", "d@1.0.0")]);
    let c = package("c@1.0.0", &[("e", "e@1.0.0")]);
    let d = package("d@1.0.0", &[]);
    let e = package("e@1.0.0", &[("f", "f@1.0.0")]);
    let f = package("f@1.0.0", &[("g", "g@1.0.0")]);
    let g = package("g@1.0.0", &[("e", "e@1.0.0")]);
    let serialized = SerializedNpmResolutionSnapshot {
      root_packages: root_pkgs(&[("a@1", "a@1.0.0"), ("f@1", "f@1.0.0")]),
      packages: vec![a, b, c, d, e.clone(), f.clone(), g.clone()],
    };
    let snapshot = NpmResolutionSnapshot::new(serialized.into_valid().unwrap());
    let subset = snapshot.subset(&reqs(["f@1", "z@1"]));
    assert_snapshot_eq(
      subset.as_valid_serialized().as_serialized(),
      &SerializedNpmResolutionSnapshot {
        root_packages: root_pkgs(&[("f@1", "f@1.0.0")]),
        packages: vec![e, f, g],
      },
    );

    let empty_subset = snapshot.subset(&reqs(["z@1"]));
    assert_snapshot_eq(
      empty_subset.as_valid_serialized().as_serialized(),
      &SerializedNpmResolutionSnapshot {
        root_packages: Default::default(),
        packages: Default::default(),
      },
    );
  }

  #[test]
  fn resolve_pkg_from_pkg_req_types_node_broad() {
    let types_a = package("@types/a@1.0.0", &[]);
    let types_node = package("@types/node@1.0.0", &[]);
    let serialized = SerializedNpmResolutionSnapshot {
      root_packages: root_pkgs(&[
        ("@types/a@1", "@types/a@1.0.0"),
        ("@types/node@1", "@types/node@1.0.0"),
      ]),
      packages: vec![types_a, types_node],
    };
    let snapshot = NpmResolutionSnapshot::new(serialized.into_valid().unwrap());
    // the cli will look up a broad @types/node package in the snapshot, so
    // support doing that even if it's not in one of the reqs.
    let pkg = snapshot
      .resolve_pkg_from_pkg_req(&PackageReq::from_str("@types/node@*").unwrap())
      .unwrap();
    assert_eq!(pkg.id.nv.version.to_string(), "1.0.0");
    assert!(
      snapshot
        .resolve_pkg_from_pkg_req(
          &PackageReq::from_str("@types/node@next").unwrap()
        )
        // shouldn't panic
        .is_err()
    );
  }

  #[tokio::test]
  async fn test_snapshot_from_lockfile_v5_with_linked_package() {
    let api = TestNpmRegistryApi::default();
    let lockfile = Lockfile::new(
      NewLockfileOptions {
        file_path: PathBuf::from("/deno.lock"),
        content: r#"{
          "version": "5",
          "specifiers": {
            "npm:cowsay@^1.6.0": "1.6.0"
          },
          "npm": {
            "cowsay@1.6.0": {}
          },
          "workspace": {
            "packageJson": {
              "dependencies": [
                "npm:cowsay@^1.6.0"
              ]
            },
            "links": {
              "npm:cowsay@1.6.0": {}
            }
          }
        }"#,
        overwrite: false,
      },
      &api,
    )
    .await
    .unwrap();
    let link_packages = &HashMap::from([(
      PackageName::from_static("cowsay"),
      vec![NpmPackageVersionInfo {
        version: Version::parse_standard("1.6.0").unwrap(),
        ..Default::default()
      }],
    )]);
    let snapshot = snapshot_from_lockfile(SnapshotFromLockfileParams {
      lockfile: &lockfile,
      link_packages,
      default_tarball_url: &TestDefaultTarballUrlProvider,
    })
    .unwrap();

    assert_eq!(
      snapshot.as_serialized().packages,
      vec![SerializedNpmResolutionSnapshotPackage {
        id: NpmPackageId::from_serialized("cowsay@1.6.0").unwrap(),
        system: Default::default(),
        dist: None, // should be None
        dependencies: Default::default(),
        optional_dependencies: Default::default(),
        optional_peer_dependencies: Default::default(),
        extra: None,
        is_deprecated: false,
        has_bin: false,
        has_scripts: false,
      }]
    );
  }
}
