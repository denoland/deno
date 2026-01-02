// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_error::JsErrorBox;
use deno_maybe_sync::MaybeSend;
use deno_maybe_sync::MaybeSync;
use deno_media_type::MediaType;
use node_resolver::analyze::CjsAnalysis as ExtNodeCjsAnalysis;
use node_resolver::analyze::CjsAnalysisExports;
use node_resolver::analyze::CjsCodeAnalyzer;
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

#[allow(clippy::disallowed_types)]
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
}

#[allow(clippy::disallowed_types)]
pub type ModuleExportAnalyzerRc =
  deno_maybe_sync::MaybeArc<dyn ModuleExportAnalyzer>;

#[allow(clippy::disallowed_types)]
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

#[allow(clippy::disallowed_types)]
pub type DenoCjsCodeAnalyzerRc<TSys> =
  deno_maybe_sync::MaybeArc<DenoCjsCodeAnalyzer<TSys>>;

pub struct DenoCjsCodeAnalyzer<TSys: DenoCjsCodeAnalyzerSys> {
  cache: NodeAnalysisCacheRc,
  cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
  module_export_analyzer: ModuleExportAnalyzerRc,
  sys: TSys,
}

impl<TSys: DenoCjsCodeAnalyzerSys> DenoCjsCodeAnalyzer<TSys> {
  pub fn new(
    cache: NodeAnalysisCacheRc,
    cjs_tracker: CjsTrackerRc<DenoInNpmPackageChecker, TSys>,
    module_export_analyzer: ModuleExportAnalyzerRc,
    sys: TSys,
  ) -> Self {
    Self {
      cache,
      cjs_tracker,
      module_export_analyzer,
      sys,
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
      None => {
        if let Ok(path) = deno_path_util::url_to_file_path(specifier) {
          if let Ok(source_from_file) = self.sys.fs_read_to_string_lossy(path) {
            source_from_file
          } else {
            return Ok(ExtNodeCjsAnalysis::Cjs(CjsAnalysisExports {
              exports: vec![],
              reexports: vec![],
            }));
          }
        } else {
          return Ok(ExtNodeCjsAnalysis::Cjs(CjsAnalysisExports {
            exports: vec![],
            reexports: vec![],
          }));
        }
      }
    };
    let analysis = self
      .inner_cjs_analysis(specifier, &source, esm_analysis_mode)
      .await?;
    match analysis {
      DenoCjsAnalysis::Esm => Ok(ExtNodeCjsAnalysis::Esm(source, None)),
      DenoCjsAnalysis::EsmAnalysis(analysis) => Ok(ExtNodeCjsAnalysis::Esm(
        source,
        Some(CjsAnalysisExports {
          exports: analysis.exports,
          reexports: analysis.reexports,
        }),
      )),
      DenoCjsAnalysis::Cjs(analysis) => {
        Ok(ExtNodeCjsAnalysis::Cjs(CjsAnalysisExports {
          exports: analysis.exports,
          reexports: analysis.reexports,
        }))
      }
    }
  }
}
