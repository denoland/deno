// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_ast::MediaType;
use deno_ast::ModuleKind;
use deno_error::JsError;
use deno_graph::JsModule;
use deno_graph::JsonModule;
use deno_graph::ModuleGraph;
use deno_graph::WasmModule;
use node_resolver::analyze::NodeCodeTranslatorSys;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageChecker;
use thiserror::Error;
use url::Url;

use super::RequestedModuleType;
use crate::cache::ParsedSourceCacheRc;
use crate::cjs::analyzer::DenoCjsCodeAnalyzerSys;
use crate::cjs::CjsTrackerRc;
use crate::emit::EmitParsedSourceHelperError;
use crate::emit::EmitterRc;
use crate::emit::EmitterSys;
use crate::factory::DenoNodeCodeTranslatorRc;
use crate::graph::enhance_graph_error;
use crate::graph::EnhanceGraphErrorMode;
use crate::npm::NpmResolverSys;

#[allow(clippy::disallowed_types)]
type ArcStr = std::sync::Arc<str>;
#[allow(clippy::disallowed_types)]
type ArcBytes = std::sync::Arc<[u8]>;

pub enum PreparedModuleSource {
  ArcStr(ArcStr),
  ArcBytes(ArcBytes),
}

impl PreparedModuleSource {
  pub fn as_bytes(&self) -> &[u8] {
    match self {
      PreparedModuleSource::ArcStr(text) => text.as_bytes(),
      PreparedModuleSource::ArcBytes(bytes) => bytes,
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[error("{message}")]
#[class(inherit)]
pub struct EnhancedGraphError {
  #[inherit]
  pub error: deno_graph::ModuleError,
  pub message: String,
}

#[derive(Debug, Error, JsError)]
pub enum LoadPreparedModuleError {
  #[class(inherit)]
  #[error(transparent)]
  Graph(#[from] EnhancedGraphError),
  #[class(inherit)]
  #[error(transparent)]
  ClosestPkgJson(#[from] ClosestPkgJsonError),
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

#[allow(clippy::disallowed_types)]
pub type PreparedModuleLoaderRc<TInNpmPackageChecker, TSys> =
  crate::sync::MaybeArc<PreparedModuleLoader<TInNpmPackageChecker, TSys>>;

#[sys_traits::auto_impl]
pub trait PreparedModuleLoaderSys:
  EmitterSys + NodeCodeTranslatorSys + DenoCjsCodeAnalyzerSys + NpmResolverSys
{
}

pub struct PreparedModule<'graph> {
  pub specifier: &'graph Url,
  pub media_type: MediaType,
  pub source: PreparedModuleSource,
}

pub enum PreparedModuleOrAsset<'graph> {
  Module(PreparedModule<'graph>),
  /// A module that the graph knows about, but the data
  /// is not stored in the graph itself. It's up to the caller
  /// to fetch this data.
  ExternalAsset {
    specifier: &'graph Url,
  },
}

enum CodeOrDeferredEmit<'a> {
  Source(PreparedModule<'a>),
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

pub struct PreparedModuleLoader<
  TInNpmPackageChecker: InNpmPackageChecker,
  TSys: PreparedModuleLoaderSys,
> {
  cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
  emitter: EmitterRc<TInNpmPackageChecker, TSys>,
  node_code_translator: DenoNodeCodeTranslatorRc<TSys>,
  parsed_source_cache: ParsedSourceCacheRc,
  sys: TSys,
}

impl<
    TInNpmPackageChecker: InNpmPackageChecker,
    TSys: PreparedModuleLoaderSys,
  > PreparedModuleLoader<TInNpmPackageChecker, TSys>
{
  pub fn new(
    cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
    emitter: EmitterRc<TInNpmPackageChecker, TSys>,
    node_code_translator: DenoNodeCodeTranslatorRc<TSys>,
    parsed_source_cache: ParsedSourceCacheRc,
    sys: TSys,
  ) -> Self {
    Self {
      cjs_tracker,
      emitter,
      node_code_translator,
      parsed_source_cache,
      sys,
    }
  }

  pub async fn load_prepared_module<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
    requested_module_type: &RequestedModuleType<'_>,
  ) -> Result<Option<PreparedModuleOrAsset<'graph>>, LoadPreparedModuleError>
  {
    // Note: keep this in sync with the sync version below
    match self.load_prepared_module_or_defer_emit(
      graph,
      specifier,
      requested_module_type,
    )? {
      Some(CodeOrDeferredEmit::Source(source)) => {
        Ok(Some(PreparedModuleOrAsset::Module(source)))
      }
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

        Ok(Some(PreparedModuleOrAsset::Module(PreparedModule {
          // note: it's faster to provide a string to v8 if we know it's a string
          source: PreparedModuleSource::ArcStr(transpile_result.into()),
          specifier,
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
          Some(PreparedModuleOrAsset::Module(PreparedModule {
            specifier,
            media_type,
            source: PreparedModuleSource::ArcStr(text),
          }))
        })
        .map_err(LoadPreparedModuleError::LoadMaybeCjs),
      Some(CodeOrDeferredEmit::ExternalAsset { specifier }) => {
        Ok(Some(PreparedModuleOrAsset::ExternalAsset { specifier }))
      }
      None => Ok(None),
    }
  }

  pub fn load_prepared_module_for_source_map_sync<'graph>(
    &self,
    graph: &'graph ModuleGraph,
    specifier: &Url,
  ) -> Result<Option<PreparedModule<'graph>>, anyhow::Error> {
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
        let transpile_result = self.emitter.emit_parsed_source_sync(
          specifier,
          media_type,
          ModuleKind::Esm,
          source,
        )?;

        // at this point, we no longer need the parsed source in memory, so free it
        self.parsed_source_cache.free(specifier);

        Ok(Some(PreparedModule {
          // note: it's faster to provide a string if we know it's a string
          source: PreparedModuleSource::ArcStr(transpile_result.into()),
          specifier,
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
    if specifier.scheme() == "node" {
      // Node built-in modules should be handled internally.
      unreachable!("Deno bug. {} was misconfigured internally.", specifier);
    }

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
          Some(bytes) => Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
            source: PreparedModuleSource::ArcBytes(bytes),
            specifier,
            media_type: *media_type,
          }))),
          None => Ok(Some(CodeOrDeferredEmit::ExternalAsset { specifier })),
        },
        RequestedModuleType::Text => {
          Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
            source: PreparedModuleSource::ArcStr(source.text.clone()),
            specifier,
            media_type: *media_type,
          })))
        }
        _ => Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
          source: PreparedModuleSource::ArcStr(source.text.clone()),
          specifier,
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
          Some(bytes) => Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
            source: PreparedModuleSource::ArcBytes(bytes),
            specifier,
            media_type: *media_type,
          }))),
          None => Ok(Some(CodeOrDeferredEmit::ExternalAsset { specifier })),
        },
        RequestedModuleType::Text => {
          Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
            source: PreparedModuleSource::ArcStr(source.text.clone()),
            specifier,
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
            | MediaType::Sql
            | MediaType::Wasm
            | MediaType::SourceMap => {
              panic!("Unexpected media type {media_type} for {specifier}")
            }
          };

          // at this point, we no longer need the parsed source in memory, so free it
          self.parsed_source_cache.free(specifier);

          Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
            source: PreparedModuleSource::ArcStr(code),
            specifier,
            media_type: *media_type,
          })))
        }
      },
      Some(deno_graph::Module::Wasm(WasmModule {
        source, specifier, ..
      })) => Ok(Some(CodeOrDeferredEmit::Source(PreparedModule {
        source: PreparedModuleSource::ArcBytes(source.clone()),
        specifier,
        media_type: MediaType::Wasm,
      }))),
      Some(deno_graph::Module::External(module))
        if matches!(
          requested_module_type,
          RequestedModuleType::Bytes | RequestedModuleType::Text
        ) =>
      {
        Ok(Some(CodeOrDeferredEmit::ExternalAsset {
          specifier: &module.specifier,
        }))
      }
      Some(
        deno_graph::Module::External(_)
        | deno_graph::Module::Node(_)
        | deno_graph::Module::Npm(_),
      )
      | None => Ok(None),
    }
  }

  async fn load_maybe_cjs(
    &self,
    specifier: &Url,
    media_type: MediaType,
    original_source: &ArcStr,
  ) -> Result<ArcStr, LoadMaybeCjsError> {
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
    Ok(match text {
      // perf: if the text is borrowed, that means it didn't make any changes
      // to the original source, so we can just provide that instead of cloning
      // the borrowed text
      Cow::Borrowed(_) => original_source.clone(),
      Cow::Owned(text) => text.into(),
    })
  }
}
