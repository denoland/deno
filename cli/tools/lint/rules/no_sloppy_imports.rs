// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::SourceRange;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::anyhow;
use deno_graph::source::ResolutionMode;
use deno_graph::source::ResolveError;
use deno_graph::Range;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use deno_lint::rules::LintRule;
use deno_resolver::sloppy_imports::SloppyImportsResolution;
use deno_resolver::sloppy_imports::SloppyImportsResolutionMode;
use text_lines::LineAndColumnIndex;

use crate::graph_util::CliJsrUrlProvider;
use crate::resolver::CliSloppyImportsResolver;

use super::ExtendedLintRule;

#[derive(Debug)]
pub struct NoSloppyImportsRule {
  sloppy_imports_resolver: Option<Arc<CliSloppyImportsResolver>>,
  // None for making printing out the lint rules easy
  workspace_resolver: Option<Arc<WorkspaceResolver>>,
}

impl NoSloppyImportsRule {
  pub fn new(
    sloppy_imports_resolver: Option<Arc<CliSloppyImportsResolver>>,
    workspace_resolver: Option<Arc<WorkspaceResolver>>,
  ) -> Self {
    NoSloppyImportsRule {
      sloppy_imports_resolver,
      workspace_resolver,
    }
  }
}

const CODE: &str = "no-sloppy-imports";
const DOCS_URL: &str = "https://docs.deno.com/runtime/manual/tools/unstable_flags/#--unstable-sloppy-imports";

impl ExtendedLintRule for NoSloppyImportsRule {
  fn supports_incremental_cache(&self) -> bool {
    // only allow the incremental cache when we don't
    // do sloppy import resolution because sloppy import
    // resolution requires knowing about the surrounding files
    // in addition to the current one
    self.sloppy_imports_resolver.is_none() || self.workspace_resolver.is_none()
  }

  fn help_docs_url(&self) -> Cow<'static, str> {
    Cow::Borrowed(DOCS_URL)
  }

  fn into_base(self: Box<Self>) -> Box<dyn LintRule> {
    self
  }
}

impl LintRule for NoSloppyImportsRule {
  fn lint_program_with_ast_view<'view>(
    &self,
    context: &mut deno_lint::context::Context<'view>,
    _program: deno_lint::Program<'view>,
  ) {
    let Some(workspace_resolver) = &self.workspace_resolver else {
      return;
    };
    let Some(sloppy_imports_resolver) = &self.sloppy_imports_resolver else {
      return;
    };
    if context.specifier().scheme() != "file" {
      return;
    }

    let resolver = SloppyImportCaptureResolver {
      workspace_resolver,
      sloppy_imports_resolver,
      captures: Default::default(),
    };

    deno_graph::parse_module_from_ast(deno_graph::ParseModuleFromAstOptions {
      graph_kind: deno_graph::GraphKind::All,
      specifier: context.specifier().clone(),
      maybe_headers: None,
      parsed_source: context.parsed_source(),
      // ignore resolving dynamic imports like import(`./dir/${something}`)
      file_system: &deno_graph::source::NullFileSystem,
      jsr_url_provider: &CliJsrUrlProvider,
      maybe_resolver: Some(&resolver),
      // don't bother resolving npm specifiers
      maybe_npm_resolver: None,
    });

    for (range, sloppy_import) in resolver.captures.borrow_mut().drain() {
      let start_range =
        context.text_info().loc_to_source_pos(LineAndColumnIndex {
          line_index: range.start.line,
          column_index: range.start.character,
        });
      let end_range =
        context.text_info().loc_to_source_pos(LineAndColumnIndex {
          line_index: range.end.line,
          column_index: range.end.character,
        });
      let source_range = SourceRange::new(start_range, end_range);
      context.add_diagnostic_details(
        Some(LintDiagnosticRange {
          range: source_range,
          description: None,
          text_info: context.text_info().clone(),
        }),
        LintDiagnosticDetails {
          message: "Sloppy imports are not allowed.".to_string(),
          code: CODE.to_string(),
          custom_docs_url: Some(DOCS_URL.to_string()),
          fixes: context
            .specifier()
            .make_relative(sloppy_import.as_specifier())
            .map(|relative| {
              vec![LintFix {
                description: Cow::Owned(sloppy_import.as_quick_fix_message()),
                changes: vec![LintFixChange {
                  new_text: Cow::Owned({
                    let relative = if relative.starts_with("../") {
                      relative
                    } else {
                      format!("./{}", relative)
                    };
                    let current_text =
                      context.text_info().range_text(&source_range);
                    if current_text.starts_with('"') {
                      format!("\"{}\"", relative)
                    } else if current_text.starts_with('\'') {
                      format!("'{}'", relative)
                    } else {
                      relative
                    }
                  }),
                  range: source_range,
                }],
              }]
            })
            .unwrap_or_default(),
          hint: None,
          info: vec![],
        },
      );
    }
  }

  fn code(&self) -> &'static str {
    CODE
  }

  fn docs(&self) -> &'static str {
    include_str!("no_sloppy_imports.md")
  }

  fn tags(&self) -> &'static [&'static str] {
    &["recommended"]
  }
}

#[derive(Debug)]
struct SloppyImportCaptureResolver<'a> {
  workspace_resolver: &'a WorkspaceResolver,
  sloppy_imports_resolver: &'a CliSloppyImportsResolver,
  captures: RefCell<HashMap<Range, SloppyImportsResolution>>,
}

impl<'a> deno_graph::source::Resolver for SloppyImportCaptureResolver<'a> {
  fn resolve(
    &self,
    specifier_text: &str,
    referrer_range: &Range,
    mode: ResolutionMode,
  ) -> Result<deno_ast::ModuleSpecifier, deno_graph::source::ResolveError> {
    let resolution = self
      .workspace_resolver
      .resolve(specifier_text, &referrer_range.specifier)
      .map_err(|err| ResolveError::Other(err.into()))?;

    match resolution {
      deno_config::workspace::MappedResolution::Normal {
        specifier, ..
      }
      | deno_config::workspace::MappedResolution::ImportMap {
        specifier, ..
      } => match self.sloppy_imports_resolver.resolve(
        &specifier,
        match mode {
          ResolutionMode::Execution => SloppyImportsResolutionMode::Execution,
          ResolutionMode::Types => SloppyImportsResolutionMode::Types,
        },
      ) {
        Some(res) => {
          self
            .captures
            .borrow_mut()
            .entry(referrer_range.clone())
            .or_insert_with(|| res.clone());
          Ok(res.into_specifier())
        }
        None => Ok(specifier),
      },
      deno_config::workspace::MappedResolution::WorkspaceJsrPackage {
        ..
      }
      | deno_config::workspace::MappedResolution::WorkspaceNpmPackage {
        ..
      }
      | deno_config::workspace::MappedResolution::PackageJson { .. } => {
        // this error is ignored
        Err(ResolveError::Other(anyhow!("")))
      }
    }
  }
}
