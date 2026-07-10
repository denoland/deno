// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;

use deno_error::JsErrorBox;
use deno_maybe_sync::MaybeDashMap;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_media_type::MediaType;
use node_resolver::analyze::CjsAnalysis as ExtNodeCjsAnalysis;
use node_resolver::analyze::CjsAnalysisExports;
use node_resolver::analyze::CjsCodeAnalyzer;
use node_resolver::analyze::CjsMemberReExport;
use node_resolver::analyze::EsmAnalysisMode;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use super::CjsTrackerRc;
use crate::npm::DenoInNpmPackageChecker;

#[cfg(feature = "deno_ast")]
mod deno_ast;

#[cfg(feature = "deno_ast")]
pub use deno_ast::DenoAstModuleExportAnalyzer;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleExportsAndReExports {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
  /// Re-exports that pin down a specific member of the inner module.
  /// Set when the analyzer detects a `module.exports = require("X").MEMBER`
  /// shape. Downstream analysis filters the inner module's exports to
  /// those statically attached to `MEMBER` rather than treating the
  /// whole inner module as the re-export source.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub member_reexports: Vec<MemberReExport>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemberReExport {
  pub specifier: String,
  pub member: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenoCjsAnalysis {
  /// The module was found to be an ES module.
  Esm,
  /// The module was found to be an ES module and
  /// it was analyzed for imports and exports.
  EsmAnalysis(ModuleExportsAndReExports),
  /// The module was CJS.
  Cjs(ModuleExportsAndReExports),
}

#[derive(Debug, Copy, Clone)]
pub struct NodeAnalysisCacheSourceHash(pub u64);

#[allow(clippy::disallowed_types, reason = "definition")]
pub type NodeAnalysisCacheRc = deno_maybe_sync::MaybeArc<dyn NodeAnalysisCache>;

pub trait NodeAnalysisCache: MaybeSend + MaybeSync {
  fn compute_source_hash(&self, source: &str) -> NodeAnalysisCacheSourceHash;
  fn get_cjs_analysis(
    &self,
    specifier: &Url,
    source_hash: NodeAnalysisCacheSourceHash,
  ) -> Option<DenoCjsAnalysis>;
  fn set_cjs_analysis(
    &self,
    specifier: &Url,
    source_hash: NodeAnalysisCacheSourceHash,
    analysis: &DenoCjsAnalysis,
  );
}

pub struct NullNodeAnalysisCache;

impl NodeAnalysisCache for NullNodeAnalysisCache {
  fn compute_source_hash(&self, _source: &str) -> NodeAnalysisCacheSourceHash {
    NodeAnalysisCacheSourceHash(0)
  }

  fn get_cjs_analysis(
    &self,
    _specifier: &Url,
    _source_hash: NodeAnalysisCacheSourceHash,
  ) -> Option<DenoCjsAnalysis> {
    None
  }

  fn set_cjs_analysis(
    &self,
    _specifier: &Url,
    _source_hash: NodeAnalysisCacheSourceHash,
    _analysis: &DenoCjsAnalysis,
  ) {
  }
}

#[sys_traits::auto_impl]
pub trait DenoCjsCodeAnalyzerSys:
  sys_traits::FsRead + sys_traits::FsMetadata + MaybeSend + MaybeSync + 'static
{
}

pub trait ModuleForExportAnalysis {
  fn specifier(&self) -> &Url;
  fn compute_is_script(&self) -> bool;
  fn analyze_cjs(&self) -> ModuleExportsAndReExports;
  fn analyze_es_runtime_exports(&self) -> ModuleExportsAndReExports;
  /// For every top-level `exports.MEMBER = IDENT` whose IDENT also
  /// receives static `IDENT.X = …` assignments at the top level, return
  /// the map from MEMBER to those X names. Used by the
  /// `module.exports = require(X).MEMBER` wrapper shape to narrow the
  /// inner module's advertised names to those statically attached to
  /// MEMBER. Built in a single walk of the top-level statements.
  fn analyze_member_export_props(&self) -> BTreeMap<String, Vec<String>>;
}

#[allow(clippy::disallowed_types, reason = "definition")]
pub type ModuleExportAnalyzerRc =
  deno_maybe_sync::MaybeArc<dyn ModuleExportAnalyzer>;

#[allow(
  clippy::disallowed_types,
  reason = "source text is always stored as Arc<str>"
)]
type ArcStr = std::sync::Arc<str>;

pub trait ModuleExportAnalyzer: MaybeSend + MaybeSync {
  fn parse_module(
    &self,
    specifier: Url,
    media_type: MediaType,
    source: ArcStr,
  ) -> Result<Box<dyn ModuleForExportAnalysis>, JsErrorBox>;
}

/// A module export analyzer that will error when parsing a module.
pub struct NotImplementedModuleExportAnalyzer;

impl ModuleExportAnalyzer for NotImplementedModuleExportAnalyzer {
  fn parse_module(
    &self,
    _specifier: Url,
    _media_type: MediaType,
    _source: ArcStr,
  ) -> Result<Box<dyn ModuleForExportAnalysis>, JsErrorBox> {
    panic!("Enable the deno_ast feature to get module export analysis.");
  }
}

#[allow(clippy::disallowed_types, reason = "definition")]
pub type DenoCjsCodeAnalyzerRc<TSys> =
  deno_maybe_sync::MaybeArc<DenoCjsCodeAnalyzer<TSys>>;

type MemberPropsMap = BTreeMap<String, Vec<String>>;

#[allow(clippy::disallowed_types, reason = "definition")]
type MemberPropsCache = deno_maybe_sync::MaybeArc<
  MaybeDashMap<(Url, u64), deno_maybe_sync::MaybeArc<MemberPropsMap>>,
>;

pub struct DenoCjsCodeAnalyzer<TSys: DenoCjsCodeAnalyzerSys> {
  cache: NodeAnalysisCacheRc,
  cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
  module_export_analyzer: ModuleExportAnalyzerRc,
  /// Memoizes the per-member property map keyed by `(specifier,
  /// source_hash)`. Built once per inner module so that repeated
  /// `analyze_cjs_member_props` calls (different members on the same
  /// module, or the same lookup across multiple importers) do not
  /// re-parse the source.
  member_props_cache: MemberPropsCache,
}

impl<TSys: DenoCjsCodeAnalyzerSys> DenoCjsCodeAnalyzer<TSys> {
  pub fn new(
    cache: NodeAnalysisCacheRc,
    cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
    module_export_analyzer: ModuleExportAnalyzerRc,
  ) -> Self {
    Self {
      cache,
      cjs_tracker,
      module_export_analyzer,
      member_props_cache: Default::default(),
    }
  }

  async fn inner_cjs_analysis(
    &self,
    specifier: &Url,
    source: &str,
    esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<DenoCjsAnalysis, JsErrorBox> {
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(source); // strip BOM
    let source_hash = self.cache.compute_source_hash(source);
    if let Some(analysis) = self.cache.get_cjs_analysis(specifier, source_hash)
    {
      return Ok(analysis);
    }

    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(DenoCjsAnalysis::Cjs(Default::default()));
    }

    // Non-script files can't carry CJS exports. Extensions answer this for
    // everything except extensionless files (`MediaType::Unknown`), which may
    // be real modules (an npm `"main"` with no extension — see
    // test-module-main-extension-lookup) OR binary assets a framework happened
    // to `require()`. Feeding binary to swc panics (it asserts on a backwards
    // span), so for `Unknown` we only proceed when the source looks like text
    // rather than blanket-skipping every extensionless module.
    let is_definitely_non_script = !matches!(
      media_type,
      MediaType::JavaScript
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::Jsx
        | MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Tsx
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Unknown
    );
    let looks_binary = source.contains('\0') || source.contains('\u{FFFD}');
    if is_definitely_non_script
      || (media_type == MediaType::Unknown && looks_binary)
    {
      return Ok(DenoCjsAnalysis::Esm);
    }

    let cjs_tracker = self.cjs_tracker.clone();
    let is_maybe_cjs = cjs_tracker
      .is_maybe_cjs(specifier, media_type)
      .map_err(JsErrorBox::from_err)?;
    let analysis = if is_maybe_cjs
      || esm_analysis_mode == EsmAnalysisMode::SourceImportsAndExports
    {
      let module_export_analyzer = self.module_export_analyzer.clone();

      let analyze = {
        let specifier = specifier.clone();
        let source: ArcStr = source.into();
        move || -> Result<_, JsErrorBox> {
          let parsed_source = module_export_analyzer
            .parse_module(specifier, media_type, source)?;
          let is_script = is_maybe_cjs && parsed_source.compute_is_script();
          let is_cjs = is_maybe_cjs
            && cjs_tracker
              .is_cjs_with_known_is_script(
                parsed_source.specifier(),
                media_type,
                is_script,
              )
              .map_err(JsErrorBox::from_err)?;
          if is_cjs {
            let analysis = parsed_source.analyze_cjs();
            Ok(DenoCjsAnalysis::Cjs(analysis))
          } else {
            match esm_analysis_mode {
              EsmAnalysisMode::SourceOnly => Ok(DenoCjsAnalysis::Esm),
              EsmAnalysisMode::SourceImportsAndExports => {
                Ok(DenoCjsAnalysis::EsmAnalysis(
                  parsed_source.analyze_es_runtime_exports(),
                ))
              }
            }
          }
        }
      };

      #[cfg(feature = "sync")]
      {
        crate::rt::spawn_blocking(analyze).await.unwrap()?
      }
      #[cfg(not(feature = "sync"))]
      analyze()?
    } else {
      DenoCjsAnalysis::Esm
    };

    self
      .cache
      .set_cjs_analysis(specifier, source_hash, &analysis);

    Ok(analysis)
  }
}

#[async_trait::async_trait(?Send)]
impl<TSys: DenoCjsCodeAnalyzerSys> CjsCodeAnalyzer
  for DenoCjsCodeAnalyzer<TSys>
{
  async fn analyze_cjs<'a>(
    &self,
    specifier: &Url,
    source: Option<Cow<'a, str>>,
    esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<ExtNodeCjsAnalysis<'a>, JsErrorBox> {
    let source = match source {
      Some(source) => source,
      // The analyzer may be asked to recursively inspect CJS re-exports
      // after only resolving the target specifier. Do not load that target
      // independently here because it may not match the source selected by
      // the caller's module loader. Callers should pass loader-owned source
      // explicitly when recursive analysis is available.
      None => return Ok(ExtNodeCjsAnalysis::Cjs(empty_cjs_analysis())),
    };
    let analysis = self
      .inner_cjs_analysis(specifier, &source, esm_analysis_mode)
      .await?;
    match analysis {
      DenoCjsAnalysis::Esm => Ok(ExtNodeCjsAnalysis::Esm(source, None)),
      DenoCjsAnalysis::EsmAnalysis(analysis) => Ok(ExtNodeCjsAnalysis::Esm(
        source,
        Some(to_ext_cjs_analysis_exports(analysis)),
      )),
      DenoCjsAnalysis::Cjs(analysis) => Ok(ExtNodeCjsAnalysis::Cjs(
        to_ext_cjs_analysis_exports(analysis),
      )),
    }
  }

  async fn analyze_cjs_member_props<'a>(
    &self,
    specifier: &Url,
    maybe_source: Option<Cow<'a, str>>,
    member: &str,
  ) -> Result<Option<Vec<String>>, JsErrorBox> {
    let source = match maybe_source {
      Some(source) => source,
      // See `analyze_cjs`: callers must provide loader-owned source. Without
      // it, report that the member could not be statically narrowed.
      None => return Ok(None),
    };
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(&source);
    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(None);
    }
    let source_hash = self.cache.compute_source_hash(source).0;
    let cache_key = (specifier.clone(), source_hash);
    if let Some(map) = self
      .member_props_cache
      .get(&cache_key)
      .map(|entry| entry.clone())
    {
      return Ok(map.get(member).cloned());
    }

    let module_export_analyzer = self.module_export_analyzer.clone();
    let parse_specifier = specifier.clone();
    let source_arc: ArcStr = source.into();
    let analyze = move || -> Result<MemberPropsMap, JsErrorBox> {
      let parsed = module_export_analyzer.parse_module(
        parse_specifier,
        media_type,
        source_arc,
      )?;
      Ok(parsed.analyze_member_export_props())
    };
    #[cfg(feature = "sync")]
    let map = crate::rt::spawn_blocking(analyze).await.unwrap()?;
    #[cfg(not(feature = "sync"))]
    let map = analyze()?;

    #[allow(clippy::disallowed_types, reason = "definition")]
    let map = deno_maybe_sync::MaybeArc::new(map);
    let result = map.get(member).cloned();
    self.member_props_cache.insert(cache_key, map);
    Ok(result)
  }
}

fn to_ext_cjs_analysis_exports(
  analysis: ModuleExportsAndReExports,
) -> CjsAnalysisExports {
  CjsAnalysisExports {
    exports: analysis.exports,
    reexports: analysis.reexports,
    member_reexports: analysis
      .member_reexports
      .into_iter()
      .map(|m| CjsMemberReExport {
        specifier: m.specifier,
        member: m.member,
      })
      .collect(),
  }
}

fn empty_cjs_analysis() -> CjsAnalysisExports {
  CjsAnalysisExports {
    exports: vec![],
    reexports: vec![],
    member_reexports: vec![],
  }
}
