// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::deno_json;
use deno_config::deno_json::CompilerOptionTypesDeserializeError;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::JsrPackageConfig;
use deno_core::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_graph::CheckJsOption;
use deno_graph::GraphKind;
use deno_graph::JsrLoadError;
use deno_graph::ModuleError;
use deno_graph::ModuleErrorKind;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleLoadError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_graph::WorkspaceFastCheckOption;
use deno_graph::packages::JsrVersionResolver;
use deno_graph::source::Loader;
use deno_graph::source::ResolveError;
use deno_lib::util::result::downcast_ref_deno_resolve_error;
use deno_npm_installer::PackageCaching;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_path_util::url_to_file_path;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::deno_json::JsxImportSourceConfigResolver;
use deno_resolver::deno_json::ToMaybeJsxImportSourceConfigError;
use deno_resolver::file_fetcher::GraphLoaderReporterRc;
use deno_resolver::graph::EnhanceGraphErrorMode;
use deno_resolver::graph::enhance_graph_error;
use deno_resolver::graph::enhanced_integrity_error_message;
use deno_resolver::graph::format_deno_graph_error;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::SmallStackString;
use deno_semver::jsr::JsrDepPackageReq;
use import_map::ImportMapErrorKind;
use indexmap::IndexMap;
use node_resolver::errors::NodeJsErrorCode;
use sys_traits::FsMetadata;

use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::config_to_deno_graph_workspace_member;
use crate::args::jsr_url;
use crate::cache;
use crate::cache::GlobalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::colors;
use crate::file_fetcher::CliDenoGraphLoader;
use crate::file_fetcher::CliFileFetcher;
use crate::npm::CliNpmGraphResolver;
use crate::npm::CliNpmInstaller;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliResolver;
use crate::sys::CliSys;
use crate::type_checker::CheckError;
use crate::type_checker::CheckOptions;
use crate::type_checker::TypeChecker;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path;
use crate::util::progress_bar::ProgressBar;

#[derive(Clone)]
pub struct GraphValidOptions<'a> {
  pub check_js: CheckJsOption<'a>,
  pub kind: GraphKind,
  pub will_type_check: bool,
  /// Whether to exit the process for integrity check errors such as
  /// lockfile checksum mismatches and JSR integrity failures.
  /// Otherwise, surfaces integrity errors as errors.
  pub exit_integrity_errors: bool,
  pub allow_unknown_media_types: bool,
  pub allow_unknown_jsr_exports: bool,
}

/// Check if `roots` and their deps are available. Returns `Ok(())` if
/// so. Returns `Err(_)` if there is a known module graph or resolution
/// error statically reachable from `roots`.
///
/// It is preferable to use this over using deno_graph's API directly
/// because it will have enhanced error message information specifically
/// for the CLI.
pub fn graph_valid(
  graph: &ModuleGraph,
  sys: &CliSys,
  roots: &[ModuleSpecifier],
  options: GraphValidOptions,
) -> Result<(), JsErrorBox> {
  if options.exit_integrity_errors {
    graph_exit_integrity_errors(graph);
  }

  let mut errors = graph_walk_errors(
    graph,
    sys,
    roots,
    GraphWalkErrorsOptions {
      check_js: options.check_js,
      kind: options.kind,
      will_type_check: options.will_type_check,
      allow_unknown_media_types: options.allow_unknown_media_types,
      allow_unknown_jsr_exports: options.allow_unknown_jsr_exports,
    },
  );
  match errors.next() {
    Some(error) => Err(error),
    _ => {
      // finally surface the npm resolution result
      if let Err(err) = &graph.npm_dep_graph_result {
        return Err(JsErrorBox::new(
          err.get_class(),
          format_deno_graph_error(err),
        ));
      }
      Ok(())
    }
  }
}

#[derive(Clone)]
pub struct GraphWalkErrorsOptions<'a> {
  pub check_js: CheckJsOption<'a>,
  pub kind: GraphKind,
  pub will_type_check: bool,
  pub allow_unknown_media_types: bool,
  pub allow_unknown_jsr_exports: bool,
}

/// Walks the errors found in the module graph that should be surfaced to users
/// and enhances them with CLI information.
pub fn graph_walk_errors<'a>(
  graph: &'a ModuleGraph,
  sys: &'a CliSys,
  roots: &'a [ModuleSpecifier],
  options: GraphWalkErrorsOptions<'a>,
) -> impl Iterator<Item = JsErrorBox> + 'a {
  fn should_ignore_error(
    sys: &CliSys,
    graph_kind: GraphKind,
    allow_unknown_media_types: bool,
    will_type_check: bool,
    error: &ModuleGraphError,
  ) -> bool {
    if (graph_kind == GraphKind::TypesOnly || allow_unknown_media_types)
      && matches!(
        error.as_module_error_kind(),
        Some(ModuleErrorKind::UnsupportedMediaType { .. })
      )
    {
      return true;
    }

    // surface these as typescript diagnostics instead
    will_type_check && has_module_graph_error_for_tsc_diagnostic(sys, error)
  }

  graph
    .walk(
      roots.iter(),
      deno_graph::WalkOptions {
        check_js: options.check_js,
        kind: options.kind,
        follow_dynamic: false,
        prefer_fast_check_graph: false,
      },
    )
    .errors()
    .flat_map(move |error| {
      if should_ignore_error(
        sys,
        graph.graph_kind(),
        options.allow_unknown_media_types,
        options.will_type_check,
        &error,
      ) {
        log::debug!("Ignoring: {}", error);
        return None;
      }

      let is_root = match &error {
        ModuleGraphError::ResolutionError(_)
        | ModuleGraphError::TypesResolutionError(_) => false,
        ModuleGraphError::ModuleError(error) => {
          roots.contains(error.specifier())
        }
      };
      if is_root
        && options.allow_unknown_jsr_exports
        && matches!(
          error.as_module_error_kind(),
          Some(ModuleErrorKind::Load {
            err: ModuleLoadError::Jsr(JsrLoadError::UnknownExport { .. }),
            ..
          })
        )
      {
        return None;
      }
      let message = enhance_graph_error(
        sys,
        &error,
        if is_root {
          EnhanceGraphErrorMode::HideRange
        } else {
          EnhanceGraphErrorMode::ShowRange
        },
      );

      Some(JsErrorBox::new(error.get_class(), message))
    })
}

fn has_module_graph_error_for_tsc_diagnostic(
  sys: &CliSys,
  error: &ModuleGraphError,
) -> bool {
  match error {
    ModuleGraphError::ModuleError(error) => {
      module_error_for_tsc_diagnostic(sys, error).is_some()
    }
    ModuleGraphError::ResolutionError(error) => {
      resolution_error_for_tsc_diagnostic(error).is_some()
    }
    ModuleGraphError::TypesResolutionError(error) => {
      resolution_error_for_tsc_diagnostic(error).is_some()
    }
  }
}

pub struct ModuleNotFoundGraphErrorRef<'a> {
  pub specifier: &'a ModuleSpecifier,
  pub maybe_range: Option<&'a deno_graph::Range>,
}

pub fn module_error_for_tsc_diagnostic<'a>(
  sys: &CliSys,
  error: &'a ModuleError,
) -> Option<ModuleNotFoundGraphErrorRef<'a>> {
  match error.as_kind() {
    ModuleErrorKind::Missing {
      specifier,
      maybe_referrer,
    } => Some(ModuleNotFoundGraphErrorRef {
      specifier,
      maybe_range: maybe_referrer.as_ref(),
    }),
    ModuleErrorKind::Load {
      specifier,
      maybe_referrer,
      err: ModuleLoadError::Loader(_),
    } => {
      if let Ok(path) = deno_path_util::url_to_file_path(specifier)
        && sys.fs_is_dir_no_err(path)
      {
        return Some(ModuleNotFoundGraphErrorRef {
          specifier,
          maybe_range: maybe_referrer.as_ref(),
        });
      }
      None
    }
    _ => None,
  }
}

#[derive(Debug)]
pub struct ResolutionErrorRef<'a> {
  pub specifier: &'a str,
  pub range: &'a deno_graph::Range,
  pub is_module_not_found: bool,
}

pub fn resolution_error_for_tsc_diagnostic(
  error: &ResolutionError,
) -> Option<ResolutionErrorRef<'_>> {
  fn is_module_not_found_code(code: NodeJsErrorCode) -> bool {
    match code {
      NodeJsErrorCode::ERR_INVALID_MODULE_SPECIFIER
      | NodeJsErrorCode::ERR_INVALID_PACKAGE_CONFIG
      | NodeJsErrorCode::ERR_INVALID_PACKAGE_TARGET
      | NodeJsErrorCode::ERR_UNKNOWN_FILE_EXTENSION
      | NodeJsErrorCode::ERR_UNSUPPORTED_DIR_IMPORT
      | NodeJsErrorCode::ERR_UNSUPPORTED_ESM_URL_SCHEME
      | NodeJsErrorCode::ERR_INVALID_FILE_URL_PATH
      | NodeJsErrorCode::ERR_PACKAGE_IMPORT_NOT_DEFINED
      | NodeJsErrorCode::ERR_PACKAGE_PATH_NOT_EXPORTED => false,
      NodeJsErrorCode::ERR_MODULE_NOT_FOUND
      | NodeJsErrorCode::ERR_TYPES_NOT_FOUND
      | NodeJsErrorCode::ERR_UNKNOWN_BUILTIN_MODULE => true,
    }
  }

  match error {
    ResolutionError::InvalidDowngrade { .. }
    | ResolutionError::InvalidJsrHttpsTypesImport { .. }
    | ResolutionError::InvalidLocalImport { .. } => None,
    ResolutionError::InvalidSpecifier { error, range } => match error {
      SpecifierError::InvalidUrl(..) => None,
      SpecifierError::ImportPrefixMissing { specifier, .. } => {
        Some(ResolutionErrorRef {
          specifier,
          range,
          is_module_not_found: false,
        })
      }
    },
    ResolutionError::ResolverError {
      error,
      specifier,
      range,
    } => match error.as_ref() {
      ResolveError::Specifier(error) => match error {
        SpecifierError::InvalidUrl(..) => None,
        SpecifierError::ImportPrefixMissing { specifier, .. } => {
          Some(ResolutionErrorRef {
            specifier,
            range,
            is_module_not_found: false,
          })
        }
      },
      ResolveError::ImportMap(error) => match error.as_kind() {
        ImportMapErrorKind::JsonParse(_)
        | ImportMapErrorKind::ImportMapNotObject
        | ImportMapErrorKind::ImportsFieldNotObject
        | ImportMapErrorKind::ScopesFieldNotObject
        | ImportMapErrorKind::ScopePrefixNotObject(_)
        | ImportMapErrorKind::BlockedByNullEntry(_)
        | ImportMapErrorKind::SpecifierResolutionFailure { .. }
        | ImportMapErrorKind::SpecifierBacktracksAbovePrefix { .. } => None,
        ImportMapErrorKind::UnmappedBareSpecifier(specifier, _) => {
          Some(ResolutionErrorRef {
            specifier,
            range,
            is_module_not_found: false,
          })
        }
      },
      ResolveError::Other(error) => {
        let is_module_not_found_error = downcast_ref_deno_resolve_error(error)
          .and_then(|err| err.maybe_node_code())
          .map(is_module_not_found_code)
          .unwrap_or(false);
        is_module_not_found_error.then(|| ResolutionErrorRef {
          specifier,
          range,
          is_module_not_found: true,
        })
      }
    },
  }
}

pub fn graph_exit_integrity_errors(graph: &ModuleGraph) {
  for error in graph.module_errors() {
    exit_for_integrity_error(error);
  }
}

fn exit_for_integrity_error(err: &ModuleError) {
  if let Some(err_message) = enhanced_integrity_error_message(err) {
    log::error!("{} {}", colors::red("error:"), err_message);
    deno_runtime::exit(10);
  }
}

pub struct CreateGraphOptions<'a> {
  pub graph_kind: GraphKind,
  pub roots: Vec<ModuleSpecifier>,
  pub is_dynamic: bool,
  /// Specify `None` to use the default CLI loader.
  pub loader: Option<&'a mut dyn Loader>,
  pub npm_caching: NpmCachingStrategy,
}

pub struct CreatePublishGraphOptions<'a> {
  pub packages: &'a [JsrPackageConfig],
  pub build_fast_check_graph: bool,
  pub validate_graph: bool,
}

pub struct ModuleGraphCreator {
  options: Arc<CliOptions>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  type_checker: Arc<TypeChecker>,
}

impl ModuleGraphCreator {
  pub fn new(
    options: Arc<CliOptions>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      module_graph_builder,
      type_checker,
    }
  }

  pub async fn create_graph(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
    npm_caching: NpmCachingStrategy,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = self
      .module_graph_builder
      .create_graph_loader_with_root_permissions();
    self
      .create_graph_with_loader(graph_kind, roots, &mut cache, npm_caching)
      .await
  }

  pub async fn create_graph_with_loader(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
    loader: &mut dyn Loader,
    npm_caching: NpmCachingStrategy,
  ) -> Result<ModuleGraph, AnyError> {
    self
      .create_graph_with_options(CreateGraphOptions {
        is_dynamic: false,
        graph_kind,
        roots,
        loader: Some(loader),
        npm_caching,
      })
      .await
  }

  pub async fn create_publish_graph(
    &self,
    options: CreatePublishGraphOptions<'_>,
  ) -> Result<ModuleGraph, AnyError> {
    struct PublishLoader(CliDenoGraphLoader);

    impl Loader for PublishLoader {
      fn load(
        &self,
        specifier: &deno_ast::ModuleSpecifier,
        options: deno_graph::source::LoadOptions,
      ) -> deno_graph::source::LoadFuture {
        if matches!(specifier.scheme(), "bun" | "virtual" | "cloudflare") {
          Box::pin(std::future::ready(Ok(Some(
            deno_graph::source::LoadResponse::External {
              specifier: specifier.clone(),
            },
          ))))
        } else if matches!(specifier.scheme(), "http" | "https")
          && !specifier.as_str().starts_with(jsr_url().as_str())
        {
          // mark non-JSR remote modules as external so we don't need --allow-import
          // permissions as these will error out later when publishing
          Box::pin(std::future::ready(Ok(Some(
            deno_graph::source::LoadResponse::External {
              specifier: specifier.clone(),
            },
          ))))
        } else {
          self.0.load(specifier, options)
        }
      }
    }

    fn graph_has_external_remote(graph: &ModuleGraph) -> bool {
      // Earlier on, we marked external non-JSR modules as external.
      // If the graph contains any of those, it would cause type checking
      // to crash, so since publishing is going to fail anyway, skip type
      // checking.
      graph.modules().any(|module| match module {
        deno_graph::Module::External(external_module) => {
          matches!(external_module.specifier.scheme(), "http" | "https")
        }
        _ => false,
      })
    }

    let mut roots = Vec::new();
    for package_config in options.packages {
      roots.extend(package_config.config_file.resolve_export_value_urls()?);
    }

    let loader = self
      .module_graph_builder
      .create_graph_loader_with_root_permissions();
    let mut publish_loader = PublishLoader(loader);
    let mut graph = self
      .create_graph_with_options(CreateGraphOptions {
        is_dynamic: false,
        graph_kind: deno_graph::GraphKind::All,
        roots,
        loader: Some(&mut publish_loader),
        npm_caching: self.options.default_npm_caching_strategy(),
      })
      .await?;
    if options.validate_graph {
      self.graph_valid(&graph)?;
    }
    if self.options.type_check_mode().is_true()
      && !graph_has_external_remote(&graph)
    {
      self.type_check_graph(graph.clone())?;
    }

    if options.build_fast_check_graph {
      let fast_check_workspace_members = options
        .packages
        .iter()
        .map(|p| config_to_deno_graph_workspace_member(&p.config_file))
        .collect::<Result<Vec<_>, _>>()?;
      self.module_graph_builder.build_fast_check_graph(
        &mut graph,
        BuildFastCheckGraphOptions {
          workspace_fast_check: WorkspaceFastCheckOption::Enabled(
            &fast_check_workspace_members,
          ),
        },
      )?;
    }

    Ok(graph)
  }

  pub async fn create_graph_with_options(
    &self,
    options: CreateGraphOptions<'_>,
  ) -> Result<ModuleGraph, AnyError> {
    let mut graph = ModuleGraph::new(options.graph_kind);

    self
      .module_graph_builder
      .build_graph_with_npm_resolution(
        &mut graph,
        BuildGraphWithNpmOptions {
          request: BuildGraphRequest::Roots(options.roots),
          is_dynamic: options.is_dynamic,
          loader: options.loader,
          npm_caching: options.npm_caching,
        },
      )
      .await?;

    Ok(graph)
  }

  pub async fn create_graph_and_maybe_check(
    &self,
    roots: Vec<ModuleSpecifier>,
  ) -> Result<Arc<deno_graph::ModuleGraph>, AnyError> {
    let graph_kind = self.options.type_check_mode().as_graph_kind();

    let graph = self
      .create_graph_with_options(CreateGraphOptions {
        is_dynamic: false,
        graph_kind,
        roots,
        loader: None,
        npm_caching: self.options.default_npm_caching_strategy(),
      })
      .await?;

    self.graph_valid(&graph)?;

    if self.options.type_check_mode().is_true() {
      // provide the graph to the type checker, then get it back after it's done
      let graph = self.type_check_graph(graph)?;
      Ok(graph)
    } else {
      Ok(Arc::new(graph))
    }
  }

  pub fn graph_valid(&self, graph: &ModuleGraph) -> Result<(), JsErrorBox> {
    self.module_graph_builder.graph_valid(graph)
  }

  #[allow(clippy::result_large_err)]
  fn type_check_graph(
    &self,
    graph: ModuleGraph,
  ) -> Result<Arc<ModuleGraph>, CheckError> {
    self.type_checker.check(
      graph,
      CheckOptions {
        build_fast_check_graph: true,
        lib: self.options.ts_type_lib_window(),
        reload: self.options.reload_flag(),
        type_check_mode: self.options.type_check_mode(),
      },
    )
  }
}

pub struct BuildFastCheckGraphOptions<'a> {
  /// Whether to do fast check on workspace members. This
  /// is mostly only useful when publishing.
  pub workspace_fast_check: deno_graph::WorkspaceFastCheckOption<'a>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BuildGraphWithNpmResolutionError {
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionTypesDeserialize(#[from] CompilerOptionTypesDeserializeError),
  #[class(inherit)]
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),
  #[class(inherit)]
  #[error(transparent)]
  ToMaybeJsxImportSourceConfig(#[from] ToMaybeJsxImportSourceConfigError),
  #[class(inherit)]
  #[error(transparent)]
  NodeModulesDirParse(#[from] deno_json::NodeModulesDirParseError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
  #[class(generic)]
  #[error(
    "Resolving npm specifier entrypoints this way is currently not supported with \"nodeModules\": \"manual\". In the meantime, try with --node-modules-dir=auto instead"
  )]
  UnsupportedNpmSpecifierEntrypointResolutionWay,
}

pub enum BuildGraphRequest {
  Roots(Vec<ModuleSpecifier>),
  Reload(Vec<ModuleSpecifier>),
}

pub struct BuildGraphWithNpmOptions<'a> {
  pub request: BuildGraphRequest,
  pub is_dynamic: bool,
  /// Specify `None` to use the default CLI loader.
  pub loader: Option<&'a mut dyn Loader>,
  pub npm_caching: NpmCachingStrategy,
}

pub struct ModuleGraphBuilder {
  caches: Arc<cache::Caches>,
  cjs_tracker: Arc<CliCjsTracker>,
  cli_options: Arc<CliOptions>,
  file_fetcher: Arc<CliFileFetcher>,
  global_http_cache: Arc<GlobalHttpCache>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  jsr_version_resolver: Arc<JsrVersionResolver>,
  lockfile: Option<Arc<CliLockfile>>,
  maybe_reporter: Option<Arc<dyn deno_graph::source::Reporter>>,
  module_info_cache: Arc<ModuleInfoCache>,
  npm_graph_resolver: Arc<CliNpmGraphResolver>,
  npm_installer: Option<Arc<CliNpmInstaller>>,
  npm_resolver: CliNpmResolver,
  parsed_source_cache: Arc<ParsedSourceCache>,
  progress_bar: ProgressBar,
  resolver: Arc<CliResolver>,
  root_permissions_container: PermissionsContainer,
  sys: CliSys,
  compiler_options_resolver: Arc<CompilerOptionsResolver>,
  load_reporter: Option<GraphLoaderReporterRc>,
}

impl ModuleGraphBuilder {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    caches: Arc<cache::Caches>,
    cjs_tracker: Arc<CliCjsTracker>,
    cli_options: Arc<CliOptions>,
    file_fetcher: Arc<CliFileFetcher>,
    global_http_cache: Arc<GlobalHttpCache>,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    jsr_version_resolver: Arc<JsrVersionResolver>,
    lockfile: Option<Arc<CliLockfile>>,
    maybe_reporter: Option<Arc<dyn deno_graph::source::Reporter>>,
    module_info_cache: Arc<ModuleInfoCache>,
    npm_graph_resolver: Arc<CliNpmGraphResolver>,
    npm_installer: Option<Arc<CliNpmInstaller>>,
    npm_resolver: CliNpmResolver,
    parsed_source_cache: Arc<ParsedSourceCache>,
    progress_bar: ProgressBar,
    resolver: Arc<CliResolver>,
    root_permissions_container: PermissionsContainer,
    sys: CliSys,
    compiler_options_resolver: Arc<CompilerOptionsResolver>,
    load_reporter: Option<GraphLoaderReporterRc>,
  ) -> Self {
    Self {
      caches,
      cjs_tracker,
      cli_options,
      file_fetcher,
      global_http_cache,
      in_npm_pkg_checker,
      jsr_version_resolver,
      lockfile,
      maybe_reporter,
      module_info_cache,
      npm_graph_resolver,
      npm_installer,
      npm_resolver,
      parsed_source_cache,
      progress_bar,
      resolver,
      root_permissions_container,
      sys,
      compiler_options_resolver,
      load_reporter,
    }
  }

  pub async fn build_graph_with_npm_resolution(
    &self,
    graph: &mut ModuleGraph,
    options: BuildGraphWithNpmOptions<'_>,
  ) -> Result<(), BuildGraphWithNpmResolutionError> {
    enum MutLoaderRef<'a> {
      Borrowed(&'a mut dyn Loader),
      Owned(CliDenoGraphLoader),
    }

    impl MutLoaderRef<'_> {
      pub fn as_mut_loader(&mut self) -> &mut dyn Loader {
        match self {
          Self::Borrowed(loader) => *loader,
          Self::Owned(loader) => loader,
        }
      }
    }

    let _clear_guard = self.progress_bar.deferred_keep_initialize_alive();
    let analyzer = self.module_info_cache.as_module_analyzer();
    let mut loader = match options.loader {
      Some(loader) => MutLoaderRef::Borrowed(loader),
      None => {
        MutLoaderRef::Owned(self.create_graph_loader_with_root_permissions())
      }
    };
    let jsx_import_source_config_resolver =
      JsxImportSourceConfigResolver::from_compiler_options_resolver(
        &self.compiler_options_resolver,
      )?;
    let graph_resolver = self.resolver.as_graph_resolver(
      self.cjs_tracker.as_ref(),
      &jsx_import_source_config_resolver,
    );
    let maybe_reporter = self.maybe_reporter.as_deref();
    let mut locker = self.lockfile.as_ref().map(|l| l.as_deno_graph_locker());
    self
      .build_graph_with_npm_resolution_and_build_options(
        graph,
        options.request,
        loader.as_mut_loader(),
        deno_graph::BuildOptions {
          skip_dynamic_deps: self.cli_options.unstable_lazy_dynamic_imports()
            && graph.graph_kind() == GraphKind::CodeOnly,
          is_dynamic: options.is_dynamic,
          passthrough_jsr_specifiers: false,
          executor: Default::default(),
          file_system: &self.sys,
          jsr_metadata_store: None,
          jsr_url_provider: &CliJsrUrlProvider,
          jsr_version_resolver: Cow::Borrowed(
            self.jsr_version_resolver.as_ref(),
          ),
          npm_resolver: Some(self.npm_graph_resolver.as_ref()),
          module_analyzer: &analyzer,
          module_info_cacher: self.module_info_cache.as_ref(),
          reporter: maybe_reporter,
          resolver: Some(&graph_resolver),
          locker: locker.as_mut().map(|l| l as _),
          unstable_bytes_imports: self.cli_options.unstable_raw_imports(),
          unstable_text_imports: self.cli_options.unstable_raw_imports(),
        },
        options.npm_caching,
      )
      .await?;

    Ok(())
  }

  async fn build_graph_with_npm_resolution_and_build_options<'a>(
    &self,
    graph: &mut ModuleGraph,
    request: BuildGraphRequest,
    loader: &'a mut dyn deno_graph::source::Loader,
    options: deno_graph::BuildOptions<'a>,
    npm_caching: NpmCachingStrategy,
  ) -> Result<(), BuildGraphWithNpmResolutionError> {
    // ensure an "npm install" is done if the user has explicitly
    // opted into using a node_modules directory
    if self
      .cli_options
      .specified_node_modules_dir()?
      .map(|m| m == NodeModulesDirMode::Auto)
      .unwrap_or(false)
      && let Some(npm_installer) = &self.npm_installer
    {
      let already_done = npm_installer
        .ensure_top_level_package_json_install()
        .await?;
      if !already_done && matches!(npm_caching, NpmCachingStrategy::Eager) {
        npm_installer.cache_packages(PackageCaching::All).await?;
      }
    }

    // fill the graph with the information from the lockfile
    let is_first_execution = graph.roots.is_empty();
    if is_first_execution {
      // populate the information from the lockfile
      if let Some(lockfile) = &self.lockfile {
        lockfile.fill_graph(graph)
      }
    }

    let initial_redirects_len = graph.redirects.len();
    let initial_package_deps_len = graph.packages.package_deps_sum();
    let initial_package_mappings_len = graph.packages.mappings().len();

    match request {
      BuildGraphRequest::Roots(roots) => {
        if roots.iter().any(|r| r.scheme() == "npm")
          && self.npm_resolver.is_byonm()
        {
          return Err(BuildGraphWithNpmResolutionError::UnsupportedNpmSpecifierEntrypointResolutionWay);
        }
        let imports = if graph.graph_kind().include_types() {
          // Resolve all the imports from every config file. We'll separate
          // them later based on the folder we're type checking.
          let mut imports_by_referrer = IndexMap::<_, Vec<_>>::with_capacity(
            self.compiler_options_resolver.size(),
          );
          for (_, compiler_options_data, maybe_files) in
            self.compiler_options_resolver.entries()
          {
            if let Some((referrer, files)) = maybe_files {
              imports_by_referrer
                .entry(referrer.as_ref())
                .or_default()
                .extend(files.iter().map(|f| f.relative_specifier.clone()));
            }
            for (referrer, types) in
              compiler_options_data.compiler_options_types().as_ref()
            {
              imports_by_referrer
                .entry(referrer)
                .or_default()
                .extend(types.iter().cloned());
            }
          }
          imports_by_referrer
            .into_iter()
            .map(|(referrer, imports)| deno_graph::ReferrerImports {
              referrer: referrer.clone(),
              imports,
            })
            .collect()
        } else {
          Vec::new()
        };
        graph.build(roots, imports, loader, options).await;
      }
      BuildGraphRequest::Reload(urls) => {
        graph.reload(urls, loader, options).await
      }
    }

    let has_redirects_changed = graph.redirects.len() != initial_redirects_len;
    let has_jsr_package_deps_changed =
      graph.packages.package_deps_sum() != initial_package_deps_len;
    let has_jsr_package_mappings_changed =
      graph.packages.mappings().len() != initial_package_mappings_len;

    if (has_redirects_changed
      || has_jsr_package_deps_changed
      || has_jsr_package_mappings_changed)
      && let Some(lockfile) = &self.lockfile
    {
      let mut lockfile = lockfile.lock();
      // https redirects
      if has_redirects_changed {
        let graph_redirects = graph.redirects.iter().filter(|(from, _)| {
          !matches!(from.scheme(), "npm" | "file" | "deno")
        });
        for (from, to) in graph_redirects {
          lockfile.insert_redirect(from.to_string(), to.to_string());
        }
      }
      // jsr package mappings
      if has_jsr_package_mappings_changed {
        for (from, to) in graph.packages.mappings() {
          lockfile.insert_package_specifier(
            JsrDepPackageReq::jsr(from.clone()),
            to.version.to_custom_string::<SmallStackString>(),
          );
        }
      }
      // jsr packages
      if has_jsr_package_deps_changed {
        for (nv, deps) in graph.packages.packages_with_deps() {
          lockfile.add_package_deps(nv, deps.cloned());
        }
      }
    }

    Ok(())
  }

  pub fn build_fast_check_graph(
    &self,
    graph: &mut ModuleGraph,
    options: BuildFastCheckGraphOptions,
  ) -> Result<(), ToMaybeJsxImportSourceConfigError> {
    if !graph.graph_kind().include_types() {
      return Ok(());
    }

    log::debug!("Building fast check graph");
    let fast_check_cache = if matches!(
      options.workspace_fast_check,
      deno_graph::WorkspaceFastCheckOption::Disabled
    ) {
      Some(cache::FastCheckCache::new(self.caches.fast_check_db()))
    } else {
      None
    };
    let parser = self.parsed_source_cache.as_capturing_parser();
    let jsx_import_source_config_resolver =
      JsxImportSourceConfigResolver::from_compiler_options_resolver(
        &self.compiler_options_resolver,
      )?;
    let graph_resolver = self.resolver.as_graph_resolver(
      self.cjs_tracker.as_ref(),
      &jsx_import_source_config_resolver,
    );

    graph.build_fast_check_type_graph(
      deno_graph::BuildFastCheckTypeGraphOptions {
        es_parser: Some(&parser),
        fast_check_cache: fast_check_cache.as_ref().map(|c| c as _),
        fast_check_dts: false,
        jsr_url_provider: &CliJsrUrlProvider,
        resolver: Some(&graph_resolver),
        workspace_fast_check: options.workspace_fast_check,
      },
    );
    Ok(())
  }

  /// Creates the default loader used for creating a graph.
  pub fn create_graph_loader_with_root_permissions(
    &self,
  ) -> CliDenoGraphLoader {
    self.create_graph_loader_with_permissions(
      self.root_permissions_container.clone(),
    )
  }

  pub fn create_graph_loader_with_permissions(
    &self,
    permissions: PermissionsContainer,
  ) -> CliDenoGraphLoader {
    CliDenoGraphLoader::new(
      self.file_fetcher.clone(),
      self.global_http_cache.clone(),
      self.in_npm_pkg_checker.clone(),
      self.sys.clone(),
      deno_resolver::file_fetcher::DenoGraphLoaderOptions {
        file_header_overrides: self.cli_options.resolve_file_header_overrides(),
        permissions: Some(permissions),
        reporter: self.load_reporter.clone(),
      },
    )
  }

  /// Check if `roots` and their deps are available. Returns `Ok(())` if
  /// so. Returns `Err(_)` if there is a known module graph or resolution
  /// error statically reachable from `roots` and not a dynamic import.
  pub fn graph_valid(&self, graph: &ModuleGraph) -> Result<(), JsErrorBox> {
    self.graph_roots_valid(
      graph,
      &graph.roots.iter().cloned().collect::<Vec<_>>(),
      false,
      false,
    )
  }

  pub fn graph_roots_valid(
    &self,
    graph: &ModuleGraph,
    roots: &[ModuleSpecifier],
    allow_unknown_media_types: bool,
    allow_unknown_jsr_exports: bool,
  ) -> Result<(), JsErrorBox> {
    let will_type_check = self.cli_options.type_check_mode().is_true();
    graph_valid(
      graph,
      &self.sys,
      roots,
      GraphValidOptions {
        kind: if will_type_check {
          GraphKind::All
        } else {
          GraphKind::CodeOnly
        },
        will_type_check,
        check_js: CheckJsOption::Custom(
          self.compiler_options_resolver.as_ref(),
        ),
        exit_integrity_errors: true,
        allow_unknown_media_types,
        allow_unknown_jsr_exports,
      },
    )
  }
}

/// Gets if any of the specified root's "file:" dependents are in the
/// provided changed set.
pub fn has_graph_root_local_dependent_changed(
  graph: &ModuleGraph,
  root: &ModuleSpecifier,
  canonicalized_changed_paths: &HashSet<PathBuf>,
) -> bool {
  let mut dependent_specifiers = graph.walk(
    std::iter::once(root),
    deno_graph::WalkOptions {
      follow_dynamic: true,
      kind: GraphKind::All,
      prefer_fast_check_graph: true,
      check_js: CheckJsOption::True,
    },
  );
  while let Some((s, _)) = dependent_specifiers.next() {
    if let Ok(path) = url_to_file_path(s) {
      if let Ok(path) = canonicalize_path(&path)
        && canonicalized_changed_paths.contains(&path)
      {
        return true;
      }
    } else {
      // skip walking this remote module's dependencies
      dependent_specifiers.skip_previous_dependencies();
    }
  }
  false
}

#[derive(Clone, Debug)]
pub struct FileWatcherReporter {
  watcher_communicator: Arc<WatcherCommunicator>,
  file_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl FileWatcherReporter {
  pub fn new(watcher_communicator: Arc<WatcherCommunicator>) -> Self {
    Self {
      watcher_communicator,
      file_paths: Default::default(),
    }
  }
}

impl deno_graph::source::Reporter for FileWatcherReporter {
  fn on_load(
    &self,
    specifier: &ModuleSpecifier,
    modules_done: usize,
    modules_total: usize,
  ) {
    let mut file_paths = self.file_paths.lock();
    if specifier.scheme() == "file" {
      // Don't trust that the path is a valid path at this point:
      // https://github.com/denoland/deno/issues/26209.
      if let Ok(file_path) = specifier.to_file_path() {
        file_paths.push(file_path);
      }
    }

    if modules_done == modules_total {
      self
        .watcher_communicator
        .watch_paths(file_paths.drain(..).collect())
        .unwrap();
    }
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CliJsrUrlProvider;

impl deno_graph::source::JsrUrlProvider for CliJsrUrlProvider {
  fn url(&self) -> &'static ModuleSpecifier {
    jsr_url()
  }
}
