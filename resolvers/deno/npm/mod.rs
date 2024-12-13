// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::fmt::Debug;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use node_resolver::env::NodeResolverEnv;
use node_resolver::errors::NodeResolveError;
use node_resolver::errors::NodeResolveErrorKind;
use node_resolver::errors::PackageFolderResolveErrorKind;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::PackageResolveErrorKind;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::ResolutionMode;
use thiserror::Error;
use url::Url;

use crate::fs::DenoResolverFs;
#[allow(clippy::disallowed_types)]
use crate::sync::MaybeArc;

pub use byonm::ByonmInNpmPackageChecker;
pub use byonm::ByonmNpmResolver;
pub use byonm::ByonmNpmResolverCreateOptions;
pub use byonm::ByonmResolvePkgFolderFromDenoReqError;
pub use local::normalize_pkg_name_for_node_modules_deno_folder;

mod byonm;
mod local;

#[derive(Debug, Error)]
#[error("Could not resolve \"{}\", but found it in a package.json. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", specifier)]
pub struct NodeModulesOutOfDateError {
  pub specifier: String,
}

#[derive(Debug, Error)]
#[error("Could not find '{}'. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", package_json_path.display())]
pub struct MissingPackageNodeModulesFolderError {
  pub package_json_path: PathBuf,
}

#[derive(Debug, Boxed)]
pub struct ResolveIfForNpmPackageError(
  pub Box<ResolveIfForNpmPackageErrorKind>,
);

#[derive(Debug, Error)]
pub enum ResolveIfForNpmPackageErrorKind {
  #[error(transparent)]
  NodeResolve(#[from] NodeResolveError),
  #[error(transparent)]
  NodeModulesOutOfDate(#[from] NodeModulesOutOfDateError),
}

#[derive(Debug, Boxed)]
pub struct ResolveReqWithSubPathError(pub Box<ResolveReqWithSubPathErrorKind>);

#[derive(Debug, Error)]
pub enum ResolveReqWithSubPathErrorKind {
  #[error(transparent)]
  MissingPackageNodeModulesFolder(#[from] MissingPackageNodeModulesFolderError),
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
}

#[derive(Debug, Error)]
pub enum ResolvePkgFolderFromDenoReqError {
  // todo(dsherret): don't use anyhow here
  #[error(transparent)]
  Managed(anyhow::Error),
  #[error(transparent)]
  Byonm(#[from] ByonmResolvePkgFolderFromDenoReqError),
}

// todo(dsherret): a temporary trait until we extract
// out the CLI npm resolver into here
pub trait CliNpmReqResolver: Debug + Send + Sync {
  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError>;
}

pub struct NpmReqResolverOptions<
  Fs: DenoResolverFs,
  TNodeResolverEnv: NodeResolverEnv,
> {
  /// The resolver when "bring your own node_modules" is enabled where Deno
  /// does not setup the node_modules directories automatically, but instead
  /// uses what already exists on the file system.
  #[allow(clippy::disallowed_types)]
  pub byonm_resolver: Option<MaybeArc<ByonmNpmResolver<Fs, TNodeResolverEnv>>>,
  pub fs: Fs,
  #[allow(clippy::disallowed_types)]
  pub in_npm_pkg_checker: MaybeArc<dyn InNpmPackageChecker>,
  #[allow(clippy::disallowed_types)]
  pub node_resolver: MaybeArc<NodeResolver<TNodeResolverEnv>>,
  #[allow(clippy::disallowed_types)]
  pub npm_req_resolver: MaybeArc<dyn CliNpmReqResolver>,
}

#[derive(Debug)]
pub struct NpmReqResolver<Fs: DenoResolverFs, TNodeResolverEnv: NodeResolverEnv>
{
  #[allow(clippy::disallowed_types)]
  byonm_resolver: Option<MaybeArc<ByonmNpmResolver<Fs, TNodeResolverEnv>>>,
  fs: Fs,
  #[allow(clippy::disallowed_types)]
  in_npm_pkg_checker: MaybeArc<dyn InNpmPackageChecker>,
  #[allow(clippy::disallowed_types)]
  node_resolver: MaybeArc<NodeResolver<TNodeResolverEnv>>,
  #[allow(clippy::disallowed_types)]
  npm_resolver: MaybeArc<dyn CliNpmReqResolver>,
}

impl<Fs: DenoResolverFs, TNodeResolverEnv: NodeResolverEnv>
  NpmReqResolver<Fs, TNodeResolverEnv>
{
  pub fn new(options: NpmReqResolverOptions<Fs, TNodeResolverEnv>) -> Self {
    Self {
      byonm_resolver: options.byonm_resolver,
      fs: options.fs,
      in_npm_pkg_checker: options.in_npm_pkg_checker,
      node_resolver: options.node_resolver,
      npm_resolver: options.npm_req_resolver,
    }
  }

  pub fn resolve_req_reference(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<Url, ResolveReqWithSubPathError> {
    self.resolve_req_with_sub_path(
      req_ref.req(),
      req_ref.sub_path(),
      referrer,
      resolution_mode,
      resolution_kind,
    )
  }

  pub fn resolve_req_with_sub_path(
    &self,
    req: &PackageReq,
    sub_path: Option<&str>,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<Url, ResolveReqWithSubPathError> {
    let package_folder = self
      .npm_resolver
      .resolve_pkg_folder_from_deno_module_req(req, referrer)?;
    let resolution_result =
      self.node_resolver.resolve_package_subpath_from_deno_module(
        &package_folder,
        sub_path,
        Some(referrer),
        resolution_mode,
        resolution_kind,
      );
    match resolution_result {
      Ok(url) => Ok(url),
      Err(err) => {
        if self.byonm_resolver.is_some() {
          let package_json_path = package_folder.join("package.json");
          if !self.fs.exists_sync(&package_json_path) {
            return Err(
              MissingPackageNodeModulesFolderError { package_json_path }.into(),
            );
          }
        }
        Err(err.into())
      }
    }
  }

  pub fn resolve_if_for_npm_pkg(
    &self,
    specifier: &str,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<Option<NodeResolution>, ResolveIfForNpmPackageError> {
    let resolution_result = self.node_resolver.resolve(
      specifier,
      referrer,
      resolution_mode,
      resolution_kind,
    );
    match resolution_result {
      Ok(res) => Ok(Some(res)),
      Err(err) => {
        let err = err.into_kind();
        match err {
          NodeResolveErrorKind::RelativeJoin(_)
          | NodeResolveErrorKind::PackageImportsResolve(_)
          | NodeResolveErrorKind::UnsupportedEsmUrlScheme(_)
          | NodeResolveErrorKind::DataUrlReferrer(_)
          | NodeResolveErrorKind::TypesNotFound(_)
          | NodeResolveErrorKind::FinalizeResolution(_) => Err(
            ResolveIfForNpmPackageErrorKind::NodeResolve(err.into()).into_box(),
          ),
          NodeResolveErrorKind::PackageResolve(err) => {
            let err = err.into_kind();
            match err {
              PackageResolveErrorKind::ClosestPkgJson(_)
              | PackageResolveErrorKind::InvalidModuleSpecifier(_)
              | PackageResolveErrorKind::ExportsResolve(_)
              | PackageResolveErrorKind::SubpathResolve(_) => Err(
                ResolveIfForNpmPackageErrorKind::NodeResolve(
                  NodeResolveErrorKind::PackageResolve(err.into()).into(),
                )
                .into_box(),
              ),
              PackageResolveErrorKind::PackageFolderResolve(err) => {
                match err.as_kind() {
                  PackageFolderResolveErrorKind::Io(
                    PackageFolderResolveIoError { package_name, .. },
                  )
                  | PackageFolderResolveErrorKind::PackageNotFound(
                    PackageNotFoundError { package_name, .. },
                  ) => {
                    if self.in_npm_pkg_checker.in_npm_package(referrer) {
                      return Err(
                        ResolveIfForNpmPackageErrorKind::NodeResolve(
                          NodeResolveErrorKind::PackageResolve(err.into())
                            .into(),
                        )
                        .into_box(),
                      );
                    }
                    if let Some(byonm_npm_resolver) = &self.byonm_resolver {
                      if byonm_npm_resolver
                        .find_ancestor_package_json_with_dep(
                          package_name,
                          referrer,
                        )
                        .is_some()
                      {
                        return Err(
                          ResolveIfForNpmPackageErrorKind::NodeModulesOutOfDate(
                            NodeModulesOutOfDateError {
                              specifier: specifier.to_string(),
                            },
                          ).into_box(),
                        );
                      }
                    }
                    Ok(None)
                  }
                  PackageFolderResolveErrorKind::ReferrerNotFound(_) => {
                    if self.in_npm_pkg_checker.in_npm_package(referrer) {
                      return Err(
                        ResolveIfForNpmPackageErrorKind::NodeResolve(
                          NodeResolveErrorKind::PackageResolve(err.into())
                            .into(),
                        )
                        .into_box(),
                      );
                    }
                    Ok(None)
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
