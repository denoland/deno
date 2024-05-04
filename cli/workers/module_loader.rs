// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::jsr_url;
use crate::args::CliOptions;
use crate::args::DenoSubcommand;
use crate::args::TsTypeLib;
use crate::cache::CodeCache;
use crate::cache::ModuleInfoCache;
use crate::cache::ParsedSourceCache;
use crate::emit::Emitter;
use crate::module_load_preparer::ModuleLoadPreparer;
use crate::node;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliNodeResolver;
use crate::resolver::ModuleCodeStringSource;
use crate::resolver::NpmModuleLoader;
use crate::util::text_encoding::code_without_source_map;
use crate::util::text_encoding::source_map_from_code;
use crate::worker::ModuleLoaderAndSourceMapGetter;
use crate::worker::ModuleLoaderFactory;

use deno_ast::MediaType;
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
use deno_core::SourceMapGetter;
use deno_graph::source::ResolutionMode;
use deno_graph::source::Resolver;
use deno_graph::GraphKind;
use deno_graph::JsModule;
use deno_graph::JsonModule;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Resolution;
use deno_runtime::code_cache;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::fs_util::code_timestamp;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use std::borrow::Cow;
use std::pin::Pin;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

use super::module_graph_container::ModuleGraphContainer;
use super::module_graph_container::ModuleGraphUpdatePermit;
use super::module_graph_container::WorkerModuleGraphContainer;

struct SharedCliModuleLoaderState {
  graph_kind: GraphKind,
  lib_window: TsTypeLib,
  lib_worker: TsTypeLib,
  is_inspecting: bool,
  is_repl: bool,
  code_cache: Option<Arc<CodeCache>>,
  emitter: Arc<Emitter>,
  resolver: Arc<CliGraphResolver>,
  module_info_cache: Arc<ModuleInfoCache>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  node_resolver: Arc<CliNodeResolver>,
  npm_module_loader: NpmModuleLoader,
  parsed_source_cache: Arc<ParsedSourceCache>,
}

pub struct CliModuleLoaderFactory {
  shared: Arc<SharedCliModuleLoaderState>,
}

impl CliModuleLoaderFactory {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    options: &CliOptions,
    code_cache: Option<Arc<CodeCache>>,
    emitter: Arc<Emitter>,
    module_info_cache: Arc<ModuleInfoCache>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
    node_resolver: Arc<CliNodeResolver>,
    npm_module_loader: NpmModuleLoader,
    parsed_source_cache: Arc<ParsedSourceCache>,
    resolver: Arc<CliGraphResolver>,
  ) -> Self {
    Self {
      shared: Arc::new(SharedCliModuleLoaderState {
        graph_kind: options.graph_kind(),
        lib_window: options.ts_type_lib_window(),
        lib_worker: options.ts_type_lib_worker(),
        is_inspecting: options.is_inspecting(),
        is_repl: matches!(
          options.sub_command(),
          DenoSubcommand::Repl(_) | DenoSubcommand::Jupyter(_)
        ),
        code_cache,
        emitter,
        module_info_cache,
        module_load_preparer,
        node_resolver,
        npm_module_loader,
        parsed_source_cache,
        resolver,
      }),
    }
  }

  fn create_with_lib(
    &self,
    module_graph: Arc<ModuleGraph>,
    lib: TsTypeLib,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> ModuleLoaderAndSourceMapGetter {
    let loader = Rc::new(CliModuleLoader {
      lib,
      root_permissions,
      dynamic_permissions,
      graph_container: WorkerModuleGraphContainer::new(module_graph),
      emitter: self.shared.emitter.clone(),
      parsed_source_cache: self.shared.parsed_source_cache.clone(),
      shared: self.shared.clone(),
    });
    ModuleLoaderAndSourceMapGetter {
      module_loader: loader.clone(),
      source_map_getter: Some(loader),
    }
  }
}

impl ModuleLoaderFactory for CliModuleLoaderFactory {
  fn create_for_main(
    &self,
    starting_module_graph: Arc<ModuleGraph>,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> ModuleLoaderAndSourceMapGetter {
    self.create_with_lib(
      starting_module_graph,
      self.shared.lib_window,
      root_permissions,
      dynamic_permissions,
    )
  }

  fn create_for_worker(
    &self,
    root_permissions: PermissionsContainer,
    dynamic_permissions: PermissionsContainer,
  ) -> ModuleLoaderAndSourceMapGetter {
    self.create_with_lib(
      // create a fresh module graph for the worker
      Arc::new(ModuleGraph::new(self.shared.graph_kind)),
      self.shared.lib_worker,
      root_permissions,
      dynamic_permissions,
    )
  }
}

struct CliModuleLoader<TGraphContainer: ModuleGraphContainer> {
  lib: TsTypeLib,
  /// The initial set of permissions used to resolve the static imports in the
  /// worker. These are "allow all" for main worker, and parent thread
  /// permissions for Web Worker.
  root_permissions: PermissionsContainer,
  /// Permissions used to resolve dynamic imports, these get passed as
  /// "root permissions" for Web Worker.
  dynamic_permissions: PermissionsContainer,
  shared: Arc<SharedCliModuleLoaderState>,
  emitter: Arc<Emitter>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  graph_container: TGraphContainer,
}

impl<TGraphContainer: ModuleGraphContainer> CliModuleLoader<TGraphContainer> {
  fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
    requested_module_type: RequestedModuleType,
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
      self.load_prepared_module(specifier, maybe_referrer)?
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
      self.shared.code_cache.as_ref().and_then(|cache| {
        let code_hash = self
          .get_code_hash_or_timestamp(specifier, code_source.media_type)
          .ok()
          .flatten();
        if let Some(code_hash) = code_hash {
          cache
            .get_sync(
              specifier.as_str(),
              code_cache::CodeCacheType::EsModule,
              &code_hash,
            )
            .map(Cow::from)
            .inspect(|_| {
              // This log line is also used by tests.
              log::debug!(
                "V8 code cache hit for ES module: {specifier}, [{code_hash:?}]"
              );
            })
        } else {
          None
        }
      })
    } else {
      None
    };

    Ok(ModuleSource::new_with_redirect(
      module_type,
      ModuleSourceCode::String(code),
      specifier,
      &code_source.found_url,
      code_cache,
    ))
  }

  fn resolve_referrer(
    &self,
    referrer: &str,
  ) -> Result<ModuleSpecifier, AnyError> {
    // TODO(bartlomieju): ideally we shouldn't need to call `current_dir()` on each
    // call - maybe it should be caller's responsibility to pass it as an arg?
    let cwd = std::env::current_dir().context("Unable to get CWD")?;
    if referrer.is_empty() && self.shared.is_repl {
      // FIXME(bartlomieju): this is a hacky way to provide compatibility with REPL
      // and `Deno.core.evalContext` API. Ideally we should always have a referrer filled
      // but sadly that's not the case due to missing APIs in V8.
      deno_core::resolve_path("./$deno$repl.ts", &cwd).map_err(|e| e.into())
    } else {
      deno_core::resolve_url_or_path(referrer, &cwd).map_err(|e| e.into())
    }
  }

  fn inner_resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, AnyError> {
    let permissions = if matches!(kind, ResolutionKind::DynamicImport) {
      &self.dynamic_permissions
    } else {
      &self.root_permissions
    };

    if let Some(result) = self.shared.node_resolver.resolve_if_in_npm_package(
      specifier,
      referrer,
      NodeResolutionMode::Execution,
      permissions,
    ) {
      return match result? {
        Some(res) => Ok(res.into_url()),
        None => Err(generic_error("not found")),
      };
    }

    let graph = self.graph_container.graph();
    let maybe_resolved = match graph.get(referrer) {
      Some(Module::Js(module)) => {
        module.dependencies.get(specifier).map(|d| &d.maybe_code)
      }
      _ => None,
    };

    match maybe_resolved {
      Some(Resolution::Ok(resolved)) => {
        let specifier = &resolved.specifier;
        let specifier = match graph.get(specifier) {
          Some(Module::Npm(module)) => {
            let package_folder = self
              .shared
              .node_resolver
              .npm_resolver
              .as_managed()
              .unwrap() // byonm won't create a Module::Npm
              .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
            let maybe_resolution = self
              .shared
              .node_resolver
              .resolve_package_sub_path_from_deno_module(
                &package_folder,
                module.nv_reference.sub_path(),
                referrer,
                NodeResolutionMode::Execution,
                permissions,
              )
              .with_context(|| {
                format!("Could not resolve '{}'.", module.nv_reference)
              })?;
            match maybe_resolution {
              Some(res) => res.into_url(),
              None => return Err(generic_error("not found")),
            }
          }
          Some(Module::Node(module)) => module.specifier.clone(),
          Some(Module::Js(module)) => module.specifier.clone(),
          Some(Module::Json(module)) => module.specifier.clone(),
          Some(Module::External(module)) => {
            node::resolve_specifier_into_node_modules(&module.specifier)
          }
          None => specifier.clone(),
        };
        return Ok(specifier);
      }
      Some(Resolution::Err(err)) => {
        return Err(custom_error(
          "TypeError",
          format!("{}\n", err.to_string_with_range()),
        ))
      }
      Some(Resolution::None) | None => {}
    }

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
          return self
            .shared
            .node_resolver
            .resolve_req_reference(
              &reference,
              permissions,
              referrer,
              NodeResolutionMode::Execution,
            )
            .map(|res| res.into_url());
        }
      }
    }

    resolution.map_err(|err| err.into())
  }

  fn get_code_hash_or_timestamp(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
  ) -> Result<Option<String>, AnyError> {
    let hash = self
      .shared
      .module_info_cache
      .get_module_source_hash(specifier, media_type)?;
    if let Some(hash) = hash {
      return Ok(Some(hash.into()));
    }

    // Use the modified timestamp from the local file system if we don't have a hash.
    let timestamp = code_timestamp(specifier.as_str())
      .map(|timestamp| timestamp.to_string())?;
    Ok(Some(timestamp))
  }

  fn load_prepared_module(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
  ) -> Result<ModuleCodeStringSource, AnyError> {
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
      })) => Ok(ModuleCodeStringSource {
        code: source.clone().into(),
        found_url: specifier.clone(),
        media_type: *media_type,
      }),
      Some(deno_graph::Module::Js(JsModule {
        source,
        media_type,
        specifier,
        ..
      })) => {
        let code: ModuleCodeString = match media_type {
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

        Ok(ModuleCodeStringSource {
          code,
          found_url: specifier.clone(),
          media_type: *media_type,
        })
      }
      Some(
        deno_graph::Module::External(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::Npm(_),
      )
      | None => {
        let mut msg = format!("Loading unprepared module: {specifier}");
        if let Some(referrer) = maybe_referrer {
          msg = format!("{}, imported from: {}", msg, referrer.as_str());
        }
        Err(anyhow!(msg))
      }
    }
  }
}

impl<TGraphContainer: ModuleGraphContainer> ModuleLoader
  for CliModuleLoader<TGraphContainer>
{
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
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

    let referrer = self.resolve_referrer(referrer)?;
    let specifier = self.inner_resolve(specifier, &referrer, kind)?;
    ensure_not_jsr_non_jsr_remote_import(&specifier, &referrer)?;
    Ok(specifier)
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<&ModuleSpecifier>,
    is_dynamic: bool,
    requested_module_type: RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    // NOTE: this block is async only because of `deno_core` interface
    // requirements; module was already loaded when constructing module graph
    // during call to `prepare_load` so we can load it synchronously.
    deno_core::ModuleLoadResponse::Sync(self.load_sync(
      specifier,
      maybe_referrer,
      is_dynamic,
      requested_module_type,
    ))
  }

  fn prepare_load(
    &self,
    specifier: &ModuleSpecifier,
    _maybe_referrer: Option<String>,
    is_dynamic: bool,
  ) -> Pin<Box<dyn Future<Output = Result<(), AnyError>>>> {
    if self.shared.node_resolver.in_npm_package(&specifier) {
      return Box::pin(deno_core::futures::future::ready(Ok(())));
    }

    let specifier = specifier.clone();
    let graph_container = self.graph_container.clone();
    let module_load_preparer = self.shared.module_load_preparer.clone();

    let root_permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let lib = self.lib;

    async move {
      let mut update_permit = graph_container.acquire_update_permit().await;
      let graph = update_permit.graph_mut();
      module_load_preparer
        .prepare_module_load(
          graph,
          &[specifier],
          is_dynamic,
          lib,
          root_permissions,
        )
        .await?;
      update_permit.commit();
      Ok(())
    }
    .boxed_local()
  }

  fn code_cache_ready(
    &self,
    specifier: &ModuleSpecifier,
    code_cache: &[u8],
  ) -> Pin<Box<dyn Future<Output = ()>>> {
    if let Some(cache) = self.shared.code_cache.as_ref() {
      let media_type = MediaType::from_specifier(specifier);
      let code_hash = self
        .get_code_hash_or_timestamp(specifier, media_type)
        .ok()
        .flatten();
      if let Some(code_hash) = code_hash {
        // This log line is also used by tests.
        log::debug!(
          "Updating V8 code cache for ES module: {specifier}, [{code_hash:?}]"
        );
        cache.set_sync(
          specifier.as_str(),
          code_cache::CodeCacheType::EsModule,
          &code_hash,
          code_cache,
        );
      }
    }
    std::future::ready(()).boxed_local()
  }
}

impl<TGraphContainer: ModuleGraphContainer> SourceMapGetter
  for CliModuleLoader<TGraphContainer>
{
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
    let specifier = resolve_url(file_name).ok()?;
    match specifier.scheme() {
      // we should only be looking for emits for schemes that denote external
      // modules, which the disk_cache supports
      "wasm" | "file" | "http" | "https" | "data" | "blob" => (),
      _ => return None,
    }
    let source = self.load_prepared_module(&specifier, None).ok()?;
    source_map_from_code(&source.code)
  }

  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String> {
    let graph = self.graph_container.graph();
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
