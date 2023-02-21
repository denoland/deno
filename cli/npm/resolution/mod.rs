// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_graph::npm::NpmPackageNv;
use deno_graph::npm::NpmPackageReq;
use deno_graph::semver::Version;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use crate::args::Lockfile;

use self::graph::GraphDependencyResolver;
use self::snapshot::NpmPackagesPartitioned;

use super::cache::NpmPackageCacheFolderId;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmRegistryApi;

mod common;
mod graph;
mod snapshot;
mod specifier;

use graph::Graph;
pub use snapshot::NpmResolutionSnapshot;
pub use specifier::resolve_graph_npm_info;

#[derive(Debug, Error)]
#[error("Invalid npm package id '{text}'. {message}")]
pub struct NpmPackageNodeIdDeserializationError {
  message: String,
  text: String,
}

/// A resolved unique identifier for an npm package. This contains
/// the resolved name, version, and peer dependency resolution identifiers.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NpmPackageId {
  pub nv: NpmPackageNv,
  pub peer_dependencies: Vec<NpmPackageId>,
}

// Custom debug implementation for more concise test output
impl std::fmt::Debug for NpmPackageId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_serialized())
  }
}

impl NpmPackageId {
  pub fn as_serialized(&self) -> String {
    self.as_serialized_with_level(0)
  }

  fn as_serialized_with_level(&self, level: usize) -> String {
    // WARNING: This should not change because it's used in the lockfile
    let mut result = format!(
      "{}@{}",
      if level == 0 {
        self.nv.name.to_string()
      } else {
        self.nv.name.replace('/', "+")
      },
      self.nv.version
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
    ) -> impl Fn(&'a str) -> ParseResult<'a, Vec<NpmPackageId>> {
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
    ) -> impl Fn(&'a str) -> ParseResult<'a, NpmPackageId> {
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
          NpmPackageId {
            nv: NpmPackageNv { name, version },
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

impl Ord for NpmPackageId {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.nv.cmp(&other.nv) {
      Ordering::Equal => self.peer_dependencies.cmp(&other.peer_dependencies),
      ordering => ordering,
    }
  }
}

impl PartialOrd for NpmPackageId {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmResolutionPackage {
  pub pkg_id: NpmPackageId,
  /// The peer dependency resolution can differ for the same
  /// package (name and version) depending on where it is in
  /// the resolution tree. This copy index indicates which
  /// copy of the package this is.
  pub copy_index: usize,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageId>,
}

impl std::fmt::Debug for NpmResolutionPackage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // custom debug implementation for deterministic output in the tests
    f.debug_struct("NpmResolutionPackage")
      .field("pkg_id", &self.pkg_id)
      .field("copy_index", &self.copy_index)
      .field("dist", &self.dist)
      .field(
        "dependencies",
        &self.dependencies.iter().collect::<BTreeMap<_, _>>(),
      )
      .finish()
  }
}

impl NpmResolutionPackage {
  pub fn get_package_cache_folder_id(&self) -> NpmPackageCacheFolderId {
    NpmPackageCacheFolderId {
      nv: self.pkg_id.nv.clone(),
      copy_index: self.copy_index,
    }
  }
}

pub struct NpmResolution {
  api: NpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_semaphore: tokio::sync::Semaphore,
}

impl std::fmt::Debug for NpmResolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let snapshot = self.snapshot.read();
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
    Self {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_semaphore: tokio::sync::Semaphore::new(1),
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_semaphore.acquire().await?;
    let snapshot = self.snapshot.read().clone();

    let snapshot =
      add_package_reqs_to_snapshot(&self.api, package_reqs, snapshot).await?;

    *self.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_semaphore.acquire().await?;
    let snapshot = self.snapshot.read().clone();

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
      &self.api,
      package_reqs.into_iter().collect(),
      snapshot,
    )
    .await?;

    *self.snapshot.write() = snapshot;

    Ok(())
  }

  pub fn resolve_package_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmResolutionPackage> {
    self.snapshot.read().package_from_id(id).cloned()
  }

  pub fn resolve_package_cache_folder_id_from_id(
    &self,
    id: &NpmPackageId,
  ) -> Option<NpmPackageCacheFolderId> {
    self
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
      .snapshot
      .read()
      .resolve_package_from_package(name, referrer)
      .cloned()
  }

  /// Resolve a node package from a deno module.
  pub fn resolve_package_from_deno_module(
    &self,
    package: &NpmPackageReq,
  ) -> Result<NpmResolutionPackage, AnyError> {
    self
      .snapshot
      .read()
      .resolve_package_from_deno_module(package)
      .cloned()
  }

  pub fn all_packages_partitioned(&self) -> NpmPackagesPartitioned {
    self.snapshot.read().all_packages_partitioned()
  }

  pub fn has_packages(&self) -> bool {
    !self.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.snapshot.read().clone()
  }

  pub fn lock(&self, lockfile: &mut Lockfile) -> Result<(), AnyError> {
    let snapshot = self.snapshot.read();
    for (package_req, nv) in snapshot.package_reqs.iter() {
      let package_id = snapshot.root_packages.get(nv).unwrap();
      lockfile.insert_npm_specifier(
        package_req.to_string(),
        package_id.as_serialized(),
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
  if package_reqs
    .iter()
    .all(|req| snapshot.package_reqs.contains_key(req))
  {
    return Ok(snapshot); // already up to date
  }

  // convert the snapshot to a traversable graph
  let mut graph = Graph::from_snapshot(snapshot).with_context(|| {
    deno_core::anyhow::anyhow!(
      "Failed creating npm state. Try recreating your lockfile."
    )
  })?;

  // avoid loading the info if this is already in the graph
  let package_reqs = package_reqs
    .into_iter()
    .filter(|r| !graph.has_package_req(r))
    .collect::<Vec<_>>();

  // go over the top level package names first, then down the tree
  // one level at a time through all the branches
  api
    .cache_in_parallel(
      package_reqs
        .iter()
        .map(|r| r.name.clone())
        .into_iter()
        .collect::<Vec<_>>(),
    )
    .await?;

  let mut resolver = GraphDependencyResolver::new(&mut graph, api);

  // The package reqs should already be sorted
  // in the order they should be resolved in.
  for package_req in package_reqs {
    let info = api.package_info(&package_req.name).await?;
    resolver.add_package_req(&package_req, &info)?;
  }

  resolver.resolve_pending().await?;

  let result = graph.into_snapshot(api).await;
  api.clear_memory_cache();
  result
}

#[cfg(test)]
mod test {
  use deno_graph::npm::NpmPackageNv;
  use deno_graph::semver::Version;

  use super::NpmPackageId;

  #[test]
  fn serialize_npm_package_id() {
    let id = NpmPackageId {
      nv: NpmPackageNv {
        name: "pkg-a".to_string(),
        version: Version::parse_from_npm("1.2.3").unwrap(),
      },
      peer_dependencies: vec![
        NpmPackageId {
          nv: NpmPackageNv {
            name: "pkg-b".to_string(),
            version: Version::parse_from_npm("3.2.1").unwrap(),
          },
          peer_dependencies: vec![
            NpmPackageId {
              nv: NpmPackageNv {
                name: "pkg-c".to_string(),
                version: Version::parse_from_npm("1.3.2").unwrap(),
              },
              peer_dependencies: vec![],
            },
            NpmPackageId {
              nv: NpmPackageNv {
                name: "pkg-d".to_string(),
                version: Version::parse_from_npm("2.3.4").unwrap(),
              },
              peer_dependencies: vec![],
            },
          ],
        },
        NpmPackageId {
          nv: NpmPackageNv {
            name: "pkg-e".to_string(),
            version: Version::parse_from_npm("2.3.1").unwrap(),
          },
          peer_dependencies: vec![NpmPackageId {
            nv: NpmPackageNv {
              name: "pkg-f".to_string(),
              version: Version::parse_from_npm("2.3.1").unwrap(),
            },
            peer_dependencies: vec![],
          }],
        },
      ],
    };

    // this shouldn't change because it's used in the lockfile
    let serialized = id.as_serialized();
    assert_eq!(serialized, "pkg-a@1.2.3_pkg-b@3.2.1__pkg-c@1.3.2__pkg-d@2.3.4_pkg-e@2.3.1__pkg-f@2.3.1");
    assert_eq!(NpmPackageId::from_serialized(&serialized).unwrap(), id);
  }
}
