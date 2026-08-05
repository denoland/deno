// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::swc::ecma_visit::noop_visit_type;
use deno_lint::diagnostic::LintDiagnosticDetails;
use deno_lint::diagnostic::LintDiagnosticRange;
use deno_lint::diagnostic::LintDocsUrl;
use deno_lint::diagnostic::LintFix;
use deno_lint::diagnostic::LintFixChange;
use deno_lint::rules::LintRule;
use deno_lint::tags;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::IsBuiltInNodeModuleChecker;

use super::ExtendedLintRule;

#[derive(Debug)]
pub struct NodeBuiltinSpecifierRule;

const CODE: &str = "node-builtin-specifier";
const MESSAGE: &str = "built-in Node modules need the \"node:\" specifier";
const HINT: &str = "Add \"node:\" prefix in front of the import specifier";
const FIX_DESC: &str = "Add \"node:\" prefix";
const DOCS_URL: &str =
  "https://docs.deno.com/lint/rules/node-builtin-specifier";

impl ExtendedLintRule for NodeBuiltinSpecifierRule {
  fn supports_incremental_cache(&self) -> bool {
    // This rule only looks at the current file, so it's safe to cache.
    true
  }

  fn help_docs_url(&self) -> Cow<'static, str> {
    Cow::Borrowed(DOCS_URL)
  }

  fn into_base(self: Box<Self>) -> Box<dyn LintRule> {
    self
  }
}

impl LintRule for NodeBuiltinSpecifierRule {
  fn lint_program_with_ast_view<'view>(
    &self,
    context: &mut deno_lint::context::Context<'view>,
    _program: deno_lint::Program<'view>,
  ) {
    let mut collector = BareNodeBuiltinCollector::default();
    context.parsed_source().program().visit_with(&mut collector);

    for (range, specifier) in collector.violations {
      let new_text = format!("\"node:{}\"", specifier);
      context.add_diagnostic_details(
        Some(LintDiagnosticRange {
          range,
          description: None,
          text_info: context.text_info().clone(),
        }),
        LintDiagnosticDetails {
          message: MESSAGE.to_string(),
          code: CODE.to_string(),
          hint: Some(HINT.to_string()),
          fixes: vec![LintFix {
            description: Cow::Borrowed(FIX_DESC),
            changes: vec![LintFixChange {
              new_text: Cow::Owned(new_text),
              range,
            }],
          }],
          custom_docs_url: LintDocsUrl::Default,
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

#[derive(Default)]
struct BareNodeBuiltinCollector {
  violations: Vec<(SourceRange, String)>,
}

impl BareNodeBuiltinCollector {
  fn maybe_add(&mut self, src: &ast::Str) {
    let Some(value) = src.value.as_str() else {
      return;
    };
    if DenoIsBuiltInNodeModuleChecker.is_builtin_node_module(value) {
      self.violations.push((src.range(), value.to_string()));
    }
  }
}

impl Visit for BareNodeBuiltinCollector {
  noop_visit_type!();

  fn visit_import_decl(&mut self, node: &ast::ImportDecl) {
    self.maybe_add(&node.src);
  }

  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    if let ast::Callee::Import(_) = &node.callee
      && let Some(arg) = node.args.first()
      && let ast::Expr::Lit(ast::Lit::Str(src)) = &*arg.expr
    {
      self.maybe_add(src);
    }
    node.visit_children_with(self);
  }
}
