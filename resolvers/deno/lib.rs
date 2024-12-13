// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

use std::path::PathBuf;
use std::sync::Arc;

use boxed_error::Boxed;
use deno_config::workspace::MappedResolution;
use deno_config::workspace::MappedResolutionDiagnostic;
use deno_config::workspace::MappedResolutionError;
use deno_config::workspace::WorkspaceResolvePkgJsonFolderError;
use deno_config::workspace::WorkspaceResolver;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_semver::npm::NpmPackageReqReference;
use fs::DenoResolverFs;
use node_resolver::env::NodeResolverEnv;
use node_resolver::errors::NodeResolveError;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolution;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::ResolutionMode;
use npm::MissingPackageNodeModulesFolderError;
use npm::NodeModulesOutOfDateError;
use npm::NpmReqResolver;
use npm::ResolveIfForNpmPackageErrorKind;
use npm::ResolvePkgFolderFromDenoReqError;
use npm::ResolveReqWithSubPathErrorKind;
use sloppy_imports::SloppyImportResolverFs;
use sloppy_imports::SloppyImportsResolutionKind;
use sloppy_imports::SloppyImportsResolver;
use thiserror::Error;
use url::Url;

pub mod cjs;
pub mod fs;
pub mod npm;
pub mod sloppy_imports;

#[derive(Debug, Clone)]
pub struct DenoResolution {
  pub url: Url,
  pub maybe_diagnostic: Option<Box<MappedResolutionDiagnostic>>,
  pub found_package_json_dep: bool,
}

#[derive(Debug, Boxed)]
pub struct DenoResolveError(pub Box<DenoResolveErrorKind>);

#[derive(Debug, Error)]
pub enum DenoResolveErrorKind {
  #[error("Importing from the vendor directory is not permitted. Use a remote specifier instead or disable vendoring.")]
  InvalidVendorFolderImport,
  #[error(transparent)]
  MappedResolution(#[from] MappedResolutionError),
  #[error(transparent)]
  MissingPackageNodeModulesFolder(#[from] MissingPackageNodeModulesFolderError),
  #[error(transparent)]
  Node(#[from] NodeResolveError),
  #[error(transparent)]
  NodeModulesOutOfDate(#[from] NodeModulesOutOfDateError),
  #[error(transparent)]
  PackageJsonDepValueParse(#[from] PackageJsonDepValueParseError),
  #[error(transparent)]
  PackageJsonDepValueUrlParse(url::ParseError),
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[error(transparent)]
  WorkspaceResolvePkgJsonFolder(#[from] WorkspaceResolvePkgJsonFolderError),
}

#[derive(Debug)]
pub struct NodeAndNpmReqResolver<
  Fs: DenoResolverFs,
  TNodeResolverEnv: NodeResolverEnv,
> {
  pub node_resolver: Arc<NodeResolver<TNodeResolverEnv>>,
  pub npm_req_resolver: Arc<NpmReqResolver<Fs, TNodeResolverEnv>>,
}

pub struct DenoResolverOptions<
  'a,
  Fs: DenoResolverFs,
  TNodeResolverEnv: NodeResolverEnv,
  TSloppyImportResolverFs: SloppyImportResolverFs,
> {
  pub in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
  pub node_and_req_resolver:
    Option<NodeAndNpmReqResolver<Fs, TNodeResolverEnv>>,
  pub sloppy_imports_resolver:
    Option<Arc<SloppyImportsResolver<TSloppyImportResolverFs>>>,
  pub workspace_resolver: Arc<WorkspaceResolver>,
  /// Whether "bring your own node_modules" is enabled where Deno does not
  /// setup the node_modules directories automatically, but instead uses
  /// what already exists on the file system.
  pub is_byonm: bool,
  pub maybe_vendor_dir: Option<&'a PathBuf>,
}

/// A resolver that takes care of resolution, taking into account loaded
/// import map, JSX settings.
#[derive(Debug)]
pub struct DenoResolver<
  Fs: DenoResolverFs,
  TNodeResolverEnv: NodeResolverEnv,
  TSloppyImportResolverFs: SloppyImportResolverFs,
> {
  in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
  node_and_npm_resolver: Option<NodeAndNpmReqResolver<Fs, TNodeResolverEnv>>,
  sloppy_imports_resolver:
    Option<Arc<SloppyImportsResolver<TSloppyImportResolverFs>>>,
  workspace_resolver: Arc<WorkspaceResolver>,
  is_byonm: bool,
  maybe_vendor_specifier: Option<Url>,
}

impl<
    Fs: DenoResolverFs,
    TNodeResolverEnv: NodeResolverEnv,
    TSloppyImportResolverFs: SloppyImportResolverFs,
  > DenoResolver<Fs, TNodeResolverEnv, TSloppyImportResolverFs>
{
  pub fn new(
    options: DenoResolverOptions<Fs, TNodeResolverEnv, TSloppyImportResolverFs>,
  ) -> Self {
    Self {
      in_npm_pkg_checker: options.in_npm_pkg_checker,
      node_and_npm_resolver: options.node_and_req_resolver,
      sloppy_imports_resolver: options.sloppy_imports_resolver,
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
          .map(|res| DenoResolution {
            url: res.into_url(),
            found_package_json_dep,
            maybe_diagnostic,
          })
          .map_err(|e| e.into());
      }
    }

    // Attempt to resolve with the workspace resolver
    let result: Result<_, DenoResolveError> = self
      .workspace_resolver
      .resolve(raw_specifier, referrer)
      .map_err(|err| err.into());
    let result = match result {
      Ok(resolution) => match resolution {
        MappedResolution::Normal {
          specifier,
          maybe_diagnostic: current_diagnostic,
        }
        | MappedResolution::ImportMap {
          specifier,
          maybe_diagnostic: current_diagnostic,
        } => {
          maybe_diagnostic = current_diagnostic;
          // do sloppy imports resolution if enabled
          if let Some(sloppy_imports_resolver) = &self.sloppy_imports_resolver {
            Ok(
              sloppy_imports_resolver
                .resolve(
                  &specifier,
                  match resolution_kind {
                    NodeResolutionKind::Execution => {
                      SloppyImportsResolutionKind::Execution
                    }
                    NodeResolutionKind::Types => {
                      SloppyImportsResolutionKind::Types
                    }
                  },
                )
                .map(|s| s.into_specifier())
                .unwrap_or(specifier),
            )
          } else {
            Ok(specifier)
          }
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
          .map_err(|e| e.into()),
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
                }),
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
              .map(|url| DenoResolution {
                url,
                maybe_diagnostic,
                found_package_json_dep,
              })
              .map_err(|e| e.into());
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
              .map(|url| DenoResolution {
                url,
                maybe_diagnostic,
                found_package_json_dep,
              })
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
              NodeResolution::Module(url) => {
                return Ok(DenoResolution {
                  url,
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
