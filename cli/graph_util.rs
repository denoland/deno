// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::config_to_deno_graph_workspace_member;
use crate::args::jsr_url;
use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::DENO_DISABLE_PEDANTIC_NODE_WARNINGS;
use crate::cache;
use crate::cache::GlobalHttpCache;
use crate::cache::ModuleInfoCache;
use crate::cache::ParsedSourceCache;
use crate::colors;
use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;
use crate::npm::CliNpmResolver;
use crate::resolver::CliGraphResolver;
use crate::resolver::SloppyImportsResolver;
use crate::tools::check;
use crate::tools::check::TypeChecker;
use crate::util::file_watcher::WatcherCommunicator;
use crate::util::fs::canonicalize_path;
use deno_config::workspace::JsrPackageConfig;
use deno_emit::LoaderChecksum;
use deno_graph::JsrLoadError;
use deno_graph::ModuleLoadError;
use deno_graph::WorkspaceFastCheckOption;
use deno_runtime::fs_util::specifier_to_file_path;

use deno_core::anyhow::bail;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use deno_graph::source::Loader;
use deno_graph::source::ResolutionMode;
use deno_graph::source::ResolveError;
use deno_graph::GraphKind;
use deno_graph::Module;
use deno_graph::ModuleError;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use import_map::ImportMapError;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct GraphValidOptions {
  pub check_js: bool,
  pub follow_type_only: bool,
  pub is_vendoring: bool,
  /// Whether to exit the process for lockfile errors.
  /// Otherwise, surfaces lockfile errors as errors.
  pub exit_lockfile_errors: bool,
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
  fs: &Arc<dyn FileSystem>,
  roots: &[ModuleSpecifier],
  options: GraphValidOptions,
) -> Result<(), AnyError> {
  if options.exit_lockfile_errors {
    graph_exit_lock_errors(graph);
  }

  let mut errors = graph
    .walk(
      roots.iter(),
      deno_graph::WalkOptions {
        check_js: options.check_js,
        follow_type_only: options.follow_type_only,
        follow_dynamic: options.is_vendoring,
        prefer_fast_check_graph: false,
      },
    )
    .errors()
    .flat_map(|error| {
      let is_root = match &error {
        ModuleGraphError::ResolutionError(_)
        | ModuleGraphError::TypesResolutionError(_) => false,
        ModuleGraphError::ModuleError(error) => {
          roots.contains(error.specifier())
        }
      };
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
          enhanced_lockfile_error_message(error)
            .or_else(|| enhanced_sloppy_imports_error_message(fs, error))
            .unwrap_or_else(|| format!("{}", error))
        }
      };

      if let Some(range) = error.maybe_range() {
        if !is_root && !range.specifier.as_str().contains("/$deno$eval") {
          message.push_str("\n    at ");
          message.push_str(&format_range_with_colors(range));
        }
      }

      if graph.graph_kind() == GraphKind::TypesOnly
        && matches!(
          error,
          ModuleGraphError::ModuleError(ModuleError::UnsupportedMediaType(..))
        )
      {
        log::debug!("Ignoring: {}", message);
        return None;
      }

      if options.is_vendoring {
        // warn about failing dynamic imports when vendoring, but don't fail completely
        if matches!(
          error,
          ModuleGraphError::ModuleError(ModuleError::MissingDynamic(_, _))
        ) {
          log::warn!("Ignoring: {}", message);
          return None;
        }

        // ignore invalid downgrades and invalid local imports when vendoring
        match &error {
          ModuleGraphError::ResolutionError(err)
          | ModuleGraphError::TypesResolutionError(err) => {
            if matches!(
              err,
              ResolutionError::InvalidDowngrade { .. }
                | ResolutionError::InvalidLocalImport { .. }
            ) {
              return None;
            }
          }
          ModuleGraphError::ModuleError(_) => {}
        }
      }

      Some(custom_error(get_error_class_name(&error.into()), message))
    });
  if let Some(error) = errors.next() {
    Err(error)
  } else {
    // finally surface the npm resolution result
    if let Err(err) = &graph.npm_dep_graph_result {
      return Err(custom_error(get_error_class_name(err), format!("{}", err)));
    }
    Ok(())
  }
}

pub fn graph_exit_lock_errors(graph: &ModuleGraph) {
  for error in graph.module_errors() {
    exit_for_lockfile_error(error);
  }
}

fn exit_for_lockfile_error(err: &ModuleError) {
  if let Some(err_message) = enhanced_lockfile_error_message(err) {
    log::error!("{} {}", colors::red("error:"), err_message);
    std::process::exit(10);
  }
}

pub struct CreateGraphOptions<'a> {
  pub graph_kind: GraphKind,
  pub roots: Vec<ModuleSpecifier>,
  pub is_dynamic: bool,
  /// Specify `None` to use the default CLI loader.
  pub loader: Option<&'a mut dyn Loader>,
}

pub struct ModuleGraphCreator {
  options: Arc<CliOptions>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  type_checker: Arc<TypeChecker>,
}

impl ModuleGraphCreator {
  pub fn new(
    options: Arc<CliOptions>,
    npm_resolver: Arc<dyn CliNpmResolver>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      npm_resolver,
      module_graph_builder,
      type_checker,
    }
  }

  pub async fn create_graph(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = self.module_graph_builder.create_graph_loader();
    self
      .create_graph_with_loader(graph_kind, roots, &mut cache)
      .await
  }

  pub async fn create_graph_with_loader(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
    loader: &mut dyn Loader,
  ) -> Result<ModuleGraph, AnyError> {
    self
      .create_graph_with_options(CreateGraphOptions {
        is_dynamic: false,
        graph_kind,
        roots,
        loader: Some(loader),
      })
      .await
  }

  pub async fn create_and_validate_publish_graph(
    &self,
    package_configs: &[JsrPackageConfig],
    build_fast_check_graph: bool,
  ) -> Result<ModuleGraph, AnyError> {
    let mut roots = Vec::new();
    for package_config in package_configs {
      roots.extend(package_config.config_file.resolve_export_value_urls()?);
    }
    let mut graph = self
      .create_graph_with_options(CreateGraphOptions {
        is_dynamic: false,
        graph_kind: deno_graph::GraphKind::All,
        roots,
        loader: None,
      })
      .await?;
    self.graph_valid(&graph)?;
    if self.options.type_check_mode().is_true() {
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

    if let Some(npm_resolver) = self.npm_resolver.as_managed() {
      if graph.has_node_specifier && self.options.type_check_mode().is_true() {
        npm_resolver.inject_synthetic_types_node_package().await?;
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

  pub fn graph_valid(&self, graph: &ModuleGraph) -> Result<(), AnyError> {
    self.module_graph_builder.graph_valid(graph)
  }

  async fn type_check_graph(
    &self,
    graph: ModuleGraph,
  ) -> Result<Arc<ModuleGraph>, AnyError> {
    self
      .type_checker
      .check(
        graph,
        check::CheckOptions {
          build_fast_check_graph: true,
          lib: self.options.ts_type_lib_window(),
          log_ignored_options: true,
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

pub struct ModuleGraphBuilder {
  options: Arc<CliOptions>,
  caches: Arc<cache::Caches>,
  fs: Arc<dyn FileSystem>,
  resolver: Arc<CliGraphResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  module_info_cache: Arc<ModuleInfoCache>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  lockfile: Option<Arc<CliLockfile>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  emit_cache: cache::EmitCache,
  file_fetcher: Arc<FileFetcher>,
  global_http_cache: Arc<GlobalHttpCache>,
}

impl ModuleGraphBuilder {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: Arc<CliOptions>,
    caches: Arc<cache::Caches>,
    fs: Arc<dyn FileSystem>,
    resolver: Arc<CliGraphResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
    module_info_cache: Arc<ModuleInfoCache>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    lockfile: Option<Arc<CliLockfile>>,
    maybe_file_watcher_reporter: Option<FileWatcherReporter>,
    emit_cache: cache::EmitCache,
    file_fetcher: Arc<FileFetcher>,
    global_http_cache: Arc<GlobalHttpCache>,
  ) -> Self {
    Self {
      options,
      caches,
      fs,
      resolver,
      npm_resolver,
      module_info_cache,
      parsed_source_cache,
      lockfile,
      maybe_file_watcher_reporter,
      emit_cache,
      file_fetcher,
      global_http_cache,
    }
  }

  pub async fn build_graph_with_npm_resolution<'a>(
    &self,
    graph: &mut ModuleGraph,
    options: CreateGraphOptions<'a>,
  ) -> Result<(), AnyError> {
    enum MutLoaderRef<'a> {
      Borrowed(&'a mut dyn Loader),
      Owned(cache::FetchCacher),
    }

    impl<'a> MutLoaderRef<'a> {
      pub fn as_mut_loader(&mut self) -> &mut dyn Loader {
        match self {
          Self::Borrowed(loader) => *loader,
          Self::Owned(loader) => loader,
        }
      }
    }

    struct LockfileLocker<'a>(&'a CliLockfile);

    impl<'a> deno_graph::source::Locker for LockfileLocker<'a> {
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
          .get(&package_nv.to_string())
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
          .insert_package(package_nv.to_string(), checksum.into_string());
      }
    }

    let maybe_imports = if options.graph_kind.include_types() {
      self.options.to_compiler_option_types()?
    } else {
      Vec::new()
    };
    let analyzer = self
      .module_info_cache
      .as_module_analyzer(&self.parsed_source_cache);
    let mut loader = match options.loader {
      Some(loader) => MutLoaderRef::Borrowed(loader),
      None => MutLoaderRef::Owned(self.create_graph_loader()),
    };
    let cli_resolver = &self.resolver;
    let graph_resolver = cli_resolver.as_graph_resolver();
    let graph_npm_resolver = cli_resolver.create_graph_npm_resolver();
    let maybe_file_watcher_reporter = self
      .maybe_file_watcher_reporter
      .as_ref()
      .map(|r| r.as_reporter());
    let workspace_members =
      self.options.resolve_deno_graph_workspace_members()?;
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
          is_dynamic: options.is_dynamic,
          passthrough_jsr_specifiers: false,
          workspace_members: &workspace_members,
          executor: Default::default(),
          file_system: &DenoGraphFsAdapter(self.fs.as_ref()),
          jsr_url_provider: &CliJsrUrlProvider,
          npm_resolver: Some(&graph_npm_resolver),
          module_analyzer: &analyzer,
          reporter: maybe_file_watcher_reporter,
          resolver: Some(graph_resolver),
          locker: locker.as_mut().map(|l| l as _),
        },
      )
      .await
  }

  async fn build_graph_with_npm_resolution_and_build_options<'a>(
    &self,
    graph: &mut ModuleGraph,
    roots: Vec<ModuleSpecifier>,
    loader: &'a mut dyn deno_graph::source::Loader,
    options: deno_graph::BuildOptions<'a>,
  ) -> Result<(), AnyError> {
    // ensure an "npm install" is done if the user has explicitly
    // opted into using a node_modules directory
    if self.options.node_modules_dir_enablement() == Some(true) {
      if let Some(npm_resolver) = self.npm_resolver.as_managed() {
        npm_resolver.ensure_top_level_package_json_install().await?;
      }
    }

    // fill the graph with the information from the lockfile
    let is_first_execution = graph.roots.is_empty();
    if is_first_execution {
      // populate the information from the lockfile
      if let Some(lockfile) = &self.lockfile {
        let lockfile = lockfile.lock();
        for (from, to) in &lockfile.content.redirects {
          if let Ok(from) = ModuleSpecifier::parse(from) {
            if let Ok(to) = ModuleSpecifier::parse(to) {
              if !matches!(from.scheme(), "file" | "npm" | "jsr") {
                graph.redirects.insert(from, to);
              }
            }
          }
        }
        for (key, value) in &lockfile.content.packages.specifiers {
          if let Some(key) = key
            .strip_prefix("jsr:")
            .and_then(|key| PackageReq::from_str(key).ok())
          {
            if let Some(value) = value
              .strip_prefix("jsr:")
              .and_then(|value| PackageNv::from_str(value).ok())
            {
              graph.packages.add_nv(key, value);
            }
          }
        }
      }
    }

    let initial_redirects_len = graph.redirects.len();
    let initial_package_deps_len = graph.packages.package_deps_sum();
    let initial_package_mappings_len = graph.packages.mappings().len();

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
              format!("jsr:{}", from),
              format!("jsr:{}", to),
            );
          }
        }
        // jsr packages
        if has_jsr_package_deps_changed {
          for (name, deps) in graph.packages.packages_with_deps() {
            lockfile
              .add_package_deps(&name.to_string(), deps.map(|s| s.to_string()));
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
  ) -> Result<(), AnyError> {
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
    let cli_resolver = &self.resolver;
    let graph_resolver = cli_resolver.as_graph_resolver();
    let graph_npm_resolver = cli_resolver.create_graph_npm_resolver();

    graph.build_fast_check_type_graph(
      deno_graph::BuildFastCheckTypeGraphOptions {
        jsr_url_provider: &CliJsrUrlProvider,
        fast_check_cache: fast_check_cache.as_ref().map(|c| c as _),
        fast_check_dts: false,
        module_parser: Some(&parser),
        resolver: Some(graph_resolver),
        npm_resolver: Some(&graph_npm_resolver),
        workspace_fast_check: options.workspace_fast_check,
      },
    );
    Ok(())
  }

  /// Creates the default loader used for creating a graph.
  pub fn create_graph_loader(&self) -> cache::FetchCacher {
    self.create_fetch_cacher(PermissionsContainer::allow_all())
  }

  pub fn create_fetch_cacher(
    &self,
    permissions: PermissionsContainer,
  ) -> cache::FetchCacher {
    cache::FetchCacher::new(
      self.emit_cache.clone(),
      self.file_fetcher.clone(),
      self.options.resolve_file_header_overrides(),
      self.global_http_cache.clone(),
      self.npm_resolver.clone(),
      self.module_info_cache.clone(),
      permissions,
    )
  }

  /// Check if `roots` and their deps are available. Returns `Ok(())` if
  /// so. Returns `Err(_)` if there is a known module graph or resolution
  /// error statically reachable from `roots` and not a dynamic import.
  pub fn graph_valid(&self, graph: &ModuleGraph) -> Result<(), AnyError> {
    self.graph_roots_valid(
      graph,
      &graph.roots.iter().cloned().collect::<Vec<_>>(),
    )
  }

  pub fn graph_roots_valid(
    &self,
    graph: &ModuleGraph,
    roots: &[ModuleSpecifier],
  ) -> Result<(), AnyError> {
    graph_valid(
      graph,
      &self.fs,
      roots,
      GraphValidOptions {
        is_vendoring: false,
        follow_type_only: self.options.type_check_mode().is_true(),
        check_js: self.options.check_js(),
        exit_lockfile_errors: true,
      },
    )
  }
}

pub fn error_for_any_npm_specifier(
  graph: &ModuleGraph,
) -> Result<(), AnyError> {
  for module in graph.modules() {
    match module {
      Module::Npm(module) => {
        bail!("npm specifiers have not yet been implemented for this subcommand (https://github.com/denoland/deno/issues/15960). Found: {}", module.specifier)
      }
      Module::Node(module) => {
        bail!("Node specifiers have not yet been implemented for this subcommand (https://github.com/denoland/deno/issues/15960). Found: node:{}", module.module_name)
      }
      Module::Js(_) | Module::Json(_) | Module::External(_) => {}
    }
  }
  Ok(())
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format!("{error}");

  if let Some(specifier) = get_resolution_error_bare_node_specifier(error) {
    if !*DENO_DISABLE_PEDANTIC_NODE_WARNINGS {
      message.push_str(&format!(
        "\nIf you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."
      ));
    }
  }

  message
}

fn enhanced_sloppy_imports_error_message(
  fs: &Arc<dyn FileSystem>,
  error: &ModuleError,
) -> Option<String> {
  match error {
    ModuleError::LoadingErr(specifier, _, ModuleLoadError::Loader(_)) // ex. "Is a directory" error
    | ModuleError::Missing(specifier, _) => {
      let additional_message = SloppyImportsResolver::new(fs.clone())
        .resolve(specifier, ResolutionMode::Execution)?
        .as_suggestion_message();
      Some(format!(
        "{} {} or run with --unstable-sloppy-imports",
        error,
        additional_message,
      ))
    }
    _ => None,
  }
}

fn enhanced_lockfile_error_message(err: &ModuleError) -> Option<String> {
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
          "Use the --lock-write flag to regenerate the lockfile or --reload to reload the source code from the server."
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
          "Use the --lock-write flag to regenerate the lockfile or --reload to reload the source code from the server."
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
    if let ResolveError::Other(error) = (*error).as_ref() {
      if let Some(ImportMapError::UnmappedBareSpecifier(specifier, _)) =
        error.downcast_ref::<ImportMapError>()
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
      follow_type_only: true,
      prefer_fast_check_graph: true,
      check_js: true,
    },
  );
  while let Some((s, _)) = dependent_specifiers.next() {
    if let Ok(path) = specifier_to_file_path(s) {
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
      file_paths.push(specifier.to_file_path().unwrap());
    }

    if modules_done == modules_total {
      self
        .watcher_communicator
        .watch_paths(file_paths.drain(..).collect())
        .unwrap();
    }
  }
}

pub struct DenoGraphFsAdapter<'a>(
  pub &'a dyn deno_runtime::deno_fs::FileSystem,
);

impl<'a> deno_graph::source::FileSystem for DenoGraphFsAdapter<'a> {
  fn read_dir(
    &self,
    dir_url: &deno_graph::ModuleSpecifier,
  ) -> Vec<deno_graph::source::DirEntry> {
    use deno_core::anyhow;
    use deno_graph::source::DirEntry;
    use deno_graph::source::DirEntryKind;

    let dir_path = match dir_url.to_file_path() {
      Ok(path) => path,
      // ignore, treat as non-analyzable
      Err(()) => return vec![],
    };
    let entries = match self.0.read_dir_sync(&dir_path) {
      Ok(dir) => dir,
      Err(err)
        if matches!(
          err.kind(),
          std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
        ) =>
      {
        return vec![];
      }
      Err(err) => {
        return vec![DirEntry {
          kind: DirEntryKind::Error(
            anyhow::Error::from(err)
              .context("Failed to read directory.".to_string()),
          ),
          url: dir_url.clone(),
        }];
      }
    };
    let mut dir_entries = Vec::with_capacity(entries.len());
    for entry in entries {
      let entry_path = dir_path.join(&entry.name);
      dir_entries.push(if entry.is_directory {
        DirEntry {
          kind: DirEntryKind::Dir,
          url: ModuleSpecifier::from_directory_path(&entry_path).unwrap(),
        }
      } else if entry.is_file {
        DirEntry {
          kind: DirEntryKind::File,
          url: ModuleSpecifier::from_file_path(&entry_path).unwrap(),
        }
      } else if entry.is_symlink {
        DirEntry {
          kind: DirEntryKind::Symlink,
          url: ModuleSpecifier::from_file_path(&entry_path).unwrap(),
        }
      } else {
        continue;
      });
    }

    dir_entries
  }
}

pub fn format_range_with_colors(range: &deno_graph::Range) -> String {
  format!(
    "{}:{}:{}",
    colors::cyan(range.specifier.as_str()),
    colors::yellow(&(range.start.line + 1).to_string()),
    colors::yellow(&(range.start.character + 1).to_string())
  )
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CliJsrUrlProvider;

impl deno_graph::source::JsrUrlProvider for CliJsrUrlProvider {
  fn url(&self) -> &'static ModuleSpecifier {
    jsr_url()
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_ast::ModuleSpecifier;
  use deno_graph::source::ResolveError;
  use deno_graph::Position;
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
        error: Arc::new(ResolveError::Other(err.into())),
        specifier: input.to_string(),
        range: Range {
          specifier,
          start: Position::zeroed(),
          end: Position::zeroed(),
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
          start: Position::zeroed(),
          end: Position::zeroed(),
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
