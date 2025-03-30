// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_lib::loader::NpmModuleLoader;
use deno_lib::standalone::binary::CjsExportAnalysisEntry;
use deno_media_type::MediaType;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolver;
use deno_runtime::deno_fs::FileSystem;
use node_resolver::analyze::CjsAnalysis;
use node_resolver::analyze::CjsAnalysisExports;
use node_resolver::analyze::EsmAnalysisMode;
use node_resolver::analyze::NodeCodeTranslator;
use node_resolver::DenoIsBuiltInNodeModuleChecker;

use crate::binary::StandaloneModules;
use crate::file_system::DenoRtSys;

pub type DenoRtCjsTracker =
  deno_resolver::cjs::CjsTracker<DenoInNpmPackageChecker, DenoRtSys>;
pub type DenoRtNpmResolver = deno_resolver::npm::NpmResolver<DenoRtSys>;
pub type DenoRtNpmModuleLoader = NpmModuleLoader<
  CjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoRtSys,
>;
pub type DenoRtNodeCodeTranslator = NodeCodeTranslator<
  CjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoRtSys,
>;
pub type DenoRtNodeResolver = deno_runtime::deno_node::NodeResolver<
  DenoInNpmPackageChecker,
  DenoRtNpmResolver,
  DenoRtSys,
>;
pub type DenoRtNpmReqResolver = NpmReqResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoRtSys,
>;

pub struct CjsCodeAnalyzer {
  cjs_tracker: Arc<DenoRtCjsTracker>,
  modules: Arc<StandaloneModules>,
  sys: DenoRtSys,
}

impl CjsCodeAnalyzer {
  pub fn new(
    cjs_tracker: Arc<DenoRtCjsTracker>,
    modules: Arc<StandaloneModules>,
    sys: DenoRtSys,
  ) -> Self {
    Self {
      cjs_tracker,
      modules,
      sys,
    }
  }

  fn inner_cjs_analysis<'a>(
    &self,
    specifier: &Url,
    source: Cow<'a, str>,
  ) -> Result<CjsAnalysis<'a>, JsErrorBox> {
    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(CjsAnalysis::Cjs(CjsAnalysisExports {
        exports: vec![],
        reexports: vec![],
      }));
    }

    let cjs_tracker = self.cjs_tracker.clone();
    let is_maybe_cjs = cjs_tracker
      .is_maybe_cjs(specifier, media_type)
      .map_err(JsErrorBox::from_err)?;
    let analysis = if is_maybe_cjs {
      let data = self
        .modules
        .read(specifier)?
        .and_then(|d| d.cjs_export_analysis);
      match data {
        Some(data) => {
          let data: CjsExportAnalysisEntry = bincode::deserialize(&data)
            .map_err(|err| JsErrorBox::generic(err.to_string()))?;
          match data {
            CjsExportAnalysisEntry::Esm => {
              cjs_tracker.set_is_known_script(specifier, false);
              CjsAnalysis::Esm(source, None)
            }
            CjsExportAnalysisEntry::Cjs(exports) => {
              cjs_tracker.set_is_known_script(specifier, true);
              CjsAnalysis::Cjs(CjsAnalysisExports {
                exports,
                reexports: Vec::new(), // already resolved
              })
            }
            CjsExportAnalysisEntry::Error(err) => {
              return Err(JsErrorBox::generic(err));
            }
          }
        }
        None => {
          if log::log_enabled!(log::Level::Debug) {
            if self.sys.is_specifier_in_vfs(specifier) {
              log::debug!(
                "No CJS export analysis was stored for '{}'. Assuming ESM. This might indicate a bug in Deno.",
                specifier
              );
            } else {
              log::debug!(
                "Analyzing potentially CommonJS files is not supported at runtime in a compiled executable ({}). Assuming ESM.",
                specifier
              );
            }
          }
          // assume ESM as we don't have access to swc here
          CjsAnalysis::Esm(source, None)
        }
      }
    } else {
      CjsAnalysis::Esm(source, None)
    };

    Ok(analysis)
  }
}

#[async_trait::async_trait(?Send)]
impl node_resolver::analyze::CjsCodeAnalyzer for CjsCodeAnalyzer {
  async fn analyze_cjs<'a>(
    &self,
    specifier: &Url,
    source: Option<Cow<'a, str>>,
    _esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<CjsAnalysis<'a>, JsErrorBox> {
    let source = match source {
      Some(source) => source,
      None => {
        if let Ok(path) = deno_path_util::url_to_file_path(specifier) {
          // todo(dsherret): should this use the sync method instead?
          if let Ok(source_from_file) =
            self.sys.read_text_file_lossy_async(path, None).await
          {
            source_from_file
          } else {
            return Ok(CjsAnalysis::Cjs(CjsAnalysisExports {
              exports: vec![],
              reexports: vec![],
            }));
          }
        } else {
          return Ok(CjsAnalysis::Cjs(CjsAnalysisExports {
            exports: vec![],
            reexports: vec![],
          }));
        }
      }
    };
    self.inner_cjs_analysis(specifier, source)
  }
}
