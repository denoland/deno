// Copyright 2018-2026 the Deno authors. MIT license.

use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Range;

use anyhow::Error as AnyError;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use deno_ast::EmittedSourceText;
use deno_ast::ModuleKind;
use deno_ast::ParsedSource;
use deno_ast::SourceMapOption;
use deno_ast::SourceRange;
use deno_ast::SourceRanged;
use deno_ast::SourceRangedForSpanned;
use deno_ast::TranspileModuleOptions;
use deno_ast::TranspileResult;
use deno_ast::swc::ast::Decorator;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::ecma_visit::noop_visit_type;
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
use sourcemap::SourceMap;
use sourcemap::SourceMapBuilder;
use url::Url;

use crate::cache::EmitCacheRc;
use crate::cache::EmitCacheSys;
use crate::cache::ParsedSourceCacheRc;
use crate::cjs::CjsTrackerRc;
use crate::deno_json::CompilerOptionsParseError;
use crate::deno_json::CompilerOptionsResolverRc;
use crate::deno_json::TranspileAndEmitOptions;

#[allow(
  clippy::disallowed_types,
  reason = "source text is always stored as Arc<str>"
)]
type ArcStr = std::sync::Arc<str>;

#[allow(clippy::disallowed_types, reason = "definition")]
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
  /// When `true`, TypeScript-family modules emitted under the inspector are
  /// padded after transpile so source-mapped generated tokens keep their
  /// original line numbers. This lets tools that don't apply source maps — most
  /// notably the Chrome DevTools performance profiler — report locations that
  /// match the original source. See denoland/deno#25349.
  line_preserving_emit: bool,
}

impl<TInNpmPackageChecker: InNpmPackageChecker, TSys: EmitterSys>
  Emitter<TInNpmPackageChecker, TSys>
{
  pub fn new(
    cjs_tracker: CjsTrackerRc<TInNpmPackageChecker, TSys>,
    emit_cache: EmitCacheRc<TSys>,
    parsed_source_cache: ParsedSourceCacheRc,
    compiler_options_resolver: CompilerOptionsResolverRc,
    line_preserving_emit: bool,
  ) -> Self {
    Self {
      cjs_tracker,
      emit_cache,
      parsed_source_cache,
      compiler_options_resolver,
      line_preserving_emit,
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
        let line_preserving_emit = self.line_preserving_emit
          && supports_line_preserving_emit(&specifier, provider.media_type());
        let emit = {
          let transpile_and_emit_options = transpile_and_emit_options.clone();
          move || {
            let parsed_source = provider.parsed_source()?;
            transpile(
              parsed_source,
              module_kind,
              &transpile_and_emit_options.transpile,
              &transpile_and_emit_options.emit,
              line_preserving_emit,
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

  #[allow(
    clippy::result_large_err,
    reason = "EmitParsedSourceHelperError is intentionally large"
  )]
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
          self.line_preserving_emit
            && supports_line_preserving_emit(specifier, media_type),
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
      // `deno compile` provides the source map to v8 separately, so it should
      // keep the normal emitted source shape.
      false,
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
      | MediaType::Markdown
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
    // Only mix this in when enabled so the on-disk cache key (and therefore the
    // emitted output) is unchanged for the common, non-inspecting case.
    if self.line_preserving_emit {
      "line_preserving_emit".hash(&mut hasher);
    }
    hasher.finish()
  }
}

#[allow(
  clippy::result_large_err,
  reason = "EmitParsedSourceHelperError is intentionally large"
)]
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

#[allow(
  clippy::result_large_err,
  reason = "EmitParsedSourceHelperError is intentionally large"
)]
fn transpile(
  parsed_source: ParsedSource,
  module_kind: deno_ast::ModuleKind,
  transpile_options: &deno_ast::TranspileOptions,
  emit_options: &deno_ast::EmitOptions,
  line_preserving_emit: bool,
) -> Result<EmittedSourceText, EmitParsedSourceHelperError> {
  ensure_no_import_assertion(&parsed_source)?;
  if let Some(diagnostics) = invalid_syntax_parse_diagnostics(&parsed_source) {
    return Err(deno_ast::TranspileError::ParseErrors(diagnostics).into());
  }
  // Skip the decorator transform when the module has no decorators. The
  // swc decorator pass otherwise hoists every computed class-member key into
  // a `var _computedKey; _computedKey = ...;` pair at module scope, which
  // looks like a side effect and blocks downstream tree-shaking (e.g. in
  // `deno bundle`). See denoland/deno#30817.
  let owned_options;
  let transpile_options = if !matches!(
    transpile_options.decorators,
    deno_ast::DecoratorsTranspileOption::None
  ) && !program_has_decorators(
    parsed_source.program_ref(),
  ) {
    owned_options = deno_ast::TranspileOptions {
      decorators: deno_ast::DecoratorsTranspileOption::None,
      ..transpile_options.clone()
    };
    &owned_options
  } else {
    transpile_options
  };
  let transpile_result = parsed_source.transpile(
    transpile_options,
    &TranspileModuleOptions {
      module_kind: Some(module_kind),
    },
    emit_options,
  )?;
  let mut transpiled_source = match transpile_result {
    TranspileResult::Owned(source) => source,
    TranspileResult::Cloned(source) => source,
  };
  patch_public_decorator_access_has(&mut transpiled_source.text);
  if line_preserving_emit {
    maybe_pad_transpiled_lines_with_source_map(&mut transpiled_source);
  }
  Ok(transpiled_source)
}

static SOURCE_MAP_PREFIX: &str =
  "//# sourceMappingURL=data:application/json;base64,";

fn supports_line_preserving_emit(
  specifier: &Url,
  media_type: MediaType,
) -> bool {
  if specifier.path().contains("/$deno$eval.") {
    return false;
  }
  matches!(
    media_type,
    MediaType::TypeScript | MediaType::Mts | MediaType::Cts | MediaType::Tsx
  )
}

fn maybe_pad_transpiled_lines_with_source_map(
  transpiled_source: &mut EmittedSourceText,
) {
  let (source_map, inline_source_map_range) =
    match source_map_from_emitted_source(transpiled_source) {
      Some(result) => result,
      None => return,
    };

  let Some((padded_text, line_offsets)) =
    pad_text_lines_to_source_lines(&transpiled_source.text, &source_map)
  else {
    return;
  };

  let Some(updated_source_map) =
    offset_source_map_lines(&source_map, &line_offsets)
  else {
    return;
  };

  let Ok(updated_source_map) = source_map_to_string(&updated_source_map) else {
    return;
  };

  if inline_source_map_range.is_some() {
    let encoded_source_map = BASE64_STANDARD.encode(updated_source_map);
    let mut text = padded_text;
    text.push_str(SOURCE_MAP_PREFIX);
    text.push_str(&encoded_source_map);
    transpiled_source.text = text;
  } else {
    transpiled_source.text = padded_text;
    transpiled_source.source_map = Some(updated_source_map);
  }
}

fn source_map_from_emitted_source(
  transpiled_source: &EmittedSourceText,
) -> Option<(SourceMap, Option<Range<usize>>)> {
  if let Some(source_map) = &transpiled_source.source_map {
    return SourceMap::from_slice(source_map.as_bytes())
      .ok()
      .map(|source_map| (source_map, None));
  }

  let source_map_range = find_inline_source_map_range(&transpiled_source.text)?;
  let source_map = &transpiled_source.text[source_map_range.clone()];
  let source_map = source_map.strip_prefix(SOURCE_MAP_PREFIX)?;
  let source_map = BASE64_STANDARD.decode(source_map).ok()?;
  SourceMap::from_slice(&source_map)
    .ok()
    .map(|source_map| (source_map, Some(source_map_range)))
}

fn find_inline_source_map_range(text: &str) -> Option<Range<usize>> {
  let line_start = text.rfind('\n').map(|index| index + 1).unwrap_or(0);
  text[line_start..]
    .starts_with(SOURCE_MAP_PREFIX)
    .then_some(line_start..text.len())
}

fn pad_text_lines_to_source_lines(
  text: &str,
  source_map: &SourceMap,
) -> Option<(String, Vec<u32>)> {
  let source_map_range = find_inline_source_map_range(text);
  let text_without_source_map = source_map_range
    .as_ref()
    .map_or(text, |range| &text[..range.start]);
  let line_count = text_without_source_map.lines().count().max(1);
  let mut insertions_before_line = vec![0u32; line_count];
  let mut inserted_lines = 0u32;
  let mut current_dst_line = None;

  for token in source_map.tokens() {
    if !token.has_source() {
      continue;
    }
    let dst_line = token.get_dst_line() as usize;
    if dst_line >= insertions_before_line.len() {
      continue;
    }
    if current_dst_line == Some(dst_line) {
      continue;
    }
    current_dst_line = Some(dst_line);
    let shifted_line = token.get_dst_line() + inserted_lines;
    let src_line = token.get_src_line();
    if src_line > shifted_line {
      let insertions = src_line - shifted_line;
      insertions_before_line[dst_line] = insertions;
      inserted_lines += insertions;
    }
  }

  if inserted_lines == 0 {
    return None;
  }

  let mut padded_text = String::with_capacity(
    text_without_source_map.len() + inserted_lines as usize,
  );
  for (index, line) in text_without_source_map.split_inclusive('\n').enumerate()
  {
    for _ in 0..insertions_before_line
      .get(index)
      .copied()
      .unwrap_or_default()
    {
      padded_text.push('\n');
    }
    padded_text.push_str(line);
  }

  let line_offsets = cumulative_line_offsets(&insertions_before_line);
  Some((padded_text, line_offsets))
}

fn cumulative_line_offsets(insertions_before_line: &[u32]) -> Vec<u32> {
  let mut inserted_lines = 0;
  let mut line_offsets = Vec::with_capacity(insertions_before_line.len());
  for insertions in insertions_before_line {
    inserted_lines += *insertions;
    line_offsets.push(inserted_lines);
  }
  line_offsets
}

fn offset_source_map_lines(
  source_map: &SourceMap,
  line_offsets: &[u32],
) -> Option<SourceMap> {
  let mut builder = SourceMapBuilder::new(source_map.get_file());
  builder.set_source_root(source_map.get_source_root());
  for source_id in 0..source_map.get_source_count() {
    let source = source_map.get_source(source_id)?;
    let new_source_id = builder.add_source(source);
    builder.set_source_contents(
      new_source_id,
      source_map.get_source_contents(source_id),
    );
  }

  for token in source_map.tokens() {
    let mut raw_token = token.get_raw_token();
    let offset = line_offsets
      .get(raw_token.dst_line as usize)
      .copied()
      .unwrap_or_else(|| line_offsets.last().copied().unwrap_or_default());
    raw_token.dst_line += offset;
    builder.add(
      raw_token.dst_line,
      raw_token.dst_col,
      raw_token.src_line,
      raw_token.src_col,
      token.get_source(),
      token.get_name(),
      raw_token.is_range,
    );
  }

  Some(builder.into_sourcemap())
}

fn source_map_to_string(
  source_map: &SourceMap,
) -> Result<String, sourcemap::Error> {
  let mut source_map_bytes = Vec::new();
  source_map.to_writer(&mut source_map_bytes)?;
  Ok(
    String::from_utf8(source_map_bytes).expect("sourcemap emitted valid utf-8"),
  )
}

pub fn patch_public_decorator_access_has(source: &mut String) {
  if !source.contains("_apply_decs_2203_r") {
    return;
  }

  const OLD_EMITTED_ACCESS_OBJECT: &str = concat!(
    "    ctx.access = get && set ? {\n",
    "      get: get,\n",
    "      set: set\n",
    "    } : get ? {\n",
    "      get: get\n",
    "    } : {\n",
    "      set: set\n",
    "    };\n",
  );
  const NEW_EMITTED_ACCESS_OBJECT: &str = concat!(
    "    if (isPrivate) {\n",
    "      ctx.access = get && set ? { get: get, set: set } : get ? { get: get } : { set: set };\n",
    "    } else {\n",
    "      if (get) { var originalGet = get; get = function(target) { if (arguments.length === 0) target = this; return originalGet.call(target); }; }\n",
    "      if (set) { var originalSet = set; set = function(target, value) { if (arguments.length === 1) { value = target; target = this; } return originalSet.call(target, value); }; }\n",
    "      var has = function(target) { return name in target; };\n",
    "      ctx.access = get && set ? { has: has, get: get, set: set } : get ? { has: has, get: get } : { has: has, set: set };\n",
    "    }\n",
  );
  *source =
    source.replace(OLD_EMITTED_ACCESS_OBJECT, NEW_EMITTED_ACCESS_OBJECT);
}

fn program_has_decorators(program: deno_ast::ProgramRef<'_>) -> bool {
  #[derive(Default)]
  struct DecoratorDetector {
    found: bool,
  }

  impl Visit for DecoratorDetector {
    noop_visit_type!();

    fn visit_decorator(&mut self, _: &Decorator) {
      self.found = true;
    }
  }

  let mut detector = DecoratorDetector::default();
  program.visit_with(&mut detector);
  detector.found
}

/// When `swc` recovers from a syntax error it leaves an `Invalid` placeholder
/// node in the AST. The code generator emits these as the literal text
/// `<invalid>`, which then surfaces downstream as a misleading
/// `Uncaught SyntaxError: Unexpected token '<'` once the emitted output is
/// executed. If any such node is present, return the (otherwise non-fatal)
/// parse diagnostics so a precise syntax error can be reported instead of
/// emitting broken JavaScript. See denoland/deno#19457.
pub fn invalid_syntax_parse_diagnostics(
  parsed_source: &ParsedSource,
) -> Option<deno_ast::ParseDiagnosticsError> {
  let diagnostics = parsed_source.diagnostics();
  // Fast path: well-formed sources have no recovered-from diagnostics, and
  // `swc` only ever inserts an `Invalid` node alongside one, so there's no
  // need to walk the AST.
  if diagnostics.is_empty() {
    return None;
  }

  #[derive(Default)]
  struct InvalidNodeDetector {
    found: bool,
  }

  impl Visit for InvalidNodeDetector {
    noop_visit_type!();

    fn visit_invalid(&mut self, _: &deno_ast::swc::ast::Invalid) {
      self.found = true;
    }
  }

  let mut detector = InvalidNodeDetector::default();
  parsed_source.program_ref().visit_with(&mut detector);
  if !detector.found {
    return None;
  }

  Some(deno_ast::ParseDiagnosticsError(diagnostics.clone()))
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

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(specifier: &str, text: &str) -> ParsedSource {
    let specifier = Url::parse(specifier).unwrap();
    deno_ast::parse_module(deno_ast::ParseParams {
      media_type: MediaType::from_specifier(&specifier),
      specifier,
      text: text.into(),
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap()
  }

  #[test]
  fn line_preserving_padding_is_typescript_only() {
    let specifier = Url::parse("file:///mod.ts").unwrap();
    assert!(supports_line_preserving_emit(
      &specifier,
      MediaType::TypeScript
    ));
    assert!(supports_line_preserving_emit(&specifier, MediaType::Mts));
    assert!(supports_line_preserving_emit(&specifier, MediaType::Cts));
    assert!(supports_line_preserving_emit(&specifier, MediaType::Tsx));
    assert!(!supports_line_preserving_emit(
      &specifier,
      MediaType::JavaScript
    ));
    assert!(!supports_line_preserving_emit(&specifier, MediaType::Mjs));
    assert!(!supports_line_preserving_emit(&specifier, MediaType::Cjs));
    assert!(!supports_line_preserving_emit(&specifier, MediaType::Jsx));
    assert!(!supports_line_preserving_emit(
      &Url::parse("file:///$deno$eval.mts").unwrap(),
      MediaType::Mts
    ));
  }

  #[test]
  fn line_preserving_padding_preserves_function_line() {
    let src = parse(
      "file:///mod.ts",
      concat!(
        "interface Foo {\n",
        "  a: number;\n",
        "  b: string;\n",
        "}\n",
        "type Bar = Foo | null;\n",
        "export function target(x: number): number {\n",
        "  return x * 2;\n",
        "}\n",
      ),
    );
    let emitted = transpile(
      src.clone(),
      ModuleKind::Esm,
      &deno_ast::TranspileOptions::default(),
      &deno_ast::EmitOptions {
        source_map: SourceMapOption::Inline,
        ..Default::default()
      },
      true,
    )
    .unwrap();

    let line_of = |text: &str, needle: &str| {
      text.lines().position(|l| l.contains(needle)).unwrap()
    };
    assert_eq!(
      line_of(&emitted.text, "function target"),
      line_of(src.text(), "function target"),
    );

    let (source_map, _) = source_map_from_emitted_source(&emitted).unwrap();
    let function_line = line_of(&emitted.text, "function target") as u32;
    let token = source_map.lookup_token(function_line, 0).unwrap();
    assert_eq!(
      token.get_src_line(),
      line_of(src.text(), "function target") as u32
    );
    // Keep the inline map valid after mutating the emitted text.
    assert!(emitted.source_map.is_none());
    assert!(find_inline_source_map_range(&emitted.text).is_some());
  }

  #[test]
  fn line_preserving_padding_updates_separate_source_map() {
    let src = parse(
      "file:///mod.ts",
      "type T = number;\nexport function target(x: T): T { return x; }\n",
    );
    let emitted = transpile(
      src,
      ModuleKind::Esm,
      &deno_ast::TranspileOptions::default(),
      &deno_ast::EmitOptions {
        source_map: SourceMapOption::Separate,
        ..Default::default()
      },
      true,
    )
    .unwrap();

    assert!(emitted.source_map.is_some());
    let source_map =
      SourceMap::from_slice(emitted.source_map.as_ref().unwrap().as_bytes())
        .unwrap();
    let function_line = emitted
      .text
      .lines()
      .position(|l| l.contains("function target"))
      .unwrap();
    assert_eq!(function_line, 1);
    assert_eq!(
      source_map
        .lookup_token(function_line as u32, 0)
        .unwrap()
        .get_src_line(),
      1,
    );
  }
}
