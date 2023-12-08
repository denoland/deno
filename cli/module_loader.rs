// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::TsTypeLib;
use crate::cache::ModuleInfoCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::graph_valid_with_cli_options;
use crate::graph_util::workspace_config_to_workspace_members;
use crate::graph_util::DenoGraphFsAdapter;
use crate::graph_util::FileWatcherReporter;
use crate::graph_util::ModuleGraphBuilder;
use crate::graph_util::ModuleGraphContainer;
use crate::node;
use crate::node::CliNodeCodeTranslator;
use crate::npm::CliNpmResolver;
use crate::resolver::CliGraphResolver;
use crate::tools::check;
use crate::tools::check::TypeChecker;
use crate::util::progress_bar::ProgressBar;
use crate::util::text_encoding::code_without_source_map;
use crate::util::text_encoding::source_map_from_code;
use crate::worker::ModuleLoaderFactory;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::resolve_url_or_path;
use deno_core::ModuleCode;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceMapGetter;
use deno_graph::source::ResolutionMode;
use deno_graph::source::Resolver;
use deno_graph::EsmModule;
use deno_graph::JsonModule;
use deno_graph::Module;
use deno_graph::Resolution;
use deno_lockfile::Lockfile;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::NodeResolution;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

pub struct ModuleLoadPreparer {
  options: Arc<CliOptions>,
  fs: Arc<dyn deno_fs::FileSystem>,
  graph_container: Arc<ModuleGraphContainer>,
  lockfile: Option<Arc<Mutex<Lockfile>>>,
  maybe_file_watcher_reporter: Option<FileWatcherReporter>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  module_info_cache: Arc<ModuleInfoCache>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  progress_bar: ProgressBar,
  resolver: Arc<CliGraphResolver>,
  type_checker: Arc<TypeChecker>,
}

impl ModuleLoadPreparer {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: Arc<CliOptions>,
    fs: Arc<dyn deno_fs::FileSystem>,
    graph_container: Arc<ModuleGraphContainer>,
    lockfile: Option<Arc<Mutex<Lockfile>>>,
    maybe_file_watcher_reporter: Option<FileWatcherReporter>,
    module_graph_builder: Arc<ModuleGraphBuilder>,
    module_info_cache: Arc<ModuleInfoCache>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    progress_bar: ProgressBar,
    resolver: Arc<CliGraphResolver>,
    type_checker: Arc<TypeChecker>,
  ) -> Self {
    Self {
      options,
      fs,
      graph_container,
      lockfile,
      maybe_file_watcher_reporter,
      module_graph_builder,
      module_info_cache,
      parsed_source_cache,
      progress_bar,
      resolver,
      type_checker,
    }
  }

  /// This method must be called for a module or a static importer of that
  /// module before attempting to `load()` it from a `JsRuntime`. It will
  /// populate the graph data in memory with the necessary source code, write
  /// emits where necessary or report any module graph / type checking errors.
  #[allow(clippy::too_many_arguments)]
  pub async fn prepare_module_load(
    &self,
    roots: Vec<ModuleSpecifier>,
    is_dynamic: bool,
    lib: TsTypeLib,
    permissions: PermissionsContainer,
  ) -> Result<(), AnyError> {
    log::debug!("Preparing module load.");
    let _pb_clear_guard = self.progress_bar.clear_guard();

    let mut cache = self.module_graph_builder.create_fetch_cacher(permissions);
    let maybe_imports = self.options.to_maybe_imports()?;
    let maybe_workspace_config = self.options.maybe_workspace_config();
    let workspace_members = if let Some(wc) = maybe_workspace_config {
      workspace_config_to_workspace_members(wc)?
    } else {
      vec![]
    };
    let graph_resolver = self.resolver.as_graph_resolver();
    let graph_npm_resolver = self.resolver.as_graph_npm_resolver();
    let maybe_file_watcher_reporter = self
      .maybe_file_watcher_reporter
      .as_ref()
      .map(|r| r.as_reporter());

    let store = self.parsed_source_cache.as_store();
    let analyzer = self.module_info_cache.as_module_analyzer(None, &*store);

    log::debug!("Creating module graph.");
    let mut graph_update_permit =
      self.graph_container.acquire_update_permit().await;
    let graph = graph_update_permit.graph_mut();

    // Determine any modules that have already been emitted this session and
    // should be skipped.
    let reload_exclusions: HashSet<ModuleSpecifier> =
      graph.specifiers().map(|(s, _)| s.clone()).collect();

    self
      .module_graph_builder
      .build_graph_with_npm_resolution(
        graph,
        roots.clone(),
        &mut cache,
        deno_graph::BuildOptions {
          is_dynamic,
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

    graph_valid_with_cli_options(
      graph,
      self.fs.as_ref(),
      &roots,
      &self.options,
    )?;

    // If there is a lockfile...
    if let Some(lockfile) = &self.lockfile {
      let mut lockfile = lockfile.lock();
      // validate the integrity of all the modules
      graph_lock_or_exit(graph, &mut lockfile);
      // update it with anything new
      lockfile.write().context("Failed writing lockfile.")?;
    }

    // save the graph and get a reference to the new graph
    let graph = graph_update_permit.commit();

    drop(_pb_clear_guard);

    // type check if necessary
    if self.options.type_check_mode().is_true()
      && !self.graph_container.is_type_checked(&roots, lib)
    {
      let graph = Arc::new(graph.segment(&roots));
      self
        .type_checker
        .check(
          graph,
          check::CheckOptions {
            lib,
            log_ignored_options: false,
            reload: self.options.reload_flag()
              && !roots.iter().all(|r| reload_exclusions.contains(r)),
          },
        )
        .await?;
      self.graph_container.set_type_checked(&roots, lib);
    }

    log::debug!("Prepared module load.");

    Ok(())
  }

  /// Helper around prepare_module_load that loads and type checks
  /// the provided files.
  pub async fn load_and_type_check_files(
    &self,
    files: &[String],
  ) -> Result<(), AnyError> {
    let lib = self.options.ts_type_lib_window();

    let specifiers = files
      .iter()
      .map(|file| resolve_url_or_path(file, self.options.initial_cwd()))
      .collect::<Result<Vec<_>, _>>()?;
    self
      .prepare_module_load(
        specifiers,
        false,
        lib,
        PermissionsContainer::allow_all(),
      )
      .await
  }
}

pub struct ModuleCodeSource {
  pub code: ModuleCode,
  pub found_url: ModuleSpecifier,
  pub media_type: MediaType,
}

struct PreparedModuleLoader {
  emitter: Arc<Emitter>,
  graph_container: Arc<ModuleGraphContainer>,
  parsed_source_cache: Arc<ParsedSourceCache>,
}

impl PreparedModuleLoader {
  pub fn load_prepared_module(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
  ) -> Result<ModuleCodeSource, AnyError> {
    if specifier.scheme() == "node" {
      unreachable!(); // Node built-in modules should be handled internally.
    }

    let graph = self.graph_container.graph();
    match graph.get(specifier) {
      Some(deno_graph::Module::Json(JsonModule {
        source,
        media_type,
        specifier,
        ..
      })) => Ok(ModuleCodeSource {
        code: source.clone().into(),
        found_url: specifier.clone(),
        media_type: *media_type,
      }),
      Some(deno_graph::Module::Esm(EsmModule {
        source,
        media_type,
        specifier,
        ..
      })) => {
        let code: ModuleCode = match media_type {
          MediaType::JavaScript
          | MediaType::Unknown
          | MediaType::Cjs
          | MediaType::Mjs
          | MediaType::Json => source.clone().into(),
          MediaType::Dts | MediaType::Dcts | MediaType::Dmts => {
            Default::default()
          }
          MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Jsx
          | MediaType::Tsx => {
            // get emit text
            self
              .emitter
              .emit_parsed_source(specifier, *media_type, source)?
          }
          MediaType::TsBuildInfo | MediaType::Wasm | MediaType::SourceMap => {
            panic!("Unexpected media type {media_type} for {specifier}")
          }
        };

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(ModuleCodeSource {
          code,
          found_url: specifier.clone(),
          media_type: *media_type,
        })
      }
      _ => {
        let mut msg = format!("Loading unprepared module: {specifier}");
        if let Some(referrer) = maybe_referrer {
          msg = format!("{}, imported from: {}", msg, referrer.as_str());
        }
        Err(anyhow!(msg))
      }
    }
  }
}

struct SharedCliModuleLoaderState {
  lib_window: TsTypeLib,
  lib_worker: TsTypeLib,
  is_inspecting: bool,
  is_repl: bool,
  graph_container: Arc<ModuleGraphContainer>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  prepared_module_loader: PreparedModuleLoader,
  resolver: Arc<CliGraphResolver>,
  node_resolver: Arc<CliNodeResolver>,
  npm_module_loader: NpmModuleLoader,
}

pub struct CliModuleLoaderFactory {
  shared: Arc<SharedCliModuleLoaderState>,
}

impl CliModuleLoaderFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: &CliOptions,
    emitter: Arc<Emitter>,
    graph_container: Arc<ModuleGraphContainer>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
    parsed_source_cache: Arc<ParsedSourceCache>,
    resolver: Arc<CliGraphResolver>,
    node_resolver: Arc<CliNodeResolver>,
    npm_module_loader: NpmModuleLoader,
  ) -> Self {
    Self {
      shared: Arc::new(SharedCliModuleLoaderState {
        lib_window: options.ts_type_lib_window(),
        lib_worker: options.ts_type_lib_worker(),
        is_inspecting: options.is_inspecting(),
        is_repl: matches!(
          options.sub_command(),
          DenoSubcommand::Repl(_) | DenoSubcommand::Jupyter(_)
        ),
        prepared_module_loader: PreparedModuleLoader {
          emitter,
          graph_container: graph_container.clone(),
          parsed_source_cache,
        },
        graph_container,
        module_load_preparer,
        resolver,
        node_resolver,
        npm_module_loader,
      }),
    }
  }

  fn create_with_lib(
    &self,
    lib: TsTypeLib,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    Rc::new(CliModuleLoader {
      lib,
      root_permissions,
      dynamic_permissions,
      shared: self.shared.clone(),
    })
  }
}

impl ModuleLoaderFactory for CliModuleLoaderFactory {
  fn create_for_main(
    &self,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    self.create_with_lib(
      self.shared.lib_window,
      root_permissions,
      dynamic_permissions,
    )
  }

  fn create_for_worker(
    &self,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> Rc<dyn ModuleLoader> {
    self.create_with_lib(
      self.shared.lib_worker,
      root_permissions,
      dynamic_permissions,
    )
  }

  fn create_source_map_getter(&self) -> Option<Box<dyn SourceMapGetter>> {
    Some(Box::new(CliSourceMapGetter {
      shared: self.shared.clone(),
    }))
  }
}

struct CliModuleLoader {
  lib: TsTypeLib,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. These are "allow all" for main worker, and parent thread
  /// permissions for Web Worker.
  root_permissions: PermissionsContainer,
  /// Permissions used to resolve dynamic imports, these get passed as
  /// "root permissions" for Web Worker.
  dynamic_permissions: PermissionsContainer,
  shared: Arc<SharedCliModuleLoaderState>,
}

impl CliModuleLoader {
  fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Result<ModuleSource, AnyError> {
    let permissions = if is_dynamic {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };
    let code_source = if let Some(result) = self
      .shared
      .npm_module_loader
      .load_sync_if_in_npm_package(specifier, maybe_referrer, permissions)
    {
      result?
    } else {
      self
        .shared
        .prepared_module_loader
        .load_prepared_module(specifier, maybe_referrer)?
    };
    let code = if self.shared.is_inspecting {
      // we need the code with the source map in order for
      // it to work with --inspect or --inspect-brk
      code_source.code
    } else {
      // reduce memory and throw away the source map
      // because we don't need it
      code_without_source_map(code_source.code)
    };
    Ok(ModuleSource::new_with_redirect(
      match code_source.media_type {
        MediaType::Json => ModuleType::Json,
        _ => ModuleType::JavaScript,
      },
      code,
      specifier,
      &code_source.found_url,
    ))
  }
}

impl ModuleLoader for CliModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    let permissions = if matches!(kind, ResolutionKind::DynamicImport) {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    // TODO(bartlomieju): ideally we shouldn't need to call `current_dir()` on each
    // call - maybe it should be caller's responsibility to pass it as an arg?
    let cwd = std::env::current_dir().context("Unable to get CWD")?;
    let referrer_result = deno_core::resolve_url_or_path(referrer, &cwd);

    if let Ok(referrer) = referrer_result.as_ref() {
      if let Some(result) = self.shared.node_resolver.resolve_if_in_npm_package(
        specifier,
        referrer,
        permissions,
      ) {
        return result;
      }

      let graph = self.shared.graph_container.graph();
      let maybe_resolved = match graph.get(referrer) {
        Some(Module::Esm(module)) => {
          module.dependencies.get(specifier).map(|d| &d.maybe_code)
        }
        _ => None,
      };

      match maybe_resolved {
        Some(Resolution::Ok(resolved)) => {
          let specifier = &resolved.specifier;

          return match graph.get(specifier) {
            Some(Module::Npm(module)) => {
              let package_folder = self
                .shared
                .node_resolver
                .npm_resolver
                .as_managed()
                .unwrap() // byonm won't create a Module::Npm
                .resolve_pkg_folder_from_deno_module(
                  module.nv_reference.nv(),
                )?;
              self
                .shared
                .node_resolver
                .resolve_package_sub_path(
                  &package_folder,
                  module.nv_reference.sub_path(),
                  referrer,
                  permissions,
                )
                .with_context(|| {
                  format!("Could not resolve '{}'.", module.nv_reference)
                })
            }
            Some(Module::Node(module)) => Ok(module.specifier.clone()),
            Some(Module::Esm(module)) => Ok(module.specifier.clone()),
            Some(Module::Json(module)) => Ok(module.specifier.clone()),
            Some(Module::External(module)) => {
              Ok(node::resolve_specifier_into_node_modules(&module.specifier))
            }
            None => Ok(specifier.clone()),
          };
        }
        Some(Resolution::Err(err)) => {
          return Err(custom_error(
            "TypeError",
            format!("{}\n", err.to_string_with_range()),
          ))
        }
        Some(Resolution::None) | None => {}
      }
    }

    // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
    // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
    // but sadly that's not the case due to missing APIs in V8.
    let referrer = if referrer.is_empty() && self.shared.is_repl {
      deno_core::resolve_path("./$deno$repl.ts", &cwd)?
    } else {
      referrer_result?
    };

    // FIXME(bartlomieju): this is another hack way to provide NPM specifier
    // support in REPL. This should be fixed.
    let resolution = self.shared.resolver.resolve(
      specifier,
      &deno_graph::Range {
        specifier: referrer.clone(),
        start: deno_graph::Position::zeroed(),
        end: deno_graph::Position::zeroed(),
      },
      ResolutionMode::Execution,
    );

    if self.shared.is_repl {
      let specifier = resolution
        .as_ref()
        .ok()
        .map(Cow::Borrowed)
        .or_else(|| ModuleSpecifier::parse(specifier).ok().map(Cow::Owned));
      if let Some(specifier) = specifier {
        if let Ok(reference) =
          NpmPackageReqReference::from_specifier(&specifier)
        {
          return self.shared.node_resolver.resolve_req_reference(
            &reference,
            permissions,
            &referrer,
          );
        }
      }
    }

    resolution.map_err(|err| err.into())
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
  ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
    // NOTE: this block is async only because of `deno_core` interface
    // requirements; module was already loaded when constructing module graph
    // during call to `prepare_load` so we can load it synchronously.
    Box::pin(deno_core::futures::future::ready(self.load_sync(
      specifier,
      maybe_referrer,
      is_dynamic,
    )))
  }

  fn prepare_load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    if let Some(result) =
      self.shared.npm_module_loader.maybe_prepare_load(specifier)
    {
      return Box::pin(deno_core::futures::future::ready(result));
    }

    let specifier = specifier.clone();
    let module_load_preparer = self.shared.module_load_preparer.clone();

    let root_permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let lib = self.lib;

    async move {
      module_load_preparer
        .prepare_module_load(vec![specifier], is_dynamic, lib, root_permissions)
        .await
    }
    .boxed_local()
  }
}

struct CliSourceMapGetter {
  shared: Arc<SharedCliModuleLoaderState>,
}

impl SourceMapGetter for CliSourceMapGetter {
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    let specifier = resolve_url(file_name).ok()?;
    match specifier.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => return None,
    }
    let source = self
      .shared
      .prepared_module_loader
      .load_prepared_module(&specifier, None)
      .ok()?;
    source_map_from_code(&source.code)
  }

  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let graph = self.shared.graph_container.graph();
    let code = match graph.get(&resolve_url(file_name).ok()?) {
      Some(deno_graph::Module::Esm(module)) => &module.source,
      Some(deno_graph::Module::Json(module)) => &module.source,
      _ => return None,
    };
    // Do NOT use .lines(): it skips the terminating empty line.
    // (due to internally using_terminator() instead of .split())
    let lines: Vec<&str> = code.split('\n').collect();
    if line_number >= lines.len() {
      Some(format!(
        "{} Couldn't format source line: Line {} is out of bounds (source may have changed at runtime)",
        crate::colors::yellow("Warning"), line_number + 1,
      ))
    } else {
      Some(lines[line_number].to_string())
    }
  }
}

pub struct CliNodeResolver {
  cjs_resolutions: Arc<CjsResolutionStore>,
  node_resolver: Arc<NodeResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
}

impl CliNodeResolver {
  pub fn new(
    cjs_resolutions: Arc<CjsResolutionStore>,
    node_resolver: Arc<NodeResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
  ) -> Self {
    Self {
      cjs_resolutions,
      node_resolver,
      npm_resolver,
    }
  }

  pub fn in_npm_package(&self, referrer: &ModuleSpecifier) -> bool {
    self.npm_resolver.in_npm_package(referrer)
  }

  pub fn resolve_if_in_npm_package(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    permissions: &PermissionsContainer,
  ) -> Option<Result<ModuleSpecifier, AnyError>> {
    if self.in_npm_package(referrer) {
      // we're in an npm package, so use node resolution
      Some(
        self
          .handle_node_resolve_result(self.node_resolver.resolve(
            specifier,
            referrer,
            NodeResolutionMode::Execution,
            permissions,
          ))
          .with_context(|| {
            format!("Could not resolve '{specifier}' from '{referrer}'.")
          }),
      )
    } else {
      None
    }
  }

  pub fn resolve_req_reference(
    &self,
    req_ref: &NpmPackageReqReference,
    permissions: &PermissionsContainer,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    let package_folder = self
      .npm_resolver
      .resolve_pkg_folder_from_deno_module_req(req_ref.req(), referrer)?;
    self
      .resolve_package_sub_path(
        &package_folder,
        req_ref.sub_path(),
        referrer,
        permissions,
      )
      .with_context(|| format!("Could not resolve '{}'.", req_ref))
  }

  pub fn resolve_package_sub_path(
    &self,
    package_folder: &Path,
    sub_path: Option<&str>,
    referrer: &ModuleSpecifier,
    permissions: &PermissionsContainer,
  ) -> Result<ModuleSpecifier, AnyError> {
    self.handle_node_resolve_result(
      self.node_resolver.resolve_package_subpath_from_deno_module(
        package_folder,
        sub_path,
        referrer,
        NodeResolutionMode::Execution,
        permissions,
      ),
    )
  }

  fn handle_node_resolve_result(
    &self,
    result: Result<Option<NodeResolution>, AnyError>,
  ) -> Result<ModuleSpecifier, AnyError> {
    let response = match result? {
      Some(response) => response,
      None => return Err(generic_error("not found")),
    };
    if let NodeResolution::CommonJs(specifier) = &response {
      // remember that this was a common js resolution
      self.cjs_resolutions.insert(specifier.clone());
    }
    Ok(response.into_url())
  }
}

pub struct NpmModuleLoader {
  cjs_resolutions: Arc<CjsResolutionStore>,
  node_code_translator: Arc<CliNodeCodeTranslator>,
  fs: Arc<dyn deno_fs::FileSystem>,
  node_resolver: Arc<CliNodeResolver>,
}

impl NpmModuleLoader {
  pub fn new(
    cjs_resolutions: Arc<CjsResolutionStore>,
    node_code_translator: Arc<CliNodeCodeTranslator>,
    fs: Arc<dyn deno_fs::FileSystem>,
    node_resolver: Arc<CliNodeResolver>,
  ) -> Self {
    Self {
      cjs_resolutions,
      node_code_translator,
      fs,
      node_resolver,
    }
  }

  pub fn maybe_prepare_load(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Result<(), AnyError>> {
    if self.node_resolver.in_npm_package(specifier) {
      // nothing to prepare
      Some(Ok(()))
    } else {
      None
    }
  }

  pub fn load_sync_if_in_npm_package(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    permissions: &PermissionsContainer,
  ) -> Option<Result<ModuleCodeSource, AnyError>> {
    if self.node_resolver.in_npm_package(specifier) {
      Some(self.load_sync(specifier, maybe_referrer, permissions))
    } else {
      None
    }
  }

  fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    permissions: &PermissionsContainer,
  ) -> Result<ModuleCodeSource, AnyError> {
    let file_path = specifier.to_file_path().unwrap();
    let code = self
      .fs
      .read_text_file_sync(&file_path)
      .map_err(AnyError::from)
      .with_context(|| {
        if file_path.is_dir() {
          // directory imports are not allowed when importing from an
          // ES module, so provide the user with a helpful error message
          let dir_path = file_path;
          let mut msg = "Directory import ".to_string();
          msg.push_str(&dir_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" is not supported resolving import from ");
            msg.push_str(referrer.as_str());
            let entrypoint_name = ["index.mjs", "index.js", "index.cjs"]
              .iter()
              .find(|e| dir_path.join(e).is_file());
            if let Some(entrypoint_name) = entrypoint_name {
              msg.push_str("\nDid you mean to import ");
              msg.push_str(entrypoint_name);
              msg.push_str(" within the directory?");
            }
          }
          msg
        } else {
          let mut msg = "Unable to load ".to_string();
          msg.push_str(&file_path.to_string_lossy());
          if let Some(referrer) = &maybe_referrer {
            msg.push_str(" imported from ");
            msg.push_str(referrer.as_str());
          }
          msg
        }
      })?;

    let code = if self.cjs_resolutions.contains(specifier) {
      // translate cjs to esm if it's cjs and inject node globals
      self.node_code_translator.translate_cjs_to_esm(
        specifier,
        Some(code.as_str()),
        permissions,
      )?
    } else {
      // esm and json code is untouched
      code
    };
    Ok(ModuleCodeSource {
      code: code.into(),
      found_url: specifier.clone(),
      media_type: MediaType::from_specifier(specifier),
    })
  }
}

/// Keeps track of what module specifiers were resolved as CJS.
#[derive(Debug, Default)]
pub struct CjsResolutionStore(Mutex<HashSet<ModuleSpecifier>>);

impl CjsResolutionStore {
  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().contains(specifier)
  }

  pub fn insert(&self, specifier: ModuleSpecifier) {
    self.0.lock().insert(specifier);
  }
}
