// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_error::JsErrorBox;
use deno_path_util::url_to_file_path;
use futures::FutureExt;
use futures::StreamExt;
use futures::future::LocalBoxFuture;
use futures::stream::FuturesUnordered;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::InNpmPackageChecker;
use crate::IsBuiltInNodeModuleChecker;
use crate::NodeResolutionKind;
use crate::NodeResolverSys;
use crate::NpmPackageFolderResolver;
use crate::PackageJsonResolverRc;
use crate::PathClean;
use crate::ResolutionMode;
use crate::UrlOrPath;
use crate::UrlOrPathRef;
use crate::errors::ModuleNotFoundError;
use crate::resolution::NodeResolverRc;
use crate::resolution::parse_npm_pkg_name;

#[derive(Debug, Clone)]
pub enum CjsAnalysis<'a> {
  /// File was found to be an ES module and the translator should
  /// load the code as ESM.
  Esm(Cow<'a, str>, Option<CjsAnalysisExports>),
  Cjs(CjsAnalysisExports),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CjsAnalysisExports {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
}

/// What parts of an ES module should be analyzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsmAnalysisMode {
  SourceOnly,
  SourceImportsAndExports,
}

/// Code analyzer for CJS and ESM files.
#[async_trait::async_trait(?Send)]
pub trait CjsCodeAnalyzer {
  /// Analyzes CommonJs code for exports and reexports, which is
  /// then used to determine the wrapper ESM module exports.
  ///
  /// Note that the source is provided by the caller when the caller
  /// already has it. If the source is needed by the implementation,
  /// then it can use the provided source, or otherwise load it if
  /// necessary.
  async fn analyze_cjs<'a>(
    &self,
    specifier: &Url,
    maybe_source: Option<Cow<'a, str>>,
    esm_analysis_mode: EsmAnalysisMode,
  ) -> Result<CjsAnalysis<'a>, JsErrorBox>;
}

pub enum ResolvedCjsAnalysis<'a> {
  Esm(Cow<'a, str>),
  Cjs(BTreeSet<String>),
}

#[sys_traits::auto_impl]
pub trait CjsModuleExportAnalyzerSys: NodeResolverSys {}

#[allow(clippy::disallowed_types)]
pub type CjsModuleExportAnalyzerRc<
  TCjsCodeAnalyzer,
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = deno_maybe_sync::MaybeArc<
  CjsModuleExportAnalyzer<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

pub struct CjsModuleExportAnalyzer<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: CjsModuleExportAnalyzerSys,
> {
  cjs_code_analyzer: TCjsCodeAnalyzer,
  in_npm_pkg_checker: TInNpmPackageChecker,
  node_resolver: NodeResolverRc<
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  npm_resolver: TNpmPackageFolderResolver,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  sys: TSys,
}

impl<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: CjsModuleExportAnalyzerSys,
>
  CjsModuleExportAnalyzer<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    cjs_code_analyzer: TCjsCodeAnalyzer,
    in_npm_pkg_checker: TInNpmPackageChecker,
    node_resolver: NodeResolverRc<
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
    npm_resolver: TNpmPackageFolderResolver,
    pkg_json_resolver: PackageJsonResolverRc<TSys>,
    sys: TSys,
  ) -> Self {
    Self {
      cjs_code_analyzer,
      in_npm_pkg_checker,
      node_resolver,
      npm_resolver,
      pkg_json_resolver,
      sys,
    }
  }

  pub async fn analyze_all_exports<'a>(
    &self,
    entry_specifier: &Url,
    source: Option<Cow<'a, str>>,
  ) -> Result<ResolvedCjsAnalysis<'a>, TranslateCjsToEsmError> {
    let analysis = self
      .cjs_code_analyzer
      .analyze_cjs(entry_specifier, source, EsmAnalysisMode::SourceOnly)
      .await
      .map_err(TranslateCjsToEsmError::CjsCodeAnalysis)?;

    let analysis = match analysis {
      CjsAnalysis::Esm(source, _) => {
        return Ok(ResolvedCjsAnalysis::Esm(source));
      }
      CjsAnalysis::Cjs(analysis) => analysis,
    };

    // use a BTreeSet to make the output deterministic for v8's code cache
    let mut all_exports = analysis.exports.into_iter().collect::<BTreeSet<_>>();

    if !analysis.reexports.is_empty() {
      let mut errors = Vec::new();
      self
        .analyze_reexports(
          entry_specifier,
          analysis.reexports,
          &mut all_exports,
          &mut errors,
        )
        .await;

      // surface errors afterwards in a deterministic way
      if !errors.is_empty() {
        errors.sort_by_cached_key(|e| e.to_string());
        return Err(TranslateCjsToEsmError::ExportAnalysis(errors.remove(0)));
      }
    }

    Ok(ResolvedCjsAnalysis::Cjs(all_exports))
  }

  #[allow(clippy::needless_lifetimes)]
  async fn analyze_reexports<'a>(
    &'a self,
    entry_specifier: &url::Url,
    reexports: Vec<String>,
    all_exports: &mut BTreeSet<String>,
    // this goes through the modules concurrently, so collect
    // the errors in order to be deterministic
    errors: &mut Vec<JsErrorBox>,
  ) {
    struct Analysis {
      reexport_specifier: url::Url,
      analysis: CjsAnalysis<'static>,
    }

    type AnalysisFuture<'a> = LocalBoxFuture<'a, Result<Analysis, JsErrorBox>>;

    let mut handled_reexports: HashSet<Url> = HashSet::default();
    handled_reexports.insert(entry_specifier.clone());
    let mut analyze_futures: FuturesUnordered<AnalysisFuture<'a>> =
      FuturesUnordered::new();
    let cjs_code_analyzer = &self.cjs_code_analyzer;
    let mut handle_reexports =
      |referrer: url::Url,
       reexports: Vec<String>,
       analyze_futures: &mut FuturesUnordered<AnalysisFuture<'a>>,
       errors: &mut Vec<JsErrorBox>| {
        // 1. Resolve the re-exports and start a future to analyze each one
        for reexport in reexports {
          let result = self
            .resolve(
              &reexport,
              &referrer,
              // FIXME(bartlomieju): check if these conditions are okay, probably
              // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
              &[
                Cow::Borrowed("deno"),
                Cow::Borrowed("node"),
                Cow::Borrowed("require"),
                Cow::Borrowed("default"),
              ],
              NodeResolutionKind::Execution,
            )
            .and_then(|value| {
              value
                .map(|url_or_path| url_or_path.into_url())
                .transpose()
                .map_err(JsErrorBox::from_err)
            });
          let reexport_specifier = match result {
            Ok(Some(specifier)) => specifier,
            Ok(None) => continue,
            Err(err) => {
              errors.push(err);
              continue;
            }
          };

          if !handled_reexports.insert(reexport_specifier.clone()) {
            continue;
          }

          let referrer = referrer.clone();
          let future = async move {
            let analysis = cjs_code_analyzer
              .analyze_cjs(
                &reexport_specifier,
                None,
                EsmAnalysisMode::SourceImportsAndExports,
              )
              .await
              .map_err(|source| {
                JsErrorBox::from_err(CjsAnalysisCouldNotLoadError {
                  reexport,
                  reexport_specifier: reexport_specifier.clone(),
                  referrer: referrer.clone(),
                  source,
                })
              })?;

            Ok(Analysis {
              reexport_specifier,
              analysis,
            })
          }
          .boxed_local();
          analyze_futures.push(future);
        }
      };

    handle_reexports(
      entry_specifier.clone(),
      reexports,
      &mut analyze_futures,
      errors,
    );

    while let Some(analysis_result) = analyze_futures.next().await {
      // 2. Look at the analysis result and resolve its exports and re-exports
      let Analysis {
        reexport_specifier,
        analysis,
      } = match analysis_result {
        Ok(analysis) => analysis,
        Err(err) => {
          errors.push(err);
          continue;
        }
      };
      match analysis {
        CjsAnalysis::Cjs(analysis) | CjsAnalysis::Esm(_, Some(analysis)) => {
          if !analysis.reexports.is_empty() {
            handle_reexports(
              reexport_specifier.clone(),
              analysis.reexports,
              &mut analyze_futures,
              errors,
            );
          }

          all_exports.extend(
            analysis
              .exports
              .into_iter()
              .filter(|e| e.as_str() != "default"),
          );
        }
        CjsAnalysis::Esm(_, None) => {
          // should not hit this due to EsmAnalysisMode::SourceImportsAndExports
          debug_assert!(false);
        }
      }
    }
  }

  // todo(dsherret): what is going on here? Isn't this a bunch of duplicate code?
  fn resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    conditions: &[Cow<'static, str>],
    resolution_kind: NodeResolutionKind,
  ) -> Result<Option<UrlOrPath>, JsErrorBox> {
    if specifier.starts_with('/') {
      todo!();
    }

    let referrer = UrlOrPathRef::from_url(referrer);
    let referrer_path = referrer.path().unwrap();
    if specifier.starts_with("./") || specifier.starts_with("../") {
      if let Some(parent) = referrer_path.parent() {
        return self
          .file_extension_probe(parent.join(specifier), referrer_path)
          .map(|p| Some(UrlOrPath::Path(p)));
      } else {
        todo!();
      }
    }

    // We've got a bare specifier or maybe bare_specifier/blah.js"
    let (package_specifier, package_subpath, _is_scoped) =
      parse_npm_pkg_name(specifier, &referrer).map_err(JsErrorBox::from_err)?;

    let module_dir = match self
      .npm_resolver
      .resolve_package_folder_from_package(package_specifier, &referrer)
    {
      Err(err)
        if matches!(
          err.as_kind(),
          crate::errors::PackageFolderResolveErrorKind::PackageNotFound(..)
        ) =>
      {
        return Ok(None);
      }
      other => other.map_err(JsErrorBox::from_err)?,
    };

    let package_json_path = module_dir.join("package.json");
    let maybe_package_json = self
      .pkg_json_resolver
      .load_package_json(&package_json_path)
      .map_err(JsErrorBox::from_err)?;
    if let Some(package_json) = maybe_package_json {
      if let Some(exports) = &package_json.exports {
        return Some(
          self
            .node_resolver
            .package_exports_resolve(
              &package_json_path,
              &package_subpath,
              exports,
              Some(&referrer),
              ResolutionMode::Require,
              conditions,
              resolution_kind,
            )
            .map_err(JsErrorBox::from_err),
        )
        .transpose();
      }

      // old school
      if package_subpath != "." {
        let d = module_dir.join(package_subpath.as_ref());
        if self.sys.fs_is_dir_no_err(&d) {
          // subdir might have a package.json that specifies the entrypoint
          let package_json_path = d.join("package.json");
          let maybe_package_json = self
            .pkg_json_resolver
            .load_package_json(&package_json_path)
            .map_err(JsErrorBox::from_err)?;
          if let Some(package_json) = maybe_package_json
            && let Some(main) =
              self.node_resolver.legacy_fallback_resolve(&package_json)
          {
            return Ok(Some(UrlOrPath::Path(d.join(main).clean())));
          }

          return Ok(Some(UrlOrPath::Path(d.join("index.js").clean())));
        }
        return self
          .file_extension_probe(d, referrer_path)
          .map(|p| Some(UrlOrPath::Path(p)));
      } else if let Some(main) =
        self.node_resolver.legacy_fallback_resolve(&package_json)
      {
        return Ok(Some(UrlOrPath::Path(module_dir.join(main).clean())));
      } else {
        return Ok(Some(UrlOrPath::Path(module_dir.join("index.js").clean())));
      }
    }

    // as a fallback, attempt to resolve it via the ancestor directories
    let mut last = referrer_path;
    while let Some(parent) = last.parent() {
      if !self.in_npm_pkg_checker.in_npm_package_at_dir_path(parent) {
        break;
      }
      let path = if parent.ends_with("node_modules") {
        parent.join(specifier)
      } else {
        parent.join("node_modules").join(specifier)
      };
      if let Ok(path) = self.file_extension_probe(path, referrer_path) {
        return Ok(Some(UrlOrPath::Path(path)));
      }
      last = parent;
    }

    Err(JsErrorBox::from_err(ModuleNotFoundError {
      specifier: UrlOrPath::Path(PathBuf::from(specifier)),
      maybe_referrer: Some(UrlOrPath::Path(referrer_path.to_path_buf())),
      suggested_ext: None,
    }))
  }

  fn file_extension_probe(
    &self,
    p: PathBuf,
    referrer: &Path,
  ) -> Result<PathBuf, JsErrorBox> {
    let p = p.clean();
    if self.sys.fs_exists_no_err(&p) {
      let file_name = p.file_name().unwrap();
      let p_js =
        p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
      if self.sys.fs_is_file_no_err(&p_js) {
        return Ok(p_js);
      } else if self.sys.fs_is_dir_no_err(&p) {
        return Ok(p.join("index.js"));
      } else {
        return Ok(p);
      }
    } else if let Some(file_name) = p.file_name() {
      {
        let p_js =
          p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
        if self.sys.fs_is_file_no_err(&p_js) {
          return Ok(p_js);
        }
      }
      {
        let p_json =
          p.with_file_name(format!("{}.json", file_name.to_str().unwrap()));
        if self.sys.fs_is_file_no_err(&p_json) {
          return Ok(p_json);
        }
      }
    }
    Err(JsErrorBox::from_err(ModuleNotFoundError {
      specifier: UrlOrPath::Path(p),
      maybe_referrer: Some(UrlOrPath::Path(referrer.to_path_buf())),
      suggested_ext: None,
    }))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TranslateCjsToEsmError {
  #[class(inherit)]
  #[error(transparent)]
  CjsCodeAnalysis(JsErrorBox),
  #[class(inherit)]
  #[error(transparent)]
  ExportAnalysis(JsErrorBox),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error(
  "Could not load '{reexport}' ({reexport_specifier}) referenced from {referrer}"
)]
pub struct CjsAnalysisCouldNotLoadError {
  reexport: String,
  reexport_specifier: Url,
  referrer: Url,
  #[source]
  source: JsErrorBox,
}

#[sys_traits::auto_impl]
pub trait NodeCodeTranslatorSys: CjsModuleExportAnalyzerSys {}

#[allow(clippy::disallowed_types)]
pub type NodeCodeTranslatorRc<
  TCjsCodeAnalyzer,
  TInNpmPackageChecker,
  TIsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver,
  TSys,
> = deno_maybe_sync::MaybeArc<
  NodeCodeTranslator<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
>;

pub struct NodeCodeTranslator<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NodeCodeTranslatorSys,
> {
  module_export_analyzer: CjsModuleExportAnalyzerRc<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >,
  mode: NodeCodeTranslatorMode,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum NodeCodeTranslatorMode {
  Disabled,
  #[default]
  ModuleLoader,
}

impl<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TInNpmPackageChecker: InNpmPackageChecker,
  TIsBuiltInNodeModuleChecker: IsBuiltInNodeModuleChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: NodeCodeTranslatorSys,
>
  NodeCodeTranslator<
    TCjsCodeAnalyzer,
    TInNpmPackageChecker,
    TIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >
{
  pub fn new(
    module_export_analyzer: CjsModuleExportAnalyzerRc<
      TCjsCodeAnalyzer,
      TInNpmPackageChecker,
      TIsBuiltInNodeModuleChecker,
      TNpmPackageFolderResolver,
      TSys,
    >,
    mode: NodeCodeTranslatorMode,
  ) -> Self {
    Self {
      module_export_analyzer,
      mode,
    }
  }

  /// Translates given CJS module into ESM. This function will perform static
  /// analysis on the file to find defined exports and reexports.
  ///
  /// For all discovered reexports the analysis will be performed recursively.
  ///
  /// If successful a source code for equivalent ES module is returned.
  pub async fn translate_cjs_to_esm<'a>(
    &self,
    entry_specifier: &Url,
    source: Option<Cow<'a, str>>,
  ) -> Result<Cow<'a, str>, TranslateCjsToEsmError> {
    let all_exports = if matches!(self.mode, NodeCodeTranslatorMode::Disabled) {
      return Ok(source.unwrap());
    } else {
      let analysis = self
        .module_export_analyzer
        .analyze_all_exports(entry_specifier, source)
        .await?;

      match analysis {
        ResolvedCjsAnalysis::Esm(source) => return Ok(source),
        ResolvedCjsAnalysis::Cjs(all_exports) => all_exports,
      }
    };
    Ok(Cow::Owned(exports_to_wrapper_module(
      entry_specifier,
      &all_exports,
    )))
  }
}

static RESERVED_WORDS: Lazy<HashSet<&str>> = Lazy::new(|| {
  HashSet::from([
    "abstract",
    "arguments",
    "async",
    "await",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "function",
    "get",
    "goto",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "int",
    "interface",
    "let",
    "long",
    "mod",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "set",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "volatile",
    "while",
    "with",
    "yield",
  ])
});

fn exports_to_wrapper_module(
  entry_specifier: &Url,
  all_exports: &BTreeSet<String>,
) -> String {
  let quoted_entry_specifier_text = to_double_quote_string(
    url_to_file_path(entry_specifier).unwrap().to_str().unwrap(),
  );
  let export_names_with_quoted = all_exports
    .iter()
    .map(|export| (export.as_str(), to_double_quote_string(export)))
    .collect::<Vec<_>>();
  capacity_builder::StringBuilder::<String>::build(|builder| {
      let mut temp_var_count = 0;
      builder.append(
        r#"import { createRequire as __internalCreateRequire, Module as __internalModule } from "node:module";
const require = __internalCreateRequire(import.meta.url);
let mod;
if (import.meta.main) {
  mod = __internalModule._load("#,
      );
      builder.append(&quoted_entry_specifier_text);
      builder.append(
        r#", null, true)
} else {
  mod = require("#,
      );
      builder.append(&quoted_entry_specifier_text);
      builder.append(r#");
}
"#);

      for (export_name, quoted_name) in &export_names_with_quoted {
        if !matches!(*export_name, "default" | "module.exports") {
          add_export(
            builder,
            export_name,
            quoted_name,
            |builder| {
              builder.append("mod[");
              builder.append(quoted_name);
              builder.append("]");
            },
            &mut temp_var_count,
          );
        }
      }

      builder.append("export default mod;\n");
      add_export(
        builder,
        "module.exports",
        "\"module.exports\"",
        |builder| builder.append("mod"),
        &mut temp_var_count,
      );
    }).unwrap()
}

fn add_export<'a>(
  builder: &mut capacity_builder::StringBuilder<'a, String>,
  name: &'a str,
  quoted_name: &'a str,
  build_initializer: impl FnOnce(&mut capacity_builder::StringBuilder<'a, String>),
  temp_var_count: &mut usize,
) {
  fn is_valid_var_decl(name: &str) -> bool {
    // it's ok to be super strict here
    if name.is_empty() {
      return false;
    }

    if let Some(first) = name.chars().next()
      && !first.is_ascii_alphabetic()
      && first != '_'
      && first != '$'
    {
      return false;
    }

    name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
  }

  // TODO(bartlomieju): Node actually checks if a given export exists in `exports` object,
  // but it might not be necessary here since our analysis is more detailed?
  if RESERVED_WORDS.contains(name) || !is_valid_var_decl(name) {
    *temp_var_count += 1;
    // we can't create an identifier with a reserved word or invalid identifier name,
    // so assign it to a temporary variable that won't have a conflict, then re-export
    // it as a string
    builder.append("const __deno_export_");
    builder.append(*temp_var_count);
    builder.append("__ = ");
    build_initializer(builder);
    builder.append(";\nexport { __deno_export_");
    builder.append(*temp_var_count);
    builder.append("__ as ");
    builder.append(quoted_name);
    builder.append(" };\n");
  } else {
    builder.append("export const ");
    builder.append(name);
    builder.append(" = ");
    build_initializer(builder);
    builder.append(";\n");
  }
}

fn to_double_quote_string(text: &str) -> String {
  // serde can handle this for us
  serde_json::to_string(text).unwrap()
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn test_exports_to_wrapper_module() {
    let url = Url::parse("file:///test/test.ts").unwrap();
    let exports = BTreeSet::from(
      ["static", "server", "app", "dashed-export", "3d"].map(|s| s.to_string()),
    );
    let text = exports_to_wrapper_module(&url, &exports);
    assert_eq!(
      text,
      r#"import { createRequire as __internalCreateRequire, Module as __internalModule } from "node:module";
const require = __internalCreateRequire(import.meta.url);
let mod;
if (import.meta.main) {
  mod = __internalModule._load("/test/test.ts", null, true)
} else {
  mod = require("/test/test.ts");
}
const __deno_export_1__ = mod["3d"];
export { __deno_export_1__ as "3d" };
export const app = mod["app"];
const __deno_export_2__ = mod["dashed-export"];
export { __deno_export_2__ as "dashed-export" };
export const server = mod["server"];
const __deno_export_3__ = mod["static"];
export { __deno_export_3__ as "static" };
export default mod;
const __deno_export_4__ = mod;
export { __deno_export_4__ as "module.exports" };
"#
    );
  }

  #[test]
  fn test_to_double_quote_string() {
    assert_eq!(to_double_quote_string("test"), "\"test\"");
    assert_eq!(
      to_double_quote_string("\r\n\t\"test"),
      "\"\\r\\n\\t\\\"test\""
    );
  }
}
