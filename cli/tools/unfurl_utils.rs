// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::SourcePos;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_ast::swc::ast::CallExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::MetaPropKind;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::noop_visit_type;

/// Collects `import.meta.resolve("...")` call sites from a parsed AST.
///
/// Used by both publish and pack unfurlers to find specifiers passed to
/// `import.meta.resolve()`.
#[derive(Default)]
pub struct ImportMetaResolveCollector {
  pub specifiers: Vec<(SourceRange<SourcePos>, String)>,
  pub diagnostic_ranges: Vec<SourceRange<SourcePos>>,
}

impl Visit for ImportMetaResolveCollector {
  noop_visit_type!();

  fn visit_call_expr(&mut self, node: &CallExpr) {
    if node.args.len() == 1
      && let Some(first_arg) = node.args.first()
      && let Callee::Expr(callee) = &node.callee
      && let Expr::Member(member) = &**callee
      && let Expr::MetaProp(prop) = &*member.obj
      && prop.kind == MetaPropKind::ImportMeta
      && let MemberProp::Ident(ident) = &member.prop
      && ident.sym == "resolve"
      && first_arg.spread.is_none()
    {
      if let Expr::Lit(Lit::Str(arg)) = &*first_arg.expr {
        let range = arg.range();
        self.specifiers.push((
          // remove quotes
          SourceRange::new(range.start + 1, range.end - 1),
          arg.value.to_string_lossy().into_owned(),
        ));
      } else {
        self.diagnostic_ranges.push(first_arg.expr.range());
      }
    }
  }
}

/// Convert a `deno_graph::PositionRange` to a byte range, stripping
/// surrounding quotes.
pub fn to_range(
  text_info: &SourceTextInfo,
  range: &deno_graph::PositionRange,
) -> std::ops::Range<usize> {
  let mut range = range
    .as_source_range(text_info)
    .as_byte_range(text_info.range().start);
  let text = &text_info.text_str()[range.clone()];
  if text.starts_with('"') || text.starts_with('\'') {
    range.start += 1;
  }
  if text.ends_with('"') || text.ends_with('\'') {
    range.end -= 1;
  }
  range
}
