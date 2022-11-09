// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;

use crate::lockfile::Lockfile;

use self::graph::GraphDependencyResolver;
use self::snapshot::NpmPackagesPartitioned;

use super::cache::should_sync_download;
use super::cache::NpmPackageCacheFolderId;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::RealNpmRegistryApi;
use super::semver::NpmVersion;
use super::semver::SpecifierVersionReq;
use super::NpmRegistryApi;

mod graph;
mod snapshot;

use graph::Graph;
pub use snapshot::NpmResolutionSnapshot;

/// The version matcher used for npm schemed urls is more strict than
/// the one used by npm packages and so we represent either via a trait.
pub trait NpmVersionMatcher {
  fn tag(&self) -> Option<&str>;
  fn matches(&self, version: &NpmVersion) -> bool;
  fn version_text(&self) -> String;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpmPackageReference {
  pub req: NpmPackageReq,
  pub sub_path: Option<String>,
}

impl NpmPackageReference {
  pub fn from_specifier(
    specifier: &ModuleSpecifier,
  ) -> Result<NpmPackageReference, AnyError> {
    Self::from_str(specifier.as_str())
  }

  pub fn from_str(specifier: &str) -> Result<NpmPackageReference, AnyError> {
    let specifier = match specifier.strip_prefix("npm:") {
      Some(s) => s,
      None => {
        bail!("Not an npm specifier: {}", specifier);
      }
    };
    let parts = specifier.split('/').collect::<Vec<_>>();
    let name_part_len = if specifier.starts_with('@') { 2 } else { 1 };
    if parts.len() < name_part_len {
      return Err(generic_error(format!("Not a valid package: {}", specifier)));
    }
    let name_parts = &parts[0..name_part_len];
    let last_name_part = &name_parts[name_part_len - 1];
    let (name, version_req) = if let Some(at_index) = last_name_part.rfind('@')
    {
      let version = &last_name_part[at_index + 1..];
      let last_name_part = &last_name_part[..at_index];
      let version_req = SpecifierVersionReq::parse(version)
        .with_context(|| "Invalid version requirement.")?;
      let name = if name_part_len == 1 {
        last_name_part.to_string()
      } else {
        format!("{}/{}", name_parts[0], last_name_part)
      };
      (name, Some(version_req))
    } else {
      (name_parts.join("/"), None)
    };
    let sub_path = if parts.len() == name_parts.len() {
      None
    } else {
      Some(parts[name_part_len..].join("/"))
    };

    if let Some(sub_path) = &sub_path {
      if let Some(at_index) = sub_path.rfind('@') {
        let (new_sub_path, version) = sub_path.split_at(at_index);
        let msg = format!(
          "Invalid package specifier 'npm:{}/{}'. Did you mean to write 'npm:{}{}/{}'?",
          name, sub_path, name, version, new_sub_path
        );
        return Err(generic_error(msg));
      }
    }

    Ok(NpmPackageReference {
      req: NpmPackageReq { name, version_req },
      sub_path,
    })
  }
}

impl std::fmt::Display for NpmPackageReference {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(sub_path) = &self.sub_path {
      write!(f, "npm:{}/{}", self.req, sub_path)
    } else {
      write!(f, "npm:{}", self.req)
    }
  }
}

#[derive(
  Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageReq {
  pub name: String,
  pub version_req: Option<SpecifierVersionReq>,
}

impl std::fmt::Display for NpmPackageReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.version_req {
      Some(req) => write!(f, "{}@{}", self.name, req),
      None => write!(f, "{}", self.name),
    }
  }
}

impl NpmPackageReq {
  pub fn from_str(text: &str) -> Result<Self, AnyError> {
    // probably should do something more targetted in the future
    let reference = NpmPackageReference::from_str(&format!("npm:{}", text))?;
    Ok(reference.req)
  }
}

impl NpmVersionMatcher for NpmPackageReq {
  fn tag(&self) -> Option<&str> {
    match &self.version_req {
      Some(version_req) => version_req.tag(),
      None => Some("latest"),
    }
  }

  fn matches(&self, version: &NpmVersion) -> bool {
    match self.version_req.as_ref() {
      Some(req) => {
        assert_eq!(self.tag(), None);
        match req.range() {
          Some(range) => range.satisfies(version),
          None => false,
        }
      }
      None => version.pre.is_empty(),
    }
  }

  fn version_text(&self) -> String {
    self
      .version_req
      .as_ref()
      .map(|v| format!("{}", v))
      .unwrap_or_else(|| "non-prerelease".to_string())
  }
}

#[derive(
  Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageId {
  pub name: String,
  pub version: NpmVersion,
  pub peer_dependencies: Vec<NpmPackageId>,
}

impl NpmPackageId {
  #[allow(unused)]
  pub fn scope(&self) -> Option<&str> {
    if self.name.starts_with('@') && self.name.contains('/') {
      self.name.split('/').next()
    } else {
      None
    }
  }

  pub fn as_serialized(&self) -> String {
    self.as_serialized_with_level(0)
  }

  fn as_serialized_with_level(&self, level: usize) -> String {
    // WARNING: This should not change because it's used in the lockfile
    let mut result = format!(
      "{}@{}",
      if level == 0 {
        self.name.to_string()
      } else {
        self.name.replace('/', "+")
      },
      self.version
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

  pub fn from_serialized(id: &str) -> Result<Self, AnyError> {
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

    fn parse_name_and_version(
      input: &str,
    ) -> ParseResult<(String, NpmVersion)> {
      let (input, name) = parse_name(input)?;
      let (input, _) = ch('@')(input)?;
      let at_version_input = input;
      let (input, version) = parse_version(input)?;
      match NpmVersion::parse(version) {
        Ok(version) => Ok((input, (name.to_string(), version))),
        Err(err) => ParseError::fail(at_version_input, format!("{:#}", err)),
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
            name,
            version,
            peer_dependencies,
          },
        ))
      }
    }

    with_failure_handling(parse_id_at_level(0))(id)
      .with_context(|| format!("Invalid npm package id '{}'.", id))
  }

  pub fn display(&self) -> String {
    // Don't implement std::fmt::Display because we don't
    // want this to be used by accident in certain scenarios.
    format!("{}@{}", self.name, self.version)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmResolutionPackage {
  pub id: NpmPackageId,
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

impl NpmResolutionPackage {
  pub fn get_package_cache_folder_id(&self) -> NpmPackageCacheFolderId {
    NpmPackageCacheFolderId {
      name: self.id.name.clone(),
      version: self.id.version.clone(),
      copy_index: self.copy_index,
    }
  }
}

pub struct NpmResolution {
  api: RealNpmRegistryApi,
  snapshot: RwLock<NpmResolutionSnapshot>,
  update_sempahore: tokio::sync::Semaphore,
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
    api: RealNpmRegistryApi,
    initial_snapshot: Option<NpmResolutionSnapshot>,
  ) -> Self {
    Self {
      api,
      snapshot: RwLock::new(initial_snapshot.unwrap_or_default()),
      update_sempahore: tokio::sync::Semaphore::new(1),
    }
  }

  pub async fn add_package_reqs(
    &self,
    package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
    let snapshot = self.snapshot.read().clone();

    let snapshot = self
      .add_package_reqs_to_snapshot(package_reqs, snapshot)
      .await?;

    *self.snapshot.write() = snapshot;
    Ok(())
  }

  pub async fn set_package_reqs(
    &self,
    package_reqs: HashSet<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
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
    let snapshot = self
      .add_package_reqs_to_snapshot(
        package_reqs.into_iter().collect(),
        snapshot,
      )
      .await?;

    *self.snapshot.write() = snapshot;

    Ok(())
  }

  async fn add_package_reqs_to_snapshot(
    &self,
    mut package_reqs: Vec<NpmPackageReq>,
    snapshot: NpmResolutionSnapshot,
  ) -> Result<NpmResolutionSnapshot, AnyError> {
    // convert the snapshot to a traversable graph
    let mut graph = Graph::from_snapshot(snapshot);

    // multiple packages are resolved in alphabetical order
    package_reqs.sort_by(|a, b| a.name.cmp(&b.name));

    // go over the top level packages first, then down the
    // tree one level at a time through all the branches
    let mut unresolved_tasks = Vec::with_capacity(package_reqs.len());
    for package_req in package_reqs {
      if graph.has_package_req(&package_req) {
        // skip analyzing this package, as there's already a matching top level package
        continue;
      }

      // no existing best version, so resolve the current packages
      let api = self.api.clone();
      let maybe_info = if should_sync_download() {
        // for deterministic test output
        Some(api.package_info(&package_req.name).await)
      } else {
        None
      };
      unresolved_tasks.push(tokio::task::spawn(async move {
        let info = match maybe_info {
          Some(info) => info?,
          None => api.package_info(&package_req.name).await?,
        };
        Result::<_, AnyError>::Ok((package_req, info))
      }));
    }

    let mut resolver = GraphDependencyResolver::new(&mut graph, &self.api);

    for result in futures::future::join_all(unresolved_tasks).await {
      let (package_req, info) = result??;
      resolver.add_package_req(&package_req, info)?;
    }

    resolver.resolve_pending().await?;

    graph.into_snapshot(&self.api).await
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

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.snapshot.read().all_packages()
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

  pub fn lock(
    &self,
    lockfile: &mut Lockfile,
    snapshot: &NpmResolutionSnapshot,
  ) -> Result<(), AnyError> {
    for (package_req, package_id) in snapshot.package_reqs.iter() {
      lockfile.insert_npm_specifier(package_req, package_id);
    }
    for package in self.all_packages() {
      lockfile.check_or_insert_npm_package(&package)?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_npm_package_ref() {
    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@1").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("1").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@^1.2").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("^1.2").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(SpecifierVersionReq::parse("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package")
        .err()
        .unwrap()
        .to_string(),
      "Not a valid package: @package"
    );
  }

  #[test]
  fn serialize_npm_package_id() {
    let id = NpmPackageId {
      name: "pkg-a".to_string(),
      version: NpmVersion::parse("1.2.3").unwrap(),
      peer_dependencies: vec![
        NpmPackageId {
          name: "pkg-b".to_string(),
          version: NpmVersion::parse("3.2.1").unwrap(),
          peer_dependencies: vec![
            NpmPackageId {
              name: "pkg-c".to_string(),
              version: NpmVersion::parse("1.3.2").unwrap(),
              peer_dependencies: vec![],
            },
            NpmPackageId {
              name: "pkg-d".to_string(),
              version: NpmVersion::parse("2.3.4").unwrap(),
              peer_dependencies: vec![],
            },
          ],
        },
        NpmPackageId {
          name: "pkg-e".to_string(),
          version: NpmVersion::parse("2.3.1").unwrap(),
          peer_dependencies: vec![NpmPackageId {
            name: "pkg-f".to_string(),
            version: NpmVersion::parse("2.3.1").unwrap(),
            peer_dependencies: vec![],
          }],
        },
      ],
    };
    let serialized = id.as_serialized();
    assert_eq!(serialized, "pkg-a@1.2.3_pkg-b@3.2.1__pkg-c@1.3.2__pkg-d@2.3.4_pkg-e@2.3.1__pkg-f@2.3.1");
    assert_eq!(NpmPackageId::from_serialized(&serialized).unwrap(), id);
  }
}
