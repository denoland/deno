// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_config::deno_json;
use deno_config::deno_json::CompilerOptionTypesDeserializeError;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::workspace::JsrPackageConfig;
use deno_config::workspace::JsxImportSourceConfig;
use deno_config::workspace::ToMaybeJsxImportSourceConfigError;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_graph::source::Loader;
use deno_graph::source::LoaderChecksum;
use deno_graph::source::ResolutionKind;
use deno_graph::source::ResolveError;
use deno_graph::CheckJsOption;
use deno_graph::FillFromLockfileOptions;
use deno_graph::GraphKind;
use deno_graph::JsrLoadError;
use deno_graph::ModuleError;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleLoadError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_graph::WorkspaceFastCheckOption;
use deno_path_util::url_to_file_path;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::workspace::sloppy_imports_resolve;
use deno_runtime::deno_node;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use deno_semver::SmallStackString;
use sys_traits::FsMetadata;

use crate::args::config_to_deno_graph_workspace_member;
use crate::args::deno_json::TsConfigResolver;
use crate::args::jsr_url;
use crate::args::CliLockfile;
use crate::args::CliOptions;
pub use crate::args::NpmCachingStrategy;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::cache;
use crate::cache::FetchCacher;
use crate::cache::GlobalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::ParsedSourceCache;
use crate::colors;
use crate::file_fetcher::CliFileFetcher;
use crate::npm::installer::NpmInstaller;
use crate::npm::installer::PackageCaching;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliNpmGraphResolver;
use crate::resolver::CliResolver;
use crate::sys::CliSys;
use crate::type_checker::CheckError;
use crate::type_checker::CheckOptions;
use crate::type_checker::TypeChecker;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path;

#[derive(Clone)]
pub struct GraphValidOptions<'a> {
  pub check_js: CheckJsOption<'a>,
  pub kind: GraphKind,
  /// Whether to exit the process for integrity check errors such as
  /// lockfile checksum mismatches and JSR integrity failures.
  /// Otherwise, surfaces integrity errors as errors.
  pub exit_integrity_errors: bool,
  pub allow_unknown_media_types: bool,
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
      allow_unknown_media_types: options.allow_unknown_media_types,
    },
  );
  if let Some(error) = errors.next() {
    Err(error)
  } else {
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

pub fn fill_graph_from_lockfile(
  graph: &mut ModuleGraph,
  lockfile: &deno_lockfile::Lockfile,
) {
  graph.fill_from_lockfile(FillFromLockfileOptions {
    redirects: lockfile
      .content
      .redirects
      .iter()
      .map(|(from, to)| (from.as_str(), to.as_str())),
    package_specifiers: lockfile
      .content
      .packages
      .specifiers
      .iter()
      .map(|(dep, id)| (dep, id.as_str())),
  });
}

#[derive(Clone)]
pub struct GraphWalkErrorsOptions<'a> {
  pub check_js: CheckJsOption<'a>,
  pub kind: GraphKind,
  pub allow_unknown_media_types: bool,
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
    error: &ModuleGraphError,
  ) -> bool {
    if (graph_kind == GraphKind::TypesOnly || allow_unknown_media_types)
      && matches!(
        error,
        ModuleGraphError::ModuleError(ModuleError::UnsupportedMediaType(..))
      )
    {
      return true;
    }

    // surface these as typescript diagnostics instead
    graph_kind.include_types()
      && has_module_graph_error_for_tsc_diagnostic(sys, error)
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
  match error {
    ModuleError::Missing(specifier, maybe_range) => {
      Some(ModuleNotFoundGraphErrorRef {
        specifier,
        maybe_range: maybe_range.as_ref(),
      })
    }
    ModuleError::LoadingErr(
      specifier,
      maybe_range,
      ModuleLoadError::Loader(_),
    ) => {
      if let Ok(path) = deno_path_util::url_to_file_path(specifier) {
        if sys.fs_is_dir_no_err(path) {
          return Some(ModuleNotFoundGraphErrorRef {
            specifier,
            maybe_range: maybe_range.as_ref(),
          });
        }
      }
      None
    }
    _ => None,
  }
}

pub struct ModuleNotFoundNodeResolutionErrorRef<'a> {
  pub specifier: &'a str,
  pub maybe_range: Option<&'a deno_graph::Range>,
}

pub fn resolution_error_for_tsc_diagnostic(
  error: &ResolutionError,
) -> Option<ModuleNotFoundNodeResolutionErrorRef> {
  match error {
    ResolutionError::ResolverError {
      error,
      specifier,
      range,
    } => match error.as_ref() {
      ResolveError::Other(error) => {
        // would be nice if there were an easier way of doing this
        let text = error.to_string();
        if text.contains("[ERR_MODULE_NOT_FOUND]") {
          Some(ModuleNotFoundNodeResolutionErrorRef {
            specifier,
            maybe_range: Some(range),
          })
        } else {
          None
        }
      }
      _ => None,
    },
    _ => None,
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnhanceGraphErrorMode {
  ShowRange,
  HideRange,
}

pub fn enhance_graph_error(
  sys: &CliSys,
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
        .unwrap_or_else(|| format_deno_graph_error(error))
    }
  };

  if let Some(range) = error.maybe_range() {
    if mode == EnhanceGraphErrorMode::ShowRange
      && !range.specifier.as_str().contains("/$deno$eval")
    {
      message.push_str("\n    at ");
      message.push_str(&format_range_with_colors(range));
    }
  }
  message
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

pub struct ModuleGraphCreator {
  options: Arc<CliOptions>,
  npm_installer: Option<Arc<NpmInstaller>>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  type_checker: Arc<TypeChecker>,
}

impl ModuleGraphCreator {
  pub fn new(
    options: Arc<CliOptions>,
    npm_installer: Option<Arc<NpmInstaller>>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      npm_installer,
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
    let mut cache = self.module_graph_builder.create_graph_loader();
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

  pub async fn create_and_validate_publish_graph(
    &self,
    package_configs: &[JsrPackageConfig],
    build_fast_check_graph: bool,
  ) -> Result<ModuleGraph, AnyError> {
    struct PublishLoader(FetchCacher);
    impl Loader for PublishLoader {
      fn load(
        &self,
        specifier: &deno_ast::ModuleSpecifier,
        options: deno_graph::source::LoadOptions,
      ) -> deno_graph::source::LoadFuture {
        if matches!(specifier.scheme(), "bun" | "virtual" | "cloudflare") {
          return Box::pin(std::future::ready(Ok(Some(
            deno_graph::source::LoadResponse::External {
              specifier: specifier.clone(),
            },
          ))));
        }
        self.0.load(specifier, options)
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
    for package_config in package_configs {
      roots.extend(package_config.config_file.resolve_export_value_urls()?);
    }

    let loader = self.module_graph_builder.create_graph_loader();
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
    self.graph_valid(&graph)?;
    if self.options.type_check_mode().is_true()
      && !graph_has_external_remote(&graph)
    {
      self.type_check_graph(graph.clone()).await?;
    }

    if build_fast_check_graph {
      let fast_check_workspace_members = package_configs
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
      .build_graph_with_npm_resolution(&mut graph, options)
      .await?;

    if let Some(npm_installer) = &self.npm_installer {
      if graph.has_node_specifier && self.options.type_check_mode().is_true() {
        npm_installer.inject_synthetic_types_node_package().await?;
      }
    }

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
      let graph = self.type_check_graph(graph).await?;
      Ok(graph)
    } else {
      Ok(Arc::new(graph))
    }
  }

  pub fn graph_valid(&self, graph: &ModuleGraph) -> Result<(), JsErrorBox> {
    self.module_graph_builder.graph_valid(graph)
  }

  async fn type_check_graph(
    &self,
    graph: ModuleGraph,
  ) -> Result<Arc<ModuleGraph>, CheckError> {
    self
      .type_checker
      .check(
        graph,
        CheckOptions {
          build_fast_check_graph: true,
          lib: self.options.ts_type_lib_window(),
          reload: self.options.reload_flag(),
          type_check_mode: self.options.type_check_mode(),
        },
      )
      .await
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
  #[error("Resolving npm specifier entrypoints this way is currently not supported with \"nodeModules\": \"manual\". In the meantime, try with --node-modules-dir=auto instead")]
  UnsupportedNpmSpecifierEntrypointResolutionWay,
}

pub struct ModuleGraphBuilder {
  caches: Arc<cache::Caches>,
  cjs_tracker: Arc<CliCjsTracker>,
  cli_options: Arc<CliOptions>,
  file_fetcher: Arc<CliFileFetcher>,
  global_http_cache: Arc<GlobalHttpCache>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  lockfile: Option<Arc<CliLockfile>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  module_info_cache: Arc<ModuleInfoCache>,
  npm_graph_resolver: Arc<CliNpmGraphResolver>,
  npm_installer: Option<Arc<NpmInstaller>>,
  npm_resolver: CliNpmResolver,
  parsed_source_cache: Arc<ParsedSourceCache>,
  resolver: Arc<CliResolver>,
  root_permissions_container: PermissionsContainer,
  sys: CliSys,
  tsconfig_resolver: Arc<TsConfigResolver>,
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
    lockfile: Option<Arc<CliLockfile>>,
    maybe_file_watcher_reporter: Option<FileWatcherReporter>,
    module_info_cache: Arc<ModuleInfoCache>,
    npm_graph_resolver: Arc<CliNpmGraphResolver>,
    npm_installer: Option<Arc<NpmInstaller>>,
    npm_resolver: CliNpmResolver,
    parsed_source_cache: Arc<ParsedSourceCache>,
    resolver: Arc<CliResolver>,
    root_permissions_container: PermissionsContainer,
    sys: CliSys,
    tsconfig_resolver: Arc<TsConfigResolver>,
  ) -> Self {
    Self {
      caches,
      cjs_tracker,
      cli_options,
      file_fetcher,
      global_http_cache,
      in_npm_pkg_checker,
      lockfile,
      maybe_file_watcher_reporter,
      module_info_cache,
      npm_graph_resolver,
      npm_installer,
      npm_resolver,
      parsed_source_cache,
      resolver,
      root_permissions_container,
      sys,
      tsconfig_resolver,
    }
  }

  pub async fn build_graph_with_npm_resolution(
    &self,
    graph: &mut ModuleGraph,
    options: CreateGraphOptions<'_>,
  ) -> Result<(), BuildGraphWithNpmResolutionError> {
    enum MutLoaderRef<'a> {
      Borrowed(&'a mut dyn Loader),
      Owned(cache::FetchCacher),
    }

    impl MutLoaderRef<'_> {
      pub fn as_mut_loader(&mut self) -> &mut dyn Loader {
        match self {
          Self::Borrowed(loader) => *loader,
          Self::Owned(loader) => loader,
        }
      }
    }

    struct LockfileLocker<'a>(&'a CliLockfile);

    impl deno_graph::source::Locker for LockfileLocker<'_> {
      fn get_remote_checksum(
        &self,
        specifier: &deno_ast::ModuleSpecifier,
      ) -> Option<LoaderChecksum> {
        self
          .0
          .lock()
          .remote()
          .get(specifier.as_str())
          .map(|s| LoaderChecksum::new(s.clone()))
      }

      fn has_remote_checksum(
        &self,
        specifier: &deno_ast::ModuleSpecifier,
      ) -> bool {
        self.0.lock().remote().contains_key(specifier.as_str())
      }

      fn set_remote_checksum(
        &mut self,
        specifier: &deno_ast::ModuleSpecifier,
        checksum: LoaderChecksum,
      ) {
        self
          .0
          .lock()
          .insert_remote(specifier.to_string(), checksum.into_string())
      }

      fn get_pkg_manifest_checksum(
        &self,
        package_nv: &PackageNv,
      ) -> Option<LoaderChecksum> {
        self
          .0
          .lock()
          .content
          .packages
          .jsr
          .get(package_nv)
          .map(|s| LoaderChecksum::new(s.integrity.clone()))
      }

      fn set_pkg_manifest_checksum(
        &mut self,
        package_nv: &PackageNv,
        checksum: LoaderChecksum,
      ) {
        // a value would only exist in here if two workers raced
        // to insert the same package manifest checksum
        self
          .0
          .lock()
          .insert_package(package_nv.clone(), checksum.into_string());
      }
    }

    let maybe_imports = if options.graph_kind.include_types() {
      // Resolve all the imports from every deno.json. We'll separate
      // them later based on the folder we're type checking.
      let mut imports = Vec::new();
      for deno_json in self.cli_options.workspace().deno_jsons() {
        let maybe_imports = deno_json.to_compiler_option_types()?;
        imports.extend(maybe_imports.into_iter().map(|(referrer, imports)| {
          deno_graph::ReferrerImports { referrer, imports }
        }));
      }
      imports
    } else {
      Vec::new()
    };
    let analyzer = self.module_info_cache.as_module_analyzer();
    let mut loader = match options.loader {
      Some(loader) => MutLoaderRef::Borrowed(loader),
      None => MutLoaderRef::Owned(self.create_graph_loader()),
    };
    let graph_resolver = self.create_graph_resolver()?;
    let maybe_file_watcher_reporter = self
      .maybe_file_watcher_reporter
      .as_ref()
      .map(|r| r.as_reporter());
    let mut locker = self
      .lockfile
      .as_ref()
      .map(|lockfile| LockfileLocker(lockfile));
    self
      .build_graph_with_npm_resolution_and_build_options(
        graph,
        options.roots,
        loader.as_mut_loader(),
        deno_graph::BuildOptions {
          imports: maybe_imports,
          skip_dynamic_deps: self.cli_options.unstable_lazy_dynamic_imports()
            && graph.graph_kind() == GraphKind::CodeOnly,
          is_dynamic: options.is_dynamic,
          passthrough_jsr_specifiers: false,
          executor: Default::default(),
          file_system: &self.sys,
          jsr_url_provider: &CliJsrUrlProvider,
          npm_resolver: Some(self.npm_graph_resolver.as_ref()),
          module_analyzer: &analyzer,
          reporter: maybe_file_watcher_reporter,
          resolver: Some(&graph_resolver),
          locker: locker.as_mut().map(|l| l as _),
        },
        options.npm_caching,
      )
      .await
  }

  async fn build_graph_with_npm_resolution_and_build_options<'a>(
    &self,
    graph: &mut ModuleGraph,
    roots: Vec<ModuleSpecifier>,
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
    {
      if let Some(npm_installer) = &self.npm_installer {
        let already_done = npm_installer
          .ensure_top_level_package_json_install()
          .await?;
        if !already_done && matches!(npm_caching, NpmCachingStrategy::Eager) {
          npm_installer.cache_packages(PackageCaching::All).await?;
        }
      }
    }

    // fill the graph with the information from the lockfile
    let is_first_execution = graph.roots.is_empty();
    if is_first_execution {
      // populate the information from the lockfile
      if let Some(lockfile) = &self.lockfile {
        let lockfile = lockfile.lock();
        fill_graph_from_lockfile(graph, &lockfile);
      }
    }

    let initial_redirects_len = graph.redirects.len();
    let initial_package_deps_len = graph.packages.package_deps_sum();
    let initial_package_mappings_len = graph.packages.mappings().len();

    if roots.iter().any(|r| r.scheme() == "npm") && self.npm_resolver.is_byonm()
    {
      return Err(BuildGraphWithNpmResolutionError::UnsupportedNpmSpecifierEntrypointResolutionWay);
    }

    graph.build(roots, loader, options).await;

    let has_redirects_changed = graph.redirects.len() != initial_redirects_len;
    let has_jsr_package_deps_changed =
      graph.packages.package_deps_sum() != initial_package_deps_len;
    let has_jsr_package_mappings_changed =
      graph.packages.mappings().len() != initial_package_mappings_len;

    if has_redirects_changed
      || has_jsr_package_deps_changed
      || has_jsr_package_mappings_changed
    {
      if let Some(lockfile) = &self.lockfile {
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
    let graph_resolver = self.create_graph_resolver()?;

    graph.build_fast_check_type_graph(
      deno_graph::BuildFastCheckTypeGraphOptions {
        es_parser: Some(&parser),
        fast_check_cache: fast_check_cache.as_ref().map(|c| c as _),
        fast_check_dts: false,
        jsr_url_provider: &CliJsrUrlProvider,
        resolver: Some(&graph_resolver),
        npm_resolver: Some(self.npm_graph_resolver.as_ref()),
        workspace_fast_check: options.workspace_fast_check,
      },
    );
    Ok(())
  }

  /// Creates the default loader used for creating a graph.
  pub fn create_graph_loader(&self) -> cache::FetchCacher {
    self.create_fetch_cacher(self.root_permissions_container.clone())
  }

  pub fn create_fetch_cacher(
    &self,
    permissions: PermissionsContainer,
  ) -> cache::FetchCacher {
    cache::FetchCacher::new(
      self.file_fetcher.clone(),
      self.global_http_cache.clone(),
      self.in_npm_pkg_checker.clone(),
      self.module_info_cache.clone(),
      self.sys.clone(),
      cache::FetchCacherOptions {
        file_header_overrides: self.cli_options.resolve_file_header_overrides(),
        permissions,
        is_deno_publish: matches!(
          self.cli_options.sub_command(),
          crate::args::DenoSubcommand::Publish { .. }
        ),
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
    )
  }

  pub fn graph_roots_valid(
    &self,
    graph: &ModuleGraph,
    roots: &[ModuleSpecifier],
    allow_unknown_media_types: bool,
  ) -> Result<(), JsErrorBox> {
    graph_valid(
      graph,
      &self.sys,
      roots,
      GraphValidOptions {
        kind: if self.cli_options.type_check_mode().is_true() {
          GraphKind::All
        } else {
          GraphKind::CodeOnly
        },
        check_js: CheckJsOption::Custom(self.tsconfig_resolver.as_ref()),
        exit_integrity_errors: true,
        allow_unknown_media_types,
      },
    )
  }

  fn create_graph_resolver(
    &self,
  ) -> Result<CliGraphResolver, ToMaybeJsxImportSourceConfigError> {
    let jsx_import_source_config_unscoped = self
      .cli_options
      .start_dir
      .to_maybe_jsx_import_source_config()?;
    let mut jsx_import_source_config_by_scope = BTreeMap::default();
    for (dir_url, _) in self.cli_options.workspace().config_folders() {
      let dir = self.cli_options.workspace().resolve_member_dir(dir_url);
      let jsx_import_source_config_unscoped =
        dir.to_maybe_jsx_import_source_config()?;
      jsx_import_source_config_by_scope
        .insert(dir_url.clone(), jsx_import_source_config_unscoped);
    }
    Ok(CliGraphResolver {
      cjs_tracker: &self.cjs_tracker,
      resolver: &self.resolver,
      jsx_import_source_config_unscoped,
      jsx_import_source_config_by_scope,
    })
  }
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format_deno_graph_error(error);

  let maybe_hint = if let Some(specifier) =
    get_resolution_error_bare_node_specifier(error)
  {
    if !*DENO_DISABLE_PEDANTIC_NODE_WARNINGS {
      Some(format!("If you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."))
    } else {
      None
    }
  } else {
    get_import_prefix_missing_error(error).map(|specifier| {
      format!(
        "If you want to use a JSR or npm package, try running `deno add jsr:{}` or `deno add npm:{}`",
        specifier, specifier
      )
    })
  };

  if let Some(hint) = maybe_hint {
    message.push_str(&format!("\n  {} {}", colors::cyan("hint:"), hint));
  }

  message
}

static RUN_WITH_SLOPPY_IMPORTS_MSG: &str =
  "or run with --unstable-sloppy-imports";

fn enhanced_sloppy_imports_error_message(
  sys: &CliSys,
  error: &ModuleError,
) -> Option<String> {
  match error {
    ModuleError::LoadingErr(specifier, _, ModuleLoadError::Loader(_)) // ex. "Is a directory" error
    | ModuleError::Missing(specifier, _) => {
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
  sys: &CliSys,
  specifier: &ModuleSpecifier,
) -> Option<String> {
  let (resolved, sloppy_reason) = sloppy_imports_resolve(
    specifier,
    deno_resolver::workspace::ResolutionKind::Execution,
    sys.clone(),
  )?;
  Some(format!(
    "{} {}",
    sloppy_reason.suggestion_message_for_specifier(&resolved),
    RUN_WITH_SLOPPY_IMPORTS_MSG
  ))
}

fn enhanced_integrity_error_message(err: &ModuleError) -> Option<String> {
  match err {
    ModuleError::LoadingErr(
      specifier,
      _,
      ModuleLoadError::Jsr(JsrLoadError::ContentChecksumIntegrity(
        checksum_err,
      )),
    ) => {
      Some(format!(
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
        specifier,
        checksum_err.actual,
        checksum_err.expected,
      ))
    }
    ModuleError::LoadingErr(
      _specifier,
      _,
      ModuleLoadError::Jsr(
        JsrLoadError::PackageVersionManifestChecksumIntegrity(
          package_nv,
          checksum_err,
        ),
      ),
    ) => {
      Some(format!(
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
        package_nv,
        checksum_err.actual,
        checksum_err.expected,
      ))
    }
    ModuleError::LoadingErr(
      specifier,
      _,
      ModuleLoadError::HttpsChecksumIntegrity(checksum_err),
    ) => {
      Some(format!(
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
        specifier,
        checksum_err.actual,
        checksum_err.expected,
      ))
    }
    _ => None,
  }
}

pub fn get_resolution_error_bare_node_specifier(
  error: &ResolutionError,
) -> Option<&str> {
  get_resolution_error_bare_specifier(error)
    .filter(|specifier| deno_node::is_builtin_node_module(specifier))
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
  } else if let ResolutionError::ResolverError { error, range, .. } = error {
    if range.specifier.scheme() == "file" {
      match error.as_ref() {
        ResolveError::Specifier(specifier_error) => {
          if let SpecifierError::ImportPrefixMissing { specifier, .. } =
            specifier_error
          {
            maybe_specifier = Some(specifier);
          }
        }
        ResolveError::Other(other_error) => {
          if let Some(SpecifierError::ImportPrefixMissing {
            specifier, ..
          }) = other_error.as_any().downcast_ref::<SpecifierError>()
          {
            maybe_specifier = Some(specifier);
          }
        }
        ResolveError::ImportMap(_) => {}
      }
    }
  }

  // NOTE(bartlomieju): For now, return None if a specifier contains a dot or a space. This is because
  // suggesting to `deno add bad-module.ts` makes no sense and is worse than not providing
  // a suggestion at all. This should be improved further in the future
  if let Some(specifier) = maybe_specifier {
    if specifier.contains('.') || specifier.contains(' ') {
      return None;
    }
  }

  maybe_specifier.map(|s| s.as_str())
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
      if let Ok(path) = canonicalize_path(&path) {
        if canonicalized_changed_paths.contains(&path) {
          return true;
        }
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

  pub fn as_reporter(&self) -> &dyn deno_graph::source::Reporter {
    self
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

pub fn format_range_with_colors(referrer: &deno_graph::Range) -> String {
  format!(
    "{}:{}:{}",
    colors::cyan(referrer.specifier.as_str()),
    colors::yellow(&(referrer.range.start.line + 1).to_string()),
    colors::yellow(&(referrer.range.start.character + 1).to_string())
  )
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CliJsrUrlProvider;

impl deno_graph::source::JsrUrlProvider for CliJsrUrlProvider {
  fn url(&self) -> &'static ModuleSpecifier {
    jsr_url()
  }
}

// todo(dsherret): We should change ModuleError to use thiserror so that
// we don't need to do this.
fn format_deno_graph_error(err: &dyn Error) -> String {
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

#[derive(Debug)]
struct CliGraphResolver<'a> {
  cjs_tracker: &'a CliCjsTracker,
  resolver: &'a CliResolver,
  jsx_import_source_config_unscoped: Option<JsxImportSourceConfig>,
  jsx_import_source_config_by_scope:
    BTreeMap<Arc<ModuleSpecifier>, Option<JsxImportSourceConfig>>,
}

impl CliGraphResolver<'_> {
  fn resolve_jsx_import_source_config(
    &self,
    referrer: &ModuleSpecifier,
  ) -> Option<&JsxImportSourceConfig> {
    self
      .jsx_import_source_config_by_scope
      .iter()
      .rfind(|(s, _)| referrer.as_str().starts_with(s.as_str()))
      .map(|(_, c)| c.as_ref())
      .unwrap_or(self.jsx_import_source_config_unscoped.as_ref())
  }
}

impl deno_graph::source::Resolver for CliGraphResolver<'_> {
  fn default_jsx_import_source(
    &self,
    referrer: &ModuleSpecifier,
  ) -> Option<String> {
    self
      .resolve_jsx_import_source_config(referrer)
      .and_then(|c| c.import_source.as_ref().map(|s| s.specifier.clone()))
  }

  fn default_jsx_import_source_types(
    &self,
    referrer: &ModuleSpecifier,
  ) -> Option<String> {
    self
      .resolve_jsx_import_source_config(referrer)
      .and_then(|c| c.import_source_types.as_ref().map(|s| s.specifier.clone()))
  }

  fn jsx_import_source_module(&self, referrer: &ModuleSpecifier) -> &str {
    self
      .resolve_jsx_import_source_config(referrer)
      .map(|c| c.module.as_str())
      .unwrap_or(deno_graph::source::DEFAULT_JSX_IMPORT_SOURCE_MODULE)
  }

  fn resolve(
    &self,
    raw_specifier: &str,
    referrer_range: &deno_graph::Range,
    resolution_kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ResolveError> {
    self.resolver.resolve(
      raw_specifier,
      &referrer_range.specifier,
      referrer_range.range.start,
      referrer_range
        .resolution_mode
        .map(to_node_resolution_mode)
        .unwrap_or_else(|| {
          self
            .cjs_tracker
            .get_referrer_kind(&referrer_range.specifier)
        }),
      to_node_resolution_kind(resolution_kind),
    )
  }
}

pub fn to_node_resolution_kind(
  kind: ResolutionKind,
) -> node_resolver::NodeResolutionKind {
  match kind {
    ResolutionKind::Execution => node_resolver::NodeResolutionKind::Execution,
    ResolutionKind::Types => node_resolver::NodeResolutionKind::Types,
  }
}

pub fn to_node_resolution_mode(
  mode: deno_graph::source::ResolutionMode,
) -> node_resolver::ResolutionMode {
  match mode {
    deno_graph::source::ResolutionMode::Import => {
      node_resolver::ResolutionMode::Import
    }
    deno_graph::source::ResolutionMode::Require => {
      node_resolver::ResolutionMode::Require
    }
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_ast::ModuleSpecifier;
  use deno_graph::source::ResolveError;
  use deno_graph::PositionRange;
  use deno_graph::Range;
  use deno_graph::ResolutionError;
  use deno_graph::SpecifierError;

  use super::*;

  #[test]
  fn import_map_node_resolution_error() {
    let cases = vec![("fs", Some("fs")), ("other", None)];
    for (input, output) in cases {
      let import_map = import_map::ImportMap::new(
        ModuleSpecifier::parse("file:///deno.json").unwrap(),
      );
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
      let err = import_map.resolve(input, &specifier).err().unwrap();
      let err = ResolutionError::ResolverError {
        error: Arc::new(ResolveError::ImportMap(err)),
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
      let specifier = ModuleSpecifier::parse("file:///file.ts").unwrap();
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
