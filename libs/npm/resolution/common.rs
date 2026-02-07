// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;

use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::WILDCARD_VERSION_REQ;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use thiserror::Error;

use crate::registry::NpmPackageInfo;
use crate::registry::NpmPackageVersionInfo;
use crate::registry::NpmPackageVersionInfosIterator;

/// Error that occurs when the version is not found in the package information.
#[derive(Debug, Error, Clone, deno_error::JsError)]
#[class(type)]
#[error("Could not find version '{}' for npm package '{}'.", .0.version, .0.name)]
pub struct NpmPackageVersionNotFound(pub PackageNv);

#[derive(Debug, Error, Clone, deno_error::JsError)]
pub enum NpmPackageVersionResolutionError {
  #[class(type)]
  #[error(
    "Could not find dist-tag '{dist_tag}' for npm package '{package_name}'."
  )]
  DistTagNotFound {
    dist_tag: String,
    package_name: StackString,
  },
  #[class(type)]
  #[error(
    "Could not find version '{version}' referenced in dist-tag '{dist_tag}' for npm package '{package_name}'."
  )]
  DistTagVersionNotFound {
    package_name: StackString,
    dist_tag: String,
    version: String,
  },
  #[class(type)]
  #[error(
    "Failed resolving tag '{package_name}@{dist_tag}' mapped to '{package_name}@{version}' because the package version was published at {publish_date}, but dependencies newer than {newest_dependency_date} are not allowed because it is newer than the specified minimum dependency date."
  )]
  DistTagVersionTooNew {
    package_name: StackString,
    dist_tag: String,
    version: String,
    publish_date: chrono::DateTime<chrono::Utc>,
    newest_dependency_date: NewestDependencyDate,
  },
  #[class(inherit)]
  #[error(transparent)]
  VersionNotFound(#[from] NpmPackageVersionNotFound),
  #[class(type)]
  #[error(
    "Could not find npm package '{}' matching '{}'.{}", package_name, version_req, newest_dependency_date.map(|v| format!("\n\nA newer matching version was found, but it was not used because it was newer than the specified minimum dependency date of {}.", v)).unwrap_or_else(String::new)
  )]
  VersionReqNotMatched {
    package_name: StackString,
    version_req: VersionReq,
    newest_dependency_date: Option<NewestDependencyDate>,
  },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NewestDependencyDate(pub chrono::DateTime<chrono::Utc>);

impl std::fmt::Display for NewestDependencyDate {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl NewestDependencyDate {
  pub fn matches(&self, date: chrono::DateTime<chrono::Utc>) -> bool {
    date < self.0
  }
}

#[derive(Debug, Default, Clone)]
pub struct NewestDependencyDateOptions {
  /// Prevents installing packages newer than the specified date.
  pub date: Option<NewestDependencyDate>,
  pub exclude: BTreeSet<PackageName>,
}

impl NewestDependencyDateOptions {
  pub fn from_date(date: chrono::DateTime<chrono::Utc>) -> Self {
    Self {
      date: Some(NewestDependencyDate(date)),
      exclude: Default::default(),
    }
  }

  pub fn get_for_package(
    &self,
    package_name: &PackageName,
  ) -> Option<NewestDependencyDate> {
    let date = self.date?;
    if self.exclude.contains(package_name) {
      None
    } else {
      Some(date)
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct NpmVersionResolver {
  /// Packages that are marked as "links" in the config file.
  pub link_packages: Arc<HashMap<PackageName, Vec<NpmPackageVersionInfo>>>,
  pub newest_dependency_date_options: NewestDependencyDateOptions,
}

impl NpmVersionResolver {
  pub fn get_for_package<'a>(
    &'a self,
    info: &'a NpmPackageInfo,
  ) -> NpmPackageVersionResolver<'a> {
    NpmPackageVersionResolver {
      info,
      newest_dependency_date: self
        .newest_dependency_date_options
        .get_for_package(&info.name),
      link_packages: self.link_packages.get(&info.name),
    }
  }
}

pub struct NpmPackageVersionResolver<'a> {
  info: &'a NpmPackageInfo,
  link_packages: Option<&'a Vec<NpmPackageVersionInfo>>,
  newest_dependency_date: Option<NewestDependencyDate>,
}

impl<'a> NpmPackageVersionResolver<'a> {
  pub fn info(&self) -> &'a NpmPackageInfo {
    self.info
  }

  /// Gets the version infos that match the link packages and newest dependency date.
  pub fn applicable_version_infos(&self) -> NpmPackageVersionInfosIterator<'a> {
    NpmPackageVersionInfosIterator::new(
      self.info,
      self.link_packages,
      self.newest_dependency_date,
    )
  }

  pub fn version_req_satisfies_and_matches_newest_dependency_date(
    &self,
    version_req: &VersionReq,
    version: &Version,
  ) -> Result<bool, NpmPackageVersionResolutionError> {
    Ok(
      self.version_req_satisfies(version_req, version)?
        && self.matches_newest_dependency_date(version),
    )
  }

  pub fn version_req_satisfies(
    &self,
    version_req: &VersionReq,
    version: &Version,
  ) -> Result<bool, NpmPackageVersionResolutionError> {
    match version_req.tag() {
      Some(tag) => {
        let version_info = self.tag_to_version_info(tag)?;
        Ok(version_info.version == *version)
      }
      None => Ok(version_req.matches(version)),
    }
  }

  /// Gets if the provided version should be ignored or not
  /// based on the `newest_dependency_date`.
  pub fn matches_newest_dependency_date(&self, version: &Version) -> bool {
    match self.newest_dependency_date {
      Some(newest_dependency_date) => match self.info.time.get(version) {
        Some(date) => newest_dependency_date.matches(*date),
        None => true,
      },
      None => true,
    }
  }

  pub fn resolve_best_package_version_info<'version>(
    &self,
    version_req: &VersionReq,
    existing_versions: impl Iterator<Item = &'version Version>,
  ) -> Result<&'a NpmPackageVersionInfo, NpmPackageVersionResolutionError> {
    // always attempt to resolve from the linked packages first
    if let Some(version_infos) = self.link_packages {
      let mut best_version: Option<&'a NpmPackageVersionInfo> = None;
      for version_info in version_infos {
        let version = &version_info.version;
        if self.version_req_satisfies(version_req, version)? {
          let is_greater =
            best_version.map(|c| *version > c.version).unwrap_or(true);
          if is_greater {
            best_version = Some(version_info);
          }
        }
      }
      if let Some(top_version) = best_version {
        return Ok(top_version);
      }
    }

    if let Some(version) = self
      .resolve_best_from_existing_versions(version_req, existing_versions)?
    {
      match self.info.versions.get(version) {
        Some(version_info) => Ok(version_info),
        None => Err(NpmPackageVersionResolutionError::VersionNotFound(
          NpmPackageVersionNotFound(PackageNv {
            name: self.info.name.clone(),
            version: version.clone(),
          }),
        )),
      }
    } else {
      // get the information
      self.get_resolved_package_version_and_info(version_req)
    }
  }

  fn get_resolved_package_version_and_info(
    &self,
    version_req: &VersionReq,
  ) -> Result<&'a NpmPackageVersionInfo, NpmPackageVersionResolutionError> {
    let mut found_matching_version = false;
    if let Some(tag) = version_req.tag() {
      self.tag_to_version_info(tag)
      // When the version is *, if there is a latest tag, use it directly.
      // No need to care about @types/node here, because it'll be handled specially below.
    } else if self.info.dist_tags.contains_key("latest")
      && self.info.name != "@types/node"
      // When the latest tag satisfies the version requirement, use it directly.
      // https://github.com/npm/npm-pick-manifest/blob/67508da8e21f7317e3159765006da0d6a0a61f84/lib/index.js#L125
      && self.info
        .dist_tags
        .get("latest")
        .filter(|version| self.matches_newest_dependency_date(version))
        .map(|version| {
          *version_req == *WILDCARD_VERSION_REQ ||
          self.version_req_satisfies(version_req, version).ok().unwrap_or(false)
        })
        .unwrap_or(false)
    {
      self.tag_to_version_info("latest")
    } else {
      let mut maybe_best_version: Option<&'a NpmPackageVersionInfo> = None;
      for version_info in self.info.versions.values() {
        let version = &version_info.version;
        if self.version_req_satisfies(version_req, version)? {
          found_matching_version = true;
          if self.matches_newest_dependency_date(version) {
            let is_best_version = maybe_best_version
              .as_ref()
              .map(|best_version| best_version.version.cmp(version).is_lt())
              .unwrap_or(true);
            if is_best_version {
              maybe_best_version = Some(version_info);
            }
          }
        }
      }

      match maybe_best_version {
        Some(v) => Ok(v),
        // Although it seems like we could make this smart by fetching the latest
        // information for this package here, we really need a full restart. There
        // could be very interesting bugs that occur if this package's version was
        // resolved by something previous using the old information, then now being
        // smart here causes a new fetch of the package information, meaning this
        // time the previous resolution of this package's version resolved to an older
        // version, but next time to a different version because it has new information.
        None => Err(NpmPackageVersionResolutionError::VersionReqNotMatched {
          package_name: self.info.name.clone(),
          version_req: version_req.clone(),
          newest_dependency_date: found_matching_version
            .then_some(self.newest_dependency_date)
            .flatten(),
        }),
      }
    }
  }

  fn resolve_best_from_existing_versions<'b>(
    &self,
    version_req: &VersionReq,
    existing_versions: impl Iterator<Item = &'b Version>,
  ) -> Result<Option<&'b Version>, NpmPackageVersionResolutionError> {
    let mut maybe_best_version: Option<&Version> = None;
    for version in existing_versions {
      if self.version_req_satisfies(version_req, version)? {
        let is_best_version = maybe_best_version
          .as_ref()
          .map(|best_version| (*best_version).cmp(version).is_lt())
          .unwrap_or(true);
        if is_best_version {
          maybe_best_version = Some(version);
        }
      }
    }
    Ok(maybe_best_version)
  }

  fn tag_to_version_info(
    &self,
    tag: &str,
  ) -> Result<&'a NpmPackageVersionInfo, NpmPackageVersionResolutionError> {
    if let Some(version) = self.info.dist_tags.get(tag) {
      match self.info.versions.get(version) {
        Some(version_info) => {
          if self.matches_newest_dependency_date(version) {
            Ok(version_info)
          } else {
            Err(NpmPackageVersionResolutionError::DistTagVersionTooNew {
              package_name: self.info.name.clone(),
              dist_tag: tag.to_string(),
              version: version.to_string(),
              newest_dependency_date: self.newest_dependency_date.unwrap(),
              publish_date: *self.info.time.get(version).unwrap(),
            })
          }
        }
        None => Err(NpmPackageVersionResolutionError::DistTagVersionNotFound {
          package_name: self.info.name.clone(),
          dist_tag: tag.to_string(),
          version: version.to_string(),
        }),
      }
    } else {
      Err(NpmPackageVersionResolutionError::DistTagNotFound {
        package_name: self.info.name.clone(),
        dist_tag: tag.to_string(),
      })
    }
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use deno_semver::package::PackageReq;

  use super::*;

  #[test]
  fn test_get_resolved_package_version_and_info() {
    // dist tag where version doesn't exist
    let package_req = PackageReq::from_str("test@latest").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::new(),
      dist_tags: HashMap::from([(
        "latest".into(),
        Version::parse_from_npm("1.0.0-alpha").unwrap(),
      )]),
      time: Default::default(),
    };
    let resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: Default::default(),
    };
    let package_version_resolver = resolver.get_for_package(&package_info);
    let result = package_version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(
      result.err().unwrap().to_string(),
      "Could not find version '1.0.0-alpha' referenced in dist-tag 'latest' for npm package 'test'."
    );

    // dist tag where version is a pre-release
    let package_req = PackageReq::from_str("test@latest").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([
        (
          Version::parse_from_npm("0.1.0").unwrap(),
          NpmPackageVersionInfo::default(),
        ),
        (
          Version::parse_from_npm("1.0.0-alpha").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("1.0.0-alpha").unwrap(),
            ..Default::default()
          },
        ),
      ]),
      dist_tags: HashMap::from([(
        "latest".into(),
        Version::parse_from_npm("1.0.0-alpha").unwrap(),
      )]),
      time: Default::default(),
    };
    let version_resolver = resolver.get_for_package(&package_info);
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-alpha");
  }

  #[test]
  fn test_wildcard_version_req() {
    let package_req = PackageReq::from_str("some-pkg").unwrap();
    let package_info = NpmPackageInfo {
      name: "some-pkg".into(),
      versions: HashMap::from([
        (
          Version::parse_from_npm("1.0.0-rc.1").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("1.0.0-rc.1").unwrap(),
            ..Default::default()
          },
        ),
        (
          Version::parse_from_npm("2.0.0").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("2.0.0").unwrap(),
            ..Default::default()
          },
        ),
      ]),
      dist_tags: HashMap::from([(
        "latest".into(),
        Version::parse_from_npm("1.0.0-rc.1").unwrap(),
      )]),
      time: Default::default(),
    };
    let resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: Default::default(),
    };
    let version_resolver = resolver.get_for_package(&package_info);
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-rc.1");
  }

  #[test]
  fn test_latest_tag_version_req() {
    let package_info = NpmPackageInfo {
      name: "some-pkg".into(),
      versions: HashMap::from([
        (
          Version::parse_from_npm("0.1.0-alpha.1").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("0.1.0-alpha.1").unwrap(),
            ..Default::default()
          },
        ),
        (
          Version::parse_from_npm("0.1.0-alpha.2").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("0.1.0-alpha.2").unwrap(),
            ..Default::default()
          },
        ),
        (
          Version::parse_from_npm("0.1.0-beta.1").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("0.1.0-beta.1").unwrap(),
            ..Default::default()
          },
        ),
        (
          Version::parse_from_npm("0.1.0-beta.2").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("0.1.0-beta.2").unwrap(),
            ..Default::default()
          },
        ),
      ]),
      dist_tags: HashMap::from([
        (
          "latest".into(),
          Version::parse_from_npm("0.1.0-alpha.2").unwrap(),
        ),
        (
          "dev".into(),
          Version::parse_from_npm("0.1.0-beta.2").unwrap(),
        ),
      ]),
      time: Default::default(),
    };
    let resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: Default::default(),
    };

    // check for when matches dist tag
    let package_req = PackageReq::from_str("some-pkg@^0.1.0-alpha.2").unwrap();
    let version_resolver = resolver.get_for_package(&package_info);
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(
      result.unwrap().version.to_string(),
      "0.1.0-alpha.2" // not "0.1.0-beta.2"
    );

    // check for when not matches dist tag
    let package_req = PackageReq::from_str("some-pkg@^0.1.0-beta.2").unwrap();
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(result.unwrap().version.to_string(), "0.1.0-beta.2");
  }
}
