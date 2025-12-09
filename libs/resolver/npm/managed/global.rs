// Copyright 2018-2025 the Deno authors. MIT license.

//! Code for global npm cache resolution.

use std::borrow::Cow;
use std::path::PathBuf;

use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::package::PackageNv;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPathRef;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::ReferrerNotFoundError;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use url::Url;

use super::NpmCacheDirRc;
use super::resolution::NpmResolutionCellRc;
use crate::npm::managed::common::join_package_name_to_path;
use crate::npmrc::ResolvedNpmRcRc;

/// Resolves packages from the global npm cache.
#[derive(Debug)]
pub struct GlobalNpmPackageResolver<TSys: FsCanonicalize + FsMetadata> {
  cache: NpmCacheDirRc,
  npm_rc: ResolvedNpmRcRc,
  resolution: NpmResolutionCellRc,
  sys: TSys,
}

impl<TSys: FsCanonicalize + FsMetadata> GlobalNpmPackageResolver<TSys> {
  pub fn new(
    cache: NpmCacheDirRc,
    npm_rc: ResolvedNpmRcRc,
    resolution: NpmResolutionCellRc,
    sys: TSys,
  ) -> Self {
    Self {
      cache,
      npm_rc,
      resolution,
      sys,
    }
  }

  pub fn maybe_package_folder(&self, id: &NpmPackageId) -> Option<PathBuf> {
    let folder_copy_index = self
      .resolution
      .resolve_pkg_cache_folder_copy_index_from_pkg_id(id)?;
    let registry_url = self.npm_rc.get_registry_url(&id.nv.name);
    Some(self.cache.package_folder_for_id(
      &id.nv.name,
      &id.nv.version.to_string(),
      folder_copy_index,
      registry_url,
    ))
  }

  pub fn resolve_package_cache_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Result<Option<NpmPackageCacheFolderId>, std::io::Error> {
    Ok(self.resolve_package_cache_folder_id_from_specifier_inner(specifier))
  }

  fn resolve_package_cache_folder_id_from_specifier_inner(
    &self,
    specifier: &Url,
  ) -> Option<NpmPackageCacheFolderId> {
    self
      .cache
      .resolve_package_folder_id_from_specifier(specifier)
      .and_then(|cache_id| {
        Some(NpmPackageCacheFolderId {
          nv: PackageNv {
            name: StackString::from_string(cache_id.name),
            version: Version::parse_from_npm(&cache_id.version).ok()?,
          },
          copy_index: cache_id.copy_index,
        })
      })
  }
}

impl<TSys: FsCanonicalize + FsMetadata> NpmPackageFolderResolver
  for GlobalNpmPackageResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    use deno_npm::resolution::PackageNotFoundFromReferrerError;
    let Some(referrer_cache_folder_id) = self
      .resolve_package_cache_folder_id_from_specifier_inner(referrer.url()?)
    else {
      return Err(
        ReferrerNotFoundError {
          referrer: referrer.display(),
          referrer_extra: None,
        }
        .into(),
      );
    };
    let resolve_result = self
      .resolution
      .resolve_package_from_package(name, &referrer_cache_folder_id);
    match resolve_result {
      Ok(pkg) => match self.maybe_package_folder(&pkg.id) {
        Some(folder) => Ok(folder),
        None => Err(
          PackageNotFoundError {
            package_name: name.to_string(),
            referrer: referrer.display(),
            referrer_extra: Some(format!(
              "{} -> {}",
              referrer_cache_folder_id,
              pkg.id.as_serialized()
            )),
          }
          .into(),
        ),
      },
      Err(err) => match *err {
        PackageNotFoundFromReferrerError::Referrer(cache_folder_id) => Err(
          ReferrerNotFoundError {
            referrer: referrer.display(),
            referrer_extra: Some(cache_folder_id.to_string()),
          }
          .into(),
        ),
        PackageNotFoundFromReferrerError::Package {
          name,
          referrer: cache_folder_id_referrer,
        } => {
          // check for any bundled dependencies within the package
          if let Ok(referrer_path) = referrer.path() {
            let cache_location = self.cache.get_cache_location();
            for current_folder in referrer_path
              .ancestors()
              .skip(1)
              .take_while(|path| path.starts_with(&cache_location))
            {
              let node_modules_folder =
                if current_folder.ends_with("node_modules") {
                  Cow::Borrowed(current_folder)
                } else {
                  Cow::Owned(current_folder.join("node_modules"))
                };

              let sub_dir =
                join_package_name_to_path(&node_modules_folder, &name);
              if self.sys.fs_is_dir_no_err(&sub_dir) {
                return Ok(self.sys.fs_canonicalize(&sub_dir).map_err(
                  |err| PackageFolderResolveIoError {
                    package_name: name.to_string(),
                    referrer: referrer.display(),
                    source: err,
                  },
                )?);
              }
            }
          }

          Err(
            PackageNotFoundError {
              package_name: name,
              referrer: referrer.display(),
              referrer_extra: Some(cache_folder_id_referrer.to_string()),
            }
            .into(),
          )
        }
      },
    }
  }

  fn resolve_types_package_folder(
    &self,
    types_package_name: &str,
    maybe_package_version: Option<&Version>,
    _maybe_referrer: Option<&UrlOrPathRef>,
  ) -> Option<PathBuf> {
    let snapshot = self.resolution.snapshot();
    let pkg_id = super::common::find_definitely_typed_package_from_snapshot(
      types_package_name,
      maybe_package_version,
      &snapshot,
    )?;
    self.maybe_package_folder(pkg_id)
  }
}
