// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_cache_dir::file_fetcher::FetchLocalOptions;
use deno_cache_dir::file_fetcher::MemoryFiles as _;
use deno_core::FastString;
use deno_core::ModuleLoadOptions;
use deno_core::ModuleLoadReferrer;
use deno_core::ModuleLoader;
use deno_core::ModuleResolutionError;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::SourceCodeCacheInfo;
use deno_core::anyhow::Context as _;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::error::ModuleLoaderError;
use deno_core::futures::StreamExt;
use deno_core::futures::future::FutureExt;
use deno_core::futures::io::BufReader;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_graph::WalkOptions;
use deno_lib::loader::as_deno_resolver_requested_module_type;
use deno_lib::loader::loaded_module_source_to_module_source_code;
use deno_lib::loader::module_type_from_media_and_requested_type;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::worker::CreateModuleLoaderResult;
use deno_lib::worker::ModuleLoaderFactory;
use deno_npm_installer::resolution::HasJsExecutionStartedFlagRc;
use deno_path_util::PathToUrlError;
use deno_path_util::resolve_url_or_path;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::file_fetcher::FetchOptions;
use deno_resolver::file_fetcher::FetchPermissionsOptionRef;
use deno_resolver::graph::ResolveWithGraphErrorKind;
use deno_resolver::graph::ResolveWithGraphOptions;
use deno_resolver::graph::format_range_with_colors;
use deno_resolver::loader::LoadCodeSourceError;
use deno_resolver::loader::LoadPreparedModuleError;
use deno_resolver::loader::LoadedModule;
use deno_resolver::loader::LoadedModuleOrAsset;
use deno_resolver::loader::MemoryFiles;
use deno_resolver::loader::StrippingTypesNodeModulesError;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::code_cache;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::ops::require::UnableToGetCwdError;
use deno_runtime::deno_permissions::CheckSpecifierKind;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use eszip::EszipV2;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use node_resolver::errors::PackageJsonLoadError;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsRead;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::TsTypeLib;
use crate::args::jsr_url;
use crate::cache::CodeCache;
use crate::file_fetcher::CliFileFetcher;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::BuildGraphRequest;
use crate::graph_util::BuildGraphWithNpmOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::resolver::CliResolver;
use crate::sys::CliSys;
use crate::type_checker::CheckError;
use crate::type_checker::CheckOptions;
use crate::type_checker::TypeChecker;
use crate::util::progress_bar::ProgressBar;
use crate::util::text_encoding::code_without_source_map;
use crate::util::text_encoding::source_map_from_code;

pub type CliEmitter =
  deno_resolver::emit::Emitter<DenoInNpmPackageChecker, CliSys>;
pub type CliDenoResolverModuleLoader =
  deno_resolver::loader::ModuleLoader<CliSys>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PrepareModuleLoadError {
  #[class(inherit)]
  #[error(transparent)]
  BuildGraphWithNpmResolution(
    #[from] crate::graph_util::BuildGraphWithNpmResolutionError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Check(#[from] CheckError),
  #[class(inherit)]
  #[error(transparent)]
  LockfileWrite(#[from] deno_resolver::lockfile::LockfileWriteError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub struct ModuleLoadPreparer {
  options: Arc<CliOptions>,
  lockfile: Option<Arc<CliLockfile>>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  progress_bar: ProgressBar,
  type_checker: Arc<TypeChecker>,
}

pub struct PrepareModuleLoadOptions<'a> {
  pub is_dynamic: bool,
  pub lib: TsTypeLib,
  pub permissions: PermissionsContainer,
  pub ext_overwrite: Option<&'a String>,
  pub allow_unknown_media_types: bool,
  /// Whether to skip validating the graph roots. This is useful
  /// for when you want to defer doing this until later (ex. get the
  /// graph back, reload some specifiers in it, then do graph validation).
  pub skip_graph_roots_validation: bool,
}

impl ModuleLoadPreparer {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: Arc<CliOptions>,
    lockfile: Option<Arc<CliLockfile>>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    progress_bar: ProgressBar,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      lockfile,
      module_graph_builder,
      progress_bar,
      type_checker,
    }
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate the graph data in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  pub async fn prepare_module_load(
    &self,
    graph: &mut ModuleGraph,
    roots: &[ModuleSpecifier],
    options: PrepareModuleLoadOptions<'_>,
  ) -> Result<(), PrepareModuleLoadError> {
    log::debug!("Preparing module load.");
    let PrepareModuleLoadOptions {
      is_dynamic,
      lib,
      permissions,
      ext_overwrite,
      allow_unknown_media_types,
      skip_graph_roots_validation,
    } = options;
    let _pb_clear_guard = self.progress_bar.deferred_keep_initialize_alive();

    let mut loader = self
      .module_graph_builder
      .create_graph_loader_with_permissions(permissions);
    if let Some(ext) = ext_overwrite {
      let maybe_content_type = match ext.as_str() {
        "ts" => Some("text/typescript"),
        "tsx" => Some("text/tsx"),
        "js" => Some("text/javascript"),
        "jsx" => Some("text/jsx"),
        _ => None,
      };
      if let Some(content_type) = maybe_content_type {
        for root in roots {
          loader.insert_file_header_override(
            root.clone(),
            std::collections::HashMap::from([(
              "content-type".to_string(),
              content_type.to_string(),
            )]),
          );
        }
      }
    }
    log::debug!("Building module graph.");
    let has_type_checked = !graph.roots.is_empty();

    self
      .module_graph_builder
      .build_graph_with_npm_resolution(
        graph,
        BuildGraphWithNpmOptions {
          is_dynamic,
          request: BuildGraphRequest::Roots(roots.to_vec()),
          loader: Some(&mut loader),
          npm_caching: self.options.default_npm_caching_strategy(),
        },
      )
      .await?;

    if !skip_graph_roots_validation {
      self.graph_roots_valid(graph, roots, allow_unknown_media_types, false)?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    if self.options.type_check_mode().is_true() && !has_type_checked {
      self.type_checker.check(
        // todo(perf): since this is only done the first time the graph is
        // created, we could avoid the clone of the graph here by providing
        // the actual graph on the first run and then getting the Arc<ModuleGraph>
        // back from the return value.
        graph.clone(),
        CheckOptions {
          build_fast_check_graph: true,
          lib,
          reload: self.options.reload_flag(),
          type_check_mode: self.options.type_check_mode(),
        },
      )?;
    }

    // write the lockfile if there is one and do so after type checking
    // as type checking might discover `@types/node`
    if let Some(lockfile) = &self.lockfile {
      lockfile.write_if_changed()?;
    }

    log::debug!("Prepared module load.");

    Ok(())
  }

  pub async fn reload_specifiers(
    &self,
    graph: &mut ModuleGraph,
    specifiers: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    permissions: PermissionsContainer,
  ) -> Result<(), PrepareModuleLoadError> {
    log::debug!(
      "Reloading modified files: {}",
      specifiers
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(", ")
    );
    let _pb_clear_guard = self.progress_bar.deferred_keep_initialize_alive();

    let mut loader = self
      .module_graph_builder
      .create_graph_loader_with_permissions(permissions);
    self
      .module_graph_builder
      .build_graph_with_npm_resolution(
        graph,
        BuildGraphWithNpmOptions {
          is_dynamic,
          request: BuildGraphRequest::Reload(specifiers),
          loader: Some(&mut loader),
          npm_caching: self.options.default_npm_caching_strategy(),
        },
      )
      .await?;

    if let Some(lockfile) = &self.lockfile {
      lockfile.write_if_changed()?;
    }

    Ok(())
  }

  pub fn graph_roots_valid(
    &self,
    graph: &ModuleGraph,
    roots: &[ModuleSpecifier],
    allow_unknown_media_types: bool,
    allow_unknown_jsr_exports: bool,
  ) -> Result<(), JsErrorBox> {
    self.module_graph_builder.graph_roots_valid(
      graph,
      roots,
      allow_unknown_media_types,
      allow_unknown_jsr_exports,
    )
  }
}

struct SharedCliModuleLoaderState {
  graph_kind: GraphKind,
  lib_window: TsTypeLib,
  lib_worker: TsTypeLib,
  initial_cwd: PathBuf,
  is_inspecting: bool,
  is_repl: bool,
  cjs_tracker: Arc<CliCjsTracker>,
  code_cache: Option<Arc<CodeCache>>,
  emitter: Arc<CliEmitter>,
  file_fetcher: Arc<CliFileFetcher>,
  has_js_execution_started_flag: HasJsExecutionStartedFlagRc,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  main_module_graph_container: Arc<MainModuleGraphContainer>,
  memory_files: Arc<MemoryFiles>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  npm_registry_permission_checker:
    Arc<NpmRegistryReadPermissionChecker<CliSys>>,
  npm_resolver: CliNpmResolver,
  parsed_source_cache: Arc<ParsedSourceCache>,
  module_loader: Arc<CliDenoResolverModuleLoader>,
  resolver: Arc<CliResolver>,
  sys: CliSys,
  in_flight_loads_tracker: InFlightModuleLoadsTracker,
  maybe_eszip_loader: Option<Arc<EszipModuleLoader>>,
}

struct InFlightModuleLoadsTracker {
  loads_number: Arc<AtomicU16>,
  cleanup_task_timeout: u64,
  cleanup_task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl InFlightModuleLoadsTracker {
  pub fn increase(&self) {
    self.loads_number.fetch_add(1, Ordering::Relaxed);
    if let Some(task) = self.cleanup_task_handle.lock().take() {
      task.abort();
    }
  }

  pub fn decrease(&self, parsed_source_cache: &Arc<ParsedSourceCache>) {
    let prev = self.loads_number.fetch_sub(1, Ordering::Relaxed);
    if prev == 1 {
      let parsed_source_cache = parsed_source_cache.clone();
      let timeout = self.cleanup_task_timeout;
      let task_handle = tokio::spawn(async move {
        // We use a timeout here, which is defined to 10s,
        // so that in situations when dynamic imports are loaded after the startup,
        // we don't need to recompute and analyze multiple modules.
        tokio::time::sleep(std::time::Duration::from_millis(timeout)).await;
        parsed_source_cache.free_all();
      });
      let maybe_prev_task =
        self.cleanup_task_handle.lock().replace(task_handle);
      if let Some(prev_task) = maybe_prev_task {
        prev_task.abort();
      }
    }
  }
}

pub struct CliModuleLoaderFactory {
  shared: Arc<SharedCliModuleLoaderState>,
}

impl CliModuleLoaderFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: &CliOptions,
    cjs_tracker: Arc<CliCjsTracker>,
    code_cache: Option<Arc<CodeCache>>,
    emitter: Arc<CliEmitter>,
    file_fetcher: Arc<CliFileFetcher>,
    has_js_execution_started_flag: HasJsExecutionStartedFlagRc,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    main_module_graph_container: Arc<MainModuleGraphContainer>,
    memory_files: Arc<MemoryFiles>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
    npm_registry_permission_checker: Arc<
      NpmRegistryReadPermissionChecker<CliSys>,
    >,
    npm_resolver: CliNpmResolver,
    parsed_source_cache: Arc<ParsedSourceCache>,
    module_loader: Arc<CliDenoResolverModuleLoader>,
    resolver: Arc<CliResolver>,
    sys: CliSys,
    maybe_eszip_loader: Option<Arc<EszipModuleLoader>>,
  ) -> Self {
    Self {
      shared: Arc::new(SharedCliModuleLoaderState {
        graph_kind: options.graph_kind(),
        lib_window: options.ts_type_lib_window(),
        lib_worker: options.ts_type_lib_worker(),
        initial_cwd: options.initial_cwd().to_path_buf(),
        is_inspecting: options.is_inspecting(),
        is_repl: matches!(
          options.sub_command(),
          DenoSubcommand::Repl(_) | DenoSubcommand::Jupyter(_)
        ),
        cjs_tracker,
        code_cache,
        emitter,
        file_fetcher,
        has_js_execution_started_flag,
        in_npm_pkg_checker,
        main_module_graph_container,
        memory_files,
        module_load_preparer,
        npm_registry_permission_checker,
        npm_resolver,
        parsed_source_cache,
        module_loader,
        resolver,
        sys,
        in_flight_loads_tracker: InFlightModuleLoadsTracker {
          loads_number: Arc::new(AtomicU16::new(0)),
          cleanup_task_timeout: 10_000,
          cleanup_task_handle: Arc::new(Mutex::new(None)),
        },
        maybe_eszip_loader,
      }),
    }
  }

  fn create_with_lib<TGraphContainer: ModuleGraphContainer>(
    &self,
    graph_container: TGraphContainer,
    lib: TsTypeLib,
    is_worker: bool,
    parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    let module_loader =
      Rc::new(CliModuleLoader(Rc::new(CliModuleLoaderInner {
        lib,
        is_worker,
        parent_permissions,
        permissions,
        graph_container: graph_container.clone(),
        shared: self.shared.clone(),
        loaded_files: Default::default(),
      })));
    let node_require_loader = Rc::new(CliNodeRequireLoader {
      cjs_tracker: self.shared.cjs_tracker.clone(),
      emitter: self.shared.emitter.clone(),
      npm_resolver: self.shared.npm_resolver.clone(),
      sys: self.shared.sys.clone(),
      graph_container,
      in_npm_pkg_checker: self.shared.in_npm_pkg_checker.clone(),
      memory_files: self.shared.memory_files.clone(),
      npm_registry_permission_checker: self
        .shared
        .npm_registry_permission_checker
        .clone(),
    });
    CreateModuleLoaderResult {
      module_loader,
      node_require_loader,
    }
  }
}

impl ModuleLoaderFactory for CliModuleLoaderFactory {
  fn create_for_main(
    &self,
    root_permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_with_lib(
      (*self.shared.main_module_graph_container).clone(),
      self.shared.lib_window,
      /* is worker */ false,
      root_permissions.clone(),
      root_permissions,
    )
  }

  fn create_for_worker(
    &self,
    parent_permissions: PermissionsContainer,
    permissions: PermissionsContainer,
  ) -> CreateModuleLoaderResult {
    self.create_with_lib(
      // create a fresh module graph for the worker
      WorkerModuleGraphContainer::new(Arc::new(ModuleGraph::new(
        self.shared.graph_kind,
      ))),
      self.shared.lib_worker,
      /* is worker */ true,
      parent_permissions,
      permissions,
    )
  }
}

struct ModuleCodeStringSource {
  pub code: ModuleSourceCode,
  pub found_url: ModuleSpecifier,
  pub module_type: ModuleType,
}

struct CliModuleLoaderInner<TGraphContainer: ModuleGraphContainer> {
  lib: TsTypeLib,
  is_worker: bool,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. These are "allow all" for main worker, and parent thread
  /// permissions for Web Worker.
  parent_permissions: PermissionsContainer,
  permissions: PermissionsContainer,
  shared: Arc<SharedCliModuleLoaderState>,
  graph_container: TGraphContainer,
  loaded_files: RefCell<HashSet<ModuleSpecifier>>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResolveReferrerError {
  #[class(inherit)]
  #[error(transparent)]
  UnableToGetCwd(#[from] UnableToGetCwdError),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ModuleResolution(#[from] ModuleResolutionError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CliModuleLoaderError {
  #[class(inherit)]
  #[error(transparent)]
  Fetch(#[from] deno_resolver::file_fetcher::FetchError),
  #[class(inherit)]
  #[error(transparent)]
  LoadCodeSource(#[from] LoadCodeSourceError),
  #[class(inherit)]
  #[error(transparent)]
  LoadPreparedModule(#[from] LoadPreparedModuleError),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveNpmReqRef(#[from] deno_resolver::npm::ResolveNpmReqRefError),
  #[class(inherit)]
  #[error(transparent)]
  ResolveReferrer(#[from] ResolveReferrerError),
}

impl<TGraphContainer: ModuleGraphContainer>
  CliModuleLoaderInner<TGraphContainer>
{
  async fn load_inner(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    requested_module_type: &RequestedModuleType,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let code_source = self
      .load_code_source(specifier, maybe_referrer, requested_module_type)
      .await
      .map_err(JsErrorBox::from_err)?;

    let code = if self.shared.is_inspecting
      || code_source.module_type == ModuleType::Wasm
    {
      // we need the code with the source map in order for
      // it to work with --inspect or --inspect-brk
      code_source.code
    } else {
      // v8 is slower when source maps are present, so we strip them
      code_without_source_map(code_source.code)
    };

    let code_cache = if code_source.module_type == ModuleType::JavaScript {
      self.shared.code_cache.as_ref().map(|cache| {
        let code_hash = FastInsecureHasher::new_deno_versioned()
          .write_hashable(&code)
          .finish();
        let data = cache
          .get_sync(specifier, code_cache::CodeCacheType::EsModule, code_hash)
          .map(Cow::from)
          .inspect(|_| {
            // This log line is also used by tests.
            log::debug!(
              "V8 code cache hit for ES module: {specifier}, [{code_hash:?}]"
            );
          });
        SourceCodeCacheInfo {
          hash: code_hash,
          data,
        }
      })
    } else {
      None
    };

    Ok(ModuleSource::new_with_redirect(
      code_source.module_type,
      code,
      specifier,
      &code_source.found_url,
      code_cache,
    ))
  }

  async fn load_code_source(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    requested_module_type: &RequestedModuleType,
  ) -> Result<ModuleCodeStringSource, CliModuleLoaderError> {
    // this loader maintains npm specifiers in dynamic imports when resolving
    // so that they can be properly preloaded, but now we might receive them
    // here, so we need to actually resolve them to a file: specifier here
    let specifier = if let Ok(reference) =
      NpmPackageReqReference::from_specifier(specifier)
    {
      let referrer = match maybe_referrer {
        // if we're here, it means it was importing from a dynamic import
        // and so there will be a referrer
        Some(r) => Cow::Borrowed(r),
        // but the repl may also end up here and it won't have
        // a referrer so create a referrer for it here
        None => Cow::Owned(self.resolve_referrer("")?),
      };
      Cow::Owned(
        self
          .shared
          .resolver
          .resolve_non_workspace_npm_req_ref_to_file(
            &reference,
            &referrer,
            ResolutionMode::Import,
            NodeResolutionKind::Execution,
          )?
          .into_url()?,
      )
    } else {
      Cow::Borrowed(specifier)
    };

    let graph = self.graph_container.graph();
    let deno_resolver_requested_module_type =
      as_deno_resolver_requested_module_type(requested_module_type);
    match self
      .shared
      .module_loader
      .load(
        &graph,
        &specifier,
        maybe_referrer,
        &deno_resolver_requested_module_type,
      )
      .await?
    {
      LoadedModuleOrAsset::Module(prepared_module) => {
        Ok(self.loaded_module_to_module_code_string_source(
          prepared_module,
          requested_module_type,
        ))
      }
      LoadedModuleOrAsset::ExternalAsset {
        specifier,
        statically_analyzable,
      } => {
        Ok(
          self
            .load_asset(
              &specifier,
              if statically_analyzable {
                CheckSpecifierKind::Static
              } else {
                // force permissions
                CheckSpecifierKind::Dynamic
              },
              requested_module_type,
            )
            .await?,
        )
      }
    }
  }

  fn loaded_module_to_module_code_string_source(
    &self,
    loaded_module: LoadedModule,
    requested_module_type: &RequestedModuleType,
  ) -> ModuleCodeStringSource {
    ModuleCodeStringSource {
      code: loaded_module_source_to_module_source_code(loaded_module.source),
      found_url: loaded_module.specifier.into_owned(),
      module_type: module_type_from_media_and_requested_type(
        loaded_module.media_type,
        requested_module_type,
      ),
    }
  }

  async fn load_asset(
    &self,
    specifier: &ModuleSpecifier,
    check_specifier_kind: CheckSpecifierKind,
    requested_module_type: &RequestedModuleType,
  ) -> Result<ModuleCodeStringSource, deno_resolver::file_fetcher::FetchError>
  {
    let file = self
      .shared
      .file_fetcher
      .fetch_with_options(
        specifier,
        FetchPermissionsOptionRef::Restricted(
          match check_specifier_kind {
            CheckSpecifierKind::Static => &self.permissions,
            CheckSpecifierKind::Dynamic => &self.parent_permissions,
          },
          check_specifier_kind,
        ),
        FetchOptions {
          local: FetchLocalOptions {
            include_mtime: false,
          },
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: Some(
            &deno_cache_dir::file_fetcher::CacheSetting::Use,
          ),
        },
      )
      .await?;

    let module_type = match requested_module_type {
      RequestedModuleType::Text => ModuleType::Text,
      RequestedModuleType::Bytes => ModuleType::Bytes,
      RequestedModuleType::None => {
        match file.resolve_media_type_and_charset().0 {
          MediaType::Wasm => ModuleType::Wasm,
          _ => ModuleType::JavaScript,
        }
      }
      t => unreachable!("{t}"),
    };

    Ok(ModuleCodeStringSource {
      code: ModuleSourceCode::Bytes(file.source.into()),
      found_url: file.url,
      module_type,
    })
  }

  async fn maybe_reload_dynamic(
    &self,
    graph: &ModuleGraph,
    specifier: &ModuleSpecifier,
    permissions: &PermissionsContainer,
  ) -> Result<bool, PrepareModuleLoadError> {
    let specifiers_to_reload =
      self.check_specifiers_to_reload_for_dynamic_import(graph, specifier);

    if specifiers_to_reload.is_empty() {
      return Ok(false);
    }

    let mut graph_permit = self.graph_container.acquire_update_permit().await;
    let graph = graph_permit.graph_mut();
    self
      .shared
      .module_load_preparer
      .reload_specifiers(
        graph,
        specifiers_to_reload,
        /* is dynamic */ true,
        permissions.clone(),
      )
      .await?;
    graph_permit.commit();
    Ok(true)
  }

  fn check_specifiers_to_reload_for_dynamic_import(
    &self,
    graph: &ModuleGraph,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    let mut specifiers_to_reload = Vec::new();
    let mut module_iter = graph.walk(
      std::iter::once(specifier),
      WalkOptions {
        check_js: deno_graph::CheckJsOption::False,
        follow_dynamic: false,
        kind: GraphKind::CodeOnly,
        prefer_fast_check_graph: false,
      },
    );
    while let Some((specifier, module_entry)) = module_iter.next() {
      if specifier.scheme() != "file"
        || self.loaded_files.borrow().contains(specifier)
      {
        module_iter.skip_previous_dependencies(); // no need to analyze this module's dependencies
        continue;
      }
      let should_reload = match module_entry {
        deno_graph::ModuleEntryRef::Module(module) => {
          self.has_module_changed_on_file_system(specifier, module.mtime())
        }
        deno_graph::ModuleEntryRef::Err(err) => {
          if matches!(
            err.as_kind(),
            deno_graph::ModuleErrorKind::Missing { .. }
          ) {
            self.mtime_of_specifier(specifier).is_some() // it exists now
          } else {
            self.has_module_changed_on_file_system(specifier, err.mtime())
          }
        }
        deno_graph::ModuleEntryRef::Redirect(_) => false,
      };
      if should_reload {
        specifiers_to_reload.push(specifier.clone());
      }
    }

    self.loaded_files.borrow_mut().insert(specifier.clone());

    specifiers_to_reload
  }

  fn has_module_changed_on_file_system(
    &self,
    specifier: &ModuleSpecifier,
    mtime: Option<SystemTime>,
  ) -> bool {
    let Some(loaded_mtime) = mtime else {
      return false;
    };
    self
      .mtime_of_specifier(specifier)
      .map(|mtime| mtime > loaded_mtime)
      .unwrap_or(false)
  }

  fn mtime_of_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<SystemTime> {
    deno_path_util::url_to_file_path(specifier)
      .ok()
      .and_then(|path| self.shared.sys.fs_symlink_metadata(&path).ok())
      .and_then(|metadata| metadata.modified().ok())
  }

  #[allow(clippy::result_large_err)]
  fn resolve_referrer(
    &self,
    referrer: &str,
  ) -> Result<ModuleSpecifier, ResolveReferrerError> {
    let referrer = if referrer.is_empty() && self.shared.is_repl {
      // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
      // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
      "./$deno$repl.mts"
    } else {
      referrer
    };

    Ok(if deno_path_util::specifier_has_uri_scheme(referrer) {
      deno_core::resolve_url(referrer)?
    } else if referrer == "." {
      // main module, use the initial cwd
      deno_path_util::resolve_path(referrer, &self.shared.initial_cwd)?
    } else {
      // this cwd check is slow, so try to avoid it
      let cwd = std::env::current_dir().map_err(UnableToGetCwdError)?;
      deno_path_util::resolve_path(referrer, &cwd)?
    })
  }

  #[allow(clippy::result_large_err)]
  fn inner_resolve(
    &self,
    raw_specifier: &str,
    raw_referrer: &str,
    kind: deno_core::ResolutionKind,
    is_import_meta: bool,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    fn ensure_not_jsr_non_jsr_remote_import(
      specifier: &ModuleSpecifier,
      referrer: &ModuleSpecifier,
    ) -> Result<(), JsErrorBox> {
      if referrer.as_str().starts_with(jsr_url().as_str())
        && !specifier.as_str().starts_with(jsr_url().as_str())
        && matches!(specifier.scheme(), "http" | "https")
      {
        return Err(JsErrorBox::generic(format!(
          "Importing {} blocked. JSR packages cannot import non-JSR remote modules for security reasons.",
          specifier
        )));
      }
      Ok(())
    }

    let referrer = self
      .resolve_referrer(raw_referrer)
      .map_err(JsErrorBox::from_err)?;
    let graph = self.graph_container.graph();
    let result = self.shared.resolver.resolve_with_graph(
      graph.as_ref(),
      raw_specifier,
      &referrer,
      deno_graph::Position::zeroed(),
      ResolveWithGraphOptions {
        mode: ResolutionMode::Import,
        kind: NodeResolutionKind::Execution,
        // leave npm specifiers as-is for dynamic imports so that
        // the loader can properly install them if necessary
        maintain_npm_specifiers: matches!(
          kind,
          deno_core::ResolutionKind::DynamicImport
        ) && !is_import_meta,
      },
    );
    let specifier = match result {
      Ok(specifier) => specifier,
      Err(err) => {
        if let Some(specifier) = err
          .maybe_specifier()
          .filter(|_| is_import_meta)
          .and_then(|s| s.into_owned().into_url().ok())
        {
          specifier
        } else {
          match err.into_kind() {
            ResolveWithGraphErrorKind::Resolution(err) => {
              // todo(dsherret): why do we have a newline here? Document it.
              return Err(JsErrorBox::type_error(format!(
                "{}\n",
                err.to_string_with_range()
              )));
            }
            err => return Err(JsErrorBox::from_err(err)),
          }
        }
      }
    };

    // only verify this for an import and not import.meta.resolve
    if !is_import_meta {
      ensure_not_jsr_non_jsr_remote_import(&specifier, &referrer)?;
    }

    Ok(specifier)
  }
}

#[derive(Clone)]
// todo(dsherret): this double Rc boxing is not ideal
pub struct CliModuleLoader<TGraphContainer: ModuleGraphContainer>(
  Rc<CliModuleLoaderInner<TGraphContainer>>,
);

impl<TGraphContainer: ModuleGraphContainer> ModuleLoader
  for CliModuleLoader<TGraphContainer>
{
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: deno_core::ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    self.0.inner_resolve(specifier, referrer, kind, false)
  }

  fn import_meta_resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    self.0.inner_resolve(
      specifier,
      referrer,
      deno_core::ResolutionKind::DynamicImport,
      true,
    )
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut deno_core::v8::PinScope<'s, '_>,
    name: &str,
  ) -> Option<deno_core::v8::Local<'s, deno_core::v8::Data>> {
    let name = deno_core::ModuleSpecifier::parse(name).ok()?;
    if self.0.shared.in_npm_pkg_checker.in_npm_package(&name) {
      Some(create_host_defined_options(scope))
    } else {
      None
    }
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleLoadReferrer>,
    options: ModuleLoadOptions,
  ) -> deno_core::ModuleLoadResponse {
    let inner = self.0.clone();

    if let Some(eszip_loader) = &inner.shared.maybe_eszip_loader {
      return eszip_loader.load(specifier);
    }

    self.0.loaded_files.borrow_mut().insert(specifier.clone());

    let specifier = specifier.clone();
    let maybe_referrer = maybe_referrer.cloned();
    deno_core::ModuleLoadResponse::Async(
      async move {
        inner
          .load_inner(
            &specifier,
            maybe_referrer.as_ref().map(|r| &r.specifier),
            &options.requested_module_type,
          )
          .await
          .map_err(|err| {
            let Some(referrer) = maybe_referrer else {
              return err;
            };
            let position = deno_graph::Position {
              line: referrer.line_number as usize - 1,
              character: referrer.column_number as usize - 1,
            };
            JsErrorBox::new(
              err.get_class(),
              format!(
                "{err}\n    at {}",
                format_range_with_colors(&deno_graph::Range {
                  specifier: referrer.specifier,
                  range: deno_graph::PositionRange {
                    start: position,
                    end: position
                  },
                  resolution_mode: None
                })
              ),
            )
          })
      }
      .boxed_local(),
    )
  }

  fn prepare_load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    options: ModuleLoadOptions,
  ) -> Pin<Box<dyn Future<Output = Result<(), ModuleLoaderError>>>> {
    // always call this first unconditionally because it will be
    // decremented unconditionally in "finish_load"
    self.0.shared.in_flight_loads_tracker.increase();

    if matches!(
      options.requested_module_type,
      RequestedModuleType::Text | RequestedModuleType::Bytes
    ) {
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    if self.0.shared.in_npm_pkg_checker.in_npm_package(specifier) {
      self.0.shared.has_js_execution_started_flag.raise();
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    if self.0.shared.maybe_eszip_loader.is_some() {
      self.0.shared.has_js_execution_started_flag.raise();
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    let specifier = specifier.clone();
    let inner = self.0.clone();

    async move {
      let graph_container = &inner.graph_container;
      let module_load_preparer = &inner.shared.module_load_preparer;
      let permissions = if options.is_dynamic_import {
        &inner.permissions
      } else {
        &inner.parent_permissions
      };

      if options.is_dynamic_import {
        // This doesn't acquire a graph update permit because that will
        // clone the graph which is a bit slow.
        let mut graph = graph_container.graph();
        // When the specifier is already in the graph then it means it
        // was previously loaded, so we can skip that and only check if
        // this part of the graph is valid.
        if !graph.roots.is_empty() && graph.get(&specifier).is_some() {
          let did_reload = inner
            .maybe_reload_dynamic(&graph, &specifier, permissions)
            .await
            .map_err(JsErrorBox::from_err)?;
          if did_reload {
            graph = inner.graph_container.graph();
          }

          log::debug!("Skipping prepare module load.");
          // roots are already validated so we can skip those
          if did_reload || !graph.roots.contains(&specifier) {
            module_load_preparer.graph_roots_valid(
              &graph,
              &[specifier],
              false,
              false,
            )?;
          }
          return Ok(());
        }
      }

      let is_dynamic = options.is_dynamic_import || inner.is_worker; // consider workers as dynamic for permissions
      let lib = inner.lib;
      let mut update_permit = graph_container.acquire_update_permit().await;
      let specifiers = &[specifier];
      {
        let graph = update_permit.graph_mut();
        module_load_preparer
          .prepare_module_load(
            graph,
            specifiers,
            PrepareModuleLoadOptions {
              is_dynamic,
              lib,
              permissions: permissions.clone(),
              ext_overwrite: None,
              allow_unknown_media_types: false,
              skip_graph_roots_validation: is_dynamic,
            },
          )
          .await
          .map_err(JsErrorBox::from_err)?;
        graph.prune_types();
        update_permit.commit();
        inner.shared.has_js_execution_started_flag.raise();
      }

      if is_dynamic {
        inner
          .maybe_reload_dynamic(
            &graph_container.graph(),
            &specifiers[0],
            permissions,
          )
          .await
          .map_err(JsErrorBox::from_err)?;
        // always validate the graph roots because we skipped doing it above
        module_load_preparer.graph_roots_valid(
          &graph_container.graph(),
          specifiers,
          false,
          false,
        )?;
      }

      Ok(())
    }
    .boxed_local()
  }

  fn finish_load(&self) {
    self
      .0
      .shared
      .in_flight_loads_tracker
      .decrease(&self.0.shared.parsed_source_cache);
  }

  fn code_cache_ready(
    &self,
    specifier: ModuleSpecifier,
    source_hash: u64,
    code_cache: &[u8],
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    if let Some(cache) = self.0.shared.code_cache.as_ref() {
      // This log line is also used by tests.
      log::debug!(
        "Updating V8 code cache for ES module: {specifier}, [{source_hash:?}]"
      );
      cache.set_sync(
        &specifier,
        code_cache::CodeCacheType::EsModule,
        source_hash,
        code_cache,
      );
    }
    std::future::ready(()).boxed_local()
  }

  fn get_source_map(&self, file_name: &str) -> Option<Cow<'_, [u8]>> {
    let specifier = resolve_url(file_name).ok()?;
    match specifier.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => return None,
    }

    // Load the prepared module and extract inline source map
    let graph = self.0.graph_container.graph();
    let source = self
      .0
      .shared
      .module_loader
      .load_prepared_module_for_source_map_sync(&graph, &specifier)
      .ok()??;
    source_map_from_code(source.source.as_bytes()).map(Cow::Owned)
  }

  fn load_external_source_map(
    &self,
    source_map_url: &str,
  ) -> Option<Cow<'_, [u8]>> {
    let specifier = resolve_url(source_map_url).ok()?;

    if let Ok(Some(file)) = self
      .0
      .shared
      .file_fetcher
      .get_cached_source_or_local(&specifier)
    {
      return Some(Cow::Owned(file.source.to_vec()));
    }

    None
  }

  // todo(dsherret): this method is actually only to determine whether
  // to show the filename in the stack traces so we should rename it
  // to something more clear that reflects that (since we skip checking
  // this for non-npm packages)
  fn source_map_source_exists(&self, source_url: &str) -> Option<bool> {
    let specifier = resolve_url(source_url).ok()?;

    // some npm packages rely on the file existing or not to end up in
    // the stack trace, so for backwards compat reasons only check this
    // for npm packages because we don't want the perf hit otherwise
    if self.0.shared.in_npm_pkg_checker.in_npm_package(&specifier)
      && let Ok(path) = deno_path_util::url_to_file_path(&specifier)
    {
      return Some(path.is_file());
    }

    Some(true)
  }

  fn get_source_mapped_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let specifier = resolve_url(file_name).ok()?;
    let graph = self.0.graph_container.graph();

    let code = match graph.get(&specifier) {
      Some(deno_graph::Module::Js(module)) => &module.source.text,
      Some(deno_graph::Module::Json(module)) => &module.source.text,
      Some(
        deno_graph::Module::Wasm(_)
        | deno_graph::Module::Npm(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::External(_),
      ) => {
        return None;
      }
      None => {
        // Not in graph, try to read from file system (for source-mapped original files)
        if let Ok(Some(file)) = self
          .0
          .shared
          .file_fetcher
          .get_cached_source_or_local(&specifier)
        {
          return extract_source_line(
            &String::from_utf8_lossy(&file.source),
            line_number,
          );
        } else {
          return None;
        }
      }
    };

    extract_source_line(code, line_number)
  }
}

/// Extracts a specific line from source code text.
fn extract_source_line(text: &str, line_number: usize) -> Option<String> {
  // Do NOT use .lines(): it skips the terminating empty line.
  // (due to internally using_terminator() instead of .split())
  match text.split('\n').nth(line_number) {
    Some(line) => Some(line.to_string()),
    None => Some(format!(
      "{} Couldn't format source line: Line {} is out of bounds (source may have changed at runtime)",
      crate::colors::yellow("Warning"),
      line_number + 1,
    )),
  }
}

/// Holds the `ModuleGraph` in workers.
#[derive(Clone)]
struct WorkerModuleGraphContainer {
  // Allow only one request to update the graph data at a time,
  // but allow other requests to read from it at any time even
  // while another request is updating the data.
  update_queue: Rc<deno_core::unsync::TaskQueue>,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
}

impl WorkerModuleGraphContainer {
  pub fn new(module_graph: Arc<ModuleGraph>) -> Self {
    Self {
      update_queue: Default::default(),
      inner: Rc::new(RefCell::new(module_graph)),
    }
  }
}

impl ModuleGraphContainer for WorkerModuleGraphContainer {
  async fn acquire_update_permit(&self) -> impl ModuleGraphUpdatePermit {
    let permit = self.update_queue.acquire().await;
    WorkerModuleGraphUpdatePermit {
      permit,
      inner: self.inner.clone(),
      graph: (**self.inner.borrow()).clone(),
    }
  }

  fn graph(&self) -> Arc<ModuleGraph> {
    self.inner.borrow().clone()
  }
}

struct WorkerModuleGraphUpdatePermit {
  permit: deno_core::unsync::TaskQueuePermit,
  inner: Rc<RefCell<Arc<ModuleGraph>>>,
  graph: ModuleGraph,
}

impl ModuleGraphUpdatePermit for WorkerModuleGraphUpdatePermit {
  fn graph_mut(&mut self) -> &mut ModuleGraph {
    &mut self.graph
  }

  fn commit(self) {
    *self.inner.borrow_mut() = Arc::new(self.graph);
    drop(self.permit); // explicit drop for clarity
  }
}

#[derive(Debug)]
struct CliNodeRequireLoader<TGraphContainer: ModuleGraphContainer> {
  cjs_tracker: Arc<CliCjsTracker>,
  emitter: Arc<CliEmitter>,
  memory_files: Arc<MemoryFiles>,
  npm_resolver: CliNpmResolver,
  sys: CliSys,
  graph_container: TGraphContainer,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  npm_registry_permission_checker:
    Arc<NpmRegistryReadPermissionChecker<CliSys>>,
}

impl<TGraphContainer: ModuleGraphContainer> NodeRequireLoader
  for CliNodeRequireLoader<TGraphContainer>
{
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut PermissionsContainer,
    path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, JsErrorBox> {
    if let Ok(url) = deno_path_util::url_from_file_path(&path) {
      // allow reading if it's in the module graph
      if self.graph_container.graph().get(&url).is_some() {
        return Ok(path);
      }
    }
    self
      .npm_registry_permission_checker
      .ensure_read_permission(permissions, path)
      .map_err(JsErrorBox::from_err)
  }

  fn load_text_file_lossy(
    &self,
    path: &Path,
  ) -> Result<FastString, JsErrorBox> {
    // todo(dsherret): use the preloaded module from the graph if available?
    let media_type = MediaType::from_path(path);
    let text_result = self.sys.fs_read_to_string_lossy(path);
    let text = match text_result {
      Ok(text) => text,
      Err(err) => {
        // only bother on error for performance reasons
        if let Some(file) = deno_path_util::url_from_file_path(path)
          .ok()
          .and_then(|s| self.memory_files.get(&s))
        {
          Cow::Owned(String::from_utf8_lossy(&file.source).into_owned())
        } else {
          return Err(JsErrorBox::from_err(err));
        }
      }
    };
    if media_type.is_emittable() {
      let specifier = deno_path_util::url_from_file_path(path)
        .map_err(JsErrorBox::from_err)?;
      if self.in_npm_pkg_checker.in_npm_package(&specifier) {
        return Err(JsErrorBox::from_err(StrippingTypesNodeModulesError {
          specifier,
        }));
      }
      let text = self
        .emitter
        .maybe_emit_source_sync(
          &specifier,
          media_type,
          // this is probably not super accurate due to require esm, but probably ok.
          // If we find this causes a lot of churn in the emit cache then we should
          // investigate how we can make this better
          ModuleKind::Cjs,
          &text.into(),
        )
        .map_err(JsErrorBox::from_err)?;
      Ok(text.into())
    } else {
      Ok(match text {
        Cow::Borrowed(s) => FastString::from_static(s),
        Cow::Owned(s) => s.into(),
      })
    }
  }

  fn is_maybe_cjs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<bool, PackageJsonLoadError> {
    let media_type = MediaType::from_specifier(specifier);
    self.cjs_tracker.is_maybe_cjs(specifier, media_type)
  }

  fn resolve_require_node_module_paths(&self, from: &Path) -> Vec<String> {
    let is_global_resolver_and_from_in_global_cache = self
      .npm_resolver
      .as_managed()
      .filter(|r| r.root_node_modules_path().is_none())
      .map(|r| r.global_cache_root_path())
      .filter(|global_cache_path| from.starts_with(global_cache_path))
      .is_some();
    if is_global_resolver_and_from_in_global_cache {
      Vec::new()
    } else {
      deno_runtime::deno_node::default_resolve_require_node_module_paths(from)
    }
  }
}

#[derive(Debug, Default)]
pub struct EszipModuleLoader {
  files: HashMap<ModuleSpecifier, Arc<[u8]>>,
}

impl EszipModuleLoader {
  pub async fn create(script: &str, cwd: &Path) -> Result<Self, AnyError> {
    // entrypoint#path1,path2,...
    let (_entrypoint, files) = script
      .split_once("#")
      .with_context(|| "eszip: invalid script string")?;

    // TODO: handle paths that contain ','
    let files = files.split(",").collect::<Vec<_>>();
    let mut loaded_eszips = FuturesOrdered::new();
    for path in files {
      let file = tokio::fs::File::open(path).await?;
      let eszip = BufReader::new(file.compat());
      let path = path.to_string();

      loaded_eszips.push_back(async move {
        let (eszip, loader) = EszipV2::parse(eszip)
          .await
          .with_context(|| format!("Error parsing eszip header at {}", path))?;
        loader
          .await
          .with_context(|| format!("Error loading eszip at {}", path))?;
        Ok(eszip)
      });
    }
    // At this point all eszips are fully loaded
    let loaded_eszips: Vec<Result<EszipV2, AnyError>> =
      loaded_eszips.collect::<Vec<_>>().await;

    let mut loader = Self::default();

    for loaded_eszip_result in loaded_eszips {
      let loaded_eszip = loaded_eszip_result?;
      let specifiers = loaded_eszip.specifiers();
      loader.files.reserve(specifiers.len());

      for specifier in specifiers {
        let module = loaded_eszip.get_module(&specifier).unwrap();
        let source = module.take_source().await.unwrap();
        let resolved_specifier = resolve_url_or_path(&specifier, cwd)?;
        let prev = loader.files.insert(resolved_specifier, source);
        assert!(prev.is_none());
      }
    }

    Ok(loader)
  }

  pub fn load_import_map_value(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<serde_json::Value, AnyError> {
    match self.files.get(specifier) {
      Some(bytes) => Ok(serde_json::from_slice(bytes.as_ref())?),
      None => bail!("Import map not found in eszip: {}", specifier),
    }
  }

  fn load(&self, specifier: &ModuleSpecifier) -> deno_core::ModuleLoadResponse {
    match self.files.get(specifier) {
      Some(source) => {
        let module_source = ModuleSource::new(
          ModuleType::JavaScript,
          ModuleSourceCode::Bytes(deno_core::ModuleCodeBytes::Arc(
            source.clone(),
          )),
          specifier,
          None,
        );
        deno_core::ModuleLoadResponse::Sync(Ok(module_source))
      }
      None => deno_core::ModuleLoadResponse::Sync(Err(JsErrorBox::generic(
        "Module not found",
      ))),
    }
  }
}

#[cfg(test)]
mod tests {
  use deno_graph::ast::ParsedSourceStore;

  use super::*;

  #[tokio::test]
  async fn test_inflight_module_loads_tracker() {
    let tracker = InFlightModuleLoadsTracker {
      loads_number: Default::default(),
      cleanup_task_timeout: 10,
      cleanup_task_handle: Default::default(),
    };

    let specifier = ModuleSpecifier::parse("file:///a.js").unwrap();
    let source = "const a = 'hello';";
    let parsed_source_cache = Arc::new(ParsedSourceCache::default());
    let parsed_source = parsed_source_cache
      .remove_or_parse_module(&specifier, MediaType::JavaScript, source.into())
      .unwrap();
    parsed_source_cache.set_parsed_source(specifier, parsed_source);

    assert_eq!(parsed_source_cache.len(), 1);
    assert!(tracker.cleanup_task_handle.lock().is_none());
    tracker.increase();
    tracker.increase();
    assert!(tracker.cleanup_task_handle.lock().is_none());
    tracker.decrease(&parsed_source_cache);
    assert!(tracker.cleanup_task_handle.lock().is_none());
    tracker.decrease(&parsed_source_cache);
    assert!(tracker.cleanup_task_handle.lock().is_some());
    assert_eq!(parsed_source_cache.len(), 1);
    tracker.increase();
    assert!(tracker.cleanup_task_handle.lock().is_none());
    assert_eq!(parsed_source_cache.len(), 1);
    tracker.decrease(&parsed_source_cache);
    // Rather long timeout, but to make sure CI is not flaking on it.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(parsed_source_cache.len(), 0);
  }
}
