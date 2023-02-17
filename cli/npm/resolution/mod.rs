// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::RwLock;
use deno_graph::npm::NpmPackageId;
use deno_graph::npm::NpmPackageIdReference;
use deno_graph::npm::NpmPackageReq;
use deno_graph::npm::NpmPackageReqReference;
use deno_graph::semver::Version;
use log::debug;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::args::Lockfile;
use crate::npm::resolution::graph::LATEST_VERSION_REQ;

use self::common::resolve_best_package_version_and_info;
use self::graph::GraphDependencyResolver;
use self::snapshot::NpmPackagesPartitioned;

use super::cache::should_sync_download;
use super::cache::NpmPackageCacheFolderId;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmRegistryApi;

mod common;
mod graph;
mod snapshot;

use graph::Graph;
pub use snapshot::NpmResolutionSnapshot;

#[derive(Debug, Error)]
#[error("Invalid npm package id '{text}'. {message}")]
pub struct NpmPackageNodeIdDeserializationError {
  message: String,
  text: String,
}

/// A resolved unique identifier for an npm package. This contains
/// the resolved name, version, and peer dependency resolution identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NpmPackageNodeId {
  pub id: NpmPackageId,
  pub peer_dependencies: Vec<NpmPackageNodeId>,
}

impl NpmPackageNodeId {
  pub fn as_serialized(&self) -> String {
    self.as_serialized_with_level(0)
  }

  fn as_serialized_with_level(&self, level: usize) -> String {
    // WARNING: This should not change because it's used in the lockfile
    let mut result = format!(
      "{}@{}",
      if level == 0 {
        self.id.name.to_string()
      } else {
        self.id.name.replace('/', "+")
      },
      self.id.version
    );
    for peer in &self.peer_dependencies {
      // unfortunately we can't do something like `_3` when
      // this gets deep because npm package names can start
      // with a number
      result.push_str(&"_".repeat(level + 1));
      result.push_str(&peer.as_serialized_with_level(level + 1));
    }
    result
  }

  pub fn from_serialized(
    id: &str,
  ) -> Result<Self, NpmPackageNodeIdDeserializationError> {
    use monch::*;

    fn parse_name(input: &str) -> ParseResult<&str> {
      if_not_empty(substring(move |input| {
        for (pos, c) in input.char_indices() {
          // first character might be a scope, so skip it
          if pos > 0 && c == '@' {
            return Ok((&input[pos..], ()));
          }
        }
        ParseError::backtrace()
      }))(input)
    }

    fn parse_version(input: &str) -> ParseResult<&str> {
      if_not_empty(substring(skip_while(|c| c != '_')))(input)
    }

    fn parse_name_and_version(input: &str) -> ParseResult<(String, Version)> {
      let (input, name) = parse_name(input)?;
      let (input, _) = ch('@')(input)?;
      let at_version_input = input;
      let (input, version) = parse_version(input)?;
      match Version::parse_from_npm(version) {
        Ok(version) => Ok((input, (name.to_string(), version))),
        Err(err) => ParseError::fail(at_version_input, format!("{err:#}")),
      }
    }

    fn parse_level_at_level<'a>(
      level: usize,
    ) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
      fn parse_level(input: &str) -> ParseResult<usize> {
        let level = input.chars().take_while(|c| *c == '_').count();
        Ok((&input[level..], level))
      }

      move |input| {
        let (input, parsed_level) = parse_level(input)?;
        if parsed_level == level {
          Ok((input, ()))
        } else {
          ParseError::backtrace()
        }
      }
    }

    fn parse_peers_at_level<'a>(
      level: usize,
    ) -> impl Fn(&'a str) -> ParseResult<'a, Vec<NpmPackageNodeId>> {
      move |mut input| {
        let mut peers = Vec::new();
        while let Ok((level_input, _)) = parse_level_at_level(level)(input) {
          input = level_input;
          let peer_result = parse_id_at_level(level)(input)?;
          input = peer_result.0;
          peers.push(peer_result.1);
        }
        Ok((input, peers))
      }
    }

    fn parse_id_at_level<'a>(
      level: usize,
    ) -> impl Fn(&'a str) -> ParseResult<'a, NpmPackageNodeId> {
      move |input| {
        let (input, (name, version)) = parse_name_and_version(input)?;
        let name = if level > 0 {
          name.replace('+', "/")
        } else {
          name
        };
        let (input, peer_dependencies) =
          parse_peers_at_level(level + 1)(input)?;
        Ok((
          input,
          NpmPackageNodeId {
            id: NpmPackageId { name, version },
            peer_dependencies,
          },
        ))
      }
    }

    with_failure_handling(parse_id_at_level(0))(id).map_err(|err| {
      NpmPackageNodeIdDeserializationError {
        message: format!("{err:#}"),
        text: id.to_string(),
      }
    })
  }
}

impl Ord for NpmPackageNodeId {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    match self.id.cmp(&other.id) {
      Ordering::Equal => self.peer_dependencies.cmp(&other.peer_dependencies),
      ordering => ordering,
    }
  }
}

impl PartialOrd for NpmPackageNodeId {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmResolutionPackage {
  pub node_id: NpmPackageNodeId,
  /// The peer dependency resolution can differ for the same
  /// package (name and version) depending on where it is in
  /// the resolution tree. This copy index indicates which
  /// copy of the package this is.
  pub copy_index: usize,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageNodeId>,
}

impl NpmResolutionPackage {
  pub fn get_package_cache_folder_id(&self) -> NpmPackageCacheFolderId {
    NpmPackageCacheFolderId {
      id: self.node_id.id.clone(),
      copy_index: self.copy_index,
    }
  }
}

#[derive(Clone)]
pub struct NpmResolution(Arc<NpmResolutionInner>);

struct NpmResolutionInner {
  api: NpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_semaphore: tokio::sync::Semaphore,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.0.snapshot.read();
    f.debug_struct("NpmResolution")
      .field("snapshot", &snapshot)
      .finish()
  }
}

impl NpmResolution {
  pub fn new(
    api: NpmRegistryApi,
    initial_snapshot: Option<NpmResolutionSnapshot>,
  ) -> Self {
    Self(Arc::new(NpmResolutionInner {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_semaphore: tokio::sync::Semaphore::new(1),
    }))
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let inner = &self.0;
    let _permit = inner.update_semaphore.acquire().await?;
    let snapshot = inner.snapshot.read().clone();

    let snapshot =
      add_package_reqs_to_snapshot(&inner.api, package_reqs, snapshot).await?;

    *inner.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    let inner = &self.0;
    // only allow one thread in here at a time
    let _permit = inner.update_semaphore.acquire().await?;
    let snapshot = inner.snapshot.read().clone();

    let has_removed_package = !snapshot
      .package_reqs
      .keys()
      .all(|req| package_reqs.contains(req));
    // if any packages were removed, we need to completely recreate the npm resolution snapshot
    let snapshot = if has_removed_package {
      NpmResolutionSnapshot::default()
    } else {
      snapshot
    };
    let snapshot = add_package_reqs_to_snapshot(
      &inner.api,
      package_reqs.into_iter().collect(),
      snapshot,
    )
    .await?;

    *inner.snapshot.write() = snapshot;

    Ok(())
  }

  pub async fn resolve_pending(&self) -> Result<(), AnyError> {
    let inner = &self.0;
    // only allow one thread in here at a time
    let _permit = inner.update_semaphore.acquire().await?;
    let snapshot = inner.snapshot.read().clone();

    let snapshot =
      add_package_reqs_to_snapshot(&inner.api, Vec::new(), snapshot).await?;

    *inner.snapshot.write() = snapshot;

    Ok(())
  }

  pub fn pkg_req_ref_to_pkg_id_ref(
    &self,
    req_ref: NpmPackageReqReference,
  ) -> Result<NpmPackageIdReference, AnyError> {
    let node_id = self.resolve_pkg_node_id_from_pkg_req(&req_ref.req)?;
    Ok(NpmPackageIdReference {
      id: node_id.id,
      sub_path: req_ref.sub_path,
    })
  }

  pub fn resolve_package_cache_folder_id_from_id(
    &self,
    id: &NpmPackageNodeId,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .0
      .snapshot
      .read()
      .package_from_id(id)
      .map(|p| p.get_package_cache_folder_id())
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageCacheFolderId,
  ) -> Result<NpmResolutionPackage, AnyError> {
    self
      .0
      .snapshot
      .read()
      .resolve_package_from_package(name, referrer)
      .cloned()
  }

  /// Resolve a node package from a deno module.
  pub fn resolve_pkg_node_id_from_pkg_req(
    &self,
    req: &NpmPackageReq,
  ) -> Result<NpmPackageNodeId, AnyError> {
    self
      .0
      .snapshot
      .read()
      .resolve_pkg_node_id_from_pkg_req(req)
      .map(|pkg| pkg.node_id.clone())
  }

  pub fn resolve_pkg_node_id_from_deno_module(
    &self,
    id: &NpmPackageId,
  ) -> Result<NpmPackageNodeId, AnyError> {
    self
      .0
      .snapshot
      .read()
      .resolve_package_from_deno_module(id)
      .map(|pkg| pkg.node_id.clone())
  }

  pub fn resolve_deno_graph_package_req(
    &self,
    pkg_req: &NpmPackageReq,
  ) -> Result<NpmPackageId, AnyError> {
    let inner = &self.0;
    let package_info = match inner.api.get_cached_package_info(&pkg_req.name) {
      Some(package_info) => package_info,
      // should never happen because we should have cached before
      None => bail!(
        "Deno bug. Please report: Could not find '{}' in npm package info cache.",
        pkg_req.name
      ),
    };

    let mut snapshot = inner.snapshot.write();
    let version_req =
      pkg_req.version_req.as_ref().unwrap_or(&*LATEST_VERSION_REQ);
    let version_and_info = resolve_best_package_version_and_info(
      version_req,
      &package_info,
      &snapshot.packages_by_name,
    )?;
    let id = NpmPackageId {
      name: package_info.name.to_string(),
      version: version_and_info.version.clone(),
    };
    debug!(
      "Resolved {}@{} to {}",
      pkg_req.name,
      version_req.version_text(),
      id.to_string(),
    );
    snapshot.package_reqs.insert(pkg_req.clone(), id.clone());
    let packages_with_name = snapshot
      .packages_by_name
      .entry(package_info.name.clone())
      .or_default();
    if !packages_with_name.iter().any(|p| p.id == id) {
      packages_with_name.push(NpmPackageNodeId {
        id: id.clone(),
        peer_dependencies: Vec::new(),
      });
    }
    snapshot.pending_unresolved_packages.push(id.clone());
    Ok(id)
  }

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    self.0.snapshot.read().all_packages_partitioned()
  }

  pub fn has_packages(&self) -> bool {
    !self.0.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.0.snapshot.read().clone()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.0.snapshot.read();
    for (package_req, package_id) in snapshot.package_reqs.iter() {
      lockfile.insert_npm_specifier(
        package_req.to_string(),
        snapshot
          .root_packages
          .get(package_id)
          .unwrap()
          .as_serialized(),
      );
    }
    for package in snapshot.all_packages() {
      lockfile.check_or_insert_npm_package(package.into())?;
    }
    Ok(())
  }
}

async fn add_package_reqs_to_snapshot(
  api: &NpmRegistryApi,
  package_reqs: Vec<NpmPackageReq>,
  snapshot: NpmResolutionSnapshot,
) -> Result<NpmResolutionSnapshot, AnyError> {
  // convert the snapshot to a traversable graph
  let mut graph = Graph::from_snapshot(snapshot);
  let pending_unresolved = graph.take_pending_unresolved();

  // avoid loading the info if this is already in the graph
  let package_reqs = package_reqs
    .into_iter()
    .filter(|r| !graph.has_package_req(r))
    .collect::<Vec<_>>();
  let pending_unresolved = pending_unresolved
    .into_iter()
    .filter(|p| !graph.has_root_package(p))
    .collect::<Vec<_>>();

  // go over the top level package names first (pending unresolved and npm package reqs),
  // then down the tree one level at a time through all the branches
  cache_package_infos_in_api(api, &pending_unresolved, &package_reqs).await?;

  let mut resolver = GraphDependencyResolver::new(&mut graph, &api);

  // The package reqs and ids should already be sorted
  // in the order they should be resolved in.
  for package_req in package_reqs {
    let info = api.package_info(&package_req.name).await?;
    resolver.add_package_req(&package_req, &info)?;
  }

  for pkg_id in pending_unresolved {
    let info = api.package_info(&pkg_id.name).await?;
    resolver.add_root_package(&pkg_id, &info)?;
  }

  resolver.resolve_pending().await?;

  let result = graph.into_snapshot(&api).await;
  api.clear_memory_cache();
  result
}

async fn cache_package_infos_in_api(
  api: &NpmRegistryApi,
  pending_unresolved: &Vec<NpmPackageId>,
  package_reqs: &Vec<NpmPackageReq>,
) -> Result<(), AnyError> {
  // go over the top level package names first (pending unresolved and npm package reqs),
  // then down the tree one level at a time through all the branches
  let mut package_names_to_cache =
    HashSet::with_capacity(package_reqs.len() + pending_unresolved.len());

  package_names_to_cache
    .extend(pending_unresolved.iter().map(|id| id.name.clone()));
  package_names_to_cache
    .extend(package_reqs.iter().map(|req| req.name.clone()));

  let mut unresolved_tasks = Vec::with_capacity(package_names_to_cache.len());

  // cache the package info up front in parallel
  if should_sync_download() {
    // for deterministic test output
    let mut ordered_names =
      package_names_to_cache.into_iter().collect::<Vec<_>>();
    ordered_names.sort();
    for name in ordered_names {
      api.package_info(&name).await?;
    }
  } else {
    for name in package_names_to_cache {
      let api = api.clone();
      unresolved_tasks.push(tokio::task::spawn(async move {
        // This is ok to call because api will internally cache
        // the package information in memory.
        api.package_info(&name).await
      }));
    }
  };

  for result in futures::future::join_all(unresolved_tasks).await {
    result??; // surface the first error
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use deno_graph::npm::NpmPackageId;
  use deno_graph::semver::Version;

  use super::NpmPackageNodeId;

  #[test]
  fn serialize_npm_package_id() {
    let id = NpmPackageNodeId {
      id: NpmPackageId {
        name: "pkg-a".to_string(),
        version: Version::parse_from_npm("1.2.3").unwrap(),
      },
      peer_dependencies: vec![
        NpmPackageNodeId {
          id: NpmPackageId {
            name: "pkg-b".to_string(),
            version: Version::parse_from_npm("3.2.1").unwrap(),
          },
          peer_dependencies: vec![
            NpmPackageNodeId {
              id: NpmPackageId {
                name: "pkg-c".to_string(),
                version: Version::parse_from_npm("1.3.2").unwrap(),
              },
              peer_dependencies: vec![],
            },
            NpmPackageNodeId {
              id: NpmPackageId {
                name: "pkg-d".to_string(),
                version: Version::parse_from_npm("2.3.4").unwrap(),
              },
              peer_dependencies: vec![],
            },
          ],
        },
        NpmPackageNodeId {
          id: NpmPackageId {
            name: "pkg-e".to_string(),
            version: Version::parse_from_npm("2.3.1").unwrap(),
          },
          peer_dependencies: vec![NpmPackageNodeId {
            id: NpmPackageId {
              name: "pkg-f".to_string(),
              version: Version::parse_from_npm("2.3.1").unwrap(),
            },
            peer_dependencies: vec![],
          }],
        },
      ],
    };

    // this shouldn't change because it's used in the CLI's lockfile
    let serialized = id.as_serialized();
    assert_eq!(serialized, "pkg-a@1.2.3_pkg-b@3.2.1__pkg-c@1.3.2__pkg-d@2.3.4_pkg-e@2.3.1__pkg-f@2.3.1");
    assert_eq!(NpmPackageNodeId::from_serialized(&serialized).unwrap(), id);
  }
}
