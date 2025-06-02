// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use boxed_error::Boxed;
use deno_error::JsErrorBox;
use deno_graph::source::ResolveError;
use deno_graph::Module;
use deno_graph::Resolution;
use deno_semver::package::PackageReq;
use deno_unsync::sync::AtomicFlag;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NpmPackageFolderResolver;
use url::Url;

use crate::cjs::CjsTracker;
use crate::npm;
use crate::workspace::MappedResolutionDiagnostic;
use crate::workspace::MappedResolutionError;
use crate::workspace::ScopedJsxImportSourceConfig;
use crate::DenoResolveErrorKind;
use crate::DenoResolverSys;
use crate::RawDenoResolverRc;

#[allow(clippy::disallowed_types)]
pub type FoundPackageJsonDepFlagRc =
  crate::sync::MaybeArc<FoundPackageJsonDepFlag>;

/// A flag that indicates if a package.json dependency was
/// found during resolution.
#[derive(Debug, Default)]
pub struct FoundPackageJsonDepFlag(AtomicFlag);

#[derive(Debug, deno_error::JsError, Boxed)]
pub struct ResolveWithGraphError(pub Box<ResolveWithGraphErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolveWithGraphErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  CouldNotResolve(#[from] CouldNotResolveError),
  #[error(transparent)]
  #[class(inherit)]
  ResolvePkgFolderFromDenoModule(
    #[from] npm::managed::ResolvePkgFolderFromDenoModuleError,
  ),
  #[error(transparent)]
  #[class(inherit)]
  Resolution(#[from] deno_graph::ResolutionError),
  #[error(transparent)]
  #[class(inherit)]
  Resolve(#[from] ResolveError),
  #[error(transparent)]
  #[class(inherit)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Could not resolve '{reference}'")]
pub struct CouldNotResolveError {
  reference: deno_semver::npm::NpmPackageNvReference,
  #[source]
  #[inherit]
  source: node_resolver::errors::PackageSubpathResolveError,
}

impl FoundPackageJsonDepFlag {
  #[inline(always)]
  pub fn raise(&self) -> bool {
    self.0.raise()
  }

  #[inline(always)]
  pub fn is_raised(&self) -> bool {
    self.0.is_raised()
  }
}

pub struct MappedResolutionDiagnosticWithPosition<'a> {
  pub diagnostic: &'a MappedResolutionDiagnostic,
  pub referrer: &'a Url,
  pub start: deno_graph::Position,
}

#[allow(clippy::disallowed_types)]
pub type OnMappedResolutionDiagnosticFn = crate::sync::MaybeArc<
  dyn Fn(MappedResolutionDiagnosticWithPosition) + Send + Sync,
>;

pub type DefaultDenoResolverRc<TSys> = DenoResolverRc<
  npm::DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  npm::NpmResolver<TSys>,
  TSys,
>;

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

/// The resolver used in the CLI for resolving and interfacing
/// with deno_graph.
pub struct DenoResolver<
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoResolverSys,
> {
  resolver: RawDenoResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  sys: TSys,
  found_package_json_dep_flag: FoundPackageJsonDepFlagRc,
  warned_pkgs: crate::sync::MaybeDashSet<PackageReq>,
  on_warning: Option<OnMappedResolutionDiagnosticFn>,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: DenoResolverSys,
  > std::fmt::Debug
  for DenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DenoResolver").finish()
  }
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: DenoResolverSys,
  >
  DenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    resolver: RawDenoResolverRc<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
    sys: TSys,
    found_package_json_dep_flag: FoundPackageJsonDepFlagRc,
    on_warning: Option<OnMappedResolutionDiagnosticFn>,
  ) -> Self {
    Self {
      resolver,
      sys,
      found_package_json_dep_flag,
      warned_pkgs: Default::default(),
      on_warning,
    }
  }

  pub fn resolve_with_graph(
    &self,
    graph: &deno_graph::ModuleGraph,
    raw_specifier: &str,
    referrer: &Url,
    referrer_range_start: deno_graph::Position,
    resolution_mode: node_resolver::ResolutionMode,
    resolution_kind: node_resolver::NodeResolutionKind,
  ) -> Result<Url, ResolveWithGraphError> {
    let resolution = match graph.get(referrer) {
      Some(Module::Js(module)) => module
        .dependencies
        .get(raw_specifier)
        .map(|d| &d.maybe_code)
        .unwrap_or(&Resolution::None),
      _ => &Resolution::None,
    };

    let specifier = match resolution {
      Resolution::Ok(resolved) => Cow::Borrowed(&resolved.specifier),
      Resolution::Err(err) => {
        return Err(
          ResolveWithGraphErrorKind::Resolution((**err).clone()).into(),
        );
      }
      Resolution::None => Cow::Owned(self.resolve(
        raw_specifier,
        referrer,
        referrer_range_start,
        resolution_mode,
        resolution_kind,
      )?),
    };

    let specifier = match graph.get(&specifier) {
      Some(Module::Npm(module)) => {
        let node_and_npm_resolver =
          self.resolver.node_and_npm_resolver.as_ref().unwrap();
        let package_folder = node_and_npm_resolver
          .npm_resolver
          .as_managed()
          .unwrap() // byonm won't create a Module::Npm
          .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
        node_and_npm_resolver
          .node_resolver
          .resolve_package_subpath_from_deno_module(
            &package_folder,
            module.nv_reference.sub_path(),
            Some(referrer),
            resolution_mode,
            resolution_kind,
          )
          .map_err(|source| CouldNotResolveError {
            reference: module.nv_reference.clone(),
            source,
          })?
          .into_url()?
      }
      Some(Module::Node(module)) => module.specifier.clone(),
      Some(Module::Js(module)) => module.specifier.clone(),
      Some(Module::Json(module)) => module.specifier.clone(),
      Some(Module::Wasm(module)) => module.specifier.clone(),
      Some(Module::External(module)) => {
        node_resolver::resolve_specifier_into_node_modules(
          &self.sys,
          &module.specifier,
        )
      }
      None => specifier.into_owned(),
    };
    Ok(specifier)
  }

  pub fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &Url,
    referrer_range_start: deno_graph::Position,
    resolution_mode: node_resolver::ResolutionMode,
    resolution_kind: node_resolver::NodeResolutionKind,
  ) -> Result<Url, ResolveError> {
    let resolution = self
      .resolver
      .resolve(raw_specifier, referrer, resolution_mode, resolution_kind)
      .map_err(|err| match err.into_kind() {
        DenoResolveErrorKind::MappedResolution(mapped_resolution_error) => {
          match mapped_resolution_error {
            MappedResolutionError::Specifier(e) => ResolveError::Specifier(e),
            // deno_graph checks specifically for an ImportMapError
            MappedResolutionError::ImportMap(e) => ResolveError::ImportMap(e),
            MappedResolutionError::Workspace(e) => {
              ResolveError::Other(JsErrorBox::from_err(e))
            }
          }
        }
        err => ResolveError::Other(JsErrorBox::from_err(err)),
      })?;

    if resolution.found_package_json_dep {
      // mark that we need to do an "npm install" later
      self.found_package_json_dep_flag.raise();
    }

    if let Some(diagnostic) = resolution.maybe_diagnostic {
      let diagnostic = &*diagnostic;
      match diagnostic {
        MappedResolutionDiagnostic::ConstraintNotMatchedLocalVersion {
          reference,
          ..
        } => {
          if let Some(on_warning) = &self.on_warning {
            if self.warned_pkgs.insert(reference.req().clone()) {
              on_warning(MappedResolutionDiagnosticWithPosition {
                diagnostic,
                referrer,
                start: referrer_range_start,
              });
            }
          }
        }
      }
    }

    Ok(resolution.url)
  }

  pub fn as_graph_resolver<'a>(
    &'a self,
    cjs_tracker: &'a CjsTracker<TInNpmPackageChecker, TSys>,
    scoped_jsx_import_source_config: &'a ScopedJsxImportSourceConfig,
  ) -> DenoGraphResolverAdapter<
    'a,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  > {
    DenoGraphResolverAdapter {
      cjs_tracker,
      resolver: self,
      scoped_jsx_import_source_config,
    }
  }
}

pub struct DenoGraphResolverAdapter<
  'a,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: DenoResolverSys,
> {
  cjs_tracker: &'a CjsTracker<TInNpmPackageChecker, TSys>,
  resolver: &'a DenoResolver<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  scoped_jsx_import_source_config: &'a ScopedJsxImportSourceConfig,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: DenoResolverSys,
  > std::fmt::Debug
  for DenoGraphResolverAdapter<
    '_,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DenoGraphResolverAdapter").finish()
  }
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver: NpmPackageFolderResolver,
    TSys: DenoResolverSys,
  > deno_graph::source::Resolver
  for DenoGraphResolverAdapter<
    '_,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  fn default_jsx_import_source(&self, referrer: &Url) -> Option<String> {
    self
      .scoped_jsx_import_source_config
      .resolve_by_referrer(referrer)
      .and_then(|c| c.import_source.as_ref().map(|s| s.specifier.clone()))
  }

  fn default_jsx_import_source_types(&self, referrer: &Url) -> Option<String> {
    self
      .scoped_jsx_import_source_config
      .resolve_by_referrer(referrer)
      .and_then(|c| c.import_source_types.as_ref().map(|s| s.specifier.clone()))
  }

  fn jsx_import_source_module(&self, referrer: &Url) -> &str {
    self
      .scoped_jsx_import_source_config
      .resolve_by_referrer(referrer)
      .map(|c| c.module.as_str())
      .unwrap_or(deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    raw_specifier: &str,
    referrer_range: &deno_graph::Range,
    resolution_kind: deno_graph::source::ResolutionKind,
  ) -> Result<Url, ResolveError> {
    self.resolver.resolve(
      raw_specifier,
      &referrer_range.specifier,
      referrer_range.range.start,
      referrer_range
        .resolution_mode
        .map(node_resolver::ResolutionMode::from_deno_graph)
        .unwrap_or_else(|| {
          self
            .cjs_tracker
            .get_referrer_kind(&referrer_range.specifier)
        }),
      node_resolver::NodeResolutionKind::from_deno_graph(resolution_kind),
    )
  }
}
