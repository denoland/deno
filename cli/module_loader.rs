// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

use crate::args::jsr_url;
use crate::args::CliLockfile;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::TsTypeLib;
use crate::cache::CodeCache;
use crate::cache::FastInsecureHasher;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::CreateGraphOptions;
use crate::graph_util::ModuleGraphBuilder;
use crate::node;
use crate::node::CliNodeCodeTranslator;
use crate::npm::CliNpmResolver;
use crate::resolver::CjsTracker;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::resolver::ModuleCodeStringSource;
use crate::resolver::NotSupportedKindInNpmError;
use crate::resolver::NpmModuleLoader;
use crate::tools::check;
use crate::tools::check::TypeChecker;
use crate::util::progress_bar::ProgressBar;
use crate::util::text_encoding::code_without_source_map;
use crate::util::text_encoding::source_map_from_code;
use crate::worker::CreateModuleLoaderResult;
use crate::worker::ModuleLoaderFactory;
use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::resolve_url;
use deno_core::ModuleCodeString;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::SourceCodeCacheInfo;
use deno_graph::source::ResolutionMode;
use deno_graph::GraphKind;
use deno_graph::JsModule;
use deno_graph::JsonModule;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Resolution;
use deno_runtime::code_cache;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::create_host_defined_options;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NodeResolutionMode;

pub struct ModuleLoadPreparer {
  options: Arc<CliOptions>,
  lockfile: Option<Arc<CliLockfile>>,
  module_graph_builder: Arc<ModuleGraphBuilder>,
  progress_bar: ProgressBar,
  type_checker: Arc<TypeChecker>,
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
  #[allow(clippy::too_many_arguments)]
  pub async fn prepare_module_load(
    &self,
    graph: &mut ModuleGraph,
    roots: &[ModuleSpecifier],
    is_dynamic: bool,
    lib: TsTypeLib,
    permissions: PermissionsContainer,
    ext_overwrite: Option<&String>,
  ) -> Result<(), AnyError> {
    log::debug!("Preparing module load.");
    let _pb_clear_guard = self.progress_bar.clear_guard();

    let mut cache = self.module_graph_builder.create_fetch_cacher(permissions);
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
          cache.file_header_overrides.insert(
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
        CreateGraphOptions {
          is_dynamic,
          graph_kind: graph.graph_kind(),
          roots: roots.to_vec(),
          loader: Some(&mut cache),
        },
      )
      .await?;

    self.graph_roots_valid(graph, roots)?;

    // write the lockfile if there is one
    if let Some(lockfile) = &self.lockfile {
      lockfile.write_if_changed()?;
    }

    drop(_pb_clear_guard);

    // type check if necessary
    if self.options.type_check_mode().is_true() && !has_type_checked {
      self
        .type_checker
        .check(
          // todo(perf): since this is only done the first time the graph is
          // created, we could avoid the clone of the graph here by providing
          // the actual graph on the first run and then getting the Arc<ModuleGraph>
          // back from the return value.
          graph.clone(),
          check::CheckOptions {
            build_fast_check_graph: true,
            lib,
            log_ignored_options: false,
            reload: self.options.reload_flag(),
            type_check_mode: self.options.type_check_mode(),
          },
        )
        .await?;
    }

    log::debug!("Prepared module load.");

    Ok(())
  }

  pub fn graph_roots_valid(
    &self,
    graph: &ModuleGraph,
    roots: &[ModuleSpecifier],
  ) -> Result<(), AnyError> {
    self.module_graph_builder.graph_roots_valid(graph, roots)
  }
}

struct SharedCliModuleLoaderState {
  graph_kind: GraphKind,
  lib_window: TsTypeLib,
  lib_worker: TsTypeLib,
  initial_cwd: PathBuf,
  is_inspecting: bool,
  is_repl: bool,
  cjs_tracker: Arc<CjsTracker>,
  code_cache: Option<Arc<CodeCache>>,
  emitter: Arc<Emitter>,
  fs: Arc<dyn FileSystem>,
  in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
  main_module_graph_container: Arc<MainModuleGraphContainer>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  node_code_translator: Arc<CliNodeCodeTranslator>,
  node_resolver: Arc<NodeResolver>,
  npm_req_resolver: Arc<CliNpmReqResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  npm_module_loader: NpmModuleLoader,
  parsed_source_cache: Arc<ParsedSourceCache>,
  resolver: Arc<CliResolver>,
}

pub struct CliModuleLoaderFactory {
  shared: Arc<SharedCliModuleLoaderState>,
}

impl CliModuleLoaderFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: &CliOptions,
    cjs_tracker: Arc<CjsTracker>,
    code_cache: Option<Arc<CodeCache>>,
    emitter: Arc<Emitter>,
    fs: Arc<dyn FileSystem>,
    in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
    main_module_graph_container: Arc<MainModuleGraphContainer>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
    node_code_translator: Arc<CliNodeCodeTranslator>,
    node_resolver: Arc<NodeResolver>,
    npm_req_resolver: Arc<CliNpmReqResolver>,
    npm_resolver: Arc<dyn CliNpmResolver>,
    npm_module_loader: NpmModuleLoader,
    parsed_source_cache: Arc<ParsedSourceCache>,
    resolver: Arc<CliResolver>,
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
        fs,
        in_npm_pkg_checker,
        main_module_graph_container,
        module_load_preparer,
        node_code_translator,
        node_resolver,
        npm_req_resolver,
        npm_resolver,
        npm_module_loader,
        parsed_source_cache,
        resolver,
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
        node_code_translator: self.shared.node_code_translator.clone(),
        emitter: self.shared.emitter.clone(),
        parsed_source_cache: self.shared.parsed_source_cache.clone(),
        shared: self.shared.clone(),
      })));
    let node_require_loader = Rc::new(CliNodeRequireLoader {
      cjs_tracker: self.shared.cjs_tracker.clone(),
      emitter: self.shared.emitter.clone(),
      fs: self.shared.fs.clone(),
      graph_container,
      in_npm_pkg_checker: self.shared.in_npm_pkg_checker.clone(),
      npm_resolver: self.shared.npm_resolver.clone(),
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

struct CliModuleLoaderInner<TGraphContainer: ModuleGraphContainer> {
  lib: TsTypeLib,
  is_worker: bool,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. These are "allow all" for main worker, and parent thread
  /// permissions for Web Worker.
  parent_permissions: PermissionsContainer,
  permissions: PermissionsContainer,
  shared: Arc<SharedCliModuleLoaderState>,
  emitter: Arc<Emitter>,
  node_code_translator: Arc<CliNodeCodeTranslator>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  graph_container: TGraphContainer,
}

impl<TGraphContainer: ModuleGraphContainer>
  CliModuleLoaderInner<TGraphContainer>
{
  async fn load_inner(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    requested_module_type: RequestedModuleType,
  ) -> Result<ModuleSource, AnyError> {
    let code_source = self.load_code_source(specifier, maybe_referrer).await?;
    let code = if self.shared.is_inspecting {
      // we need the code with the source map in order for
      // it to work with --inspect or --inspect-brk
      code_source.code
    } else {
      // v8 is slower when source maps are present, so we strip them
      code_without_source_map(code_source.code)
    };
    let module_type = match code_source.media_type {
      MediaType::Json => ModuleType::Json,
      _ => ModuleType::JavaScript,
    };

    // If we loaded a JSON file, but the "requested_module_type" (that is computed from
    // import attributes) is not JSON we need to fail.
    if module_type == ModuleType::Json
      && requested_module_type != RequestedModuleType::Json
    {
      return Err(generic_error("Attempted to load JSON module without specifying \"type\": \"json\" attribute in the import statement."));
    }

    let code_cache = if module_type == ModuleType::JavaScript {
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
      module_type,
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
  ) -> Result<ModuleCodeStringSource, AnyError> {
    if let Some(code_source) = self.load_prepared_module(specifier).await? {
      return Ok(code_source);
    }
    if self.shared.in_npm_pkg_checker.in_npm_package(specifier) {
      return self
        .shared
        .npm_module_loader
        .load(specifier, maybe_referrer)
        .await;
    }

    let mut msg = format!("Loading unprepared module: {specifier}");
    if let Some(referrer) = maybe_referrer {
      msg = format!("{}, imported from: {}", msg, referrer.as_str());
    }
    Err(anyhow!(msg))
  }

  fn resolve_referrer(
    &self,
    referrer: &str,
  ) -> Result<ModuleSpecifier, AnyError> {
    let referrer = if referrer.is_empty() && self.shared.is_repl {
      // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
      // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
      "./$deno$repl.mts"
    } else {
      referrer
    };

    if deno_core::specifier_has_uri_scheme(referrer) {
      deno_core::resolve_url(referrer).map_err(|e| e.into())
    } else if referrer == "." {
      // main module, use the initial cwd
      deno_core::resolve_path(referrer, &self.shared.initial_cwd)
        .map_err(|e| e.into())
    } else {
      // this cwd check is slow, so try to avoid it
      let cwd = std::env::current_dir().context("Unable to get CWD")?;
      deno_core::resolve_path(referrer, &cwd).map_err(|e| e.into())
    }
  }

  fn inner_resolve(
    &self,
    raw_specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<ModuleSpecifier, AnyError> {
    let graph = self.graph_container.graph();
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
        return Err(custom_error(
          "TypeError",
          format!("{}\n", err.to_string_with_range()),
        ));
      }
      Resolution::None => Cow::Owned(self.shared.resolver.resolve(
        raw_specifier,
        &deno_graph::Range {
          specifier: referrer.clone(),
          start: deno_graph::Position::zeroed(),
          end: deno_graph::Position::zeroed(),
        },
        self.shared.cjs_tracker.get_referrer_kind(referrer),
        ResolutionMode::Execution,
      )?),
    };

    if self.shared.is_repl {
      if let Ok(reference) = NpmPackageReqReference::from_specifier(&specifier)
      {
        return self
          .shared
          .npm_req_resolver
          .resolve_req_reference(
            &reference,
            referrer,
            self.shared.cjs_tracker.get_referrer_kind(referrer),
            NodeResolutionMode::Execution,
          )
          .map_err(AnyError::from);
      }
    }

    let specifier = match graph.get(&specifier) {
      Some(Module::Npm(module)) => {
        let package_folder = self
          .shared
          .npm_resolver
          .as_managed()
          .unwrap() // byonm won't create a Module::Npm
          .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
        self
          .shared
          .node_resolver
          .resolve_package_subpath_from_deno_module(
            &package_folder,
            module.nv_reference.sub_path(),
            Some(referrer),
            self.shared.cjs_tracker.get_referrer_kind(referrer),
            NodeResolutionMode::Execution,
          )
          .with_context(|| {
            format!("Could not resolve '{}'.", module.nv_reference)
          })?
      }
      Some(Module::Node(module)) => module.specifier.clone(),
      Some(Module::Js(module)) => module.specifier.clone(),
      Some(Module::Json(module)) => module.specifier.clone(),
      Some(Module::External(module)) => {
        node::resolve_specifier_into_node_modules(
          &module.specifier,
          self.shared.fs.as_ref(),
        )
      }
      None => specifier.into_owned(),
      Some(Module::Wasm(_)) => todo!("@dsherret"),
    };
    Ok(specifier)
  }

  async fn load_prepared_module(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<ModuleCodeStringSource>, AnyError> {
    // Note: keep this in sync with the sync version below
    let graph = self.graph_container.graph();
    match self.load_prepared_module_or_defer_emit(&graph, specifier)? {
      Some(CodeOrDeferredEmit::Code(code_source)) => Ok(Some(code_source)),
      Some(CodeOrDeferredEmit::DeferredEmit {
        specifier,
        media_type,
        source,
      }) => {
        let transpile_result = self
          .emitter
          .emit_parsed_source(specifier, media_type, ModuleKind::Esm, source)
          .await?;

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(ModuleCodeStringSource {
          // note: it's faster to provide a string if we know it's a string
          code: ModuleSourceCode::String(transpile_result.into()),
          found_url: specifier.clone(),
          media_type,
        }))
      }
      Some(CodeOrDeferredEmit::Cjs {
        specifier,
        media_type,
        source,
      }) => self
        .load_maybe_cjs(specifier, media_type, source)
        .await
        .map(Some),
      None => Ok(None),
    }
  }

  fn load_prepared_module_for_source_map_sync(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<ModuleCodeStringSource>, AnyError> {
    // Note: keep this in sync with the async version above
    let graph = self.graph_container.graph();
    match self.load_prepared_module_or_defer_emit(&graph, specifier)? {
      Some(CodeOrDeferredEmit::Code(code_source)) => Ok(Some(code_source)),
      Some(CodeOrDeferredEmit::DeferredEmit {
        specifier,
        media_type,
        source,
      }) => {
        let transpile_result = self.emitter.emit_parsed_source_sync(
          specifier,
          media_type,
          ModuleKind::Esm,
          source,
        )?;

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(ModuleCodeStringSource {
          // note: it's faster to provide a string if we know it's a string
          code: ModuleSourceCode::String(transpile_result.into()),
          found_url: specifier.clone(),
          media_type,
        }))
      }
      Some(CodeOrDeferredEmit::Cjs { .. }) => {
        self.parsed_source_cache.free(specifier);

        // todo(dsherret): to make this work, we should probably just
        // rely on the CJS export cache. At the moment this is hard because
        // cjs export analysis is only async
        Ok(None)
      }
      None => Ok(None),
    }
  }

  fn load_prepared_module_or_defer_emit<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<CodeOrDeferredEmit<'graph>>, AnyError> {
    if specifier.scheme() == "node" {
      // Node built-in modules should be handled internally.
      unreachable!("Deno bug. {} was misconfigured internally.", specifier);
    }

    match graph.get(specifier) {
      Some(deno_graph::Module::Json(JsonModule {
        source,
        media_type,
        specifier,
        ..
      })) => Ok(Some(CodeOrDeferredEmit::Code(ModuleCodeStringSource {
        code: ModuleSourceCode::String(source.clone().into()),
        found_url: specifier.clone(),
        media_type: *media_type,
      }))),
      Some(deno_graph::Module::Js(JsModule {
        source,
        media_type,
        specifier,
        is_script,
        ..
      })) => {
        if self.shared.cjs_tracker.is_cjs_with_known_is_script(
          specifier,
          *media_type,
          *is_script,
        )? {
          return Ok(Some(CodeOrDeferredEmit::Cjs {
            specifier,
            media_type: *media_type,
            source,
          }));
        }
        let code: ModuleCodeString = match media_type {
          MediaType::JavaScript
          | MediaType::Unknown
          | MediaType::Mjs
          | MediaType::Json => source.clone().into(),
          MediaType::Dts | MediaType::Dcts | MediaType::Dmts => {
            Default::default()
          }
          MediaType::Cjs | MediaType::Cts => {
            return Ok(Some(CodeOrDeferredEmit::Cjs {
              specifier,
              media_type: *media_type,
              source,
            }));
          }
          MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Jsx
          | MediaType::Tsx => {
            return Ok(Some(CodeOrDeferredEmit::DeferredEmit {
              specifier,
              media_type: *media_type,
              source,
            }));
          }
          MediaType::Css | MediaType::Wasm | MediaType::SourceMap => {
            panic!("Unexpected media type {media_type} for {specifier}")
          }
        };

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(CodeOrDeferredEmit::Code(ModuleCodeStringSource {
          code: ModuleSourceCode::String(code),
          found_url: specifier.clone(),
          media_type: *media_type,
        })))
      }
      Some(
        deno_graph::Module::External(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::Npm(_),
      )
      | None => Ok(None),
      Some(deno_graph::Module::Wasm(_)) => todo!("@dsherret"),
    }
  }

  async fn load_maybe_cjs(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    original_source: &Arc<str>,
  ) -> Result<ModuleCodeStringSource, AnyError> {
    let js_source = if media_type.is_emittable() {
      Cow::Owned(
        self
          .emitter
          .emit_parsed_source(
            specifier,
            media_type,
            ModuleKind::Cjs,
            original_source,
          )
          .await?,
      )
    } else {
      Cow::Borrowed(original_source.as_ref())
    };
    let text = self
      .node_code_translator
      .translate_cjs_to_esm(specifier, Some(js_source))
      .await?;
    // at this point, we no longer need the parsed source in memory, so free it
    self.parsed_source_cache.free(specifier);
    Ok(ModuleCodeStringSource {
      code: match text {
        // perf: if the text is borrowed, that means it didn't make any changes
        // to the original source, so we can just provide that instead of cloning
        // the borrowed text
        Cow::Borrowed(_) => {
          ModuleSourceCode::String(original_source.clone().into())
        }
        Cow::Owned(text) => ModuleSourceCode::String(text.into()),
      },
      found_url: specifier.clone(),
      media_type,
    })
  }
}

enum CodeOrDeferredEmit<'a> {
  Code(ModuleCodeStringSource),
  DeferredEmit {
    specifier: &'a ModuleSpecifier,
    media_type: MediaType,
    source: &'a Arc<str>,
  },
  Cjs {
    specifier: &'a ModuleSpecifier,
    media_type: MediaType,
    source: &'a Arc<str>,
  },
}

// todo(dsherret): this double Rc boxing is not ideal
struct CliModuleLoader<TGraphContainer: ModuleGraphContainer>(
  Rc<CliModuleLoaderInner<TGraphContainer>>,
);

impl<TGraphContainer: ModuleGraphContainer> ModuleLoader
  for CliModuleLoader<TGraphContainer>
{
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    fn ensure_not_jsr_non_jsr_remote_import(
      specifier: &ModuleSpecifier,
      referrer: &ModuleSpecifier,
    ) -> Result<(), AnyError> {
      if referrer.as_str().starts_with(jsr_url().as_str())
        && !specifier.as_str().starts_with(jsr_url().as_str())
        && matches!(specifier.scheme(), "http" | "https")
      {
        bail!("Importing {} blocked. JSR packages cannot import non-JSR remote modules for security reasons.", specifier);
      }
      Ok(())
    }

    let referrer = self.0.resolve_referrer(referrer)?;
    let specifier = self.0.inner_resolve(specifier, &referrer)?;
    ensure_not_jsr_non_jsr_remote_import(&specifier, &referrer)?;
    Ok(specifier)
  }

  fn get_host_defined_options<'s>(
    &self,
    scope: &mut deno_core::v8::HandleScope<'s>,
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
    maybe_referrer: Option<&ModuleSpecifier>,
    _is_dynamic: bool,
    requested_module_type: RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    let inner = self.0.clone();
    let specifier = specifier.clone();
    let maybe_referrer = maybe_referrer.cloned();
    deno_core::ModuleLoadResponse::Async(
      async move {
        inner
          .load_inner(
            &specifier,
            maybe_referrer.as_ref(),
            requested_module_type,
          )
          .await
      }
      .boxed_local(),
    )
  }

  fn prepare_load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    if self.0.shared.in_npm_pkg_checker.in_npm_package(specifier) {
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    let specifier = specifier.clone();
    let inner = self.0.clone();

    async move {
      let graph_container = &inner.graph_container;
      let module_load_preparer = &inner.shared.module_load_preparer;

      if is_dynamic {
        // When the specifier is already in the graph then it means it
        // was previously loaded, so we can skip that and only check if
        // this part of the graph is valid.
        //
        // This doesn't acquire a graph update permit because that will
        // clone the graph which is a bit slow.
        let graph = graph_container.graph();
        if !graph.roots.is_empty() && graph.get(&specifier).is_some() {
          log::debug!("Skipping prepare module load.");
          // roots are already validated so we can skip those
          if !graph.roots.contains(&specifier) {
            module_load_preparer.graph_roots_valid(&graph, &[specifier])?;
          }
          return Ok(());
        }
      }

      let permissions = if is_dynamic {
        inner.permissions.clone()
      } else {
        inner.parent_permissions.clone()
      };
      let is_dynamic = is_dynamic || inner.is_worker; // consider workers as dynamic for permissions
      let lib = inner.lib;
      let mut update_permit = graph_container.acquire_update_permit().await;
      let graph = update_permit.graph_mut();
      module_load_preparer
        .prepare_module_load(
          graph,
          &[specifier],
          is_dynamic,
          lib,
          permissions,
          None,
        )
        .await?;
      update_permit.commit();
      Ok(())
    }
    .boxed_local()
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

  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    let specifier = resolve_url(file_name).ok()?;
    match specifier.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => return None,
    }
    let source = self
      .0
      .load_prepared_module_for_source_map_sync(&specifier)
      .ok()??;
    source_map_from_code(source.code.as_bytes())
  }

  fn get_source_mapped_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let graph = self.0.graph_container.graph();
    let code = match graph.get(&resolve_url(file_name).ok()?) {
      Some(deno_graph::Module::Js(module)) => &module.source,
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
  cjs_tracker: Arc<CjsTracker>,
  emitter: Arc<Emitter>,
  fs: Arc<dyn FileSystem>,
  graph_container: TGraphContainer,
  in_npm_pkg_checker: Arc<dyn InNpmPackageChecker>,
  npm_resolver: Arc<dyn CliNpmResolver>,
}

impl<TGraphContainer: ModuleGraphContainer> NodeRequireLoader
  for CliNodeRequireLoader<TGraphContainer>
{
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
    path: &'a Path,
  ) -> Result<std::borrow::Cow<'a, Path>, AnyError> {
    if let Ok(url) = deno_path_util::url_from_file_path(path) {
      // allow reading if it's in the module graph
      if self.graph_container.graph().get(&url).is_some() {
        return Ok(std::borrow::Cow::Borrowed(path));
      }
    }
    self.npm_resolver.ensure_read_permission(permissions, path)
  }

  fn load_text_file_lossy(&self, path: &Path) -> Result<String, AnyError> {
    // todo(dsherret): use the preloaded module from the graph if available?
    let media_type = MediaType::from_path(path);
    let text = self.fs.read_text_file_lossy_sync(path, None)?;
    if media_type.is_emittable() {
      let specifier = deno_path_util::url_from_file_path(path)?;
      if self.in_npm_pkg_checker.in_npm_package(&specifier) {
        return Err(
          NotSupportedKindInNpmError {
            media_type,
            specifier,
          }
          .into(),
        );
      }
      self.emitter.emit_parsed_source_sync(
        &specifier,
        media_type,
        // this is probably not super accurate due to require esm, but probably ok.
        // If we find this causes a lot of churn in the emit cache then we should
        // investigate how we can make this better
        ModuleKind::Cjs,
        &text.into(),
      )
    } else {
      Ok(text)
    }
  }

  fn is_maybe_cjs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<bool, ClosestPkgJsonError> {
    let media_type = MediaType::from_specifier(specifier);
    self.cjs_tracker.is_maybe_cjs(specifier, media_type)
  }
}
