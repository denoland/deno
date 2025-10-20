// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

use std::borrow::Cow;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_cache_dir::npm::NpmCacheDir;
use deno_error::JsError;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_semver::npm::NpmPackageReqReference;
pub use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionKind;
pub use node_resolver::NodeResolverOptions;
use node_resolver::NodeResolverRc;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
use node_resolver::UrlOrPath;
use node_resolver::UrlOrPathRef;
use node_resolver::errors::NodeJsErrorCode;
use node_resolver::errors::NodeResolveError;
use node_resolver::errors::NodeResolveErrorKind;
use node_resolver::errors::UnknownBuiltInNodeModuleError;
use npm::NodeModulesOutOfDateError;
use npm::NpmReqResolverRc;
use npm::ResolveIfForNpmPackageErrorKind;
use npm::ResolvePkgFolderFromDenoReqError;
use thiserror::Error;
use url::Url;

use self::npm::NpmResolver;
use self::npm::NpmResolverSys;
use self::npm::ResolveNpmReqRefError;
use crate::workspace::MappedResolution;
use crate::workspace::MappedResolutionDiagnostic;
use crate::workspace::MappedResolutionError;
use crate::workspace::WorkspaceResolvePkgJsonFolderError;
use crate::workspace::WorkspaceResolver;

pub mod cache;
pub mod cjs;
pub mod collections;
pub mod deno_json;
pub mod display;
#[cfg(feature = "deno_ast")]
pub mod emit;
pub mod factory;
#[cfg(feature = "graph")]
pub mod file_fetcher;
#[cfg(feature = "graph")]
pub mod graph;
pub mod import_map;
pub mod loader;
pub mod lockfile;
pub mod npm;
pub mod npmrc;
#[cfg(feature = "sync")]
mod rt;
pub mod workspace;

#[allow(clippy::disallowed_types)]
pub type WorkspaceResolverRc<TSys> =
  deno_maybe_sync::MaybeArc<WorkspaceResolver<TSys>>;

#[allow(clippy::disallowed_types)]
pub(crate) type NpmCacheDirRc = deno_maybe_sync::MaybeArc<NpmCacheDir>;

#[derive(Debug, Clone)]
pub struct DenoResolution {
  pub url: Url,
  pub maybe_diagnostic: Option<Box<MappedResolutionDiagnostic>>,
  pub found_package_json_dep: bool,
}

#[derive(Debug, Boxed, JsError)]
pub struct DenoResolveError(pub Box<DenoResolveErrorKind>);

impl DenoResolveError {
  #[cfg(feature = "graph")]
  pub fn into_deno_graph_error(self) -> deno_graph::source::ResolveError {
    use deno_error::JsErrorBox;
    use deno_graph::source::ResolveError;

    match self.into_kind() {
      DenoResolveErrorKind::MappedResolution(mapped_resolution_error) => {
        match mapped_resolution_error {
          MappedResolutionError::Specifier(e) => ResolveError::Specifier(e),
          // deno_graph checks specifically for an ImportMapError
          MappedResolutionError::ImportMap(e) => ResolveError::ImportMap(e),
          MappedResolutionError::Workspace(e) => {
            ResolveError::Other(JsErrorBox::from_err(e))
          }
          MappedResolutionError::NotFoundInCompilerOptionsPaths(e) => {
            ResolveError::Other(JsErrorBox::from_err(e))
          }
        }
      }
      err => ResolveError::Other(JsErrorBox::from_err(err)),
    }
  }

  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      DenoResolveErrorKind::Node(err) => err.maybe_specifier(),
      DenoResolveErrorKind::PathToUrl(err) => {
        Some(Cow::Owned(UrlOrPath::Path(err.0.clone())))
      }
      DenoResolveErrorKind::ResolveNpmReqRef(err) => err.err.maybe_specifier(),
      DenoResolveErrorKind::MappedResolution(_)
      | DenoResolveErrorKind::WorkspaceResolvePkgJsonFolder(_)
      | DenoResolveErrorKind::ResolvePkgFolderFromDenoReq(_)
      | DenoResolveErrorKind::InvalidVendorFolderImport
      | DenoResolveErrorKind::UnsupportedPackageJsonFileSpecifier
      | DenoResolveErrorKind::UnsupportedPackageJsonJsrReq
      | DenoResolveErrorKind::NodeModulesOutOfDate(_)
      | DenoResolveErrorKind::PackageJsonDepValueParse(_)
      | DenoResolveErrorKind::PackageJsonDepValueUrlParse(_) => None,
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum DenoResolveErrorKind {
  #[class(type)]
  #[error(
    "Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring."
  )]
  InvalidVendorFolderImport,
  #[class(type)]
  #[error(
    "Importing npm packages via a file: specifier is only supported with --node-modules-dir=manual"
  )]
  UnsupportedPackageJsonFileSpecifier,
  #[class(type)]
  #[error("JSR specifiers are not yet supported in package.json")]
  UnsupportedPackageJsonJsrReq,
  #[class(inherit)]
  #[error(transparent)]
  MappedResolution(#[from] MappedResolutionError),
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
  PathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveNpmReqRef(#[from] ResolveNpmReqRefError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[class(inherit)]
  #[error(transparent)]
  WorkspaceResolvePkgJsonFolder(#[from] WorkspaceResolvePkgJsonFolderError),
}

impl DenoResolveErrorKind {
  pub fn maybe_node_code(&self) -> Option<NodeJsErrorCode> {
    match self {
      DenoResolveErrorKind::InvalidVendorFolderImport
      | DenoResolveErrorKind::UnsupportedPackageJsonFileSpecifier
      | DenoResolveErrorKind::UnsupportedPackageJsonJsrReq
      | DenoResolveErrorKind::MappedResolution { .. }
      | DenoResolveErrorKind::NodeModulesOutOfDate { .. }
      | DenoResolveErrorKind::PackageJsonDepValueParse { .. }
      | DenoResolveErrorKind::PackageJsonDepValueUrlParse { .. }
      | DenoResolveErrorKind::PathToUrl { .. }
      | DenoResolveErrorKind::ResolvePkgFolderFromDenoReq { .. }
      | DenoResolveErrorKind::WorkspaceResolvePkgJsonFolder { .. } => None,
      DenoResolveErrorKind::ResolveNpmReqRef(err) => {
        err.err.as_kind().maybe_code()
      }
      DenoResolveErrorKind::Node(err) => err.as_kind().maybe_code(),
    }
  }
}

#[derive(Debug)]
pub struct NodeAndNpmResolvers<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NpmResolverSys,
> {
  pub node_resolver: NodeResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  pub npm_resolver: NpmResolver<TSys>,
  pub npm_req_resolver: NpmReqResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
}

#[sys_traits::auto_impl]
pub trait DenoResolverSys: NpmResolverSys {}

pub struct DenoResolverOptions<
  'a,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoResolverSys,
> {
  pub in_npm_pkg_checker: TInNpmPackageChecker,
  pub node_and_req_resolver: Option<
    NodeAndNpmResolvers<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  >,
  pub workspace_resolver: WorkspaceResolverRc<TSys>,
  /// Whether bare node built-ins are enabled (ex. resolve "path" as "node:path").
  pub bare_node_builtins: bool,
  /// Whether "bring your own node_modules" is enabled where Deno does not
  /// setup the node_modules directories automatically, but instead uses
  /// what already exists on the file system.
  pub is_byonm: bool,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
}

#[allow(clippy::disallowed_types)]
pub type RawDenoResolverRc<
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = deno_maybe_sync::MaybeArc<
  RawDenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

/// Helper type for a RawDenoResolverRc that has the implementations
/// used by the Deno CLI.
pub type DefaultRawDenoResolverRc<TSys> = RawDenoResolverRc<
  npm::DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  npm::NpmResolver<TSys>,
  TSys,
>;

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct RawDenoResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoResolverSys,
> {
  in_npm_pkg_checker: TInNpmPackageChecker,
  node_and_npm_resolver: Option<
    NodeAndNpmResolvers<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
  >,
  workspace_resolver: WorkspaceResolverRc<TSys>,
  bare_node_builtins: bool,
  is_byonm: bool,
  maybe_vendor_specifier: Option<Url>,
}

impl<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoResolverSys,
>
  RawDenoResolver<
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
      bare_node_builtins: options.bare_node_builtins,
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
        log::debug!(
          "{}: specifier={} referrer={} mode={:?} kind={:?}",
          deno_terminal::colors::magenta("resolving in npm package"),
          raw_specifier,
          referrer,
          resolution_mode,
          resolution_kind
        );
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
    let result = self.workspace_resolver.resolve(
      raw_specifier,
      referrer,
      resolution_kind.into(),
    );
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
          .map_err(|e| {
            DenoResolveErrorKind::Node(e.into_node_resolve_error()).into_box()
          })
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
              PackageJsonDepValue::JsrReq(_) => Err(
                DenoResolveErrorKind::UnsupportedPackageJsonJsrReq.into_box(),
              ),
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
                      DenoResolveErrorKind::Node(e.into_node_resolve_error())
                        .into_box()
                    })
                })
                .and_then(|r| Ok(r.into_url()?)),
            })
        }
        MappedResolution::PackageJsonImport { pkg_json } => self
          .node_and_npm_resolver
          .as_ref()
          .unwrap()
          .node_resolver
          .resolve_package_import(
            raw_specifier,
            Some(&UrlOrPathRef::from_url(referrer)),
            Some(pkg_json),
            resolution_mode,
            resolution_kind,
          )
          .map_err(|e| {
            DenoResolveErrorKind::Node(
              NodeResolveErrorKind::PackageImportsResolve(e).into_box(),
            )
            .into_box()
          })
          .and_then(|r| Ok(r.into_url()?)),
      },
      Err(err) => Err(err.into()),
    };

    // When the user is vendoring, don't allow them to import directly from the vendor/ directory
    // as it might cause them confusion or duplicate dependencies. Additionally, this folder has
    // special treatment in the language server so it will definitely cause issues/confusion there
    // if they do this.
    if let Some(vendor_specifier) = &self.maybe_vendor_specifier
      && let Ok(specifier) = &result
      && specifier.as_str().starts_with(vendor_specifier.as_str())
    {
      return Err(DenoResolveErrorKind::InvalidVendorFolderImport.into_box());
    }

    let Some(NodeAndNpmResolvers {
      node_resolver,
      npm_req_resolver,
      ..
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
        if specifier.scheme() == "node" {
          let module_name = specifier.path();
          return if node_resolver.is_builtin_node_module(module_name) {
            Ok(DenoResolution {
              url: specifier,
              maybe_diagnostic,
              found_package_json_dep,
            })
          } else {
            Err(
              NodeResolveErrorKind::UnknownBuiltInNodeModule(
                UnknownBuiltInNodeModuleError {
                  module_name: module_name.to_string(),
                },
              )
              .into_box()
              .into(),
            )
          };
        }

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
              .map_err(|err| {
                DenoResolveErrorKind::ResolveNpmReqRef(ResolveNpmReqRefError {
                  npm_req_ref: npm_req_ref.clone(),
                  err: err.into(),
                })
                .into_box()
              })
              .and_then(|url_or_path| {
                Ok(DenoResolution {
                  url: url_or_path.into_url()?,
                  maybe_diagnostic,
                  found_package_json_dep,
                })
              });
          }

          if self.is_byonm {
            return npm_req_resolver
              .resolve_req_reference(
                &npm_req_ref,
                referrer,
                resolution_mode,
                resolution_kind,
              )
              .map_err(|err| {
                DenoResolveErrorKind::ResolveNpmReqRef(err).into_box()
              })
              .and_then(|url_or_path| {
                Ok(DenoResolution {
                  url: url_or_path.into_url()?,
                  maybe_diagnostic,
                  found_package_json_dep,
                })
              });
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
                });
              }
              NodeResolution::BuiltIn(ref _module) => {
                if self.bare_node_builtins {
                  return Ok(DenoResolution {
                    url: res.into_url()?,
                    maybe_diagnostic,
                    found_package_json_dep,
                  });
                }
              }
            }
          }
        } else if self.bare_node_builtins
          && matches!(err.as_kind(), DenoResolveErrorKind::MappedResolution(err) if err.is_unmapped_bare_specifier())
          && node_resolver.is_builtin_node_module(raw_specifier)
        {
          return Ok(DenoResolution {
            url: Url::parse(&format!("node:{}", raw_specifier)).unwrap(),
            maybe_diagnostic,
            found_package_json_dep,
          });
        }

        Err(err)
      }
    }
  }

  #[cfg(feature = "graph")]
  pub(crate) fn resolve_non_workspace_npm_req_ref_to_file(
    &self,
    npm_req_ref: &NpmPackageReqReference,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    resolution_kind: NodeResolutionKind,
  ) -> Result<node_resolver::UrlOrPath, npm::ResolveNpmReqRefError> {
    let Some(NodeAndNpmResolvers {
      npm_req_resolver, ..
    }) = &self.node_and_npm_resolver
    else {
      return Err(npm::ResolveNpmReqRefError {
        npm_req_ref: npm_req_ref.clone(),
        err: npm::ResolveReqWithSubPathErrorKind::NoNpm(npm::NoNpmError)
          .into_box(),
      });
    };
    npm_req_resolver.resolve_req_reference(
      npm_req_ref,
      referrer,
      resolution_mode,
      resolution_kind,
    )
  }
}
