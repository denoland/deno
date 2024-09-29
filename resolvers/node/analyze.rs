// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use futures::future::LocalBoxFuture;
use futures::stream::FuturesUnordered;
use futures::FutureExt;
use futures::StreamExt;
use once_cell::sync::Lazy;

use anyhow::Context;
use anyhow::Error as AnyError;
use url::Url;

use crate::env::NodeResolverEnv;
use crate::package_json::load_pkg_json;
use crate::resolution::NodeResolverRc;
use crate::NodeModuleKind;
use crate::NodeResolutionMode;
use crate::NpmResolverRc;
use crate::PathClean;

#[derive(Debug, Clone)]
pub enum CjsAnalysis {
  /// File was found to be an ES module and the translator should
  /// load the code as ESM.
  Esm(String),
  Cjs(CjsAnalysisExports),
}

#[derive(Debug, Clone)]
pub struct CjsAnalysisExports {
  pub exports: Vec<String>,
  pub reexports: Vec<String>,
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
  async fn analyze_cjs(
    &self,
    specifier: &Url,
    maybe_source: Option<String>,
  ) -> Result<CjsAnalysis, AnyError>;
}

pub struct NodeCodeTranslator<
  TCjsCodeAnalyzer: CjsCodeAnalyzer,
  TNodeResolverEnv: NodeResolverEnv,
> {
  cjs_code_analyzer: TCjsCodeAnalyzer,
  env: TNodeResolverEnv,
  node_resolver: NodeResolverRc<TNodeResolverEnv>,
  npm_resolver: NpmResolverRc,
}

impl<TCjsCodeAnalyzer: CjsCodeAnalyzer, TNodeResolverEnv: NodeResolverEnv>
  NodeCodeTranslator<TCjsCodeAnalyzer, TNodeResolverEnv>
{
  pub fn new(
    cjs_code_analyzer: TCjsCodeAnalyzer,
    env: TNodeResolverEnv,
    node_resolver: NodeResolverRc<TNodeResolverEnv>,
    npm_resolver: NpmResolverRc,
  ) -> Self {
    Self {
      cjs_code_analyzer,
      env,
      node_resolver,
      npm_resolver,
    }
  }

  /// Translates given CJS module into ESM. This function will perform static
  /// analysis on the file to find defined exports and reexports.
  ///
  /// For all discovered reexports the analysis will be performed recursively.
  ///
  /// If successful a source code for equivalent ES module is returned.
  pub async fn translate_cjs_to_esm(
    &self,
    entry_specifier: &Url,
    source: Option<String>,
  ) -> Result<String, AnyError> {
    let mut temp_var_count = 0;

    let analysis = self
      .cjs_code_analyzer
      .analyze_cjs(entry_specifier, source)
      .await?;

    let analysis = match analysis {
      CjsAnalysis::Esm(source) => return Ok(source),
      CjsAnalysis::Cjs(analysis) => analysis,
    };

    let mut source = vec![
      r#"import {createRequire as __internalCreateRequire} from "node:module";
      const require = __internalCreateRequire(import.meta.url);"#
        .to_string(),
    ];

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
        return Err(errors.remove(0));
      }
    }

    source.push(format!(
      "const mod = require(\"{}\");",
      url_to_file_path(entry_specifier)
        .unwrap()
        .to_str()
        .unwrap()
        .replace('\\', "\\\\")
        .replace('\'', "\\\'")
        .replace('\"', "\\\"")
    ));

    for export in &all_exports {
      if export.as_str() != "default" {
        add_export(
          &mut source,
          export,
          &format!("mod[\"{}\"]", escape_for_double_quote_string(export)),
          &mut temp_var_count,
        );
      }
    }

    source.push("export default mod;".to_string());

    let translated_source = source.join("\n");
    Ok(translated_source)
  }

  async fn analyze_reexports<'a>(
    &'a self,
    entry_specifier: &url::Url,
    reexports: Vec<String>,
    all_exports: &mut BTreeSet<String>,
    // this goes through the modules concurrently, so collect
    // the errors in order to be deterministic
    errors: &mut Vec<anyhow::Error>,
  ) {
    struct Analysis {
      reexport_specifier: url::Url,
      referrer: url::Url,
      analysis: CjsAnalysis,
    }

    type AnalysisFuture<'a> = LocalBoxFuture<'a, Result<Analysis, AnyError>>;

    let mut handled_reexports: HashSet<Url> = HashSet::default();
    handled_reexports.insert(entry_specifier.clone());
    let mut analyze_futures: FuturesUnordered<AnalysisFuture<'a>> =
      FuturesUnordered::new();
    let cjs_code_analyzer = &self.cjs_code_analyzer;
    let mut handle_reexports =
      |referrer: url::Url,
       reexports: Vec<String>,
       analyze_futures: &mut FuturesUnordered<AnalysisFuture<'a>>,
       errors: &mut Vec<anyhow::Error>| {
        // 1. Resolve the re-exports and start a future to analyze each one
        for reexport in reexports {
          let result = self.resolve(
            &reexport,
            &referrer,
            // FIXME(bartlomieju): check if these conditions are okay, probably
            // should be `deno-require`, because `deno` is already used in `esm_resolver.rs`
            &["deno", "node", "require", "default"],
            NodeResolutionMode::Execution,
          );
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
              .analyze_cjs(&reexport_specifier, None)
              .await
              .with_context(|| {
                format!(
                  "Could not load '{}' ({}) referenced from {}",
                  reexport, reexport_specifier, referrer
                )
              })?;

            Ok(Analysis {
              reexport_specifier,
              referrer,
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
        referrer,
        analysis,
      } = match analysis_result {
        Ok(analysis) => analysis,
        Err(err) => {
          errors.push(err);
          continue;
        }
      };
      match analysis {
        CjsAnalysis::Esm(_) => {
          // todo(dsherret): support this once supporting requiring ES modules
          errors.push(anyhow::anyhow!(
            "Cannot require ES module '{}' from '{}'",
            reexport_specifier,
            referrer,
          ));
        }
        CjsAnalysis::Cjs(analysis) => {
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
      }
    }
  }

  // todo(dsherret): what is going on here? Isn't this a bunch of duplicate code?
  fn resolve(
    &self,
    specifier: &str,
    referrer: &Url,
    conditions: &[&str],
    mode: NodeResolutionMode,
  ) -> Result<Option<Url>, AnyError> {
    if specifier.starts_with('/') {
      todo!();
    }

    let referrer_path = url_to_file_path(referrer).unwrap();
    if specifier.starts_with("./") || specifier.starts_with("../") {
      if let Some(parent) = referrer_path.parent() {
        return self
          .file_extension_probe(parent.join(specifier), &referrer_path)
          .and_then(|p| url_from_file_path(&p).map_err(AnyError::from))
          .map(Some);
      } else {
        todo!();
      }
    }

    // We've got a bare specifier or maybe bare_specifier/blah.js"
    let (package_specifier, package_subpath) =
      parse_specifier(specifier).unwrap();

    let module_dir = match self
      .npm_resolver
      .resolve_package_folder_from_package(package_specifier.as_str(), referrer)
    {
      Err(err)
        if matches!(
          err.as_kind(),
          crate::errors::PackageFolderResolveErrorKind::PackageNotFound(..)
        ) =>
      {
        return Ok(None);
      }
      other => other,
    }?;

    let package_json_path = module_dir.join("package.json");
    let maybe_package_json =
      load_pkg_json(self.env.pkg_json_fs(), &package_json_path)?;
    if let Some(package_json) = maybe_package_json {
      if let Some(exports) = &package_json.exports {
        return Some(
          self
            .node_resolver
            .package_exports_resolve(
              &package_json_path,
              &package_subpath,
              exports,
              Some(referrer),
              NodeModuleKind::Esm,
              conditions,
              mode,
            )
            .map_err(AnyError::from),
        )
        .transpose();
      }

      // old school
      if package_subpath != "." {
        let d = module_dir.join(package_subpath);
        if self.env.is_dir_sync(&d) {
          // subdir might have a package.json that specifies the entrypoint
          let package_json_path = d.join("package.json");
          let maybe_package_json =
            load_pkg_json(self.env.pkg_json_fs(), &package_json_path)?;
          if let Some(package_json) = maybe_package_json {
            if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
              return Ok(Some(url_from_file_path(&d.join(main).clean())?));
            }
          }

          return Ok(Some(url_from_file_path(&d.join("index.js").clean())?));
        }
        return self
          .file_extension_probe(d, &referrer_path)
          .and_then(|p| url_from_file_path(&p).map_err(AnyError::from))
          .map(Some);
      } else if let Some(main) = package_json.main(NodeModuleKind::Cjs) {
        return Ok(Some(url_from_file_path(&module_dir.join(main).clean())?));
      } else {
        return Ok(Some(url_from_file_path(
          &module_dir.join("index.js").clean(),
        )?));
      }
    }

    // as a fallback, attempt to resolve it via the ancestor directories
    let mut last = referrer_path.as_path();
    while let Some(parent) = last.parent() {
      if !self.npm_resolver.in_npm_package_at_dir_path(parent) {
        break;
      }
      let path = if parent.ends_with("node_modules") {
        parent.join(specifier)
      } else {
        parent.join("node_modules").join(specifier)
      };
      if let Ok(path) = self.file_extension_probe(path, &referrer_path) {
        return Ok(Some(url_from_file_path(&path)?));
      }
      last = parent;
    }

    Err(not_found(specifier, &referrer_path))
  }

  fn file_extension_probe(
    &self,
    p: PathBuf,
    referrer: &Path,
  ) -> Result<PathBuf, AnyError> {
    let p = p.clean();
    if self.env.exists_sync(&p) {
      let file_name = p.file_name().unwrap();
      let p_js =
        p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
      if self.env.is_file_sync(&p_js) {
        return Ok(p_js);
      } else if self.env.is_dir_sync(&p) {
        return Ok(p.join("index.js"));
      } else {
        return Ok(p);
      }
    } else if let Some(file_name) = p.file_name() {
      {
        let p_js =
          p.with_file_name(format!("{}.js", file_name.to_str().unwrap()));
        if self.env.is_file_sync(&p_js) {
          return Ok(p_js);
        }
      }
      {
        let p_json =
          p.with_file_name(format!("{}.json", file_name.to_str().unwrap()));
        if self.env.is_file_sync(&p_json) {
          return Ok(p_json);
        }
      }
    }
    Err(not_found(&p.to_string_lossy(), referrer))
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

fn add_export(
  source: &mut Vec<String>,
  name: &str,
  initializer: &str,
  temp_var_count: &mut usize,
) {
  fn is_valid_var_decl(name: &str) -> bool {
    // it's ok to be super strict here
    if name.is_empty() {
      return false;
    }

    if let Some(first) = name.chars().next() {
      if !first.is_ascii_alphabetic() && first != '_' && first != '$' {
        return false;
      }
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
    source.push(format!(
      "const __deno_export_{temp_var_count}__ = {initializer};"
    ));
    source.push(format!(
      "export {{ __deno_export_{temp_var_count}__ as \"{}\" }};",
      escape_for_double_quote_string(name)
    ));
  } else {
    source.push(format!("export const {name} = {initializer};"));
  }
}

fn parse_specifier(specifier: &str) -> Option<(String, String)> {
  let mut separator_index = specifier.find('/');
  let mut valid_package_name = true;
  // let mut is_scoped = false;
  if specifier.is_empty() {
    valid_package_name = false;
  } else if specifier.starts_with('@') {
    // is_scoped = true;
    if let Some(index) = separator_index {
      separator_index = specifier[index + 1..].find('/').map(|i| i + index + 1);
    } else {
      valid_package_name = false;
    }
  }

  let package_name = if let Some(index) = separator_index {
    specifier[0..index].to_string()
  } else {
    specifier.to_string()
  };

  // Package name cannot have leading . and cannot have percent-encoding or separators.
  for ch in package_name.chars() {
    if ch == '%' || ch == '\\' {
      valid_package_name = false;
      break;
    }
  }

  if !valid_package_name {
    return None;
  }

  let package_subpath = if let Some(index) = separator_index {
    format!(".{}", specifier.chars().skip(index).collect::<String>())
  } else {
    ".".to_string()
  };

  Some((package_name, package_subpath))
}

fn not_found(path: &str, referrer: &Path) -> AnyError {
  let msg = format!(
    "[ERR_MODULE_NOT_FOUND] Cannot find module \"{}\" imported from \"{}\"",
    path,
    referrer.to_string_lossy()
  );
  std::io::Error::new(std::io::ErrorKind::NotFound, msg).into()
}

fn escape_for_double_quote_string(text: &str) -> Cow<str> {
  // this should be rare, so doing a scan first before allocating is ok
  if text.chars().any(|c| matches!(c, '"' | '\\')) {
    // don't bother making this more complex for perf because it's rare
    Cow::Owned(text.replace('\\', "\\\\").replace('"', "\\\""))
  } else {
    Cow::Borrowed(text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_add_export() {
    let mut temp_var_count = 0;
    let mut source = vec![];

    let exports = vec!["static", "server", "app", "dashed-export", "3d"];
    for export in exports {
      add_export(&mut source, export, "init", &mut temp_var_count);
    }
    assert_eq!(
      source,
      vec![
        "const __deno_export_1__ = init;".to_string(),
        "export { __deno_export_1__ as \"static\" };".to_string(),
        "export const server = init;".to_string(),
        "export const app = init;".to_string(),
        "const __deno_export_2__ = init;".to_string(),
        "export { __deno_export_2__ as \"dashed-export\" };".to_string(),
        "const __deno_export_3__ = init;".to_string(),
        "export { __deno_export_3__ as \"3d\" };".to_string(),
      ]
    )
  }

  #[test]
  fn test_parse_specifier() {
    assert_eq!(
      parse_specifier("@some-package/core/actions"),
      Some(("@some-package/core".to_string(), "./actions".to_string()))
    );
  }
}
