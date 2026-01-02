// Copyright 2018-2025 the Deno authors. MIT license.

use std::hash::Hash;
use std::hash::Hasher;

use anyhow::Error as AnyError;
use deno_ast::EmittedSourceText;
use deno_ast::ModuleKind;
use deno_ast::ParsedSource;
use deno_ast::SourceMapOption;
use deno_ast::SourceRange;
use deno_ast::SourceRanged;
use deno_ast::SourceRangedForSpanned;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileResult;
use deno_error::JsErrorBox;
use deno_graph::MediaType;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use node_resolver::InNpmPackageChecker;
use url::Url;

use crate::cache::EmitCacheRc;
use crate::cache::EmitCacheSys;
use crate::cache::ParsedSourceCacheRc;
use crate::cjs::CjsTrackerRc;
use crate::deno_json::CompilerOptionsParseError;
use crate::deno_json::CompilerOptionsResolverRc;
use crate::deno_json::TranspileAndEmitOptions;

#[allow(clippy::disallowed_types)] // ok because we always store source text as Arc<str>
type ArcStr = std::sync::Arc<str>;

#[allow(clippy::disallowed_types)]
pub type EmitterRc<TInNpmPackageChecker, TSys> =
  deno_maybe_sync::MaybeArc<Emitter<TInNpmPackageChecker, TSys>>;

#[sys_traits::auto_impl]
pub trait EmitterSys: EmitCacheSys {}

#[derive(Debug)]
pub struct Emitter<TInNpmPackageChecker: InNpmPackageChecker, TSys: EmitterSys>
{
  cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
  emit_cache: EmitCacheRc<TSys>,
  parsed_source_cache: ParsedSourceCacheRc,
  compiler_options_resolver: CompilerOptionsResolverRc,
}

impl<TInNpmPackageChecker: InNpmPackageChecker, TSys: EmitterSys>
  Emitter<TInNpmPackageChecker, TSys>
{
  pub fn new(
    cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
    emit_cache: EmitCacheRc<TSys>,
    parsed_source_cache: ParsedSourceCacheRc,
    compiler_options_resolver: CompilerOptionsResolverRc,
  ) -> Self {
    Self {
      cjs_tracker,
      emit_cache,
      parsed_source_cache,
      compiler_options_resolver,
    }
  }

  pub async fn cache_module_emits(
    &self,
    graph: &ModuleGraph,
  ) -> Result<(), AnyError> {
    let mut futures = FuturesUnordered::new();
    for module in graph.modules() {
      let Module::Js(module) = module else {
        continue;
      };

      if module.media_type.is_emittable() {
        futures.push(
          self
            .maybe_emit_source(
              &module.specifier,
              module.media_type,
              ModuleKind::from_is_cjs(
                self.cjs_tracker.is_cjs_with_known_is_script(
                  &module.specifier,
                  module.media_type,
                  module.is_script,
                )?,
              ),
              &module.source.text,
            )
            .boxed_local(),
        );
      }
    }

    while let Some(result) = futures.next().await {
      result?; // surface errors
    }

    Ok(())
  }

  /// Gets a cached emit if the source matches the hash found in the cache.
  pub fn maybe_cached_emit(
    &self,
    specifier: &Url,
    module_kind: deno_ast::ModuleKind,
    source: &str,
  ) -> Result<Option<String>, AnyError> {
    let transpile_and_emit_options = self
      .compiler_options_resolver
      .for_specifier(specifier)
      .transpile_options()?;
    let source_hash =
      self.get_source_hash(module_kind, transpile_and_emit_options, source);
    Ok(self.emit_cache.get_emit_code(specifier, source_hash))
  }

  pub async fn maybe_emit_source(
    &self,
    specifier: &Url,
    media_type: MediaType,
    module_kind: ModuleKind,
    source: &ArcStr,
  ) -> Result<ArcStr, EmitParsedSourceHelperError> {
    self
      .maybe_emit_parsed_source_provider(
        ParsedSourceCacheParsedSourceProvider {
          parsed_source_cache: self.parsed_source_cache.clone(),
          specifier: specifier.clone(),
          media_type,
          source: source.clone(),
        },
        module_kind,
      )
      .await
  }

  pub async fn maybe_emit_parsed_source(
    &self,
    parsed_source: deno_ast::ParsedSource,
    module_kind: ModuleKind,
  ) -> Result<ArcStr, EmitParsedSourceHelperError> {
    // note: this method is used in deno-js-loader
    self
      .maybe_emit_parsed_source_provider(parsed_source, module_kind)
      .await
  }

  async fn maybe_emit_parsed_source_provider<
    TProvider: ParsedSourceProvider,
  >(
    &self,
    provider: TProvider,
    module_kind: ModuleKind,
  ) -> Result<ArcStr, EmitParsedSourceHelperError> {
    // Note: keep this in sync with the sync version below
    if !provider.media_type().is_emittable() {
      return Ok(provider.into_source());
    }
    let transpile_and_emit_options = self
      .compiler_options_resolver
      .for_specifier(provider.specifier())
      .transpile_options()?;
    if transpile_and_emit_options.no_transpile {
      return Ok(provider.into_source());
    }
    let transpile_options = &transpile_and_emit_options.transpile;
    if matches!(provider.media_type(), MediaType::Jsx)
      && transpile_options.jsx.is_none()
    {
      // jsx disabled, so skip
      return Ok(provider.into_source());
    }
    let helper = EmitParsedSourceHelper(self);
    match helper.pre_emit_parsed_source(
      provider.specifier(),
      module_kind,
      transpile_and_emit_options,
      provider.source(),
    ) {
      PreEmitResult::Cached(emitted_text) => Ok(emitted_text.into()),
      PreEmitResult::NotCached { source_hash } => {
        let specifier = provider.specifier().clone();
        let emit = {
          let transpile_and_emit_options = transpile_and_emit_options.clone();
          move || {
            let parsed_source = provider.parsed_source()?;
            transpile(
              parsed_source,
              module_kind,
              &transpile_and_emit_options.transpile,
              &transpile_and_emit_options.emit,
            )
            .map(|r| r.text)
          }
        };
        #[cfg(feature = "sync")]
        let transpiled_source =
          crate::rt::spawn_blocking(emit).await.unwrap()?;
        #[cfg(not(feature = "sync"))]
        let transpiled_source = emit()?;
        helper.post_emit_parsed_source(
          &specifier,
          &transpiled_source,
          source_hash,
        );
        Ok(transpiled_source.into())
      }
    }
  }

  #[allow(clippy::result_large_err)]
  pub fn maybe_emit_source_sync(
    &self,
    specifier: &Url,
    media_type: MediaType,
    module_kind: deno_ast::ModuleKind,
    source: &ArcStr,
  ) -> Result<ArcStr, EmitParsedSourceHelperError> {
    // Note: keep this in sync with the async version above
    if !media_type.is_emittable() {
      return Ok(source.clone());
    }
    let transpile_and_emit_options = self
      .compiler_options_resolver
      .for_specifier(specifier)
      .transpile_options()?;
    if transpile_and_emit_options.no_transpile {
      return Ok(source.clone());
    }
    let transpile_options = &transpile_and_emit_options.transpile;
    if matches!(media_type, MediaType::Jsx) && transpile_options.jsx.is_none() {
      // jsx disabled, so skip
      return Ok(source.clone());
    }
    let helper = EmitParsedSourceHelper(self);
    match helper.pre_emit_parsed_source(
      specifier,
      module_kind,
      transpile_and_emit_options,
      source,
    ) {
      PreEmitResult::Cached(emitted_text) => Ok(emitted_text.into()),
      PreEmitResult::NotCached { source_hash } => {
        let parsed_source = self.parsed_source_cache.remove_or_parse_module(
          specifier,
          media_type,
          source.clone(),
        )?;
        let transpiled_source = transpile(
          parsed_source,
          module_kind,
          &transpile_and_emit_options.transpile,
          &transpile_and_emit_options.emit,
        )?
        .text;
        helper.post_emit_parsed_source(
          specifier,
          &transpiled_source,
          source_hash,
        );
        Ok(transpiled_source.into())
      }
    }
  }

  pub fn emit_source_for_deno_compile(
    &self,
    specifier: &Url,
    media_type: MediaType,
    module_kind: deno_ast::ModuleKind,
    source: &ArcStr,
  ) -> Result<(String, String), AnyError> {
    let transpile_and_emit_options = self
      .compiler_options_resolver
      .for_specifier(specifier)
      .transpile_options()?;
    let mut emit_options = transpile_and_emit_options.emit.clone();
    emit_options.inline_sources = false;
    emit_options.source_map = SourceMapOption::Separate;
    // strip off the path to have more deterministic builds as we don't care
    // about the source name because we manually provide the source map to v8
    emit_options.source_map_base = Some(deno_path_util::url_parent(specifier));
    let parsed_source = self.parsed_source_cache.remove_or_parse_module(
      specifier,
      media_type,
      source.clone(),
    )?;
    let source = transpile(
      parsed_source,
      module_kind,
      &transpile_and_emit_options.transpile,
      &emit_options,
    )?;
    Ok((source.text, source.source_map.unwrap()))
  }

  /// Expects a file URL, panics otherwise.
  pub fn emit_for_hmr(
    &self,
    specifier: &Url,
    source_code: String,
  ) -> Result<String, JsErrorBox> {
    let media_type = MediaType::from_specifier(specifier);
    match media_type {
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Jsx
      | MediaType::Tsx => {
        let source_arc: ArcStr = source_code.into();
        let parsed_source = self
          .parsed_source_cache
          .remove_or_parse_module(specifier, media_type, source_arc)
          .map_err(JsErrorBox::from_err)?;
        // HMR doesn't work with embedded source maps for some reason, so set
        // the option to not use them (though you should test this out because
        // this statement is probably wrong)
        let transpile_and_emit_options = self
          .compiler_options_resolver
          .for_specifier(specifier)
          .transpile_options()
          .map_err(JsErrorBox::from_err)?;
        let mut options = transpile_and_emit_options.emit.clone();
        options.source_map = SourceMapOption::None;
        let is_cjs = self
          .cjs_tracker
          .is_cjs_with_known_is_script(
            specifier,
            media_type,
            parsed_source.compute_is_script(),
          )
          .map_err(JsErrorBox::from_err)?;
        let transpiled_source = parsed_source
          .transpile(
            &transpile_and_emit_options.transpile,
            &deno_ast::TranspileModuleOptions {
              module_kind: Some(ModuleKind::from_is_cjs(is_cjs)),
            },
            &options,
          )
          .map_err(JsErrorBox::from_err)?
          .into_source();
        Ok(transpiled_source.text)
      }
      MediaType::JavaScript
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Json
      | MediaType::Jsonc
      | MediaType::Json5
      | MediaType::Wasm
      | MediaType::Css
      | MediaType::Html
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => {
        // clear this specifier from the parsed source cache as it's now out of date
        self.parsed_source_cache.free(specifier);
        Ok(source_code)
      }
    }
  }

  /// A hashing function that takes the source code and uses the global emit
  /// options then generates a string hash which can be stored to
  /// determine if the cached emit is valid or not.
  fn get_source_hash(
    &self,
    module_kind: ModuleKind,
    transpile_and_emit: &TranspileAndEmitOptions,
    source_text: &str,
  ) -> u64 {
    let mut hasher = twox_hash::XxHash64::default();
    source_text.hash(&mut hasher);
    transpile_and_emit.pre_computed_hash.hash(&mut hasher);
    module_kind.hash(&mut hasher);
    hasher.finish()
  }
}

#[allow(clippy::result_large_err)]
trait ParsedSourceProvider: MaybeSend + MaybeSync + Clone + 'static {
  fn specifier(&self) -> &Url;
  fn media_type(&self) -> MediaType;
  fn source(&self) -> &ArcStr;
  fn into_source(self) -> ArcStr;
  fn parsed_source(self) -> Result<ParsedSource, deno_ast::ParseDiagnostic>;
}

#[derive(Clone)]
struct ParsedSourceCacheParsedSourceProvider {
  parsed_source_cache: ParsedSourceCacheRc,
  specifier: Url,
  media_type: MediaType,
  source: ArcStr,
}

impl ParsedSourceProvider for ParsedSourceCacheParsedSourceProvider {
  fn specifier(&self) -> &Url {
    &self.specifier
  }
  fn media_type(&self) -> MediaType {
    self.media_type
  }
  fn source(&self) -> &ArcStr {
    &self.source
  }
  fn into_source(self) -> ArcStr {
    self.source
  }
  fn parsed_source(self) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    self.parsed_source_cache.remove_or_parse_module(
      &self.specifier,
      self.media_type,
      self.source.clone(),
    )
  }
}

impl ParsedSourceProvider for ParsedSource {
  fn specifier(&self) -> &Url {
    ParsedSource::specifier(self)
  }
  fn media_type(&self) -> MediaType {
    ParsedSource::media_type(self)
  }
  fn source(&self) -> &ArcStr {
    self.text()
  }
  fn into_source(self) -> ArcStr {
    self.text().clone()
  }
  fn parsed_source(self) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    Ok(self)
  }
}

enum PreEmitResult {
  Cached(String),
  NotCached { source_hash: u64 },
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EmitParsedSourceHelperError {
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionsParse(#[from] CompilerOptionsParseError),
  #[class(inherit)]
  #[error(transparent)]
  ParseDiagnostic(#[from] deno_ast::ParseDiagnostic),
  #[class(inherit)]
  #[error(transparent)]
  Transpile(#[from] deno_ast::TranspileError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

/// Helper to share code between async and sync emit_parsed_source methods.
struct EmitParsedSourceHelper<
  'a,
  TInNpmPackageChecker: InNpmPackageChecker,
  TSys: EmitterSys,
>(&'a Emitter<TInNpmPackageChecker, TSys>);

impl<TInNpmPackageChecker: InNpmPackageChecker, TSys: EmitterSys>
  EmitParsedSourceHelper<'_, TInNpmPackageChecker, TSys>
{
  pub fn pre_emit_parsed_source(
    &self,
    specifier: &Url,
    module_kind: deno_ast::ModuleKind,
    transpile_and_emit_options: &TranspileAndEmitOptions,
    source: &ArcStr,
  ) -> PreEmitResult {
    let source_hash =
      self
        .0
        .get_source_hash(module_kind, transpile_and_emit_options, source);

    if let Some(emit_code) =
      self.0.emit_cache.get_emit_code(specifier, source_hash)
    {
      PreEmitResult::Cached(emit_code)
    } else {
      PreEmitResult::NotCached { source_hash }
    }
  }

  pub fn post_emit_parsed_source(
    &self,
    specifier: &Url,
    transpiled_source: &str,
    source_hash: u64,
  ) {
    self.0.emit_cache.set_emit_code(
      specifier,
      source_hash,
      transpiled_source.as_bytes(),
    );
  }
}

#[allow(clippy::result_large_err)]
fn transpile(
  parsed_source: ParsedSource,
  module_kind: deno_ast::ModuleKind,
  transpile_options: &deno_ast::TranspileOptions,
  emit_options: &deno_ast::EmitOptions,
) -> Result<EmittedSourceText, EmitParsedSourceHelperError> {
  ensure_no_import_assertion(&parsed_source)?;
  let transpile_result = parsed_source.transpile(
    transpile_options,
    &TranspileModuleOptions {
      module_kind: Some(module_kind),
    },
    emit_options,
  )?;
  let transpiled_source = match transpile_result {
    TranspileResult::Owned(source) => source,
    TranspileResult::Cloned(source) => {
      debug_assert!(false, "Transpile owned failed.");
      source
    }
  };
  Ok(transpiled_source)
}

// todo(dsherret): this is a temporary measure until we have swc erroring for this
fn ensure_no_import_assertion(
  parsed_source: &deno_ast::ParsedSource,
) -> Result<(), JsErrorBox> {
  fn has_import_assertion(text: &str) -> bool {
    // good enough
    text.contains(" assert ") && !text.contains(" with ")
  }

  fn create_err(
    parsed_source: &deno_ast::ParsedSource,
    range: SourceRange,
  ) -> JsErrorBox {
    let text_info = parsed_source.text_info_lazy();
    let loc = text_info.line_and_column_display(range.start);
    let mut msg = "Import assertions are deprecated. Use `with` keyword, instead of 'assert' keyword.".to_string();
    msg.push_str("\n\n");
    msg.push_str(range.text_fast(text_info));
    msg.push_str("\n\n");
    msg.push_str(&format!(
      "  at {}:{}:{}\n",
      parsed_source.specifier(),
      loc.line_number,
      loc.column_number,
    ));
    JsErrorBox::generic(msg)
  }

  let deno_ast::ProgramRef::Module(module) = parsed_source.program_ref() else {
    return Ok(());
  };

  for item in &module.body {
    match item {
      deno_ast::swc::ast::ModuleItem::ModuleDecl(decl) => match decl {
        deno_ast::swc::ast::ModuleDecl::Import(n) => {
          if n.with.is_some()
            && has_import_assertion(n.text_fast(parsed_source.text_info_lazy()))
          {
            return Err(create_err(parsed_source, n.range()));
          }
        }
        deno_ast::swc::ast::ModuleDecl::ExportAll(n) => {
          if n.with.is_some()
            && has_import_assertion(n.text_fast(parsed_source.text_info_lazy()))
          {
            return Err(create_err(parsed_source, n.range()));
          }
        }
        deno_ast::swc::ast::ModuleDecl::ExportNamed(n) => {
          if n.with.is_some()
            && has_import_assertion(n.text_fast(parsed_source.text_info_lazy()))
          {
            return Err(create_err(parsed_source, n.range()));
          }
        }
        deno_ast::swc::ast::ModuleDecl::ExportDecl(_)
        | deno_ast::swc::ast::ModuleDecl::ExportDefaultDecl(_)
        | deno_ast::swc::ast::ModuleDecl::ExportDefaultExpr(_)
        | deno_ast::swc::ast::ModuleDecl::TsImportEquals(_)
        | deno_ast::swc::ast::ModuleDecl::TsExportAssignment(_)
        | deno_ast::swc::ast::ModuleDecl::TsNamespaceExport(_) => {}
      },
      deno_ast::swc::ast::ModuleItem::Stmt(_) => {}
    }
  }

  Ok(())
}
