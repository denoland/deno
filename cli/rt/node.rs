use std::borrow::Cow;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_lib::loader::NpmModuleLoader;
use deno_media_type::MediaType;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmReqResolver;
use deno_runtime::deno_fs;
use deno_runtime::deno_node::RealIsBuiltInNodeModuleChecker;
use node_resolver::analyze::CjsAnalysis;
use node_resolver::analyze::CjsAnalysisExports;
use node_resolver::analyze::NodeCodeTranslator;

use crate::file_system::DenoCompileFileSystem;

pub type DenoRtCjsTracker = deno_resolver::cjs::CjsTracker<
  DenoInNpmPackageChecker,
  DenoCompileFileSystem,
>;
pub type DenoRtNpmResolver =
  deno_resolver::npm::NpmResolver<DenoCompileFileSystem>;
pub type DenoRtNpmModuleLoader = NpmModuleLoader<
  CjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  RealIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoCompileFileSystem,
>;
pub type DenoRtNodeCodeTranslator = NodeCodeTranslator<
  CjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  RealIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoCompileFileSystem,
>;
pub type DenoRtNodeResolver = deno_runtime::deno_node::NodeResolver<
  DenoInNpmPackageChecker,
  DenoRtNpmResolver,
  DenoCompileFileSystem,
>;
pub type DenoRtNpmReqResolver = NpmReqResolver<
  DenoInNpmPackageChecker,
  RealIsBuiltInNodeModuleChecker,
  DenoRtNpmResolver,
  DenoCompileFileSystem,
>;

pub struct CjsCodeAnalyzer {
  cjs_tracker: Arc<DenoRtCjsTracker>,
  fs: deno_fs::FileSystemRc,
}

impl CjsCodeAnalyzer {
  pub fn new(
    cjs_tracker: Arc<DenoRtCjsTracker>,
    fs: deno_fs::FileSystemRc,
  ) -> Self {
    Self { cjs_tracker, fs }
  }

  async fn inner_cjs_analysis<'a>(
    &self,
    specifier: &Url,
    source: Cow<'a, str>,
  ) -> Result<CjsAnalysis<'a>, AnyError> {
    let media_type = MediaType::from_specifier(specifier);
    if media_type == MediaType::Json {
      return Ok(CjsAnalysis::Cjs(CjsAnalysisExports {
        exports: vec![],
        reexports: vec![],
      }));
    }

    let cjs_tracker = self.cjs_tracker.clone();
    let is_maybe_cjs = cjs_tracker.is_maybe_cjs(specifier, media_type)?;
    let analysis = if is_maybe_cjs {
      let maybe_cjs = deno_core::unsync::spawn_blocking({
        let specifier = specifier.clone();
        let source: Arc<str> = source.to_string().into();
        move || -> Result<_, AnyError> {
          let parsed_source = deno_ast::parse_program(deno_ast::ParseParams {
            specifier,
            text: source.clone(),
            media_type,
            capture_tokens: true,
            scope_analysis: false,
            maybe_syntax: None,
          })?;
          let is_script = parsed_source.compute_is_script();
          let is_cjs = cjs_tracker.is_cjs_with_known_is_script(
            parsed_source.specifier(),
            media_type,
            is_script,
          )?;
          if is_cjs {
            let analysis = parsed_source.analyze_cjs();
            Ok(Some(CjsAnalysisExports {
              exports: analysis.exports,
              reexports: analysis.reexports,
            }))
          } else {
            Ok(None)
          }
        }
      })
      .await
      .unwrap()?;
      match maybe_cjs {
        Some(cjs) => CjsAnalysis::Cjs(cjs),
        None => CjsAnalysis::Esm(source),
      }
    } else {
      CjsAnalysis::Esm(source)
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
  ) -> Result<CjsAnalysis<'a>, AnyError> {
    let source = match source {
      Some(source) => source,
      None => {
        if let Ok(path) = specifier.to_file_path() {
          if let Ok(source_from_file) =
            self.fs.read_text_file_lossy_async(path, None).await
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
    self.inner_cjs_analysis(specifier, source).await
  }
}
