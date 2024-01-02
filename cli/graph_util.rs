// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::Lockfile;
use crate::args::TsTypeLib;
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
use crate::util::sync::TaskQueue;
use crate::util::sync::TaskQueuePermit;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::ModuleSpecifier;
use deno_graph::source::Loader;
use deno_graph::source::ResolveError;
use deno_graph::GraphKind;
use deno_graph::Module;
use deno_graph::ModuleAnalyzer;
use deno_graph::ModuleError;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use import_map::ImportMapError;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct GraphValidOptions {
  pub check_js: bool,
  pub follow_type_only: bool,
  pub is_vendoring: bool,
}

/// Check if `roots` and their deps are available. Returns `Ok(())` if
/// so. Returns `Err(_)` if there is a known module graph or resolution
/// error statically reachable from `roots` and not a dynamic import.
pub fn graph_valid_with_cli_options(
  graph: &ModuleGraph,
  fs: &dyn FileSystem,
  roots: &[ModuleSpecifier],
  options: &CliOptions,
) -> Result<(), AnyError> {
  graph_valid(
    graph,
    fs,
    roots,
    GraphValidOptions {
      is_vendoring: false,
      follow_type_only: options.type_check_mode().is_true(),
      check_js: options.check_js(),
    },
  )
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
  fs: &dyn FileSystem,
  roots: &[ModuleSpecifier],
  options: GraphValidOptions,
) -> Result<(), AnyError> {
  let mut errors = graph
    .walk(
      roots,
      deno_graph::WalkOptions {
        check_js: options.check_js,
        follow_type_only: options.follow_type_only,
        follow_dynamic: options.is_vendoring,
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
        ModuleGraphError::ModuleError(e) => {
          enhanced_module_error_message(fs, e)
        }
      };

      if let Some(range) = error.maybe_range() {
        if !is_root && !range.specifier.as_str().contains("/$deno$eval") {
          message.push_str("\n    at ");
          message.push_str(&format_range_with_colors(range));
        }
      }

      if options.is_vendoring {
        // warn about failing dynamic imports when vendoring, but don't fail completely
        if matches!(
          error,
          ModuleGraphError::ModuleError(ModuleError::MissingDynamic(_, _))
        ) {
          log::warn!("Ignoring: {:#}", message);
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
    Ok(())
  }
}

/// Checks the lockfile against the graph and and exits on errors.
pub fn graph_lock_or_exit(graph: &ModuleGraph, lockfile: &mut Lockfile) {
  for module in graph.modules() {
    let source = match module {
      Module::Esm(module) if module.media_type.is_declaration() => continue, // skip declaration files
      Module::Esm(module) => &module.source,
      Module::Json(module) => &module.source,
      Module::Node(_) | Module::Npm(_) | Module::External(_) => continue,
    };
    if !lockfile.check_or_insert_remote(module.specifier().as_str(), source) {
      let err = format!(
        concat!(
          "The source code is invalid, as it does not match the expected hash in the lock file.\n",
          "  Specifier: {}\n",
          "  Lock file: {}",
        ),
        module.specifier(),
        lockfile.filename.display(),
      );
      log::error!("{} {}", colors::red("error:"), err);
      std::process::exit(10);
    }
  }
}

pub struct CreateGraphOptions<'a> {
  pub graph_kind: GraphKind,
  pub roots: Vec<ModuleSpecifier>,
  pub loader: &'a mut dyn Loader,
  pub analyzer: &'a dyn ModuleAnalyzer,
}

pub struct ModuleGraphBuilder {
  options: Arc<CliOptions>,
  fs: Arc<dyn FileSystem>,
  resolver: Arc<CliGraphResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  module_info_cache: Arc<ModuleInfoCache>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  lockfile: Option<Arc<Mutex<Lockfile>>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  emit_cache: cache::EmitCache,
  file_fetcher: Arc<FileFetcher>,
  global_http_cache: Arc<GlobalHttpCache>,
  type_checker: Arc<TypeChecker>,
}

impl ModuleGraphBuilder {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: Arc<CliOptions>,
    fs: Arc<dyn FileSystem>,
    resolver: Arc<CliGraphResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
    module_info_cache: Arc<ModuleInfoCache>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    lockfile: Option<Arc<Mutex<Lockfile>>>,
    maybe_file_watcher_reporter: Option<FileWatcherReporter>,
    emit_cache: cache::EmitCache,
    file_fetcher: Arc<FileFetcher>,
    global_http_cache: Arc<GlobalHttpCache>,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
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
      type_checker,
    }
  }

  pub async fn create_graph(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let mut cache = self.create_graph_loader();
    self
      .create_graph_with_loader(graph_kind, roots, &mut cache)
      .await
  }

  pub async fn create_graph_with_loader(
    &self,
    graph_kind: GraphKind,
    roots: Vec<ModuleSpecifier>,
    loader: &mut dyn Loader,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let store = self.parsed_source_cache.as_store();
    let analyzer = self.module_info_cache.as_module_analyzer(None, &*store);
    self
      .create_graph_with_options(CreateGraphOptions {
        graph_kind,
        roots,
        loader,
        analyzer: &analyzer,
      })
      .await
  }

  pub async fn create_graph_with_options(
    &self,
    options: CreateGraphOptions<'_>,
  ) -> Result<deno_graph::ModuleGraph, AnyError> {
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_workspace_config = self.options.maybe_workspace_config();
    let workspace_members = if let Some(wc) = maybe_workspace_config {
      workspace_config_to_workspace_members(wc)?
    } else {
      vec![]
    };

    let cli_resolver = self.resolver.clone();
    let graph_resolver = cli_resolver.as_graph_resolver();
    let graph_npm_resolver = cli_resolver.as_graph_npm_resolver();
    let maybe_file_watcher_reporter = self
      .maybe_file_watcher_reporter
      .as_ref()
      .map(|r| r.as_reporter());

    let mut graph = ModuleGraph::new(options.graph_kind);
    self
      .build_graph_with_npm_resolution(
        &mut graph,
        options.roots,
        options.loader,
        deno_graph::BuildOptions {
          is_dynamic: false,
          imports: maybe_imports,
          resolver: Some(graph_resolver),
          file_system: Some(&DenoGraphFsAdapter(self.fs.as_ref())),
          npm_resolver: Some(graph_npm_resolver),
          module_analyzer: Some(options.analyzer),
          reporter: maybe_file_watcher_reporter,
          workspace_members,
        },
      )
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
    let mut cache = self.create_graph_loader();
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_workspace_config = self.options.maybe_workspace_config();
    let workspace_members = if let Some(wc) = maybe_workspace_config {
      workspace_config_to_workspace_members(wc)?
    } else {
      vec![]
    };
    let cli_resolver = self.resolver.clone();
    let graph_resolver = cli_resolver.as_graph_resolver();
    let graph_npm_resolver = cli_resolver.as_graph_npm_resolver();
    let store = self.parsed_source_cache.as_store();
    let analyzer = self.module_info_cache.as_module_analyzer(None, &*store);
    let graph_kind = self.options.type_check_mode().as_graph_kind();
    let mut graph = ModuleGraph::new(graph_kind);
    let maybe_file_watcher_reporter = self
      .maybe_file_watcher_reporter
      .as_ref()
      .map(|r| r.as_reporter());

    self
      .build_graph_with_npm_resolution(
        &mut graph,
        roots,
        &mut cache,
        deno_graph::BuildOptions {
          is_dynamic: false,
          imports: maybe_imports,
          file_system: Some(&DenoGraphFsAdapter(self.fs.as_ref())),
          resolver: Some(graph_resolver),
          npm_resolver: Some(graph_npm_resolver),
          module_analyzer: Some(&analyzer),
          reporter: maybe_file_watcher_reporter,
          workspace_members,
        },
      )
      .await?;

    let graph = Arc::new(graph);
    graph_valid_with_cli_options(
      &graph,
      self.fs.as_ref(),
      &graph.roots,
      &self.options,
    )?;
    if let Some(lockfile) = &self.lockfile {
      graph_lock_or_exit(&graph, &mut lockfile.lock());
    }

    if self.options.type_check_mode().is_true() {
      self
        .type_checker
        .check(
          graph.clone(),
          check::CheckOptions {
            lib: self.options.ts_type_lib_window(),
            log_ignored_options: true,
            reload: self.options.reload_flag(),
          },
        )
        .await?;
    }

    Ok(graph)
  }

  pub async fn build_graph_with_npm_resolution<'a>(
    &self,
    graph: &mut ModuleGraph,
    roots: Vec<ModuleSpecifier>,
    loader: &mut dyn deno_graph::source::Loader,
    options: deno_graph::BuildOptions<'a>,
  ) -> Result<(), AnyError> {
    // ensure an "npm install" is done if the user has explicitly
    // opted into using a node_modules directory
    if self.options.node_modules_dir_enablement() == Some(true) {
      if let Some(npm_resolver) = self.npm_resolver.as_managed() {
        npm_resolver.ensure_top_level_package_json_install().await?;
      }
    }

    // add the lockfile redirects to the graph if it's the first time executing
    if graph.redirects.is_empty() {
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
      }
    }

    // add the jsr specifiers to the graph if it's the first time executing
    if graph.packages.is_empty() {
      if let Some(lockfile) = &self.lockfile {
        let lockfile = lockfile.lock();
        for (key, value) in &lockfile.content.packages.specifiers {
          if let Some(key) = key
            .strip_prefix("jsr:")
            .and_then(|key| PackageReq::from_str(key).ok())
          {
            if let Some(value) = value
              .strip_prefix("jsr:")
              .and_then(|value| PackageNv::from_str(value).ok())
            {
              graph.packages.add(key, value);
            }
          }
        }
      }
    }

    graph.build(roots, loader, options).await;

    // add the redirects in the graph to the lockfile
    if !graph.redirects.is_empty() {
      if let Some(lockfile) = &self.lockfile {
        let graph_redirects = graph.redirects.iter().filter(|(from, _)| {
          !matches!(from.scheme(), "npm" | "file" | "deno")
        });
        let mut lockfile = lockfile.lock();
        for (from, to) in graph_redirects {
          lockfile.insert_redirect(from.to_string(), to.to_string());
        }
      }
    }

    // add the jsr specifiers in the graph to the lockfile
    if !graph.packages.is_empty() {
      if let Some(lockfile) = &self.lockfile {
        let mappings = graph.packages.mappings();
        let mut lockfile = lockfile.lock();
        for (from, to) in mappings {
          lockfile.insert_package_specifier(
            format!("jsr:{}", from),
            format!("jsr:{}", to),
          );
        }
      }
    }

    if let Some(npm_resolver) = self.npm_resolver.as_managed() {
      // ensure that the top level package.json is installed if a
      // specifier was matched in the package.json
      if self.resolver.found_package_json_dep() {
        npm_resolver.ensure_top_level_package_json_install().await?;
      }

      // resolve the dependencies of any pending dependencies
      // that were inserted by building the graph
      npm_resolver.resolve_pending().await?;
    }

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
      Module::Esm(_) | Module::Json(_) | Module::External(_) => {}
    }
  }
  Ok(())
}

/// Adds more explanatory information to a resolution error.
pub fn enhanced_resolution_error_message(error: &ResolutionError) -> String {
  let mut message = format!("{error}");

  if let Some(specifier) = get_resolution_error_bare_node_specifier(error) {
    message.push_str(&format!(
        "\nIf you want to use a built-in Node module, add a \"node:\" prefix (ex. \"node:{specifier}\")."
      ));
  }

  message
}

pub fn enhanced_module_error_message(
  fs: &dyn FileSystem,
  error: &ModuleError,
) -> String {
  let additional_message = match error {
    ModuleError::Missing(specifier, _) => {
      SloppyImportsResolver::resolve_with_fs(fs, specifier)
        .as_suggestion_message()
    }
    _ => None,
  };
  if let Some(message) = additional_message {
    format!(
      "{} {} or run with --unstable-sloppy-imports",
      error, message
    )
  } else {
    format!("{}", error)
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
    error: SpecifierError::ImportPrefixMissing(specifier, _),
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

#[derive(Debug)]
struct GraphData {
  graph: Arc<ModuleGraph>,
  checked_libs: HashMap<TsTypeLib, HashSet<ModuleSpecifier>>,
}

/// Holds the `ModuleGraph` and what parts of it are type checked.
pub struct ModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Arc<TaskQueue>,
  graph_data: Arc<RwLock<GraphData>>,
}

impl ModuleGraphContainer {
  pub fn new(graph_kind: GraphKind) -> Self {
    Self {
      update_queue: Default::default(),
      graph_data: Arc::new(RwLock::new(GraphData {
        graph: Arc::new(ModuleGraph::new(graph_kind)),
        checked_libs: Default::default(),
      })),
    }
  }

  /// Acquires a permit to modify the module graph without other code
  /// having the chance to modify it. In the meantime, other code may
  /// still read from the existing module graph.
  pub async fn acquire_update_permit(&self) -> ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    ModuleGraphUpdatePermit {
      permit,
      graph_data: self.graph_data.clone(),
      graph: (*self.graph_data.read().graph).clone(),
    }
  }

  pub fn graph(&self) -> Arc<ModuleGraph> {
    self.graph_data.read().graph.clone()
  }

  /// Mark `roots` and all of their dependencies as type checked under `lib`.
  /// Assumes that all of those modules are known.
  pub fn set_type_checked(&self, roots: &[ModuleSpecifier], lib: TsTypeLib) {
    // It's ok to analyze and update this while the module graph itself is
    // being updated in a permit because the module graph update is always
    // additive and this will be a subset of the original graph
    let graph = self.graph();
    let entries = graph.walk(
      roots,
      deno_graph::WalkOptions {
        check_js: true,
        follow_dynamic: true,
        follow_type_only: true,
      },
    );

    // now update
    let mut data = self.graph_data.write();
    let checked_lib_set = data.checked_libs.entry(lib).or_default();
    for (specifier, _) in entries {
      checked_lib_set.insert(specifier.clone());
    }
  }

  /// Check if `roots` are all marked as type checked under `lib`.
  pub fn is_type_checked(
    &self,
    roots: &[ModuleSpecifier],
    lib: TsTypeLib,
  ) -> bool {
    let data = self.graph_data.read();
    match data.checked_libs.get(&lib) {
      Some(checked_lib_set) => roots.iter().all(|r| {
        let found = data.graph.resolve(r);
        checked_lib_set.contains(&found)
      }),
      None => false,
    }
  }
}

/// Gets if any of the specified root's "file:" dependents are in the
/// provided changed set.
pub fn has_graph_root_local_dependent_changed(
  graph: &ModuleGraph,
  root: &ModuleSpecifier,
  changed_specifiers: &HashSet<ModuleSpecifier>,
) -> bool {
  let roots = vec![root.clone()];
  let mut dependent_specifiers = graph.walk(
    &roots,
    deno_graph::WalkOptions {
      follow_dynamic: true,
      follow_type_only: true,
      check_js: true,
    },
  );
  while let Some((s, _)) = dependent_specifiers.next() {
    if s.scheme() != "file" {
      // skip walking this remote module's dependencies
      dependent_specifiers.skip_previous_dependencies();
    } else if changed_specifiers.contains(s) {
      return true;
    }
  }
  false
}

/// A permit for updating the module graph. When complete and
/// everything looks fine, calling `.commit()` will store the
/// new graph in the ModuleGraphContainer.
pub struct ModuleGraphUpdatePermit<'a> {
  permit: TaskQueuePermit<'a>,
  graph_data: Arc<RwLock<GraphData>>,
  graph: ModuleGraph,
}

impl<'a> ModuleGraphUpdatePermit<'a> {
  /// Gets the module graph for mutation.
  pub fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  /// Saves the mutated module graph in the container
  /// and returns an Arc to the new module graph.
  pub fn commit(self) -> Arc<ModuleGraph> {
    let graph = Arc::new(self.graph);
    self.graph_data.write().graph = graph.clone();
    drop(self.permit); // explicit drop for clarity
    graph
  }
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

pub fn workspace_config_to_workspace_members(
  workspace_config: &deno_config::WorkspaceConfig,
) -> Result<Vec<deno_graph::WorkspaceMember>, AnyError> {
  workspace_config
    .members
    .iter()
    .map(|member| {
      workspace_member_config_try_into_workspace_member(member).with_context(
        || {
          format!(
            "Failed to resolve configuration for '{}' workspace member at '{}'",
            member.member_name,
            member.config_file.specifier.as_str()
          )
        },
      )
    })
    .collect()
}

fn workspace_member_config_try_into_workspace_member(
  config: &deno_config::WorkspaceMemberConfig,
) -> Result<deno_graph::WorkspaceMember, AnyError> {
  let nv = deno_semver::package::PackageNv {
    name: config.package_name.clone(),
    version: deno_semver::Version::parse_standard(&config.package_version)?,
  };
  Ok(deno_graph::WorkspaceMember {
    base: ModuleSpecifier::from_directory_path(&config.path).unwrap(),
    nv,
    exports: config
      .config_file
      .to_exports_config()?
      .into_map()
      // todo(dsherret): deno_graph should use an IndexMap
      .into_iter()
      .collect(),
  })
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
        error: SpecifierError::ImportPrefixMissing(input.to_string(), None),
      };
      assert_eq!(get_resolution_error_bare_node_specifier(&err), output,);
    }
  }
}
