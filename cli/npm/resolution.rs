// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::VecDeque;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;

use super::registry::NpmPackageInfo;
use super::registry::NpmPackageVersionDistInfo;
use super::registry::NpmPackageVersionInfo;
use super::registry::NpmRegistryApi;

/// The version matcher used for npm schemed urls is more strict than
/// the one used by npm packages.
pub trait NpmVersionMatcher {
  fn matches(&self, version: &semver::Version) -> bool;
  fn version_text(&self) -> String;
}

#[derive(Clone, Debug, Default)]
pub struct NpmPackageReference {
  pub req: NpmPackageReq,
  pub sub_path: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NpmPackageReq {
  pub name: String,
  pub version_req: Option<semver::VersionReq>,
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
  fn matches(&self, version: &semver::Version) -> bool {
    match &self.version_req {
      Some(req) => req.matches(version),
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
        bail!("Not an npm specifier: '{}'", specifier);
      }
    };
    let (name, version_req) = match specifier.rsplit_once('@') {
      Some((name, version_req)) => (
        name,
        match semver::VersionReq::parse(version_req) {
          Ok(v) => Some(v),
          Err(_) => None, // not a version requirement
        },
      ),
      None => (specifier, None),
    };
    Ok(NpmPackageReference {
      req: NpmPackageReq {
        name: name.to_string(),
        version_req,
      },
      // todo: implement and support this
      sub_path: None,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NpmPackageId {
  pub name: String,
  pub version: semver::Version,
}

impl NpmPackageId {
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

#[derive(Debug, Clone)]
pub struct NpmResolutionPackage {
  pub id: NpmPackageId,
  pub dist: NpmPackageVersionDistInfo,
  /// Key is what the package refers to the other package as,
  /// which could be different from the package name.
  pub dependencies: HashMap<String, NpmPackageId>,
}

#[derive(Debug, Clone, Default)]
pub struct NpmResolutionSnapshot {
  package_reqs: HashMap<NpmPackageReq, semver::Version>,
  packages_by_name: HashMap<String, Vec<semver::Version>>,
  packages: HashMap<NpmPackageId, NpmResolutionPackage>,
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

  pub fn resolve_package_from_package(
    &self,
    name: &str,
    referrer: &NpmPackageId,
  ) -> Result<&NpmResolutionPackage, AnyError> {
    match self.packages.get(referrer) {
      Some(referrer_package) => match referrer_package.dependencies.get(name) {
        Some(id) => Ok(self.packages.get(id).unwrap()),
        None => {
          bail!(
            "could not find package '{}' referenced by '{}'",
            name,
            referrer
          )
        }
      },
      None => bail!("could not find referrer package '{}'", referrer),
    }
  }

  pub fn all_packages(&self) -> Vec<NpmResolutionPackage> {
    self.packages.values().cloned().collect()
  }

  pub fn resolve_best_package_version(
    &self,
    name: &str,
    version_matcher: &impl NpmVersionMatcher,
  ) -> Option<semver::Version> {
    let mut maybe_best_version: Option<&semver::Version> = None;
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
  pub fn new(api: NpmRegistryApi) -> Self {
    Self {
      api,
      snapshot: Default::default(),
      update_sempahore: tokio::sync::Semaphore::new(1),
    }
  }

  pub async fn add_package_reqs(
    &self,
    mut packages: Vec<NpmPackageReq>,
  ) -> Result<(), AnyError> {
    // multiple packages are resolved in alphabetical order
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    // only allow one thread in here at a time
    let _permit = self.update_sempahore.acquire().await.unwrap();
    let mut snapshot = self.snapshot.read().clone();
    let mut pending_dependencies = VecDeque::new();

    // go over the top level packages first, then down the
    // tree one level at a time through all the branches
    for package_ref in packages {
      if snapshot.package_reqs.contains_key(&package_ref) {
        // skip analyzing this package, as there's already a matching top level package
        continue;
      }
      // inspect the list of current packages
      if let Some(version) =
        snapshot.resolve_best_package_version(&package_ref.name, &package_ref)
      {
        snapshot.package_reqs.insert(package_ref, version);
        continue; // done, no need to continue
      }

      // no existing best version, so resolve the current packages
      let info = self.api.package_info(&package_ref.name).await?;
      let version_and_info = get_resolved_package_version_and_info(
        &package_ref.name,
        &package_ref,
        info,
        None,
      )?;
      let id = NpmPackageId {
        name: package_ref.name.clone(),
        version: version_and_info.version.clone(),
      };
      let dependencies = version_and_info
        .info
        .dependencies_as_entries()
        .with_context(|| format!("Package: {}", id))?;

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
        .entry(package_ref.name.clone())
        .or_default()
        .push(version_and_info.version.clone());
      snapshot
        .package_reqs
        .insert(package_ref, version_and_info.version);
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
              format!("Package: {}@{}", dep.name, version_and_info.version)
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
  version: semver::Version,
  info: NpmPackageVersionInfo,
}

fn get_resolved_package_version_and_info(
  pkg_name: &str,
  version_matcher: &impl NpmVersionMatcher,
  info: NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo, AnyError> {
  let mut maybe_best_version: Option<VersionAndInfo> = None;
  for (_, version_info) in info.versions.into_iter() {
    let version = semver::Version::parse(&version_info.version)?;
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
        "Could not find package '{}' matching {}{}. ",
        "Try retreiving the latest npm package information by running with --reload",
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
