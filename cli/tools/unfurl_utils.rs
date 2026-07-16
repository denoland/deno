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

/// Gets the exact source range for a cooked dynamic import prefix.
///
/// Dynamic dependency ranges may cover an entire concatenation or template
/// expression. Only accept prefixes that occur immediately after the opening
/// string or template delimiter so a cooked value cannot match unrelated text
/// later in the expression.
pub fn dynamic_argument_prefix_range(
  text_info: &SourceTextInfo,
  range: &deno_graph::PositionRange,
  prefix: &str,
) -> Option<std::ops::Range<usize>> {
  let range = range
    .as_source_range(text_info)
    .as_byte_range(text_info.range().start);
  let text = &text_info.text_str()[range.clone()];
  let delimiter = match text.as_bytes().first() {
    Some(delimiter @ (b'\'' | b'"' | b'`')) => *delimiter,
    _ => return None,
  };
  let start = range.start + 1;
  let end = start.checked_add(prefix.len())?;
  if end > range.end || text_info.text_str().get(start..end) != Some(prefix) {
    return None;
  }

  let remaining = text_info.text_str().as_bytes().get(end..range.end)?;
  let has_exact_boundary = match delimiter {
    b'\'' | b'"' => remaining.first() == Some(&delimiter),
    b'`' => remaining.starts_with(b"${") || remaining.first() == Some(&b'`'),
    _ => unreachable!(),
  };
  has_exact_boundary.then_some(start..end)
}

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
