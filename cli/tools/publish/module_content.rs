// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_resolver::cache::LazyGraphSourceParser;
use deno_resolver::cache::ParsedSourceCache;
use deno_resolver::deno_json::CompilerOptionsResolver;
use deno_resolver::workspace::ResolutionKind;
use lazy_regex::Lazy;
use sys_traits::FsMetadata;
use sys_traits::FsRead;

use super::diagnostics::PublishDiagnostic;
use super::diagnostics::PublishDiagnosticsCollector;
use super::unfurl::PositionOrSourceRangeRef;
use super::unfurl::SpecifierUnfurler;
use super::unfurl::SpecifierUnfurlerDiagnostic;
use crate::sys::CliSys;

struct JsxFolderOptions<'a> {
  jsx_runtime: &'static str,
  jsx_classic: Option<Cow<'a, deno_ast::JsxClassicOptions>>,
  jsx_import_source: Option<String>,
  jsx_import_source_types: Option<String>,
}

pub struct ModuleContentProvider<TSys: FsMetadata + FsRead = CliSys> {
  specifier_unfurler: SpecifierUnfurler<TSys>,
  parsed_source_cache: Arc<ParsedSourceCache>,
  sys: TSys,
  compiler_options_resolver: Arc<CompilerOptionsResolver>,
}

impl<TSys: FsMetadata + FsRead> ModuleContentProvider<TSys> {
  pub fn new(
    parsed_source_cache: Arc<ParsedSourceCache>,
    specifier_unfurler: SpecifierUnfurler<TSys>,
    sys: TSys,
    compiler_options_resolver: Arc<CompilerOptionsResolver>,
  ) -> Self {
    Self {
      specifier_unfurler,
      parsed_source_cache,
      sys,
      compiler_options_resolver,
    }
  }

  pub fn resolve_content_maybe_unfurling(
    &self,
    graph: &ModuleGraph,
    diagnostics_collector: &PublishDiagnosticsCollector,
    path: &Path,
    specifier: &Url,
  ) -> Result<Vec<u8>, AnyError> {
    let source_parser =
      LazyGraphSourceParser::new(&self.parsed_source_cache, graph);
    let media_type = MediaType::from_specifier(specifier);
    let parsed_source = match source_parser.get_or_parse_source(specifier)? {
      Some(parsed_source) => parsed_source,
      None => {
        let data = self.sys.fs_read(path).with_context(|| {
          format!("Unable to read file '{}'", path.display())
        })?;

        match media_type {
          MediaType::JavaScript
          | MediaType::Jsx
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::TypeScript
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
          | MediaType::Tsx => {
            // continue
          }
          MediaType::SourceMap
          | MediaType::Unknown
          | MediaType::Html
          | MediaType::Sql
          | MediaType::Json
          | MediaType::Jsonc
          | MediaType::Json5
          | MediaType::Wasm
          | MediaType::Css => {
            // not unfurlable data
            return Ok(data.into_owned());
          }
        }

        let text = String::from_utf8_lossy(&data);
        deno_ast::parse_module(deno_ast::ParseParams {
          specifier: specifier.clone(),
          text: text.into(),
          media_type,
          capture_tokens: false,
          maybe_syntax: None,
          scope_analysis: false,
        })?
      }
    };

    log::debug!("Unfurling {}", specifier);
    let mut reporter = |diagnostic| {
      diagnostics_collector
        .push(PublishDiagnostic::SpecifierUnfurl(diagnostic));
    };
    let text_info = parsed_source.text_info_lazy();
    let module_info =
      deno_graph::ast::ParserModuleAnalyzer::module_info(&parsed_source);
    let mut text_changes = Vec::new();
    if media_type.is_jsx() {
      self.add_jsx_text_changes(
        specifier,
        &parsed_source,
        text_info,
        &module_info,
        &mut reporter,
        &mut text_changes,
      )?;
    }

    self.specifier_unfurler.unfurl_to_changes(
      specifier,
      &parsed_source,
      &module_info,
      &mut text_changes,
      &mut reporter,
    );
    let rewritten_text =
      deno_ast::apply_text_changes(text_info.text_str(), text_changes);

    Ok(rewritten_text.into_bytes())
  }

  fn add_jsx_text_changes(
    &self,
    specifier: &Url,
    parsed_source: &ParsedSource,
    text_info: &SourceTextInfo,
    module_info: &deno_graph::analysis::ModuleInfo,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
    text_changes: &mut Vec<TextChange>,
  ) -> Result<(), AnyError> {
    static JSX_RUNTIME_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxRuntime\s+(\S+)");
    static JSX_FACTORY_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxFactory\s+(\S+)");
    static JSX_FRAGMENT_FACTORY_RE: Lazy<regex::Regex> =
      lazy_regex::lazy_regex!(r"(?i)^[\s*]*@jsxFragmentFactory\s+(\S+)");

    let start_pos = if parsed_source.program_ref().shebang().is_some() {
      match text_info.text_str().find('\n') {
        Some(index) => index + 1,
        None => return Ok(()), // nothing in this file
      }
    } else {
      0
    };
    let mut add_text_change = |new_text: String| {
      text_changes.push(TextChange {
        range: start_pos..start_pos,
        new_text,
      })
    };
    let jsx_options =
      self.resolve_jsx_options(specifier, text_info, diagnostic_reporter)?;
    let leading_comments = parsed_source.get_leading_comments();
    let leading_comments_has_re = |regex: &regex::Regex| {
      leading_comments
        .as_ref()
        .map(|comments| {
          comments.iter().any(|c| {
            c.kind == deno_ast::swc::common::comments::CommentKind::Block
              && regex.is_match(c.text.as_str())
          })
        })
        .unwrap_or(false)
    };
    if !leading_comments_has_re(&JSX_RUNTIME_RE) {
      add_text_change(format!(
        "/** @jsxRuntime {} */",
        jsx_options.jsx_runtime,
      ));
    }
    if module_info.jsx_import_source.is_none()
      && let Some(import_source) = jsx_options.jsx_import_source
    {
      add_text_change(format!("/** @jsxImportSource {} */", import_source));
    }
    if module_info.jsx_import_source_types.is_none()
      && let Some(import_source) = jsx_options.jsx_import_source_types
    {
      add_text_change(format!(
        "/** @jsxImportSourceTypes {} */",
        import_source
      ));
    }
    if let Some(classic_options) = &jsx_options.jsx_classic {
      if !leading_comments_has_re(&JSX_FACTORY_RE) {
        add_text_change(format!(
          "/** @jsxFactory {} */",
          classic_options.factory,
        ));
      }
      if !leading_comments_has_re(&JSX_FRAGMENT_FACTORY_RE) {
        add_text_change(format!(
          "/** @jsxFragmentFactory {} */",
          classic_options.fragment_factory,
        ));
      }
    }
    Ok(())
  }

  fn resolve_jsx_options<'a>(
    &'a self,
    specifier: &Url,
    text_info: &SourceTextInfo,
    diagnostic_reporter: &mut dyn FnMut(SpecifierUnfurlerDiagnostic),
  ) -> Result<JsxFolderOptions<'a>, AnyError> {
    let compiler_options =
      self.compiler_options_resolver.for_specifier(specifier);
    let jsx_config = compiler_options.jsx_import_source_config()?;
    let transpile_options = &compiler_options.transpile_options()?.transpile;
    let jsx_runtime = match &transpile_options.jsx {
      Some(
        deno_ast::JsxRuntime::Automatic(_)
        | deno_ast::JsxRuntime::Precompile(_),
      ) => "automatic",
      None | Some(deno_ast::JsxRuntime::Classic(_)) => "classic",
    };
    let mut unfurl_import_source =
      |import_source: &str, referrer: &Url, resolution_kind: ResolutionKind| {
        let maybe_import_source = self
          .specifier_unfurler
          .unfurl_specifier_reporting_diagnostic(
            referrer,
            import_source,
            resolution_kind,
            text_info,
            PositionOrSourceRangeRef::PositionRange(
              &deno_graph::PositionRange::zeroed(),
            ),
            diagnostic_reporter,
          );
        maybe_import_source.unwrap_or_else(|| import_source.to_string())
      };
    let jsx_import_source = jsx_config
      .and_then(|c| c.import_source.as_ref())
      .map(|jsx_import_source| {
        unfurl_import_source(
          &jsx_import_source.specifier,
          &jsx_import_source.base,
          ResolutionKind::Execution,
        )
      });
    let jsx_import_source_types = jsx_config
      .and_then(|c| c.import_source_types.as_ref())
      .map(|jsx_import_source_types| {
        unfurl_import_source(
          &jsx_import_source_types.specifier,
          &jsx_import_source_types.base,
          ResolutionKind::Types,
        )
      });
    let classic_options = match &transpile_options.jsx {
      None => Some(Cow::Owned(deno_ast::JsxClassicOptions::default())),
      Some(deno_ast::JsxRuntime::Classic(classic_options)) => {
        Some(Cow::Borrowed(classic_options))
      }
      Some(
        deno_ast::JsxRuntime::Precompile(_)
        | deno_ast::JsxRuntime::Automatic(_),
      ) => None,
    };
    Ok(JsxFolderOptions {
      jsx_runtime,
      jsx_classic: classic_options,
      jsx_import_source,
      jsx_import_source_types,
    })
  }
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use deno_config::workspace::WorkspaceDiscoverStart;
  use deno_path_util::url_from_file_path;
  use deno_resolver::deno_json::CompilerOptionsOverrides;
  use deno_resolver::factory::ConfigDiscoveryOption;
  use deno_resolver::npm::ByonmNpmResolverCreateOptions;
  use deno_resolver::npm::CreateInNpmPkgCheckerOptions;
  use deno_resolver::npm::DenoInNpmPackageChecker;
  use deno_resolver::npm::NpmResolverCreateOptions;
  use deno_resolver::workspace::WorkspaceResolver;
  use node_resolver::DenoIsBuiltInNodeModuleChecker;
  use node_resolver::NodeResolver;
  use node_resolver::NodeResolverOptions;
  use node_resolver::PackageJsonResolver;
  use node_resolver::cache::NodeResolutionSys;
  use pretty_assertions::assert_eq;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsWrite;
  use sys_traits::impls::InMemorySys;

  use super::*;
  use crate::npm::CliNpmResolver;

  #[test]
  fn test_module_content_jsx() {
    run_test(&[
      (
        "/deno.json",
        r#"{ "workspace": ["package-a", "package-b", "package-c", "package-d"] }"#,
        None,
      ),
      (
        "/package-a/deno.json",
        r#"{ "compilerOptions": {
        "jsx": "react-jsx",
        "jsxImportSource": "react",
        "jsxImportSourceTypes": "@types/react",
      },
      "imports": {
        "react": "npm:react"
        "@types/react": "npm:@types/react"
      }
    }"#,
        None,
      ),
      (
        "/package-b/deno.json",
        r#"{
        "compilerOptions": { "jsx": "react-jsx" },
        "imports": {
          "react": "npm:react"
          "@types/react": "npm:@types/react"
        }
      }"#,
        None,
      ),
      (
        "/package-c/deno.json",
        r#"{
        "compilerOptions": {
          "jsx": "precompile",
          "jsxImportSource": "react",
          "jsxImportSourceTypes": "@types/react",
        },
        "imports": {
          "react": "npm:react"
          "@types/react": "npm:@types/react"
        }
      }"#,
        None,
      ),
      (
        "/package-d/deno.json",
        r#"{
        "compilerOptions": { "jsx": "react" },
        "imports": {
          "react": "npm:react"
          "@types/react": "npm:@types/react"
        }
      }"#,
        None,
      ),
      (
        "/package-a/main.tsx",
        "export const component = <div></div>;",
        Some(
          "/** @jsxRuntime automatic *//** @jsxImportSource npm:react *//** @jsxImportSourceTypes npm:@types/react */export const component = <div></div>;",
        ),
      ),
      (
        "/package-b/main.tsx",
        "export const componentB = <div></div>;",
        Some(
          "/** @jsxRuntime automatic *//** @jsxImportSource npm:react *//** @jsxImportSourceTypes npm:react */export const componentB = <div></div>;",
        ),
      ),
      (
        "/package-a/other.tsx",
        "/** @jsxImportSource npm:preact */
        /** @jsxFragmentFactory h1 */
        /** @jsxImportSourceTypes npm:@types/example */
        /** @jsxFactory h2 */
        /** @jsxRuntime automatic */
        export const component = <div></div>;",
        Some(
          "/** @jsxImportSource npm:preact */
        /** @jsxFragmentFactory h1 */
        /** @jsxImportSourceTypes npm:@types/example */
        /** @jsxFactory h2 */
        /** @jsxRuntime automatic */
        export const component = <div></div>;",
        ),
      ),
      (
        "/package-c/main.tsx",
        "export const component = <div></div>;",
        Some(
          "/** @jsxRuntime automatic *//** @jsxImportSource npm:react *//** @jsxImportSourceTypes npm:@types/react */export const component = <div></div>;",
        ),
      ),
      (
        "/package-d/main.tsx",
        "export const component = <div></div>;",
        Some(
          "/** @jsxRuntime classic *//** @jsxFactory React.createElement *//** @jsxFragmentFactory React.Fragment */export const component = <div></div>;",
        ),
      ),
    ]);
  }

  fn get_path(path: &str) -> PathBuf {
    PathBuf::from(if cfg!(windows) {
      format!("C:{}", path.replace('/', "\\"))
    } else {
      path.to_string()
    })
  }

  fn run_test(files: &[(&'static str, &'static str, Option<&'static str>)]) {
    let in_memory_sys = InMemorySys::default();
    for (path, text, _) in files {
      let path = get_path(path);
      in_memory_sys
        .fs_create_dir_all(path.parent().unwrap())
        .unwrap();
      in_memory_sys.fs_write(path, text).unwrap();
    }
    let provider = module_content_provider(in_memory_sys);
    for (path, _, expected) in files {
      let Some(expected) = expected else {
        continue;
      };
      let path = get_path(path);
      let bytes = provider
        .resolve_content_maybe_unfurling(
          &ModuleGraph::new(deno_graph::GraphKind::All),
          &Default::default(),
          &path,
          &url_from_file_path(&path).unwrap(),
        )
        .unwrap();
      assert_eq!(String::from_utf8_lossy(&bytes), *expected);
    }
  }

  fn module_content_provider(
    sys: InMemorySys,
  ) -> ModuleContentProvider<InMemorySys> {
    let workspace_dir = deno_config::workspace::WorkspaceDirectory::discover(
      &sys,
      WorkspaceDiscoverStart::Paths(&[get_path("/")]),
      &Default::default(),
    )
    .unwrap();
    let resolver = Arc::new(
      WorkspaceResolver::from_workspace(
        &workspace_dir.workspace,
        sys.clone(),
        Default::default(),
      )
      .unwrap(),
    );
    let package_json_resolver =
      Arc::new(PackageJsonResolver::new(sys.clone(), None));
    let node_resolver = Arc::new(NodeResolver::new(
      DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm),
      DenoIsBuiltInNodeModuleChecker,
      CliNpmResolver::new(NpmResolverCreateOptions::Byonm(
        ByonmNpmResolverCreateOptions {
          root_node_modules_dir: None,
          sys: NodeResolutionSys::new(sys.clone(), None),
          pkg_json_resolver: package_json_resolver.clone(),
        },
      )),
      package_json_resolver,
      NodeResolutionSys::new(sys.clone(), None),
      NodeResolverOptions::default(),
    ));
    let compiler_options_resolver = Arc::new(CompilerOptionsResolver::new(
      &sys,
      &workspace_dir.workspace,
      &node_resolver,
      &ConfigDiscoveryOption::DiscoverCwd,
      &CompilerOptionsOverrides::default(),
    ));
    resolver.set_compiler_options_resolver(compiler_options_resolver.clone());
    let specifier_unfurler = SpecifierUnfurler::new(None, resolver, false);
    ModuleContentProvider::new(
      Arc::new(ParsedSourceCache::default()),
      specifier_unfurler,
      sys,
      compiler_options_resolver,
    )
  }
}
