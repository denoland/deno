// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_error::JsError;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use node_resolver::errors::NodeResolveError;
use node_resolver::errors::NodeResolveErrorKind;
use node_resolver::errors::PackageFolderResolveErrorKind;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::errors::PackageResolveErrorKind;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::errors::TypesNotFoundError;
use node_resolver::types_package_name;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolverRc;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
use node_resolver::UrlOrPath;
use node_resolver::UrlOrPathRef;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use thiserror::Error;
use url::Url;

pub use self::byonm::ByonmInNpmPackageChecker;
pub use self::byonm::ByonmNpmResolver;
pub use self::byonm::ByonmNpmResolverCreateOptions;
pub use self::byonm::ByonmNpmResolverRc;
pub use self::byonm::ByonmResolvePkgFolderFromDenoReqError;
pub use self::local::get_package_folder_id_folder_name;
pub use self::local::normalize_pkg_name_for_node_modules_deno_folder;
use self::managed::create_managed_in_npm_pkg_checker;
use self::managed::ManagedInNpmPackageChecker;
use self::managed::ManagedInNpmPkgCheckerCreateOptions;
pub use self::managed::ManagedNpmResolver;
use self::managed::ManagedNpmResolverCreateOptions;
pub use self::managed::ManagedNpmResolverRc;
use crate::sync::new_rc;
use crate::sync::MaybeSend;
use crate::sync::MaybeSync;

mod byonm;
mod local;
pub mod managed;

#[derive(Debug)]
pub enum CreateInNpmPkgCheckerOptions<'a> {
  Managed(ManagedInNpmPkgCheckerCreateOptions<'a>),
  Byonm,
}

#[derive(Debug, Clone)]
pub enum DenoInNpmPackageChecker {
  Managed(ManagedInNpmPackageChecker),
  Byonm(ByonmInNpmPackageChecker),
}

impl DenoInNpmPackageChecker {
  pub fn new(options: CreateInNpmPkgCheckerOptions) -> Self {
    match options {
      CreateInNpmPkgCheckerOptions::Managed(options) => {
        DenoInNpmPackageChecker::Managed(create_managed_in_npm_pkg_checker(
          options,
        ))
      }
      CreateInNpmPkgCheckerOptions::Byonm => {
        DenoInNpmPackageChecker::Byonm(ByonmInNpmPackageChecker)
      }
    }
  }
}

impl InNpmPackageChecker for DenoInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    match self {
      DenoInNpmPackageChecker::Managed(c) => c.in_npm_package(specifier),
      DenoInNpmPackageChecker::Byonm(c) => c.in_npm_package(specifier),
    }
  }
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Could not resolve \"{}\", but found it in a package.json. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", specifier)]
pub struct NodeModulesOutOfDateError {
  pub specifier: String,
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Could not find '{}'. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", package_json_path.display())]
pub struct MissingPackageNodeModulesFolderError {
  pub package_json_path: PathBuf,
}

#[derive(Debug, Boxed, JsError)]
pub struct ResolveIfForNpmPackageError(
  pub Box<ResolveIfForNpmPackageErrorKind>,
);

#[derive(Debug, Error, JsError)]
pub enum ResolveIfForNpmPackageErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  NodeResolve(#[from] NodeResolveError),
  #[class(inherit)]
  #[error(transparent)]
  NodeModulesOutOfDate(#[from] NodeModulesOutOfDateError),
}

#[derive(Debug, Boxed, JsError)]
pub struct ResolveReqWithSubPathError(pub Box<ResolveReqWithSubPathErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum ResolveReqWithSubPathErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  MissingPackageNodeModulesFolder(#[from] MissingPackageNodeModulesFolderError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[class(inherit)]
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
}

impl ResolveReqWithSubPathErrorKind {
  pub fn as_types_not_found(&self) -> Option<&TypesNotFoundError> {
    match self {
      ResolveReqWithSubPathErrorKind::MissingPackageNodeModulesFolder(_)
      | ResolveReqWithSubPathErrorKind::ResolvePkgFolderFromDenoReq(_) => None,
      ResolveReqWithSubPathErrorKind::PackageSubpathResolve(
        package_subpath_resolve_error,
      ) => package_subpath_resolve_error.as_types_not_found(),
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum ResolvePkgFolderFromDenoReqError {
  #[class(inherit)]
  #[error(transparent)]
  Managed(managed::ManagedResolvePkgFolderFromDenoReqError),
  #[class(inherit)]
  #[error(transparent)]
  Byonm(byonm::ByonmResolvePkgFolderFromDenoReqError),
}

pub enum NpmResolverCreateOptions<
  TSys: FsRead
    + FsCanonicalize
    + FsMetadata
    + std::fmt::Debug
    + MaybeSend
    + MaybeSync
    + Clone
    + 'static,
> {
  Managed(ManagedNpmResolverCreateOptions<TSys>),
  Byonm(ByonmNpmResolverCreateOptions<TSys>),
}

#[derive(Debug, Clone)]
pub enum NpmResolver<TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir> {
  /// The resolver when "bring your own node_modules" is enabled where Deno
  /// does not setup the node_modules directories automatically, but instead
  /// uses what already exists on the file system.
  Byonm(ByonmNpmResolverRc<TSys>),
  Managed(ManagedNpmResolverRc<TSys>),
}

impl<TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir> NpmResolver<TSys> {
  pub fn new<
    TCreateSys: FsCanonicalize
      + FsMetadata
      + FsRead
      + FsReadDir
      + std::fmt::Debug
      + MaybeSend
      + MaybeSync
      + Clone
      + 'static,
  >(
    options: NpmResolverCreateOptions<TCreateSys>,
  ) -> NpmResolver<TCreateSys> {
    match options {
      NpmResolverCreateOptions::Managed(options) => {
        NpmResolver::Managed(new_rc(ManagedNpmResolver::<TCreateSys>::new::<
          TCreateSys,
        >(options)))
      }
      NpmResolverCreateOptions::Byonm(options) => {
        NpmResolver::Byonm(new_rc(ByonmNpmResolver::new(options)))
      }
    }
  }

  pub fn is_byonm(&self) -> bool {
    matches!(self, NpmResolver::Byonm(_))
  }

  pub fn is_managed(&self) -> bool {
    matches!(self, NpmResolver::Managed(_))
  }

  pub fn as_managed(&self) -> Option<&ManagedNpmResolver<TSys>> {
    match self {
      NpmResolver::Managed(resolver) => Some(resolver),
      NpmResolver::Byonm(_) => None,
    }
  }

  pub fn root_node_modules_path(&self) -> Option<&Path> {
    match self {
      NpmResolver::Byonm(resolver) => resolver.root_node_modules_path(),
      NpmResolver::Managed(resolver) => resolver.root_node_modules_path(),
    }
  }

  pub fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ResolvePkgFolderFromDenoReqError> {
    match self {
      NpmResolver::Byonm(byonm_resolver) => byonm_resolver
        .resolve_pkg_folder_from_deno_module_req(req, referrer)
        .map_err(ResolvePkgFolderFromDenoReqError::Byonm),
      NpmResolver::Managed(managed_resolver) => managed_resolver
        .resolve_pkg_folder_from_deno_module_req(req, referrer)
        .map_err(ResolvePkgFolderFromDenoReqError::Managed),
    }
  }
}

impl<TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir>
  NpmPackageFolderResolver for NpmResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, node_resolver::errors::PackageFolderResolveError> {
    match self {
      NpmResolver::Byonm(byonm_resolver) => {
        byonm_resolver.resolve_package_folder_from_package(specifier, referrer)
      }
      NpmResolver::Managed(managed_resolver) => managed_resolver
        .resolve_package_folder_from_package(specifier, referrer),
    }
  }
}

pub struct NpmReqResolverOptions<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
> {
  pub in_npm_pkg_checker: TInNpmPackageChecker,
  pub node_resolver: NodeResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  pub npm_resolver: NpmResolver<TSys>,
  pub sys: TSys,
}

#[allow(clippy::disallowed_types)]
pub type NpmReqResolverRc<
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = crate::sync::MaybeArc<
  NpmReqResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

#[derive(Debug)]
pub struct NpmReqResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
> {
  sys: TSys,
  in_npm_pkg_checker: TInNpmPackageChecker,
  node_resolver: NodeResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  npm_resolver: NpmResolver<TSys>,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
  >
  NpmReqResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    options: NpmReqResolverOptions<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  ) -> Self {
    Self {
      sys: options.sys,
      in_npm_pkg_checker: options.in_npm_pkg_checker,
      node_resolver: options.node_resolver,
      npm_resolver: options.npm_resolver,
    }
  }

  pub fn resolve_req_reference(
    &self,
    req_ref: &NpmPackageReqReference,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<UrlOrPath, ResolveReqWithSubPathError> {
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
  ) -> Result<UrlOrPath, ResolveReqWithSubPathError> {
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
        if err.as_types_not_found().is_some() {
          let maybe_definitely_typed_req =
            if let Some(npm_resolver) = self.npm_resolver.as_managed() {
              let snapshot = npm_resolver.resolution().snapshot();
              if let Some(nv) = snapshot.package_reqs().get(req) {
                let type_req = find_definitely_typed_package(
                  nv,
                  snapshot.package_reqs().iter(),
                );

                type_req.map(|(r, _)| r).cloned()
              } else {
                None
              }
            } else {
              Some(
                PackageReq::from_str(&format!(
                  "{}@*",
                  types_package_name(&req.name)
                ))
                .unwrap(),
              )
            };
          if let Some(req) = maybe_definitely_typed_req {
            if let Ok(resolved) = self.resolve_req_with_sub_path(
              &req,
              sub_path,
              referrer,
              resolution_mode,
              resolution_kind,
            ) {
              return Ok(resolved);
            }
          }
        }
        if matches!(self.npm_resolver, NpmResolver::Byonm(_)) {
          let package_json_path = package_folder.join("package.json");
          if !self.sys.fs_exists_no_err(&package_json_path) {
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
          | NodeResolveErrorKind::PathToUrl(_)
          | NodeResolveErrorKind::UrlToFilePath(_)
          | NodeResolveErrorKind::TypesNotFound(_)
          | NodeResolveErrorKind::FinalizeResolution(_) => Err(
            ResolveIfForNpmPackageErrorKind::NodeResolve(err.into()).into_box(),
          ),
          NodeResolveErrorKind::PackageResolve(err) => {
            let err = err.into_kind();
            match err {
              PackageResolveErrorKind::UrlToFilePath(err) => Err(
                ResolveIfForNpmPackageErrorKind::NodeResolve(
                  NodeResolveErrorKind::UrlToFilePath(err).into_box(),
                )
                .into_box(),
              ),
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
                  PackageFolderResolveErrorKind::PathToUrl(err) => Err(
                    ResolveIfForNpmPackageErrorKind::NodeResolve(
                      NodeResolveErrorKind::PathToUrl(err.clone()).into_box(),
                    )
                    .into_box(),
                  ),
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
                    if let NpmResolver::Byonm(byonm_npm_resolver) =
                      &self.npm_resolver
                    {
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

/// Attempt to choose the "best" `@types/*` package
/// if possible. If multiple versions exist, try to match
/// the major and minor versions of the `@types` package with the
/// actual package, falling back to the latest @types version present.
pub fn find_definitely_typed_package<'a>(
  nv: &'a PackageNv,
  packages: impl IntoIterator<Item = (&'a PackageReq, &'a PackageNv)>,
) -> Option<(&PackageReq, &PackageNv)> {
  let types_name = types_package_name(&nv.name);
  let mut best_patch = 0;
  let mut highest: Option<(&PackageReq, &PackageNv)> = None;
  let mut best = None;

  for (req, type_nv) in packages {
    if type_nv.name != types_name {
      continue;
    }
    if type_nv.version.major == nv.version.major
      && type_nv.version.minor == nv.version.minor
      && type_nv.version.patch >= best_patch
      && type_nv.version.pre == nv.version.pre
    {
      best = Some((req, type_nv));
      best_patch = type_nv.version.patch;
    }

    if let Some((_, highest_nv)) = highest {
      if type_nv.version > highest_nv.version {
        highest = Some((req, type_nv));
      }
    } else {
      highest = Some((req, type_nv));
    }
  }

  best.or(highest)
}
