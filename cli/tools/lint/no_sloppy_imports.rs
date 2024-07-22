// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::SourceRange;
use deno_graph::source::ResolutionMode;
use deno_graph::Range;
use deno_lint::rules::LintRule;
use text_lines::LineAndColumnIndex;

use crate::graph_util::CliJsrUrlProvider;
use crate::resolver::CliGraphResolver;
use crate::resolver::ResolutionDetail;
use crate::resolver::SloppyImportsResolution;

#[derive(Debug)]
pub struct NoSloppyImports {
  resolver: Arc<CliGraphResolver>,
}

const CODE: &str = "no-sloppy-imports";

impl LintRule for NoSloppyImports {
  fn lint_program_with_ast_view<'view>(
    &self,
    context: &mut deno_lint::context::Context<'view>,
    program: deno_lint::Program<'view>,
  ) {
    if context.specifier().scheme() != "file" {
      return;
    }

    let resolver = SloppyImportCaptureResolver {
      graph_resolver: &self.resolver,
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
      context.add_diagnostic_with_fixes(
        SourceRange::new(start_range, end_range),
        CODE,
        "Sloppy imports are not allowed.",
        Some(sloppy_import.as_suggestion_message()),
        // todo: fixes
        vec![],
      );
    }
  }

  fn code(&self) -> &'static str {
    CODE
  }

  fn docs(&self) -> &'static str {
    include_str!("no_sloppy_imports.md")
  }
}

#[derive(Debug)]
struct SloppyImportCaptureResolver<'a> {
  graph_resolver: &'a CliGraphResolver,
  captures: RefCell<HashMap<Range, SloppyImportsResolution>>,
}

impl<'a> deno_graph::source::Resolver for SloppyImportCaptureResolver<'a> {
  fn resolve(
    &self,
    specifier_text: &str,
    referrer_range: &Range,
    mode: ResolutionMode,
  ) -> Result<deno_ast::ModuleSpecifier, deno_graph::source::ResolveError> {
    let resolution = self.graph_resolver.resolve_detail(
      specifier_text,
      referrer_range,
      mode,
    )?;
    match &resolution {
      ResolutionDetail::Sloppy(res) => {
        self
          .captures
          .borrow_mut()
          .insert(referrer_range.clone(), res.clone());
      }
      ResolutionDetail::Normal(_) => {}
    }
    Ok(resolution)
  }

  fn default_jsx_import_source(&self) -> Option<String> {
    self.graph_resolver.default_jsx_import_source()
  }

  fn default_jsx_import_source_types(&self) -> Option<String> {
    self.graph_resolver.default_jsx_import_source_types()
  }

  fn jsx_import_source_module(&self) -> &str {
    self.graph_resolver.jsx_import_source_module()
  }

  fn resolve_types(
    &self,
    specifier: &deno_ast::ModuleSpecifier,
  ) -> Result<
    Option<(deno_ast::ModuleSpecifier, Option<deno_graph::Range>)>,
    deno_graph::source::ResolveError,
  > {
    self.graph_resolver.resolve_types(specifier)
  }
}
