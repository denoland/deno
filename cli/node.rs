// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleExportsAndReExports;
use deno_ast::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_graph::ParsedSourceStore;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_fs;
use node_resolver::analyze::CjsAnalysis as ExtNodeCjsAnalysis;
use node_resolver::analyze::CjsAnalysisExports;
use node_resolver::analyze::CjsCodeAnalyzer;
use node_resolver::analyze::CjsModuleExportAnalyzer;
use node_resolver::analyze::EsmAnalysisMode;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use serde::Deserialize;
use serde::Serialize;

use crate::cache::CacheDBHash;
use crate::cache::NodeAnalysisCache;
use crate::cache::ParsedSourceCache;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::sys::CliSys;

pub type CliCjsModuleExportAnalyzer = CjsModuleExportAnalyzer<
  CliCjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliNodeCodeTranslator = NodeCodeTranslator<
  CliCjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliNodeResolver = deno_runtime::deno_node::NodeResolver<
  DenoInNpmPackageChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliPackageJsonResolver = node_resolver::PackageJsonResolver<CliSys>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CliCjsAnalysis {
  /// The module was found to be an ES module.
  Esm,
  /// The module was found to be an ES module and
  /// it was analyzed for imports and exports.
  EsmAnalysis(ModuleExportsAndReExports),
  /// The module was CJS.
  Cjs(ModuleExportsAndReExports),
}

pub struct CliCjsCodeAnalyzer {
  cache: NodeAnalysisCache,
  cjs_tracker: Arc<CliCjsTracker>,
  fs: deno_fs::FileSystemRc,
  parsed_source_cache: Option<Arc<ParsedSourceCache>>,
}

impl CliCjsCodeAnalyzer {
  pub fn new(
    cache: NodeAnalysisCache,
    cjs_tracker: Arc<CliCjsTracker>,
    fs: deno_fs::FileSystemRc,
    parsed_source_cache: Option<Arc<ParsedSourceCache>>,
  ) -> Self {
    Self {
      cache,
      cjs_tracker,
      fs,
      parsed_source_cache,
    }
  }

  async fn inner_cjs_analysis(
    &self,
    specifier: &ModuleSpecifier,
    source: &str,
    esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<CliCjsAnalysis, JsErrorBox> {
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(source); // strip BOM
    let source_hash = CacheDBHash::from_hashable(source);
    if let Some(analysis) =
      self.cache.get_cjs_analysis(specifier.as_str(), source_hash)
    {
      return Ok(analysis);
    }

    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(CliCjsAnalysis::Cjs(Default::default()));
    }

    let cjs_tracker = self.cjs_tracker.clone();
    let is_maybe_cjs = cjs_tracker
      .is_maybe_cjs(specifier, media_type)
      .map_err(JsErrorBox::from_err)?;
    let analysis = if is_maybe_cjs
      || esm_analysis_mode == EsmAnalysisMode::SourceImportsAndExports
    {
      let maybe_parsed_source = self
        .parsed_source_cache
        .as_ref()
        .and_then(|c| c.remove_parsed_source(specifier));

      deno_core::unsync::spawn_blocking({
        let specifier = specifier.clone();
        let source: Arc<str> = source.into();
        move || -> Result<_, JsErrorBox> {
          let parsed_source = maybe_parsed_source
            .map(Ok)
            .unwrap_or_else(|| {
              deno_ast::parse_program(deno_ast::ParseParams {
                specifier,
                text: source,
                media_type,
                capture_tokens: true,
                scope_analysis: false,
                maybe_syntax: None,
              })
            })
            .map_err(JsErrorBox::from_err)?;
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
            Ok(CliCjsAnalysis::Cjs(analysis))
          } else {
            match esm_analysis_mode {
              EsmAnalysisMode::SourceOnly => Ok(CliCjsAnalysis::Esm),
              EsmAnalysisMode::SourceImportsAndExports => {
                Ok(CliCjsAnalysis::EsmAnalysis(
                  parsed_source.analyze_es_runtime_exports(),
                ))
              }
            }
          }
        }
      })
      .await
      .unwrap()?
    } else {
      CliCjsAnalysis::Esm
    };

    self
      .cache
      .set_cjs_analysis(specifier.as_str(), source_hash, &analysis);

    Ok(analysis)
  }
}

#[async_trait::async_trait(?Send)]
impl CjsCodeAnalyzer for CliCjsCodeAnalyzer {
  async fn analyze_cjs<'a>(
    &self,
    specifier: &ModuleSpecifier,
    source: Option<Cow<'a, str>>,
    esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<ExtNodeCjsAnalysis<'a>, JsErrorBox> {
    let source = match source {
      Some(source) => source,
      None => {
        if let Ok(path) = specifier.to_file_path() {
          if let Ok(source_from_file) =
            self.fs.read_text_file_lossy_async(path, None).await
          {
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
      CliCjsAnalysis::Esm => Ok(ExtNodeCjsAnalysis::Esm(source, None)),
      CliCjsAnalysis::EsmAnalysis(analysis) => Ok(ExtNodeCjsAnalysis::Esm(
        source,
        Some(CjsAnalysisExports {
          exports: analysis.exports,
          reexports: analysis.reexports,
        }),
      )),
      CliCjsAnalysis::Cjs(analysis) => {
        Ok(ExtNodeCjsAnalysis::Cjs(CjsAnalysisExports {
          exports: analysis.exports,
          reexports: analysis.reexports,
        }))
      }
    }
  }
}
