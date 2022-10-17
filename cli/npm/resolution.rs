// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;

use super::cache::should_sync_download;
use super::registry::NpmPackageInfo;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmPackageVersionInfo;
use super::registry::NpmRegistryApi;
use super::semver::NpmVersion;
use super::semver::SpecifierVersionReq;

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
      write!(f, "{}/{}", self.req, sub_path)
    } else {
      write!(f, "{}", self.req)
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
}

impl std::fmt::Display for NpmPackageId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}@{}", self.name, self.version)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmResolutionPackage {
  pub id: NpmPackageId,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NpmResolutionSnapshot {
  #[serde(with = "map_to_vec")]
  package_reqs: HashMap<NpmPackageReq, NpmVersion>,
  packages_by_name: HashMap<String, Vec<NpmVersion>>,
  #[serde(with = "map_to_vec")]
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
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
      Some(version) => Ok(
        self
          .packages
          .get(&NpmPackageId {
            name: req.name.clone(),
            version: version.clone(),
          })
          .unwrap(),
      ),
      None => bail!("could not find npm package directory for '{}'", req),
    }
  }

  pub fn top_level_packages(&self) -> Vec<NpmPackageId> {
    self
      .package_reqs
      .iter()
      .map(|(req, version)| NpmPackageId {
        name: req.name.clone(),
        version: version.clone(),
      })
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
    referrer: &NpmPackageId,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    match self.packages.get(referrer) {
      Some(referrer_package) => {
        let name_ = name_without_path(name);
        if let Some(id) = referrer_package.dependencies.get(name_) {
          return Ok(self.packages.get(id).unwrap());
        }

        if referrer_package.id.name == name_ {
          return Ok(referrer_package);
        }

        // TODO(bartlomieju): this should use a reverse lookup table in the
        // snapshot instead of resolving best version again.
        let req = NpmPackageReq {
          name: name_.to_string(),
          version_req: None,
        };

        if let Some(version) = self.resolve_best_package_version(name_, &req) {
          let id = NpmPackageId {
            name: name_.to_string(),
            version,
          };
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
      None => bail!("could not find referrer npm package '{}'", referrer),
    }
  }

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.packages.values().cloned().collect()
  }

  pub fn resolve_best_package_version(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Option<NpmVersion> {
    let mut maybe_best_version: Option<&NpmVersion> = None;
    if let Some(versions) = self.packages_by_name.get(name) {
      for version in versions {
        if version_matcher.matches(version) {
          let is_best_version = maybe_best_version
            .as_ref()
            .map(|best_version| (*best_version).cmp(version).is_lt())
            .unwrap_or(true);
          if is_best_version {
            maybe_best_version = Some(version);
          }
        }
      }
    }
    maybe_best_version.cloned()
  }
}

pub struct NpmResolution {
  api: NpmRegistryApi,
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
    api: NpmRegistryApi,
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
    mut package_reqs: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // multiple packages are resolved in alphabetical order
    package_reqs.sort_by(|a, b| a.name.cmp(&b.name));

    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
    let mut snapshot = self.snapshot.read().clone();
    let mut pending_dependencies = VecDeque::new();

    // go over the top level packages first, then down the
    // tree one level at a time through all the branches
    let mut unresolved_tasks = Vec::with_capacity(package_reqs.len());
    for package_req in package_reqs {
      if snapshot.package_reqs.contains_key(&package_req) {
        // skip analyzing this package, as there's already a matching top level package
        continue;
      }
      // inspect the list of current packages
      if let Some(version) =
        snapshot.resolve_best_package_version(&package_req.name, &package_req)
      {
        snapshot.package_reqs.insert(package_req, version);
        continue; // done, no need to continue
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

    for result in futures::future::join_all(unresolved_tasks).await {
      let (package_req, info) = result??;
      let version_and_info = get_resolved_package_version_and_info(
        &package_req.name,
        &package_req,
        info,
        None,
      )?;
      let id = NpmPackageId {
        name: package_req.name.clone(),
        version: version_and_info.version.clone(),
      };
      let dependencies = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| format!("npm package: {}", id))?;

      pending_dependencies.push_back((id.clone(), dependencies));
      snapshot.packages.insert(
        id.clone(),
        NpmResolutionPackage {
          id,
          dist: version_and_info.info.dist,
          dependencies: Default::default(),
        },
      );
      snapshot
        .packages_by_name
        .entry(package_req.name.clone())
        .or_default()
        .push(version_and_info.version.clone());
      snapshot
        .package_reqs
        .insert(package_req, version_and_info.version);
    }

    // now go down through the dependencies by tree depth
    while let Some((parent_package_id, mut deps)) =
      pending_dependencies.pop_front()
    {
      // sort the dependencies alphabetically by name then by version descending
      deps.sort_by(|a, b| match a.name.cmp(&b.name) {
        // sort by newest to oldest
        Ordering::Equal => b
          .version_req
          .version_text()
          .cmp(&a.version_req.version_text()),
        ordering => ordering,
      });

      // cache all the dependencies' registry infos in parallel if should
      if !should_sync_download() {
        let handles = deps
          .iter()
          .map(|dep| {
            let name = dep.name.clone();
            let api = self.api.clone();
            tokio::task::spawn(async move {
              // it's ok to call this without storing the result, because
              // NpmRegistryApi will cache the package info in memory
              api.package_info(&name).await
            })
          })
          .collect::<Vec<_>>();
        let results = futures::future::join_all(handles).await;
        for result in results {
          result??; // surface the first error
        }
      }

      // now resolve them
      for dep in deps {
        // check if an existing dependency matches this
        let id = if let Some(version) =
          snapshot.resolve_best_package_version(&dep.name, &dep.version_req)
        {
          NpmPackageId {
            name: dep.name.clone(),
            version,
          }
        } else {
          // get the information
          let info = self.api.package_info(&dep.name).await?;
          let version_and_info = get_resolved_package_version_and_info(
            &dep.name,
            &dep.version_req,
            info,
            None,
          )?;
          let dependencies = version_and_info
            .info
            .dependencies_as_entries()
            .with_context(|| {
              format!("npm package: {}@{}", dep.name, version_and_info.version)
            })?;

          let id = NpmPackageId {
            name: dep.name.clone(),
            version: version_and_info.version.clone(),
          };
          pending_dependencies.push_back((id.clone(), dependencies));
          snapshot.packages.insert(
            id.clone(),
            NpmResolutionPackage {
              id: id.clone(),
              dist: version_and_info.info.dist,
              dependencies: Default::default(),
            },
          );
          snapshot
            .packages_by_name
            .entry(dep.name.clone())
            .or_default()
            .push(id.version.clone());

          id
        };

        // add this version as a dependency of the package
        snapshot
          .packages
          .get_mut(&parent_package_id)
          .unwrap()
          .dependencies
          .insert(dep.bare_specifier.clone(), id);
      }
    }

    *self.snapshot.write() = snapshot;
    Ok(())
  }

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageId,
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

  pub fn has_packages(&self) -> bool {
    !self.snapshot.read().packages.is_empty()
  }

  pub fn snapshot(&self) -> NpmResolutionSnapshot {
    self.snapshot.read().clone()
  }
}

#[derive(Clone)]
struct VersionAndInfo {
  version: NpmVersion,
  info: NpmPackageVersionInfo,
}

fn get_resolved_package_version_and_info(
  pkg_name: &str,
  version_matcher: &impl NpmVersionMatcher,
  info: NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo, AnyError> {
  let mut maybe_best_version: Option<VersionAndInfo> = None;
  if let Some(tag) = version_matcher.tag() {
    if let Some(version) = info.dist_tags.get(tag) {
      match info.versions.get(version) {
        Some(info) => {
          return Ok(VersionAndInfo {
            version: NpmVersion::parse(version)?,
            info: info.clone(),
          });
        }
        None => {
          bail!(
            "Could not find version '{}' referenced in dist-tag '{}'.",
            version,
            tag,
          )
        }
      }
    } else {
      bail!("Could not find dist-tag '{}'.", tag,)
    }
  } else {
    for (_, version_info) in info.versions.into_iter() {
      let version = NpmVersion::parse(&version_info.version)?;
      if version_matcher.matches(&version) {
        let is_best_version = maybe_best_version
          .as_ref()
          .map(|best_version| best_version.version.cmp(&version).is_lt())
          .unwrap_or(true);
        if is_best_version {
          maybe_best_version = Some(VersionAndInfo {
            version,
            info: version_info,
          });
        }
      }
    }
  }

  match maybe_best_version {
    Some(v) => Ok(v),
    // If the package isn't found, it likely means that the user needs to use
    // `--reload` to get the latest npm package information. Although it seems
    // like we could make this smart by fetching the latest information for
    // this package here, we really need a full restart. There could be very
    // interesting bugs that occur if this package's version was resolved by
    // something previous using the old information, then now being smart here
    // causes a new fetch of the package information, meaning this time the
    // previous resolution of this package's version resolved to an older
    // version, but next time to a different version because it has new information.
    None => bail!(
      concat!(
        "Could not find npm package '{}' matching {}{}. ",
        "Try retrieving the latest npm package information by running with --reload",
      ),
      pkg_name,
      version_matcher.version_text(),
      match parent {
        Some(id) => format!(" as specified in {}", id),
        None => String::new(),
      }
    ),
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
  fn test_name_without_path() {
    assert_eq!(name_without_path("foo"), "foo");
    assert_eq!(name_without_path("@foo/bar"), "@foo/bar");
    assert_eq!(name_without_path("@foo/bar/baz"), "@foo/bar");
    assert_eq!(name_without_path("@hello"), "@hello");
  }

  #[test]
  fn test_get_resolved_package_version_and_info() {
    // dist tag where version doesn't exist
    let package_ref = NpmPackageReference::from_str("npm:test").unwrap();
    let result = get_resolved_package_version_and_info(
      "test",
      &package_ref.req,
      NpmPackageInfo {
        name: "test".to_string(),
        versions: HashMap::new(),
        dist_tags: HashMap::from([(
          "latest".to_string(),
          "1.0.0-alpha".to_string(),
        )]),
      },
      None,
    );
    assert_eq!(
      result.err().unwrap().to_string(),
      "Could not find version '1.0.0-alpha' referenced in dist-tag 'latest'."
    );

    // dist tag where version is a pre-release
    let package_ref = NpmPackageReference::from_str("npm:test").unwrap();
    let result = get_resolved_package_version_and_info(
      "test",
      &package_ref.req,
      NpmPackageInfo {
        name: "test".to_string(),
        versions: HashMap::from([
          ("0.1.0".to_string(), NpmPackageVersionInfo::default()),
          (
            "1.0.0-alpha".to_string(),
            NpmPackageVersionInfo {
              version: "0.1.0-alpha".to_string(),
              ..Default::default()
            },
          ),
        ]),
        dist_tags: HashMap::from([(
          "latest".to_string(),
          "1.0.0-alpha".to_string(),
        )]),
      },
      None,
    );
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-alpha");
  }
}
