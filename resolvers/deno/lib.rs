// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

use std::path::PathBuf;

use boxed_error::Boxed;
use deno_cache_dir::npm::NpmCacheDir;
use deno_error::JsError;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::errors::NodeResolveError;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolverRc;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
use npm::MissingPackageNodeModulesFolderError;
use npm::NodeModulesOutOfDateError;
use npm::NpmReqResolverRc;
use npm::ResolveIfForNpmPackageErrorKind;
use npm::ResolvePkgFolderFromDenoReqError;
use npm::ResolveReqWithSubPathErrorKind;
use sys_traits::FsCanonicalize;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use thiserror::Error;
use url::Url;

use crate::workspace::MappedResolution;
use crate::workspace::MappedResolutionDiagnostic;
use crate::workspace::MappedResolutionError;
use crate::workspace::WorkspaceResolvePkgJsonFolderError;
use crate::workspace::WorkspaceResolver;

pub mod cjs;
pub mod factory;
pub mod npm;
pub mod npmrc;
mod sync;
pub mod workspace;

#[allow(clippy::disallowed_types)]
pub type WorkspaceResolverRc<TSys> =
  crate::sync::MaybeArc<WorkspaceResolver<TSys>>;

#[allow(clippy::disallowed_types)]
pub(crate) type NpmCacheDirRc = crate::sync::MaybeArc<NpmCacheDir>;

#[derive(Debug, Clone)]
pub struct DenoResolution {
  pub url: Url,
  pub maybe_diagnostic: Option<Box<MappedResolutionDiagnostic>>,
  pub found_package_json_dep: bool,
}

#[derive(Debug, Boxed, JsError)]
pub struct DenoResolveError(pub Box<DenoResolveErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum DenoResolveErrorKind {
  #[class(type)]
  #[error("Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.")]
  InvalidVendorFolderImport,
  #[class(type)]
  #[error("Importing npm packages via a file: specifier is only supported with --node-modules-dir=manual")]
  UnsupportedPackageJsonFileSpecifier,
  #[class(inherit)]
  #[error(transparent)]
  MappedResolution(#[from] MappedResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  MissingPackageNodeModulesFolder(#[from] MissingPackageNodeModulesFolderError),
  #[class(inherit)]
  #[error(transparent)]
  Node(#[from] NodeResolveError),
  #[class(inherit)]
  #[error(transparent)]
  NodeModulesOutOfDate(#[from] NodeModulesOutOfDateError),
  #[class(inherit)]
  #[error(transparent)]
  PackageJsonDepValueParse(#[from] PackageJsonDepValueParseError),
  #[class(inherit)]
  #[error(transparent)]
  PackageJsonDepValueUrlParse(url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[class(inherit)]
  #[error(transparent)]
  WorkspaceResolvePkgJsonFolder(#[from] WorkspaceResolvePkgJsonFolderError),
}

#[derive(Debug)]
pub struct NodeAndNpmReqResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
> {
  pub node_resolver: NodeResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  pub npm_req_resolver: NpmReqResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
}

pub struct DenoResolverOptions<
  'a,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
> {
  pub in_npm_pkg_checker: TInNpmPackageChecker,
  pub node_and_req_resolver: Option<
    NodeAndNpmReqResolver<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  >,
  pub workspace_resolver: WorkspaceResolverRc<TSys>,
  /// Whether "bring your own node_modules" is enabled where Deno does not
  /// setup the node_modules directories automatically, but instead uses
  /// what already exists on the file system.
  pub is_byonm: bool,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
}

#[allow(clippy::disallowed_types)]
pub type DenoResolverRc<
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = crate::sync::MaybeArc<
  DenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

/// Helper type for a DenoResolverRc that has the implementations
/// used by the Deno CLI.
pub type DefaultDenoResolverRc<TSys> = DenoResolverRc<
  npm::DenoInNpmPackageChecker,
  node_resolver::DenoIsBuiltInNodeModuleChecker,
  npm::NpmResolver<TSys>,
  TSys,
>;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct DenoResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
> {
  in_npm_pkg_checker: TInNpmPackageChecker,
  node_and_npm_resolver: Option<
    NodeAndNpmReqResolver<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  >,
  workspace_resolver: WorkspaceResolverRc<TSys>,
  is_byonm: bool,
  maybe_vendor_specifier: Option<Url>,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir,
  >
  DenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    options: DenoResolverOptions<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  ) -> Self {
    Self {
      in_npm_pkg_checker: options.in_npm_pkg_checker,
      node_and_npm_resolver: options.node_and_req_resolver,
      workspace_resolver: options.workspace_resolver,
      is_byonm: options.is_byonm,
      maybe_vendor_specifier: options
        .maybe_vendor_dir
        .and_then(|v| deno_path_util::url_from_directory_path(v).ok()),
    }
  }

  pub fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<DenoResolution, DenoResolveError> {
    let mut found_package_json_dep = false;
    let mut maybe_diagnostic = None;
    // Use node resolution if we're in an npm package
    if let Some(node_and_npm_resolver) = self.node_and_npm_resolver.as_ref() {
      let node_resolver = &node_and_npm_resolver.node_resolver;
      if referrer.scheme() == "file"
        && self.in_npm_pkg_checker.in_npm_package(referrer)
      {
        return node_resolver
          .resolve(raw_specifier, referrer, resolution_mode, resolution_kind)
          .and_then(|res| {
            Ok(DenoResolution {
              url: res.into_url()?,
              found_package_json_dep,
              maybe_diagnostic,
            })
          })
          .map_err(|e| e.into());
      }
    }

    // Attempt to resolve with the workspace resolver
    let result: Result<_, DenoResolveError> = self
      .workspace_resolver
      .resolve(raw_specifier, referrer, resolution_kind.into())
      .map_err(|err| err.into());
    let result = match result {
      Ok(resolution) => match resolution {
        MappedResolution::Normal {
          specifier,
          maybe_diagnostic: current_diagnostic,
          ..
        } => {
          maybe_diagnostic = current_diagnostic;
          Ok(specifier)
        }
        MappedResolution::WorkspaceJsrPackage { specifier, .. } => {
          Ok(specifier)
        }
        MappedResolution::WorkspaceNpmPackage {
          target_pkg_json: pkg_json,
          sub_path,
          ..
        } => self
          .node_and_npm_resolver
          .as_ref()
          .unwrap()
          .node_resolver
          .resolve_package_subpath_from_deno_module(
            pkg_json.dir_path(),
            sub_path.as_deref(),
            Some(referrer),
            resolution_mode,
            resolution_kind,
          )
          .map_err(DenoResolveError::from)
          .and_then(|r| Ok(r.into_url()?)),
        MappedResolution::PackageJson {
          dep_result,
          alias,
          sub_path,
          ..
        } => {
          // found a specifier in the package.json, so mark that
          // we need to do an "npm install" later
          found_package_json_dep = true;

          dep_result
            .as_ref()
            .map_err(|e| {
              DenoResolveErrorKind::PackageJsonDepValueParse(e.clone())
                .into_box()
            })
            .and_then(|dep| match dep {
              PackageJsonDepValue::File(_) => {
                // We don't support --node-modules-dir=auto/none because it's too
                // much work to get this to work with a lockfile properly and for
                // multiple managed node_modules directories to work. If someone wants
                // to do this, then they need to use the default (manual)
                Err(
                  DenoResolveErrorKind::UnsupportedPackageJsonFileSpecifier
                    .into_box(),
                )
              }
              // todo(dsherret): it seems bad that we're converting this
              // to a url because the req might not be a valid url.
              PackageJsonDepValue::Req(req) => Url::parse(&format!(
                "npm:{}{}",
                req,
                sub_path.map(|s| format!("/{}", s)).unwrap_or_default()
              ))
              .map_err(|e| {
                DenoResolveErrorKind::PackageJsonDepValueUrlParse(e).into_box()
              }),
              PackageJsonDepValue::Workspace(version_req) => self
                .workspace_resolver
                .resolve_workspace_pkg_json_folder_for_pkg_json_dep(
                  alias,
                  version_req,
                )
                .map_err(|e| {
                  DenoResolveErrorKind::WorkspaceResolvePkgJsonFolder(e)
                    .into_box()
                })
                .and_then(|pkg_folder| {
                  self
                    .node_and_npm_resolver
                    .as_ref()
                    .unwrap()
                    .node_resolver
                    .resolve_package_subpath_from_deno_module(
                      pkg_folder,
                      sub_path.as_deref(),
                      Some(referrer),
                      resolution_mode,
                      resolution_kind,
                    )
                    .map_err(|e| {
                      DenoResolveErrorKind::PackageSubpathResolve(e).into_box()
                    })
                })
                .and_then(|r| Ok(r.into_url()?)),
            })
        }
      },
      Err(err) => Err(err),
    };

    // When the user is vendoring, don't allow them to import directly from the vendor/ directory
    // as it might cause them confusion or duplicate dependencies. Additionally, this folder has
    // special treatment in the language server so it will definitely cause issues/confusion there
    // if they do this.
    if let Some(vendor_specifier) = &self.maybe_vendor_specifier {
      if let Ok(specifier) = &result {
        if specifier.as_str().starts_with(vendor_specifier.as_str()) {
          return Err(
            DenoResolveErrorKind::InvalidVendorFolderImport.into_box(),
          );
        }
      }
    }

    let Some(NodeAndNpmReqResolver {
      node_resolver,
      npm_req_resolver,
    }) = &self.node_and_npm_resolver
    else {
      return Ok(DenoResolution {
        url: result?,
        maybe_diagnostic,
        found_package_json_dep,
      });
    };

    match result {
      Ok(specifier) => {
        if let Ok(npm_req_ref) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          // check if the npm specifier resolves to a workspace member
          if let Some(pkg_folder) = self
            .workspace_resolver
            .resolve_workspace_pkg_json_folder_for_npm_specifier(
              npm_req_ref.req(),
            )
          {
            return node_resolver
              .resolve_package_subpath_from_deno_module(
                pkg_folder,
                npm_req_ref.sub_path(),
                Some(referrer),
                resolution_mode,
                resolution_kind,
              )
              .map_err(DenoResolveError::from)
              .and_then(|url_or_path| {
                Ok(DenoResolution {
                  url: url_or_path.into_url()?,
                  maybe_diagnostic,
                  found_package_json_dep,
                })
              });
          }

          // do npm resolution for byonm
          if self.is_byonm {
            return npm_req_resolver
              .resolve_req_reference(
                &npm_req_ref,
                referrer,
                resolution_mode,
                resolution_kind,
              )
              .map_err(|err| {
                match err.into_kind() {
                  ResolveReqWithSubPathErrorKind::MissingPackageNodeModulesFolder(
                    err,
                  ) => err.into(),
                  ResolveReqWithSubPathErrorKind::ResolvePkgFolderFromDenoReq(
                    err,
                  ) => err.into(),
                  ResolveReqWithSubPathErrorKind::PackageSubpathResolve(err) => {
                    err.into()
                  }
                }
              })
              .and_then(|url_or_path| Ok(DenoResolution {
                url: url_or_path.into_url()?,
                maybe_diagnostic,
                found_package_json_dep,
              }));
          }
        }

        Ok(DenoResolution {
          url: node_resolver
            .handle_if_in_node_modules(&specifier)
            .unwrap_or(specifier),
          maybe_diagnostic,
          found_package_json_dep,
        })
      }
      Err(err) => {
        // If byonm, check if the bare specifier resolves to an npm package
        if self.is_byonm && referrer.scheme() == "file" {
          let maybe_resolution = npm_req_resolver
            .resolve_if_for_npm_pkg(
              raw_specifier,
              referrer,
              resolution_mode,
              resolution_kind,
            )
            .map_err(|e| match e.into_kind() {
              ResolveIfForNpmPackageErrorKind::NodeResolve(e) => {
                DenoResolveErrorKind::Node(e).into_box()
              }
              ResolveIfForNpmPackageErrorKind::NodeModulesOutOfDate(e) => {
                e.into()
              }
            })?;
          if let Some(res) = maybe_resolution {
            match res {
              NodeResolution::Module(ref _url) => {
                return Ok(DenoResolution {
                  url: res.into_url()?,
                  maybe_diagnostic,
                  found_package_json_dep,
                })
              }
              NodeResolution::BuiltIn(_) => {
                // don't resolve bare specifiers for built-in modules via node resolution
              }
            }
          }
        }

        Err(err)
      }
    }
  }
}
