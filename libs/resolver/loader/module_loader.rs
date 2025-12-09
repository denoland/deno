// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use boxed_error::Boxed;
use deno_ast::ModuleKind;
use deno_graph::JsModule;
use deno_graph::JsonModule;
use deno_graph::ModuleGraph;
use deno_graph::WasmModule;
use deno_media_type::MediaType;
use node_resolver::InNpmPackageChecker;
use node_resolver::errors::PackageJsonLoadError;
use url::Url;

use super::AllowJsonImports;
use super::DenoNpmModuleLoaderRc;
use super::LoadedModule;
use super::LoadedModuleOrAsset;
use super::LoadedModuleSource;
use super::NpmModuleLoadError;
use super::RequestedModuleType;
use crate::cache::ParsedSourceCacheRc;
use crate::cjs::CjsTrackerRc;
use crate::emit::EmitParsedSourceHelperError;
use crate::emit::EmitterRc;
use crate::factory::DenoNodeCodeTranslatorRc;
use crate::graph::EnhanceGraphErrorMode;
use crate::graph::enhance_graph_error;
use crate::npm::DenoInNpmPackageChecker;

#[allow(clippy::disallowed_types)]
type ArcStr = std::sync::Arc<str>;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error("{message}")]
#[class(inherit)]
pub struct EnhancedGraphError {
  #[inherit]
  pub error: deno_graph::ModuleError,
  pub message: String,
}

#[derive(Debug, deno_error::JsError, Boxed)]
#[class(inherit)]
pub struct LoadPreparedModuleError(pub Box<LoadPreparedModuleErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LoadPreparedModuleErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  Graph(#[from] EnhancedGraphError),
  #[class(inherit)]
  #[error(transparent)]
  ClosestPkgJson(#[from] PackageJsonLoadError),
  #[class(inherit)]
  #[error(transparent)]
  LoadMaybeCjs(#[from] LoadMaybeCjsError),
  #[class(inherit)]
  #[error(transparent)]
  Emit(#[from] EmitParsedSourceHelperError),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LoadMaybeCjsError {
  #[class(inherit)]
  #[error(transparent)]
  NpmModuleLoad(#[from] crate::emit::EmitParsedSourceHelperError),
  #[class(inherit)]
  #[error(transparent)]
  TranslateCjsToEsm(#[from] node_resolver::analyze::TranslateCjsToEsmError),
}

#[derive(Debug, deno_error::JsError, Boxed)]
#[class(inherit)]
pub struct LoadCodeSourceError(pub Box<LoadCodeSourceErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LoadCodeSourceErrorKind {
  #[class(inherit)]
  #[error(transparent)]
  LoadPreparedModule(#[from] LoadPreparedModuleError),
  #[class(inherit)]
  #[error(transparent)]
  LoadUnpreparedModule(#[from] LoadUnpreparedModuleError),
  #[class(generic)]
  #[error(
    "Attempted to load JSON module without specifying \"type\": \"json\" attribute in the import statement."
  )]
  MissingJsonAttribute,
  #[class(inherit)]
  #[error(transparent)]
  NpmModuleLoad(#[from] NpmModuleLoadError),
  #[class(inherit)]
  #[error(transparent)]
  PathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedScheme(#[from] UnsupportedSchemeError),
}

// this message list additional `npm` and `jsr` schemes, but they should actually be handled
// before these APIs are even hit.
#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[error(
  "Unsupported scheme \"{}\" for module \"{}\". Supported schemes:\n - \"blob\"\n - \"data\"\n - \"file\"\n - \"http\"\n - \"https\"\n - \"jsr\"\n - \"npm\"", url.scheme(), url
)]
pub struct UnsupportedSchemeError {
  pub url: Url,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Loading unprepared module: {}{}", .specifier, .maybe_referrer.as_ref().map(|r| format!(", imported from: {}", r)).unwrap_or_default())]
pub struct LoadUnpreparedModuleError {
  specifier: Url,
  maybe_referrer: Option<Url>,
}

#[allow(clippy::disallowed_types)]
pub type ModuleLoaderRc<TSys> = deno_maybe_sync::MaybeArc<ModuleLoader<TSys>>;

#[sys_traits::auto_impl]
pub trait ModuleLoaderSys:
  super::NpmModuleLoaderSys
  + crate::emit::EmitterSys
  + node_resolver::analyze::NodeCodeTranslatorSys
  + crate::cjs::analyzer::DenoCjsCodeAnalyzerSys
  + crate::npm::NpmResolverSys
{
}

enum CodeOrDeferredEmit<'a> {
  Source(LoadedModule<'a>),
  DeferredEmit {
    specifier: &'a Url,
    media_type: MediaType,
    source: &'a ArcStr,
  },
  Cjs {
    specifier: &'a Url,
    media_type: MediaType,
    source: &'a ArcStr,
  },
  ExternalAsset {
    specifier: &'a Url,
  },
}

pub struct ModuleLoader<TSys: ModuleLoaderSys> {
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  npm_module_loader: DenoNpmModuleLoaderRc<TSys>,
  prepared_module_loader: PreparedModuleLoader<TSys>,
  allow_json_imports: AllowJsonImports,
}

impl<TSys: ModuleLoaderSys> ModuleLoader<TSys> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
    emitter: EmitterRc<DenoInNpmPackageChecker, TSys>,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    node_code_translator: DenoNodeCodeTranslatorRc<TSys>,
    npm_module_loader: DenoNpmModuleLoaderRc<TSys>,
    parsed_source_cache: ParsedSourceCacheRc,
    sys: TSys,
    allow_json_imports: AllowJsonImports,
  ) -> Self {
    Self {
      in_npm_pkg_checker,
      npm_module_loader,
      prepared_module_loader: PreparedModuleLoader {
        cjs_tracker,
        emitter,
        node_code_translator,
        parsed_source_cache,
        sys,
      },
      allow_json_imports,
    }
  }

  /// Loads a module using the graph or file system.
  ///
  /// Note that the referrer is only used to enhance error messages and
  /// doesn't need to be provided.
  pub async fn load<'a>(
    &self,
    graph: &'a ModuleGraph,
    specifier: &'a Url,
    // todo(#30074): we should remove passing the referrer in here and remove the
    // referrer from all error messages. This should be up to deno_core to display.
    maybe_referrer: Option<&Url>,
    requested_module_type: &RequestedModuleType<'_>,
  ) -> Result<LoadedModuleOrAsset<'a>, LoadCodeSourceError> {
    let source = match self
      .prepared_module_loader
      .load_prepared_module(graph, specifier, requested_module_type)
      .await
      .map_err(LoadCodeSourceError::from)?
    {
      Some(module_or_asset) => module_or_asset,
      None => {
        if !matches!(
          specifier.scheme(),
          "https" | "http" | "file" | "blob" | "data"
        ) {
          return Err(
            UnsupportedSchemeError {
              url: specifier.clone(),
            }
            .into(),
          );
        } else if self.in_npm_pkg_checker.in_npm_package(specifier) {
          let loaded_module = self
            .npm_module_loader
            .load(
              Cow::Borrowed(specifier),
              maybe_referrer,
              requested_module_type,
            )
            .await
            .map_err(LoadCodeSourceError::from)?;
          LoadedModuleOrAsset::Module(loaded_module)
        } else {
          match requested_module_type {
            RequestedModuleType::Text | RequestedModuleType::Bytes => {
              LoadedModuleOrAsset::ExternalAsset {
                specifier: Cow::Borrowed(specifier),
                statically_analyzable: false,
              }
            }
            _ => {
              return Err(LoadCodeSourceError::from(
                LoadUnpreparedModuleError {
                  specifier: specifier.clone(),
                  maybe_referrer: maybe_referrer.cloned(),
                },
              ));
            }
          }
        }
      }
    };

    match &source {
      LoadedModuleOrAsset::Module(loaded_module) => {
        // If we loaded a JSON file, but the "requested_module_type" (that is computed from
        // import attributes) is not JSON we need to fail.
        if loaded_module.media_type == MediaType::Json
          && !matches!(requested_module_type, RequestedModuleType::Json)
          && matches!(self.allow_json_imports, AllowJsonImports::WithAttribute)
        {
          Err(LoadCodeSourceErrorKind::MissingJsonAttribute.into_box())
        } else {
          Ok(source)
        }
      }
      LoadedModuleOrAsset::ExternalAsset { .. } => {
        // these are never type: "json"

        Ok(source)
      }
    }
  }

  pub fn load_prepared_module_for_source_map_sync<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
  ) -> Result<Option<LoadedModule<'graph>>, anyhow::Error> {
    self
      .prepared_module_loader
      .load_prepared_module_for_source_map_sync(graph, specifier)
  }
}

struct PreparedModuleLoader<TSys: ModuleLoaderSys> {
  cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
  emitter: EmitterRc<DenoInNpmPackageChecker, TSys>,
  node_code_translator: DenoNodeCodeTranslatorRc<TSys>,
  parsed_source_cache: ParsedSourceCacheRc,
  sys: TSys,
}

impl<TSys: ModuleLoaderSys> PreparedModuleLoader<TSys> {
  pub async fn load_prepared_module<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
    requested_module_type: &RequestedModuleType<'_>,
  ) -> Result<Option<LoadedModuleOrAsset<'graph>>, LoadPreparedModuleError> {
    // Note: keep this in sync with the sync version below
    match self.load_prepared_module_or_defer_emit(
      graph,
      specifier,
      requested_module_type,
    )? {
      Some(CodeOrDeferredEmit::Source(source)) => {
        Ok(Some(LoadedModuleOrAsset::Module(source)))
      }
      Some(CodeOrDeferredEmit::DeferredEmit {
        specifier,
        media_type,
        source,
      }) => {
        let transpile_result = self
          .emitter
          .maybe_emit_source(specifier, media_type, ModuleKind::Esm, source)
          .await?;

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(LoadedModuleOrAsset::Module(LoadedModule {
          // note: it's faster to provide a string to v8 if we know it's a string
          source: LoadedModuleSource::ArcStr(transpile_result),
          specifier: Cow::Borrowed(specifier),
          media_type,
        })))
      }
      Some(CodeOrDeferredEmit::Cjs {
        specifier,
        media_type,
        source,
      }) => self
        .load_maybe_cjs(specifier, media_type, source)
        .await
        .map(|text| {
          Some(LoadedModuleOrAsset::Module(LoadedModule {
            specifier: Cow::Borrowed(specifier),
            media_type,
            source: LoadedModuleSource::ArcStr(text),
          }))
        })
        .map_err(|e| LoadPreparedModuleErrorKind::LoadMaybeCjs(e).into_box()),
      Some(CodeOrDeferredEmit::ExternalAsset { specifier }) => {
        Ok(Some(LoadedModuleOrAsset::ExternalAsset {
          specifier: Cow::Borrowed(specifier),
          // came from graph, so yes
          statically_analyzable: true,
        }))
      }
      None => Ok(None),
    }
  }

  pub fn load_prepared_module_for_source_map_sync<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
  ) -> Result<Option<LoadedModule<'graph>>, anyhow::Error> {
    // Note: keep this in sync with the async version above
    match self.load_prepared_module_or_defer_emit(
      graph,
      specifier,
      &RequestedModuleType::None,
    )? {
      Some(CodeOrDeferredEmit::Source(code_source)) => Ok(Some(code_source)),
      Some(CodeOrDeferredEmit::DeferredEmit {
        specifier,
        media_type,
        source,
      }) => {
        let transpile_result = self.emitter.maybe_emit_source_sync(
          specifier,
          media_type,
          ModuleKind::Esm,
          source,
        )?;

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(LoadedModule {
          // note: it's faster to provide a string if we know it's a string
          source: LoadedModuleSource::ArcStr(transpile_result),
          specifier: Cow::Borrowed(specifier),
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
      Some(CodeOrDeferredEmit::ExternalAsset { .. }) | None => Ok(None),
    }
  }

  fn load_prepared_module_or_defer_emit<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
    requested_module_type: &RequestedModuleType,
  ) -> Result<Option<CodeOrDeferredEmit<'graph>>, LoadPreparedModuleError> {
    let maybe_module =
      graph.try_get(specifier).map_err(|err| EnhancedGraphError {
        message: enhance_graph_error(
          &self.sys,
          &deno_graph::ModuleGraphError::ModuleError(err.clone()),
          EnhanceGraphErrorMode::ShowRange,
        ),
        error: err.clone(),
      })?;

    match maybe_module {
      Some(deno_graph::Module::Json(JsonModule {
        source,
        media_type,
        specifier,
        ..
      })) => match requested_module_type {
        RequestedModuleType::Bytes => match source.try_get_original_bytes() {
          Some(bytes) => Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
            source: LoadedModuleSource::ArcBytes(bytes),
            specifier: Cow::Borrowed(specifier),
            media_type: *media_type,
          }))),
          None => Ok(Some(CodeOrDeferredEmit::ExternalAsset { specifier })),
        },
        RequestedModuleType::Text => {
          Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
            source: LoadedModuleSource::ArcStr(source.text.clone()),
            specifier: Cow::Borrowed(specifier),
            media_type: *media_type,
          })))
        }
        _ => Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
          source: LoadedModuleSource::ArcStr(source.text.clone()),
          specifier: Cow::Borrowed(specifier),
          media_type: *media_type,
        }))),
      },
      Some(deno_graph::Module::Js(JsModule {
        source,
        media_type,
        specifier,
        is_script,
        ..
      })) => match requested_module_type {
        RequestedModuleType::Bytes => match source.try_get_original_bytes() {
          Some(bytes) => Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
            source: LoadedModuleSource::ArcBytes(bytes),
            specifier: Cow::Borrowed(specifier),
            media_type: *media_type,
          }))),
          None => Ok(Some(CodeOrDeferredEmit::ExternalAsset { specifier })),
        },
        RequestedModuleType::Text => {
          Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
            source: LoadedModuleSource::ArcStr(source.text.clone()),
            specifier: Cow::Borrowed(specifier),
            media_type: *media_type,
          })))
        }
        _ => {
          if self.cjs_tracker.is_cjs_with_known_is_script(
            specifier,
            *media_type,
            *is_script,
          )? {
            return Ok(Some(CodeOrDeferredEmit::Cjs {
              specifier,
              media_type: *media_type,
              source: &source.text,
            }));
          }
          let code = match media_type {
            MediaType::JavaScript
            | MediaType::Unknown
            | MediaType::Mjs
            | MediaType::Json => source.text.clone(),
            MediaType::Dts | MediaType::Dcts | MediaType::Dmts => {
              Default::default()
            }
            MediaType::Cjs | MediaType::Cts => {
              return Ok(Some(CodeOrDeferredEmit::Cjs {
                specifier,
                media_type: *media_type,
                source: &source.text,
              }));
            }
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Jsx
            | MediaType::Tsx => {
              return Ok(Some(CodeOrDeferredEmit::DeferredEmit {
                specifier,
                media_type: *media_type,
                source: &source.text,
              }));
            }
            MediaType::Css
            | MediaType::Html
            | MediaType::Jsonc
            | MediaType::Json5
            | MediaType::Sql
            | MediaType::Wasm
            | MediaType::SourceMap => {
              panic!("Unexpected media type {media_type} for {specifier}")
            }
          };

          // at this point, we no longer need the parsed source in memory, so free it
          self.parsed_source_cache.free(specifier);

          Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
            source: LoadedModuleSource::ArcStr(code),
            specifier: Cow::Borrowed(specifier),
            media_type: *media_type,
          })))
        }
      },
      Some(deno_graph::Module::Wasm(WasmModule {
        source, specifier, ..
      })) => Ok(Some(CodeOrDeferredEmit::Source(LoadedModule {
        source: LoadedModuleSource::ArcBytes(source.clone()),
        specifier: Cow::Borrowed(specifier),
        media_type: MediaType::Wasm,
      }))),
      Some(deno_graph::Module::External(module)) => {
        if module.specifier.as_str().contains("/node_modules/") {
          return Ok(None);
        }
        Ok(Some(CodeOrDeferredEmit::ExternalAsset {
          specifier: &module.specifier,
        }))
      }
      Some(deno_graph::Module::Node(_) | deno_graph::Module::Npm(_)) | None => {
        Ok(None)
      }
    }
  }

  async fn load_maybe_cjs(
    &self,
    specifier: &Url,
    media_type: MediaType,
    original_source: &ArcStr,
  ) -> Result<ArcStr, LoadMaybeCjsError> {
    let js_source = self
      .emitter
      .maybe_emit_source(
        specifier,
        media_type,
        ModuleKind::Cjs,
        original_source,
      )
      .await?;
    let text = self
      .node_code_translator
      .translate_cjs_to_esm(specifier, Some(Cow::Borrowed(js_source.as_ref())))
      .await?;
    // at this point, we no longer need the parsed source in memory, so free it
    self.parsed_source_cache.free(specifier);
    Ok(match text {
      // perf: if the text is borrowed, that means it didn't make any changes
      // to the original source, so we can just provide that instead of cloning
      // the borrowed text
      Cow::Borrowed(_) => js_source.clone(),
      Cow::Owned(text) => text.into(),
    })
  }
}
