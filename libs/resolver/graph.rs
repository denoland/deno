// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use boxed_error::Boxed;
use deno_error::JsErrorClass;
use deno_graph::JsrLoadError;
use deno_graph::Module;
use deno_graph::ModuleError;
use deno_graph::ModuleErrorKind;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleLoadError;
use deno_graph::Resolution;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_graph::source::ResolveError;
use deno_media_type::MediaType;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use deno_unsync::sync::AtomicFlag;
use import_map::ImportMapErrorKind;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::UrlOrPath;
use node_resolver::errors::NodeJsErrorCoded;
use url::Url;

use crate::DenoResolveError;
use crate::DenoResolverSys;
use crate::RawDenoResolverRc;
use crate::cjs::CjsTracker;
use crate::deno_json::JsxImportSourceConfigResolver;
use crate::npm;
use crate::npm::managed::ManagedResolvePkgFolderFromDenoReqError;
use crate::workspace::MappedResolutionDiagnostic;
use crate::workspace::sloppy_imports_resolve;

#[allow(clippy::disallowed_types)]
pub type FoundPackageJsonDepFlagRc =
  deno_maybe_sync::MaybeArc<FoundPackageJsonDepFlag>;

/// A flag that indicates if a package.json dependency was
/// found during resolution.
#[derive(Debug, Default)]
pub struct FoundPackageJsonDepFlag(AtomicFlag);

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

#[derive(Debug, deno_error::JsError, Boxed)]
pub struct ResolveWithGraphError(pub Box<ResolveWithGraphErrorKind>);

impl ResolveWithGraphError {
  pub fn maybe_specifier(&self) -> Option<Cow<'_, UrlOrPath>> {
    match self.as_kind() {
      ResolveWithGraphErrorKind::ManagedResolvePkgFolderFromDenoReq(_) => None,
      ResolveWithGraphErrorKind::CouldNotResolveNpmReqRef(err) => {
        err.source.maybe_specifier()
      }
      ResolveWithGraphErrorKind::ResolveNpmReqRef(err) => {
        err.err.maybe_specifier()
      }
      ResolveWithGraphErrorKind::Resolution(err) => match err {
        deno_graph::ResolutionError::InvalidDowngrade { specifier, .. } => {
          Some(specifier)
        }
        deno_graph::ResolutionError::InvalidJsrHttpsTypesImport {
          specifier,
          ..
        } => Some(specifier),
        deno_graph::ResolutionError::InvalidLocalImport {
          specifier, ..
        } => Some(specifier),
        deno_graph::ResolutionError::ResolverError { .. }
        | deno_graph::ResolutionError::InvalidSpecifier { .. } => None,
      }
      .map(|s| Cow::Owned(UrlOrPath::Url(s.clone()))),
      ResolveWithGraphErrorKind::Resolve(err) => err.maybe_specifier(),
      ResolveWithGraphErrorKind::PathToUrl(err) => {
        Some(Cow::Owned(UrlOrPath::Path(err.0.clone())))
      }
      ResolveWithGraphErrorKind::ResolvePkgFolderFromDenoModule(_) => None,
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolveWithGraphErrorKind {
  #[error(transparent)]
  #[class(inherit)]
  ManagedResolvePkgFolderFromDenoReq(
    #[from] ManagedResolvePkgFolderFromDenoReqError,
  ),
  #[error(transparent)]
  #[class(inherit)]
  CouldNotResolveNpmReqRef(#[from] CouldNotResolveNpmReqRefError),
  #[error(transparent)]
  #[class(inherit)]
  ResolvePkgFolderFromDenoModule(
    #[from] npm::managed::ResolvePkgFolderFromDenoModuleError,
  ),
  #[error(transparent)]
  #[class(inherit)]
  ResolveNpmReqRef(#[from] npm::ResolveNpmReqRefError),
  #[error(transparent)]
  #[class(inherit)]
  Resolution(#[from] deno_graph::ResolutionError),
  #[error(transparent)]
  #[class(inherit)]
  Resolve(#[from] DenoResolveError),
  #[error(transparent)]
  #[class(inherit)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Could not resolve '{reference}'")]
pub struct CouldNotResolveNpmReqRefError {
  pub reference: deno_semver::npm::NpmPackageReqReference,
  #[source]
  #[inherit]
  pub source: node_resolver::errors::PackageSubpathFromDenoModuleResolveError,
}

impl NodeJsErrorCoded for CouldNotResolveNpmReqRefError {
  fn code(&self) -> node_resolver::errors::NodeJsErrorCode {
    self.source.code()
  }
}

pub struct MappedResolutionDiagnosticWithPosition<'a> {
  pub diagnostic: &'a MappedResolutionDiagnostic,
  pub referrer: &'a Url,
  pub start: deno_graph::Position,
}

#[allow(clippy::disallowed_types)]
pub type OnMappedResolutionDiagnosticFn = deno_maybe_sync::MaybeArc<
  dyn Fn(MappedResolutionDiagnosticWithPosition) + Send + Sync,
>;

pub struct ResolveWithGraphOptions {
  pub mode: node_resolver::ResolutionMode,
  pub kind: node_resolver::NodeResolutionKind,
  /// Whether to maintain npm specifiers as-is. It's necessary for the
  /// deno_core module loader to resolve npm specifiers as-is so that
  /// the loader can properly dynamic import and install npm packages
  /// when managed.
  pub maintain_npm_specifiers: bool,
}

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
> = deno_maybe_sync::MaybeArc<
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
  warned_pkgs: deno_maybe_sync::MaybeDashSet<PackageReq>,
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
    options: ResolveWithGraphOptions,
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
        options.mode,
        options.kind,
      )?),
    };

    let specifier = match graph.get(&specifier) {
      Some(Module::Npm(_)) => {
        if options.maintain_npm_specifiers {
          specifier.into_owned()
        } else {
          let req_ref =
            NpmPackageReqReference::from_specifier(&specifier).unwrap();
          self.resolve_managed_npm_req_ref(
            &req_ref,
            Some(referrer),
            options.mode,
            options.kind,
          )?
        }
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
      None => {
        if options.maintain_npm_specifiers {
          specifier.into_owned()
        } else {
          match NpmPackageReqReference::from_specifier(&specifier) {
            Ok(reference) => {
              let url =
                self.resolver.resolve_non_workspace_npm_req_ref_to_file(
                  &reference,
                  referrer,
                  options.mode,
                  options.kind,
                )?;
              url.into_url()?
            }
            _ => specifier.into_owned(),
          }
        }
      }
    };
    Ok(specifier)
  }

  pub fn resolve_non_workspace_npm_req_ref_to_file(
    &self,
    npm_req_ref: &NpmPackageReqReference,
    referrer: &Url,
    resolution_mode: node_resolver::ResolutionMode,
    resolution_kind: node_resolver::NodeResolutionKind,
  ) -> Result<node_resolver::UrlOrPath, npm::ResolveNpmReqRefError> {
    self.resolver.resolve_non_workspace_npm_req_ref_to_file(
      npm_req_ref,
      referrer,
      resolution_mode,
      resolution_kind,
    )
  }

  pub fn resolve_managed_npm_req_ref(
    &self,
    req_ref: &NpmPackageReqReference,
    maybe_referrer: Option<&Url>,
    resolution_mode: node_resolver::ResolutionMode,
    resolution_kind: node_resolver::NodeResolutionKind,
  ) -> Result<Url, ResolveWithGraphError> {
    let node_and_npm_resolver =
      self.resolver.node_and_npm_resolver.as_ref().unwrap();
    let managed_resolver = node_and_npm_resolver
      .npm_resolver
      .as_managed()
      .expect("do not call this unless managed");
    let package_folder = managed_resolver
      .resolve_pkg_folder_from_deno_module_req(req_ref.req())?;
    Ok(
      node_and_npm_resolver
        .node_resolver
        .resolve_package_subpath_from_deno_module(
          &package_folder,
          req_ref.sub_path(),
          maybe_referrer,
          resolution_mode,
          resolution_kind,
        )
        .map_err(|source| CouldNotResolveNpmReqRefError {
          reference: req_ref.clone(),
          source,
        })?
        .into_url()?,
    )
  }

  pub fn resolve(
    &self,
    raw_specifier: &str,
    referrer: &Url,
    referrer_range_start: deno_graph::Position,
    resolution_mode: node_resolver::ResolutionMode,
    resolution_kind: node_resolver::NodeResolutionKind,
  ) -> Result<Url, DenoResolveError> {
    let resolution = self.resolver.resolve(
      raw_specifier,
      referrer,
      resolution_mode,
      resolution_kind,
    )?;

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
          if let Some(on_warning) = &self.on_warning
            && self.warned_pkgs.insert(reference.req().clone())
          {
            on_warning(MappedResolutionDiagnosticWithPosition {
              diagnostic,
              referrer,
              start: referrer_range_start,
            });
          }
        }
      }
    }

    Ok(resolution.url)
  }

  pub fn as_graph_resolver<'a>(
    &'a self,
    cjs_tracker: &'a CjsTracker<TInNpmPackageChecker, TSys>,
    jsx_import_source_config_resolver: &'a JsxImportSourceConfigResolver,
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
      jsx_import_source_config_resolver,
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
  jsx_import_source_config_resolver: &'a JsxImportSourceConfigResolver,
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
      .jsx_import_source_config_resolver
      .for_specifier(referrer)
      .and_then(|c| c.import_source.as_ref().map(|s| s.specifier.clone()))
  }

  fn default_jsx_import_source_types(&self, referrer: &Url) -> Option<String> {
    self
      .jsx_import_source_config_resolver
      .for_specifier(referrer)
      .and_then(|c| c.import_source_types.as_ref().map(|s| s.specifier.clone()))
  }

  fn jsx_import_source_module(&self, referrer: &Url) -> &str {
    self
      .jsx_import_source_config_resolver
      .for_specifier(referrer)
      .map(|c| c.module.as_str())
      .unwrap_or(deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    raw_specifier: &str,
    referrer_range: &deno_graph::Range,
    resolution_kind: deno_graph::source::ResolutionKind,
  ) -> Result<Url, ResolveError> {
    self
      .resolver
      .resolve(
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
      .map_err(|err| err.into_deno_graph_error())
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnhanceGraphErrorMode {
  ShowRange,
  HideRange,
}

pub fn enhance_graph_error(
  sys: &(impl sys_traits::FsMetadata + Clone),
  error: &ModuleGraphError,
  mode: EnhanceGraphErrorMode,
) -> String {
  let mut message = match &error {
    ModuleGraphError::ResolutionError(resolution_error) => {
      enhanced_resolution_error_message(resolution_error)
    }
    ModuleGraphError::TypesResolutionError(resolution_error) => {
      format!(
        "Failed resolving types. {}",
        enhanced_resolution_error_message(resolution_error)
      )
    }
    ModuleGraphError::ModuleError(error) => {
      enhanced_integrity_error_message(error)
        .or_else(|| enhanced_sloppy_imports_error_message(sys, error))
        .or_else(|| enhanced_unsupported_import_attribute(error))
        .unwrap_or_else(|| format_deno_graph_error(error))
    }
  };

  if let Some(range) = error.maybe_range()
    && mode == EnhanceGraphErrorMode::ShowRange
    && !range.specifier.as_str().contains("/$deno$eval")
  {
    message.push_str("\n    at ");
    message.push_str(&format_range_with_colors(range));
  }
  message
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format_deno_graph_error(error);

  let maybe_hint = if let Some(specifier) =
    get_resolution_error_bare_node_specifier(error)
  {
    Some(format!(
      "If you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."
    ))
  } else {
    get_import_prefix_missing_error(error).map(|specifier| {
      if specifier.starts_with("@std/") {
        format!(
          "If you want to use the JSR package, try running `deno add jsr:{}`",
          specifier
        )
      } else if specifier.starts_with('@') {
        format!(
          "If you want to use a JSR or npm package, try running `deno add jsr:{0}` or `deno add npm:{0}`",
          specifier
        )
      } else {
        format!(
          "If you want to use the npm package, try running `deno add npm:{0}`",
          specifier
        )
      }
    })
  };

  if let Some(hint) = maybe_hint {
    message.push_str(&format!(
      "\n  {} {}",
      deno_terminal::colors::cyan("hint:"),
      hint
    ));
  }

  message
}

static RUN_WITH_SLOPPY_IMPORTS_MSG: &str = "or run with --sloppy-imports";

fn enhanced_sloppy_imports_error_message(
  sys: &(impl sys_traits::FsMetadata + Clone),
  error: &ModuleError,
) -> Option<String> {
  match error.as_kind() {
    ModuleErrorKind::Load { specifier, err: ModuleLoadError::Loader(_), .. } // ex. "Is a directory" error
    | ModuleErrorKind::Missing { specifier, .. } => {
      let additional_message = maybe_additional_sloppy_imports_message(sys, specifier)?;
      Some(format!(
        "{} {}",
        error,
        additional_message,
      ))
    }
    _ => None,
  }
}

pub fn maybe_additional_sloppy_imports_message(
  sys: &(impl sys_traits::FsMetadata + Clone),
  specifier: &Url,
) -> Option<String> {
  let (resolved, sloppy_reason) = sloppy_imports_resolve(
    specifier,
    crate::workspace::ResolutionKind::Execution,
    sys.clone(),
  )?;
  Some(format!(
    "{} {}",
    sloppy_reason.suggestion_message_for_specifier(&resolved),
    RUN_WITH_SLOPPY_IMPORTS_MSG
  ))
}

pub fn enhanced_integrity_error_message(err: &ModuleError) -> Option<String> {
  match err.as_kind() {
    ModuleErrorKind::Load {
      specifier,
      err:
        ModuleLoadError::Jsr(JsrLoadError::ContentChecksumIntegrity(checksum_err)),
      ..
    } => Some(format!(
      concat!(
        "Integrity check failed in package. The package may have been tampered with.\n\n",
        "  Specifier: {}\n",
        "  Actual: {}\n",
        "  Expected: {}\n\n",
        "If you modified your global cache, run again with the --reload flag to restore ",
        "its state. If you want to modify dependencies locally run again with the ",
        "--vendor flag or specify `\"vendor\": true` in a deno.json then modify the contents ",
        "of the vendor/ folder."
      ),
      specifier, checksum_err.actual, checksum_err.expected,
    )),
    ModuleErrorKind::Load {
      err:
        ModuleLoadError::Jsr(
          JsrLoadError::PackageVersionManifestChecksumIntegrity(
            package_nv,
            checksum_err,
          ),
        ),
      ..
    } => Some(format!(
      concat!(
        "Integrity check failed for package. The source code is invalid, as it does not match the expected hash in the lock file.\n\n",
        "  Package: {}\n",
        "  Actual: {}\n",
        "  Expected: {}\n\n",
        "This could be caused by:\n",
        "  * the lock file may be corrupt\n",
        "  * the source itself may be corrupt\n\n",
        "Investigate the lockfile; delete it to regenerate the lockfile or --reload to reload the source code from the server."
      ),
      package_nv, checksum_err.actual, checksum_err.expected,
    )),
    ModuleErrorKind::Load {
      specifier,
      err: ModuleLoadError::HttpsChecksumIntegrity(checksum_err),
      ..
    } => Some(format!(
      concat!(
        "Integrity check failed for remote specifier. The source code is invalid, as it does not match the expected hash in the lock file.\n\n",
        "  Specifier: {}\n",
        "  Actual: {}\n",
        "  Expected: {}\n\n",
        "This could be caused by:\n",
        "  * the lock file may be corrupt\n",
        "  * the source itself may be corrupt\n\n",
        "Investigate the lockfile; delete it to regenerate the lockfile or --reload to reload the source code from the server."
      ),
      specifier, checksum_err.actual, checksum_err.expected,
    )),
    _ => None,
  }
}

fn enhanced_unsupported_import_attribute(err: &ModuleError) -> Option<String> {
  match err.as_kind() {
    ModuleErrorKind::UnsupportedImportAttributeType { kind, .. }
      if matches!(kind.as_str(), "bytes" | "text") =>
    {
      let mut text = format_deno_graph_error(err);
      text.push_str(&format!(
        "\n  {} run with --unstable-raw-imports",
        deno_terminal::colors::cyan("hint:")
      ));
      Some(text)
    }
    _ => None,
  }
}

pub fn get_resolution_error_bare_node_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  get_resolution_error_bare_specifier(error).filter(|specifier| {
    DenoIsBuiltInNodeModuleChecker.is_builtin_node_module(specifier)
  })
}

fn get_resolution_error_bare_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  if let ResolutionError::InvalidSpecifier {
    error: SpecifierError::ImportPrefixMissing { specifier, .. },
    ..
  } = error
  {
    Some(specifier.as_str())
  } else if let ResolutionError::ResolverError { error, .. } = error {
    if let ResolveError::ImportMap(error) = (*error).as_ref() {
      if let import_map::ImportMapErrorKind::UnmappedBareSpecifier(
        specifier,
        _,
      ) = error.as_kind()
      {
        Some(specifier.as_str())
      } else {
        None
      }
    } else {
      None
    }
  } else {
    None
  }
}

fn get_import_prefix_missing_error(error: &ResolutionError) -> Option<&str> {
  // not exact, but ok because this is just a hint
  let media_type =
    MediaType::from_specifier_and_headers(&error.range().specifier, None);
  if media_type == MediaType::Wasm {
    return None;
  }

  let mut maybe_specifier = None;
  if let ResolutionError::InvalidSpecifier {
    error: SpecifierError::ImportPrefixMissing { specifier, .. },
    range,
  } = error
  {
    if range.specifier.scheme() == "file" {
      maybe_specifier = Some(specifier);
    }
  } else if let ResolutionError::ResolverError { error, range, .. } = error
    && range.specifier.scheme() == "file"
  {
    match error.as_ref() {
      ResolveError::Specifier(specifier_error) => {
        if let SpecifierError::ImportPrefixMissing { specifier, .. } =
          specifier_error
        {
          maybe_specifier = Some(specifier);
        }
      }
      ResolveError::Other(other_error) => {
        if let Some(SpecifierError::ImportPrefixMissing { specifier, .. }) =
          other_error.get_ref().downcast_ref::<SpecifierError>()
        {
          maybe_specifier = Some(specifier);
        }
      }
      ResolveError::ImportMap(import_map_err) => {
        if let ImportMapErrorKind::UnmappedBareSpecifier(specifier, _referrer) =
          import_map_err.as_kind()
        {
          maybe_specifier = Some(specifier);
        }
      }
    }
  }

  if let Some(specifier) = maybe_specifier {
    // NOTE(bartlomieju): For now, return None if a specifier contains a dot or a space. This is because
    // suggesting to `deno add bad-module.ts` makes no sense and is worse than not providing
    // a suggestion at all. This should be improved further in the future
    if specifier.contains('.') || specifier.contains(' ') {
      return None;
    }
    // Do not return a hint for specifiers starting with `@`, but not containing a `/`
    if specifier.starts_with('@') && !specifier.contains('/') {
      return None;
    }
  }

  maybe_specifier.map(|s| s.as_str())
}

pub fn format_range_with_colors(referrer: &deno_graph::Range) -> String {
  use deno_terminal::colors;
  format!(
    "{}:{}:{}",
    colors::cyan(referrer.specifier.as_str()),
    colors::yellow(&(referrer.range.start.line + 1).to_string()),
    colors::yellow(&(referrer.range.start.character + 1).to_string())
  )
}

pub fn format_deno_graph_error(err: &dyn std::error::Error) -> String {
  use std::fmt::Write;

  let mut message = format!("{}", err);
  let mut maybe_source = err.source();

  if maybe_source.is_some() {
    let mut past_message = message.clone();
    let mut count = 0;
    let mut display_count = 0;
    while let Some(source) = maybe_source {
      let current_message = format!("{}", source);
      maybe_source = source.source();

      // sometimes an error might be repeated due to
      // being boxed multiple times in another AnyError
      if current_message != past_message {
        write!(message, "\n    {}: ", display_count,).unwrap();
        for (i, line) in current_message.lines().enumerate() {
          if i > 0 {
            write!(message, "\n       {}", line).unwrap();
          } else {
            write!(message, "{}", line).unwrap();
          }
        }
        display_count += 1;
      }

      if count > 8 {
        write!(message, "\n    {}: ...", count).unwrap();
        break;
      }

      past_message = current_message;
      count += 1;
    }
  }

  message
}

#[cfg(test)]
mod test {
  use deno_graph::PositionRange;
  use deno_graph::Range;
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;
  use deno_graph::source::ResolveError;

  use super::*;

  #[test]
  fn import_map_node_resolution_error() {
    let cases = vec![("fs", Some("fs")), ("other", None)];
    for (input, output) in cases {
      let import_map =
        import_map::ImportMap::new(Url::parse("file:///deno.json").unwrap());
      let specifier = Url::parse("file:///file.ts").unwrap();
      let err = import_map.resolve(input, &specifier).err().unwrap();
      let err = ResolutionError::ResolverError {
        #[allow(clippy::disallowed_types)]
        error: std::sync::Arc::new(ResolveError::ImportMap(err)),
        specifier: input.to_string(),
        range: Range {
          specifier,
          resolution_mode: None,
          range: PositionRange::zeroed(),
        },
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output);
    }
  }

  #[test]
  fn bare_specifier_node_resolution_error() {
    let cases = vec![("process", Some("process")), ("other", None)];
    for (input, output) in cases {
      let specifier = Url::parse("file:///file.ts").unwrap();
      let err = ResolutionError::InvalidSpecifier {
        range: Range {
          specifier,
          resolution_mode: None,
          range: PositionRange::zeroed(),
        },
        error: SpecifierError::ImportPrefixMissing {
          specifier: input.to_string(),
          referrer: None,
        },
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output,);
    }
  }
}
