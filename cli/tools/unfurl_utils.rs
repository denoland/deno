// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::SourcePos;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_ast::swc::ast::CallExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::MetaPropKind;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::noop_visit_type;

#[derive(Clone, Copy)]
pub enum SpecifierTextContext {
  JavaScript,
  QuotedComment,
  UnquotedComment,
}

/// Creates a source edit for a cooked specifier in its original syntax
/// context. Returns `None` when the value cannot be represented without
/// changing how that context is parsed.
pub fn specifier_text_change(
  text_info: &SourceTextInfo,
  range: std::ops::Range<usize>,
  replacement: &str,
  context: SpecifierTextContext,
) -> Option<TextChange> {
  let new_text = match context {
    SpecifierTextContext::JavaScript => {
      javascript_literal_content(text_info.text_str(), &range, replacement)?
    }
    SpecifierTextContext::QuotedComment => {
      let source = text_info.text_str().as_bytes();
      let delimiter = *source.get(range.start.checked_sub(1)?)?;
      if !matches!(delimiter, b'\'' | b'"')
        || source.get(range.end) != Some(&delimiter)
        || !is_safe_quoted_comment_value(replacement)
      {
        return None;
      }
      replacement.to_string()
    }
    SpecifierTextContext::UnquotedComment => {
      if !is_safe_unquoted_comment_value(replacement) {
        return None;
      }
      replacement.to_string()
    }
  };
  Some(TextChange { range, new_text })
}

fn javascript_literal_content(
  source: &str,
  range: &std::ops::Range<usize>,
  replacement: &str,
) -> Option<String> {
  let delimiter = *source.as_bytes().get(range.start.checked_sub(1)?)?;
  let remaining = source.as_bytes().get(range.end..)?;
  let valid_boundary = match delimiter {
    b'\'' | b'"' => remaining.first() == Some(&delimiter),
    b'`' => remaining.starts_with(b"${") || remaining.first() == Some(&b'`'),
    _ => false,
  };
  if !valid_boundary {
    return None;
  }

  let serialized = deno_core::serde_json::to_string(replacement).ok()?;
  let mut content = serialized
    .get(1..serialized.len().checked_sub(1)?)?
    .to_string();
  content = content
    .replace('\u{2028}', "\\u2028")
    .replace('\u{2029}', "\\u2029");
  match delimiter {
    b'\'' => Some(content.replace('\'', "\\'")),
    b'"' => Some(content),
    b'`' => Some(content.replace('`', "\\`").replace("${", "\\${")),
    _ => None,
  }
}

fn is_safe_quoted_comment_value(value: &str) -> bool {
  !value.contains("*/")
    && !value.chars().any(|c| {
      matches!(c, '\'' | '"' | '\u{2028}' | '\u{2029}') || c.is_control()
    })
}

pub fn is_safe_unquoted_comment_value(value: &str) -> bool {
  !value.is_empty()
    && !value.contains("*/")
    && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

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

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_graph::analysis::DependencyDescriptor;
  use deno_graph::analysis::DynamicArgument;
  use deno_graph::ast::ParserModuleAnalyzer;

  use super::*;

  #[test]
  fn javascript_specifier_changes_preserve_cooked_values() {
    let replacement = "double\" single' tick` slash\\${value} slash-tick\\`\n\r\u{2028}\u{2029}";
    for source in [r#"import("old");"#, "import('old');", "import(`old`);"] {
      let text_info = SourceTextInfo::from_string(source.to_string());
      let start = source.find("old").unwrap();
      let change = specifier_text_change(
        &text_info,
        start..start + 3,
        replacement,
        SpecifierTextContext::JavaScript,
      )
      .unwrap();
      let output = deno_ast::apply_text_changes(source, vec![change]);
      let specifier = ModuleSpecifier::parse("file:///mod.ts").unwrap();
      let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
        specifier,
        text: output.into(),
        media_type: MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
      })
      .unwrap();
      let module_info = ParserModuleAnalyzer::module_info(&parsed_source);
      let DependencyDescriptor::Dynamic(dependency) =
        &module_info.dependencies[0]
      else {
        panic!("expected dynamic dependency");
      };
      let DynamicArgument::String(actual) = &dependency.argument else {
        panic!("expected string argument");
      };
      assert_eq!(actual, replacement);
      assert_eq!(module_info.dependencies.len(), 1);
    }
  }

  #[test]
  fn comment_specifier_changes_reject_unsafe_values() {
    let quoted_source = r#"/// <reference types="old" />"#;
    let quoted_text_info =
      SourceTextInfo::from_string(quoted_source.to_string());
    let quoted_start = quoted_source.find("old").unwrap();
    let quoted_range = quoted_start..quoted_start + 3;
    assert!(
      specifier_text_change(
        &quoted_text_info,
        quoted_range.clone(),
        "npm:package/path",
        SpecifierTextContext::QuotedComment,
      )
      .is_some()
    );
    for unsafe_value in [
      "npm:package/sub*/path",
      "npm:package/single'quote",
      "npm:package/double\"quote",
      "npm:package/line\nbreak",
    ] {
      assert!(
        specifier_text_change(
          &quoted_text_info,
          quoted_range.clone(),
          unsafe_value,
          SpecifierTextContext::QuotedComment,
        )
        .is_none()
      );
    }

    let unquoted_source = "/** @jsxImportSource old */";
    let unquoted_text_info =
      SourceTextInfo::from_string(unquoted_source.to_string());
    let unquoted_start = unquoted_source.find("old").unwrap();
    let unquoted_range = unquoted_start..unquoted_start + 3;
    assert!(
      specifier_text_change(
        &unquoted_text_info,
        unquoted_range.clone(),
        "npm:package/path",
        SpecifierTextContext::UnquotedComment,
      )
      .is_some()
    );
    for unsafe_value in ["", "npm:package/sub*/path", "npm:package/with space"]
    {
      assert!(
        specifier_text_change(
          &unquoted_text_info,
          unquoted_range.clone(),
          unsafe_value,
          SpecifierTextContext::UnquotedComment,
        )
        .is_none()
      );
    }
  }
}
