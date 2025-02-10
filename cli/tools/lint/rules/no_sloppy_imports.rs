// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::SourceRange;
use deno_error::JsErrorBox;
use deno_graph::source::ResolutionKind;
use deno_graph::source::ResolveError;
use deno_graph::Range;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use deno_lint::rules::LintRule;
use deno_lint::tags;
use deno_resolver::workspace::SloppyImportsResolutionReason;
use deno_resolver::workspace::WorkspaceResolver;
use text_lines::LineAndColumnIndex;

use super::ExtendedLintRule;
use crate::graph_util::CliJsrUrlProvider;
use crate::sys::CliSys;

#[derive(Debug)]
pub struct NoSloppyImportsRule {
  // None for making printing out the lint rules easy
  workspace_resolver: Option<Arc<WorkspaceResolver<CliSys>>>,
}

impl NoSloppyImportsRule {
  pub fn new(
    workspace_resolver: Option<Arc<WorkspaceResolver<CliSys>>>,
  ) -> Self {
    NoSloppyImportsRule { workspace_resolver }
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
    let Some(workspace_resolver) = &self.workspace_resolver else {
      return true;
    };
    !workspace_resolver.sloppy_imports_enabled()
      && !workspace_resolver.has_compiler_options_root_dirs()
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
    if context.specifier().scheme() != "file" {
      return;
    }

    let resolver = SloppyImportCaptureResolver {
      workspace_resolver,
      captures: Default::default(),
    };

    // fill this and capture the sloppy imports in the resolver
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

    for (referrer, (specifier, sloppy_reason)) in
      resolver.captures.borrow_mut().drain()
    {
      let start_range =
        context.text_info().loc_to_source_pos(LineAndColumnIndex {
          line_index: referrer.range.start.line,
          column_index: referrer.range.start.character,
        });
      let end_range =
        context.text_info().loc_to_source_pos(LineAndColumnIndex {
          line_index: referrer.range.end.line,
          column_index: referrer.range.end.character,
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
          custom_docs_url: LintDocsUrl::Custom(DOCS_URL.to_string()),
          fixes: context
            .specifier()
            .make_relative(&specifier)
            .map(|relative| {
              vec![LintFix {
                description: Cow::Owned(
                  sloppy_reason.quick_fix_message_for_specifier(&specifier),
                ),
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

  fn tags(&self) -> tags::Tags {
    &[tags::RECOMMENDED]
  }
}

#[derive(Debug)]
struct SloppyImportCaptureResolver<'a> {
  workspace_resolver: &'a WorkspaceResolver<CliSys>,
  captures: RefCell<
    HashMap<Range, (deno_ast::ModuleSpecifier, SloppyImportsResolutionReason)>,
  >,
}

impl<'a> deno_graph::source::Resolver for SloppyImportCaptureResolver<'a> {
  fn resolve(
    &self,
    specifier_text: &str,
    referrer_range: &Range,
    resolution_kind: ResolutionKind,
  ) -> Result<deno_ast::ModuleSpecifier, deno_graph::source::ResolveError> {
    let resolution = self
      .workspace_resolver
      .resolve(
        specifier_text,
        &referrer_range.specifier,
        match resolution_kind {
          ResolutionKind::Execution => {
            deno_resolver::workspace::ResolutionKind::Execution
          }
          ResolutionKind::Types => {
            deno_resolver::workspace::ResolutionKind::Types
          }
        },
      )
      .map_err(|err| ResolveError::Other(JsErrorBox::from_err(err)))?;

    match resolution {
      deno_resolver::workspace::MappedResolution::Normal {
        specifier,
        sloppy_reason,
        ..
      } => {
        if let Some(sloppy_reason) = sloppy_reason {
          self
            .captures
            .borrow_mut()
            .entry(referrer_range.clone())
            .or_insert_with(|| (specifier.clone(), sloppy_reason));
        }
        Ok(specifier)
      }
      deno_resolver::workspace::MappedResolution::WorkspaceJsrPackage {
        ..
      }
      | deno_resolver::workspace::MappedResolution::WorkspaceNpmPackage {
        ..
      }
      | deno_resolver::workspace::MappedResolution::PackageJson { .. } => {
        // this error is ignored
        Err(ResolveError::Other(JsErrorBox::generic("")))
      }
    }
  }
}
