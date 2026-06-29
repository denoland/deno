// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;

use deno_semver::RangeSetOrTag;
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
use crate::registry::NpmRegistryApi;
use crate::registry::NpmRegistryPackageInfoLoadError;
use crate::registry::TrustEvidence;
use crate::resolution::overrides::NpmOverrides;

/// Policy controlling whether a resolved version may have weaker publishing
/// trust evidence than an earlier-published version of the same package.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NpmTrustPolicy {
  /// Trust evidence is ignored during resolution (the default).
  #[default]
  Off,
  /// Refuse to resolve a version whose publishing-trust evidence is weaker
  /// than the strongest evidence on any earlier-published version of the same
  /// package. Mirrors pnpm's `trustPolicy: no-downgrade`.
  NoDowngrade,
}

/// The `trust-policy` configuration, resolved from `.npmrc`.
#[derive(Debug, Default, Clone)]
pub struct TrustPolicyOptions {
  /// The active trust policy.
  pub policy: NpmTrustPolicy,
  /// `trust-policy-ignore-after` cutoff: a version published strictly before
  /// this instant skips the downgrade check, on the theory that a genuine
  /// downgrade would have surfaced by now. Also lets genuinely pre-provenance
  /// releases install. Computed by the caller as `now - ignore_after`; `None`
  /// means "always check".
  pub ignore_after_cutoff: Option<chrono::DateTime<chrono::Utc>>,
  /// Package names exempted from the `no-downgrade` policy
  /// (`trust-policy-exclude[]`). An excluded package resolves as if the policy
  /// were off. Mirrors pnpm's `trustPolicyExclude`.
  pub exclude: Arc<BTreeSet<String>>,
}

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
  #[class(type)]
  #[error(
    "High-risk trust downgrade for npm package '{package_name}@{version}' (possible package takeover).\n\nThe 'no-downgrade' trust policy is based solely on publish date, not semver: a version cannot be installed if an earlier-published version of the same package had stronger publishing-trust evidence. Earlier versions had {past_evidence}, but this version has {current_evidence}. A trust downgrade may indicate a supply chain incident.\n\nReview the release, then set 'trust-policy=off' in .npmrc once you trust it (or use 'trust-policy-ignore-after')."
  )]
  TrustPolicyDowngrade {
    package_name: StackString,
    version: Version,
    past_evidence: &'static str,
    current_evidence: &'static str,
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
  /// npm overrides from the root package.json.
  pub overrides: Arc<NpmOverrides>,
  /// The active publishing-trust policy.
  pub trust_policy: TrustPolicyOptions,
}

impl NpmVersionResolver {
  pub fn get_for_package<'a>(
    &'a self,
    info: &'a NpmPackageInfo,
  ) -> NpmPackageVersionResolver<'a> {
    let trust_check = match self.trust_policy.policy {
      // `trust-policy-exclude[]` packages are resolved as if the policy were
      // off
      NpmTrustPolicy::NoDowngrade
        if !self
          .trust_policy
          .exclude
          .contains(&info.name.to_ascii_lowercase()) =>
      {
        Some(TrustDowngradeCheck {
          ignore_after_cutoff: self.trust_policy.ignore_after_cutoff,
        })
      }
      _ => None,
    };
    NpmPackageVersionResolver {
      info,
      newest_dependency_date: self
        .newest_dependency_date_options
        .get_for_package(&info.name),
      link_packages: self.link_packages.get(&info.name),
      trust_check,
    }
  }
}

/// Per-package state for the `no-downgrade` trust policy.
#[derive(Debug, Clone, Copy)]
struct TrustDowngradeCheck {
  ignore_after_cutoff: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct NpmPackageVersionResolver<'a> {
  info: &'a NpmPackageInfo,
  link_packages: Option<&'a Vec<NpmPackageVersionInfo>>,
  newest_dependency_date: Option<NewestDependencyDate>,
  /// When set (no-downgrade policy), a resolved registry version is rejected
  /// if an earlier-published version had stronger trust evidence.
  trust_check: Option<TrustDowngradeCheck>,
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

  /// Enforces the `no-downgrade` trust policy on a resolved registry version.
  ///
  /// Port of pnpm's
  /// [`failIfTrustDowngraded`](https://github.com/pnpm/pnpm/blob/main/resolving/npm-resolver/src/trustChecks.ts):
  /// a version is rejected when an *earlier-published* version of the same
  /// package carried stronger publishing-trust evidence. The comparison is by
  /// publish date, not semver. Prereleases are excluded from the historical
  /// scan unless the resolved version is itself a prerelease.
  ///
  /// Fails open: if the policy is off, or the resolved version has no publish
  /// timestamp, or no earlier version had any evidence, the version is
  /// accepted.
  fn check_trust_policy(
    &self,
    version_info: &NpmPackageVersionInfo,
  ) -> Result<(), NpmPackageVersionResolutionError> {
    let Some(check) = self.trust_check else {
      return Ok(());
    };
    let version = &version_info.version;
    // Without a publish timestamp we can't order the history, so fail open.
    let Some(version_date) = self.info.time.get(version).copied() else {
      return Ok(());
    };
    if let Some(cutoff) = check.ignore_after_cutoff
      && version_date < cutoff
    {
      // published before the cutoff (older than the ignore-after window)
      return Ok(());
    }
    let exclude_prerelease = version.pre.is_empty();
    let Some(strongest_prior) =
      self.strongest_trust_evidence_before(version_date, exclude_prerelease)
    else {
      return Ok(());
    };
    let current = version_info.get_trust_evidence();
    let current_rank = current.map_or(0, TrustEvidence::rank);
    if current_rank < strongest_prior.rank() {
      return Err(NpmPackageVersionResolutionError::TrustPolicyDowngrade {
        package_name: self.info.name.clone(),
        version: version.clone(),
        past_evidence: strongest_prior.pretty(),
        current_evidence: current
          .map_or("no trust evidence", TrustEvidence::pretty),
      });
    }
    Ok(())
  }

  /// Walks every version published strictly before `before_date` and returns
  /// the strongest [`TrustEvidence`] seen. Prereleases are skipped when
  /// `exclude_prerelease` is set. A version missing a publish timestamp is
  /// skipped (matching pnpm's per-version timestamp check) rather than
  /// aborting the walk.
  fn strongest_trust_evidence_before(
    &self,
    before_date: chrono::DateTime<chrono::Utc>,
    exclude_prerelease: bool,
  ) -> Option<TrustEvidence> {
    let mut best: Option<TrustEvidence> = None;
    for version_info in self.info.versions.values() {
      let version = &version_info.version;
      if exclude_prerelease && !version.pre.is_empty() {
        continue;
      }
      let Some(published_at) = self.info.time.get(version) else {
        continue;
      };
      if *published_at >= before_date {
        continue;
      }
      let Some(evidence) = version_info.get_trust_evidence() else {
        continue;
      };
      if best.is_none_or(|current| evidence > current) {
        best = Some(evidence);
        // `StagedPublish` is the maximum rank, so we can stop early.
        if evidence == TrustEvidence::StagedPublish {
          return best;
        }
      }
    }
    best
  }

  /// Like `version_req_satisfies`, but also matches prerelease versions that
  /// fall within the requirement's range bounds.
  ///
  /// This is used only for linked (workspace) packages. npm's default semver
  /// rules exclude prereleases from ranges like `*` or `^1.0.0` to avoid
  /// silently selecting an unstable version from the registry. Linked packages
  /// are provided explicitly by the user though, so a local workspace member
  /// with a prerelease version (e.g. `0.40.0-pre`) should still be selectable
  /// for a bare `npm:<pkg>` (`*`) requirement instead of falling back to the
  /// registry.
  ///
  /// This is the canonical prerelease fallback used by the npm graph resolver.
  /// `deno_config`'s `version_req_matches_including_pre` keeps an identical
  /// copy for the workspace/byonm paths (it can't depend on this crate, since
  /// the dependency goes the other way). Keep the two in sync.
  fn link_version_req_satisfies(
    &self,
    version_req: &VersionReq,
    version: &Version,
  ) -> Result<bool, NpmPackageVersionResolutionError> {
    if self.version_req_satisfies(version_req, version)? {
      return Ok(true);
    }
    Ok(match version_req.inner() {
      RangeSetOrTag::RangeSet(set) => {
        !version.pre.is_empty()
          && set.0.iter().any(|range| range.intersects_version(version))
      }
      RangeSetOrTag::Tag(_) => false,
    })
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
        if self.link_version_req_satisfies(version_req, version)? {
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
        Some(version_info) => {
          self.check_trust_policy(version_info)?;
          Ok(version_info)
        }
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
    let version_info = if let Some(tag) = version_req.tag() {
      match self.tag_to_version_info(tag) {
        Ok(version_info) => Ok(version_info),
        Err(NpmPackageVersionResolutionError::DistTagVersionTooNew {
          ..
        }) if tag == "latest" => self.resolve_best_matching_version_info(
          &WILDCARD_VERSION_REQ,
          version_req,
        ),
        Err(err) => Err(err),
      }
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
      self.resolve_best_matching_version_info(version_req, version_req)
    }?;
    // enforce the `no-downgrade` trust policy on the chosen registry version
    self.check_trust_policy(version_info)?;
    Ok(version_info)
  }

  fn resolve_best_matching_version_info(
    &self,
    matching_version_req: &VersionReq,
    error_version_req: &VersionReq,
  ) -> Result<&'a NpmPackageVersionInfo, NpmPackageVersionResolutionError> {
    let mut found_matching_version = false;
    let mut maybe_best_version: Option<&'a NpmPackageVersionInfo> = None;
    for version_info in self.info.versions.values() {
      let version = &version_info.version;
      if self.version_req_satisfies(matching_version_req, version)? {
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
        version_req: error_version_req.clone(),
        newest_dependency_date: found_matching_version
          .then_some(self.newest_dependency_date)
          .flatten(),
      }),
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

/// Creates a synthetic `NpmPackageInfo` from link package version infos.
/// Used when a package exists only as a workspace link and is not published
/// to the npm registry.
fn create_link_package_info(
  name: &PackageName,
  versions: &[NpmPackageVersionInfo],
) -> Arc<NpmPackageInfo> {
  let mut version_map = HashMap::new();
  let mut latest_version: Option<&Version> = None;
  for info in versions {
    let is_newer = latest_version.map(|v| info.version > *v).unwrap_or(true);
    if is_newer {
      latest_version = Some(&info.version);
    }
    version_map.insert(info.version.clone(), info.clone());
  }
  let mut dist_tags = HashMap::new();
  if let Some(latest) = latest_version {
    dist_tags.insert("latest".to_string(), latest.clone());
  }
  Arc::new(NpmPackageInfo {
    name: name.clone(),
    versions: version_map,
    dist_tags,
    time: HashMap::new(),
  })
}

/// Fetches package info from the npm registry, falling back to link packages
/// if the package does not exist on the registry. This allows packages that
/// are only linked locally (not published to npm) to be resolved.
pub async fn package_info_or_link_fallback(
  api: &(impl NpmRegistryApi + ?Sized),
  name: &str,
  link_packages: &HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
  let package_name = PackageName::from_str(name);
  let link_versions = link_packages.get(&package_name);
  let has_link = link_versions.is_some_and(|v| !v.is_empty());

  match api.package_info(name).await {
    Ok(info) => Ok(info),
    Err(NpmRegistryPackageInfoLoadError::PackageNotExists { .. })
      if has_link =>
    {
      Ok(create_link_package_info(
        &package_name,
        link_versions.unwrap(),
      ))
    }
    Err(err) => Err(err),
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
      overrides: Default::default(),
      trust_policy: Default::default(),
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
  fn test_latest_tag_too_new_uses_newest_allowed_version() {
    let package_req = PackageReq::from_str("test@latest").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([
        (
          Version::parse_from_npm("1.0.0").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("1.0.0").unwrap(),
            ..Default::default()
          },
        ),
        (
          Version::parse_from_npm("1.1.0").unwrap(),
          NpmPackageVersionInfo {
            version: Version::parse_from_npm("1.1.0").unwrap(),
            ..Default::default()
          },
        ),
      ]),
      dist_tags: HashMap::from([(
        "latest".into(),
        Version::parse_from_npm("1.1.0").unwrap(),
      )]),
      time: HashMap::from([
        (
          Version::parse_from_npm("1.0.0").unwrap(),
          "2025-05-15T00:00:00.000Z".parse().unwrap(),
        ),
        (
          Version::parse_from_npm("1.1.0").unwrap(),
          "2025-05-29T00:00:00.000Z".parse().unwrap(),
        ),
      ]),
    };
    let resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: NewestDependencyDateOptions::from_date(
        "2025-05-20T00:00:00.000Z".parse().unwrap(),
      ),
      overrides: Default::default(),
      trust_policy: Default::default(),
    };
    let version_resolver = resolver.get_for_package(&package_info);
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert_eq!(result.unwrap().version.to_string(), "1.0.0");
  }

  #[test]
  fn test_trust_policy_no_downgrade() {
    fn version_info(json: &str) -> NpmPackageVersionInfo {
      serde_json::from_str(json).unwrap()
    }
    fn ver(v: &str) -> Version {
      Version::parse_from_npm(v).unwrap()
    }

    // History, by publish date:
    //   1.0.0 (2025-01-01) provenance only            -> rank 1
    //   1.1.0 (2025-02-01) trusted publisher + provenance -> rank 2
    //   1.2.0 (2025-03-01) plain token publish         -> rank 0 (downgrade!)
    //   1.3.0 (2025-04-01) staged publish              -> rank 3
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([
        (
          ver("1.0.0"),
          version_info(
            r#"{ "version": "1.0.0", "dist": { "tarball": "t", "attestations": { "provenance": {} } } }"#,
          ),
        ),
        (
          ver("1.1.0"),
          version_info(
            r#"{ "version": "1.1.0", "_npmUser": { "trustedPublisher": {} }, "dist": { "tarball": "t", "attestations": { "provenance": {} } } }"#,
          ),
        ),
        (ver("1.2.0"), version_info(r#"{ "version": "1.2.0" }"#)),
        (
          ver("1.3.0"),
          version_info(
            r#"{ "version": "1.3.0", "_npmUser": { "approver": {} } }"#,
          ),
        ),
      ]),
      dist_tags: Default::default(),
      time: HashMap::from([
        (ver("1.0.0"), "2025-01-01T00:00:00.000Z".parse().unwrap()),
        (ver("1.1.0"), "2025-02-01T00:00:00.000Z".parse().unwrap()),
        (ver("1.2.0"), "2025-03-01T00:00:00.000Z".parse().unwrap()),
        (ver("1.3.0"), "2025-04-01T00:00:00.000Z".parse().unwrap()),
      ]),
    };

    let no_downgrade = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Default::default(),
      },
      ..Default::default()
    };
    let resolve = |resolver: &NpmVersionResolver, req: &str| {
      let package_req = PackageReq::from_str(req).unwrap();
      resolver
        .get_for_package(&package_info)
        .get_resolved_package_version_and_info(&package_req.version_req)
        .map(|info| info.version.to_string())
    };

    // `^1` picks the newest (1.3.0, staged, rank 3); the strongest earlier
    // evidence is rank 2, so it's not a downgrade.
    assert_eq!(resolve(&no_downgrade, "test@^1").unwrap(), "1.3.0");

    // the first published version has no earlier history -> always allowed.
    assert_eq!(resolve(&no_downgrade, "test@1.0.0").unwrap(), "1.0.0");

    // 1.1.0 (rank 2) vs its only predecessor 1.0.0 (rank 1) -> not a downgrade.
    assert_eq!(resolve(&no_downgrade, "test@1.1.0").unwrap(), "1.1.0");

    // pinning to 1.2.0 (plain, rank 0) is a downgrade from 1.1.0 (rank 2):
    // a dedicated error explaining the rejection, not a fallback to an older
    // version (pnpm fails closed here too).
    let err = resolve(&no_downgrade, "test@1.2.0").unwrap_err();
    assert!(
      matches!(
        err,
        NpmPackageVersionResolutionError::TrustPolicyDowngrade { .. }
      ),
      "expected TrustPolicyDowngrade, got {err:?}"
    );
    let msg = err.to_string();
    assert!(msg.contains("trusted publisher"), "{msg}");
    assert!(msg.contains("no trust evidence"), "{msg}");

    // with the policy off, the same pin resolves without complaint.
    let off = NpmVersionResolver::default();
    assert_eq!(resolve(&off, "test@1.2.0").unwrap(), "1.2.0");

    // `trust-policy-ignore-after` exempts versions published before the cutoff:
    // 1.2.0 (2025-03-01) is before a 2025-06 cutoff, so it installs anyway.
    let ignore_after = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: Some("2025-06-01T00:00:00.000Z".parse().unwrap()),
        exclude: Default::default(),
      },
      ..Default::default()
    };
    assert_eq!(resolve(&ignore_after, "test@1.2.0").unwrap(), "1.2.0");
  }

  #[test]
  fn test_trust_policy_exclude_package() {
    fn version_info(json: &str) -> NpmPackageVersionInfo {
      serde_json::from_str(json).unwrap()
    }
    fn ver(v: &str) -> Version {
      Version::parse_from_npm(v).unwrap()
    }

    // 1.0.0 is a staged publish (rank 3), 1.1.0 is a plain token publish
    // (rank 0): resolving 1.1.0 is a downgrade and is normally rejected.
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([
        (
          ver("1.0.0"),
          version_info(
            r#"{ "version": "1.0.0", "_npmUser": { "approver": {} } }"#,
          ),
        ),
        (ver("1.1.0"), version_info(r#"{ "version": "1.1.0" }"#)),
      ]),
      dist_tags: Default::default(),
      time: HashMap::from([
        (ver("1.0.0"), "2025-01-01T00:00:00.000Z".parse().unwrap()),
        (ver("1.1.0"), "2025-02-01T00:00:00.000Z".parse().unwrap()),
      ]),
    };
    let resolve = |resolver: &NpmVersionResolver| {
      let package_req = PackageReq::from_str("test@1.1.0").unwrap();
      resolver
        .get_for_package(&package_info)
        .get_resolved_package_version_and_info(&package_req.version_req)
        .map(|info| info.version.to_string())
    };

    // without an exclude, the downgrade is rejected
    let enforced = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Default::default(),
      },
      ..Default::default()
    };
    assert!(matches!(
      resolve(&enforced).unwrap_err(),
      NpmPackageVersionResolutionError::TrustPolicyDowngrade { .. }
    ));

    // with `test` in `trust-policy-exclude`, it resolves as if the policy were
    // off
    let excluded = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Arc::new(BTreeSet::from(["test".to_string()])),
      },
      ..Default::default()
    };
    assert_eq!(resolve(&excluded).unwrap(), "1.1.0");

    // an unrelated package in the exclude list does not exempt `test`
    let other_excluded = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Arc::new(BTreeSet::from(["other".to_string()])),
      },
      ..Default::default()
    };
    assert!(matches!(
      resolve(&other_excluded).unwrap_err(),
      NpmPackageVersionResolutionError::TrustPolicyDowngrade { .. }
    ));
  }

  #[test]
  fn test_trust_policy_excludes_prereleases_from_history() {
    fn version_info(json: &str) -> NpmPackageVersionInfo {
      serde_json::from_str(json).unwrap()
    }
    fn ver(v: &str) -> Version {
      Version::parse_from_npm(v).unwrap()
    }

    // A prerelease carried a staged publish, then a plain stable shipped.
    // Resolving the stable must NOT treat the prerelease as the baseline
    // (pnpm excludes prereleases from the scan for a stable candidate).
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([
        (
          ver("1.0.0-pre.1"),
          version_info(
            r#"{ "version": "1.0.0-pre.1", "_npmUser": { "approver": {} } }"#,
          ),
        ),
        (ver("1.0.0"), version_info(r#"{ "version": "1.0.0" }"#)),
      ]),
      dist_tags: Default::default(),
      time: HashMap::from([
        (
          ver("1.0.0-pre.1"),
          "2025-01-01T00:00:00.000Z".parse().unwrap(),
        ),
        (ver("1.0.0"), "2025-02-01T00:00:00.000Z".parse().unwrap()),
      ]),
    };
    let resolver = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Default::default(),
      },
      ..Default::default()
    };
    let package_req = PackageReq::from_str("test@1.0.0").unwrap();
    assert_eq!(
      resolver
        .get_for_package(&package_info)
        .get_resolved_package_version_and_info(&package_req.version_req)
        .unwrap()
        .version
        .to_string(),
      "1.0.0"
    );
  }

  #[test]
  fn test_non_latest_tag_too_new_errors() {
    let package_req = PackageReq::from_str("test@next").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".into(),
      versions: HashMap::from([(
        Version::parse_from_npm("1.1.0").unwrap(),
        NpmPackageVersionInfo {
          version: Version::parse_from_npm("1.1.0").unwrap(),
          ..Default::default()
        },
      )]),
      dist_tags: HashMap::from([(
        "next".into(),
        Version::parse_from_npm("1.1.0").unwrap(),
      )]),
      time: HashMap::from([(
        Version::parse_from_npm("1.1.0").unwrap(),
        "2025-05-29T00:00:00.000Z".parse().unwrap(),
      )]),
    };
    let resolver = NpmVersionResolver {
      link_packages: Default::default(),
      newest_dependency_date_options: NewestDependencyDateOptions::from_date(
        "2025-05-20T00:00:00.000Z".parse().unwrap(),
      ),
      overrides: Default::default(),
      trust_policy: Default::default(),
    };
    let version_resolver = resolver.get_for_package(&package_info);
    let result = version_resolver
      .get_resolved_package_version_and_info(&package_req.version_req);
    assert!(matches!(
      result,
      Err(NpmPackageVersionResolutionError::DistTagVersionTooNew { .. })
    ));
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
      overrides: Default::default(),
      trust_policy: Default::default(),
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
      overrides: Default::default(),
      trust_policy: Default::default(),
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

  // Tests for case-insensitive `trust-policy-exclude[]` matching.
  // npm package names are case-insensitive by specification; the exclude check
  // must use `eq_ignore_ascii_case` rather than an exact `BTreeSet::contains`.

  // shared helper — builds a simple 1.0.0 (staged) → 1.1.0 (plain) package
  fn make_downgrade_package(name: &str) -> NpmPackageInfo {
    fn vi(json: &str) -> NpmPackageVersionInfo {
      serde_json::from_str(json).unwrap()
    }
    fn ver(v: &str) -> Version {
      Version::parse_from_npm(v).unwrap()
    }
    NpmPackageInfo {
      name: name.into(),
      versions: HashMap::from([
        (
          ver("1.0.0"),
          vi(r#"{ "version": "1.0.0", "_npmUser": { "approver": {} } }"#),
        ),
        (ver("1.1.0"), vi(r#"{ "version": "1.1.0" }"#)),
      ]),
      dist_tags: Default::default(),
      time: HashMap::from([
        (ver("1.0.0"), "2025-01-01T00:00:00.000Z".parse().unwrap()),
        (ver("1.1.0"), "2025-02-01T00:00:00.000Z".parse().unwrap()),
      ]),
    }
  }

  fn resolve_with_exclude(
    package_info: &NpmPackageInfo,
    req: &str,
    exclude: &[&str],
  ) -> Result<String, NpmPackageVersionResolutionError> {
    let resolver = NpmVersionResolver {
      trust_policy: TrustPolicyOptions {
        policy: NpmTrustPolicy::NoDowngrade,
        ignore_after_cutoff: None,
        exclude: Arc::new(
          exclude.iter().map(|s| s.to_ascii_lowercase()).collect(),
        ),
      },
      ..Default::default()
    };
    let package_req = PackageReq::from_str(req).unwrap();
    resolver
      .get_for_package(package_info)
      .get_resolved_package_version_and_info(&package_req.version_req)
      .map(|info| info.version.to_string())
  }

  #[test]
  fn test_trust_policy_exclude_case_insensitive_simple_name() {
    // EC1: .npmrc has uppercase `TEST`, package name from registry is `test`.
    // npm names are case-insensitive — the exclude should match regardless of case.
    let pkg = make_downgrade_package("test");

    // baseline: exact-case exclude works
    assert_eq!(
      resolve_with_exclude(&pkg, "test@1.1.0", &["test"]).unwrap(),
      "1.1.0",
      "EC5 baseline: exact-case lowercase exclude should allow the downgrade"
    );

    // EC1: uppercase exclude in .npmrc should still match
    let result = resolve_with_exclude(&pkg, "test@1.1.0", &["TEST"]);
    assert_eq!(
      result.unwrap(),
      "1.1.0",
      "EC1: uppercase exclude 'TEST' should match lowercase package 'test' (npm is case-insensitive)"
    );
  }

  #[test]
  fn test_trust_policy_exclude_case_insensitive_mixed_case() {
    // EC2: .npmrc has mixed-case `Test`, registry name is `test`.
    let pkg = make_downgrade_package("test");
    let result = resolve_with_exclude(&pkg, "test@1.1.0", &["Test"]);
    assert_eq!(
      result.unwrap(),
      "1.1.0",
      "EC2: mixed-case exclude 'Test' should match registry name 'test'"
    );
  }

  #[test]
  fn test_trust_policy_exclude_case_insensitive_scoped_package() {
    // EC3: scoped package — user writes @MyScope/MyPkg in .npmrc,
    // registry returns @myscope/mypkg (npm normalises scopes to lowercase).
    // The exclude check must be case-insensitive for scoped names too.
    let pkg = make_downgrade_package("@myscope/mypkg");
    let result =
      resolve_with_exclude(&pkg, "@myscope/mypkg@1.1.0", &["@MyScope/MyPkg"]);
    assert_eq!(
      result.unwrap(),
      "1.1.0",
      "EC3: mixed-case scoped exclude '@MyScope/MyPkg' should match '@myscope/mypkg'"
    );
  }

  #[test]
  fn test_trust_policy_exclude_case_insensitive_registry_uppercase() {
    // EC4: reverse — registry name has mixed-case (unusual but valid for some
    // private registries), user wrote lowercase in .npmrc exclude.
    // Both directions must work after the fix.
    let pkg = make_downgrade_package("MyPkg");
    let result = resolve_with_exclude(&pkg, "MyPkg@1.1.0", &["mypkg"]);
    assert_eq!(
      result.unwrap(),
      "1.1.0",
      "EC4: lowercase exclude 'mypkg' should match mixed-case registry name 'MyPkg'"
    );
  }
}
