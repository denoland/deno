// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_graph::semver::Version;
use deno_graph::semver::VersionReq;
use once_cell::sync::Lazy;

use super::NpmPackageId;
use crate::npm::registry::NpmPackageInfo;
use crate::npm::registry::NpmPackageVersionInfo;

pub static LATEST_VERSION_REQ: Lazy<VersionReq> =
  Lazy::new(|| VersionReq::parse_from_specifier("latest").unwrap());

pub fn resolve_best_package_version_and_info<'info, 'version>(
  version_req: &VersionReq,
  package_info: &'info NpmPackageInfo,
  existing_versions: impl Iterator<Item = &'version Version>,
) -> Result<VersionAndInfo<'info>, AnyError> {
  if let Some(version) = resolve_best_from_existing_versions(
    version_req,
    package_info,
    existing_versions,
  )? {
    match package_info.versions.get(&version.to_string()) {
      Some(version_info) => Ok(VersionAndInfo {
        version,
        info: version_info,
      }),
      None => {
        bail!(
          "could not find version '{}' for '{}'",
          version,
          &package_info.name
        )
      }
    }
  } else {
    // get the information
    get_resolved_package_version_and_info(version_req, package_info, None)
  }
}

#[derive(Clone)]
pub struct VersionAndInfo<'a> {
  pub version: Version,
  pub info: &'a NpmPackageVersionInfo,
}

fn get_resolved_package_version_and_info<'a>(
  version_req: &VersionReq,
  info: &'a NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo<'a>, AnyError> {
  if let Some(tag) = version_req.tag() {
    tag_to_version_info(info, tag, parent)
  } else {
    let mut maybe_best_version: Option<VersionAndInfo> = None;
    for version_info in info.versions.values() {
      let version = Version::parse_from_npm(&version_info.version)?;
      if version_req.matches(&version) {
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
          "Could not find npm package '{}' matching {}{}. ",
          "Try retrieving the latest npm package information by running with --reload",
        ),
        info.name,
        version_req.version_text(),
        match parent {
          Some(resolved_id) => format!(" as specified in {}", resolved_id.nv),
          None => String::new(),
        }
      ),
    }
  }
}

pub fn version_req_satisfies(
  version_req: &VersionReq,
  version: &Version,
  package_info: &NpmPackageInfo,
  parent: Option<&NpmPackageId>,
) -> Result<bool, AnyError> {
  match version_req.tag() {
    Some(tag) => {
      let tag_version = tag_to_version_info(package_info, tag, parent)?.version;
      Ok(tag_version == *version)
    }
    None => Ok(version_req.matches(version)),
  }
}

fn resolve_best_from_existing_versions<'a>(
  version_req: &VersionReq,
  package_info: &NpmPackageInfo,
  existing_versions: impl Iterator<Item = &'a Version>,
) -> Result<Option<Version>, AnyError> {
  let mut maybe_best_version: Option<&Version> = None;
  for version in existing_versions {
    if version_req_satisfies(version_req, version, package_info, None)? {
      let is_best_version = maybe_best_version
        .as_ref()
        .map(|best_version| (*best_version).cmp(version).is_lt())
        .unwrap_or(true);
      if is_best_version {
        maybe_best_version = Some(version);
      }
    }
  }
  Ok(maybe_best_version.cloned())
}

fn tag_to_version_info<'a>(
  info: &'a NpmPackageInfo,
  tag: &str,
  parent: Option<&NpmPackageId>,
) -> Result<VersionAndInfo<'a>, AnyError> {
  // For when someone just specifies @types/node, we want to pull in a
  // "known good" version of @types/node that works well with Deno and
  // not necessarily the latest version. For example, we might only be
  // compatible with Node vX, but then Node vY is published so we wouldn't
  // want to pull that in.
  // Note: If the user doesn't want this behavior, then they can specify an
  // explicit version.
  if tag == "latest" && info.name == "@types/node" {
    return get_resolved_package_version_and_info(
      &VersionReq::parse_from_npm("18.0.0 - 18.11.18").unwrap(),
      info,
      parent,
    );
  }

  if let Some(version) = info.dist_tags.get(tag) {
    match info.versions.get(version) {
      Some(info) => Ok(VersionAndInfo {
        version: Version::parse_from_npm(version)?,
        info,
      }),
      None => {
        bail!(
          "Could not find version '{}' referenced in dist-tag '{}'.",
          version,
          tag,
        )
      }
    }
  } else {
    bail!("Could not find dist-tag '{}'.", tag)
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use deno_graph::npm::NpmPackageReqReference;

  use super::*;

  #[test]
  fn test_get_resolved_package_version_and_info() {
    // dist tag where version doesn't exist
    let package_ref = NpmPackageReqReference::from_str("npm:test").unwrap();
    let package_info = NpmPackageInfo {
      name: "test".to_string(),
      versions: HashMap::new(),
      dist_tags: HashMap::from([(
        "latest".to_string(),
        "1.0.0-alpha".to_string(),
      )]),
    };
    let result = get_resolved_package_version_and_info(
      package_ref
        .req
        .version_req
        .as_ref()
        .unwrap_or(&*LATEST_VERSION_REQ),
      &package_info,
      None,
    );
    assert_eq!(
      result.err().unwrap().to_string(),
      "Could not find version '1.0.0-alpha' referenced in dist-tag 'latest'."
    );

    // dist tag where version is a pre-release
    let package_ref = NpmPackageReqReference::from_str("npm:test").unwrap();
    let package_info = NpmPackageInfo {
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
    };
    let result = get_resolved_package_version_and_info(
      package_ref
        .req
        .version_req
        .as_ref()
        .unwrap_or(&*LATEST_VERSION_REQ),
      &package_info,
      None,
    );
    assert_eq!(result.unwrap().version.to_string(), "1.0.0-alpha");
  }
}
