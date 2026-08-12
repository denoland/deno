// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;

use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use tower_lsp::lsp_types as lsp;

use super::analysis::source_range_to_lsp_range;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum TestToggleKind {
  Ignore,
  Only,
}

impl TestToggleKind {
  fn property_name(self) -> &'static str {
    match self {
      Self::Ignore => "ignore",
      Self::Only => "only",
    }
  }

  fn enable_title(self) -> &'static str {
    match self {
      Self::Ignore => "Ignore test",
      Self::Only => "Only test",
    }
  }

  fn disable_title(self) -> &'static str {
    match self {
      Self::Ignore => "Unignore test",
      Self::Only => "Unfocus test",
    }
  }
}

struct TestCodeActionCollector<'a> {
  actions: Vec<lsp::CodeAction>,
  uri: &'a lsp::Uri,
  parsed_source: &'a ParsedSource,
  range: lsp::Range,
}

impl<'a> TestCodeActionCollector<'a> {
  fn new(
    uri: &'a lsp::Uri,
    parsed_source: &'a ParsedSource,
    range: lsp::Range,
  ) -> Self {
    Self {
      actions: Vec::new(),
      uri,
      parsed_source,
      range,
    }
  }

  fn take(self) -> Vec<lsp::CodeAction> {
    self.actions
  }

  fn add_actions_for_call(&mut self, node: &ast::CallExpr) {
    if !ranges_intersect(
      self.range,
      source_range_to_lsp_range(
        &node.range(),
        self.parsed_source.text_info_lazy(),
      ),
    ) {
      return;
    }

    let Some((is_deno_test, maybe_modifier)) = deno_test_call(node) else {
      return;
    };
    if !is_deno_test {
      return;
    }

    match maybe_modifier {
      Some(TestToggleKind::Ignore) => {
        self.push_replace_callee_action(
          TestToggleKind::Ignore.disable_title(),
          node,
          "Deno.test",
        );
      }
      Some(TestToggleKind::Only) => {
        self.push_replace_callee_action(
          TestToggleKind::Only.disable_title(),
          node,
          "Deno.test",
        );
      }
      None => {
        if let Some(ast::Expr::Object(obj_lit)) =
          node.args.first().map(|arg| arg.expr.as_ref())
        {
          for kind in [TestToggleKind::Ignore, TestToggleKind::Only] {
            self.push_object_toggle_action(obj_lit, kind);
          }
        } else {
          for kind in [TestToggleKind::Ignore, TestToggleKind::Only] {
            self.push_convert_call_action(node, kind);
          }
        }
      }
    }
  }

  fn push_replace_callee_action(
    &mut self,
    title: &str,
    node: &ast::CallExpr,
    new_text: &str,
  ) {
    let ast::Callee::Expr(callee) = &node.callee else {
      return;
    };
    self.push_action(
      title,
      lsp::TextEdit {
        range: source_range_to_lsp_range(
          &callee.range(),
          self.parsed_source.text_info_lazy(),
        ),
        new_text: new_text.to_string(),
      },
    );
  }

  fn push_convert_call_action(
    &mut self,
    node: &ast::CallExpr,
    kind: TestToggleKind,
  ) {
    let Some(name_arg) = node.args.first() else {
      return;
    };
    let Some(fn_arg) = node.args.get(1) else {
      return;
    };
    if name_arg.spread.is_some() || fn_arg.spread.is_some() {
      return;
    }

    let text_info = self.parsed_source.text_info_lazy();
    let name_range = name_arg.expr.range();
    let fn_range = fn_arg.expr.range();
    let new_text = format!(
      "{{ name: {}, fn: {}, {}: true }}",
      text_info.range_text(&name_range),
      text_info.range_text(&fn_range),
      kind.property_name(),
    );
    self.push_action(
      kind.enable_title(),
      lsp::TextEdit {
        range: source_range_to_lsp_range(
          &SourceRange {
            start: name_range.start,
            end: fn_range.end,
          },
          text_info,
        ),
        new_text,
      },
    );
  }

  fn push_object_toggle_action(
    &mut self,
    obj_lit: &ast::ObjectLit,
    kind: TestToggleKind,
  ) {
    let text_info = self.parsed_source.text_info_lazy();
    for prop in &obj_lit.props {
      let ast::PropOrSpread::Prop(prop) = prop else {
        continue;
      };
      let ast::Prop::KeyValue(key_value_prop) = prop.as_ref() else {
        continue;
      };
      if !prop_name_matches(&key_value_prop.key, kind.property_name()) {
        continue;
      }
      let enabled = matches!(
        key_value_prop.value.as_ref(),
        ast::Expr::Lit(ast::Lit::Bool(ast::Bool { value: true, .. }))
      );
      self.push_action(
        if enabled {
          kind.disable_title()
        } else {
          kind.enable_title()
        },
        lsp::TextEdit {
          range: source_range_to_lsp_range(
            &key_value_prop.value.range(),
            text_info,
          ),
          new_text: (!enabled).to_string(),
        },
      );
      return;
    }

    let obj_range = obj_lit.range();
    let obj_text = text_info.range_text(&obj_range);
    let Some(close_brace) = obj_text.rfind('}') else {
      return;
    };
    let insert_range = SourceRange {
      start: obj_range.start + close_brace,
      end: obj_range.start + close_brace,
    };
    self.push_action(
      kind.enable_title(),
      lsp::TextEdit {
        range: source_range_to_lsp_range(&insert_range, text_info),
        new_text: if obj_lit.props.is_empty() {
          format!("{}: true", kind.property_name())
        } else {
          format!(", {}: true", kind.property_name())
        },
      },
    );
  }

  fn push_action(&mut self, title: &str, edit: lsp::TextEdit) {
    self.actions.push(lsp::CodeAction {
      title: title.to_string(),
      kind: Some(lsp::CodeActionKind::REFACTOR_REWRITE),
      edit: Some(lsp::WorkspaceEdit {
        changes: Some(HashMap::from([(self.uri.clone(), vec![edit])])),
        ..Default::default()
      }),
      ..Default::default()
    });
  }
}

impl Visit for TestCodeActionCollector<'_> {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    self.add_actions_for_call(node);
    node.visit_children_with(self);
  }
}

pub fn collect_test_code_actions(
  uri: &lsp::Uri,
  parsed_source: &ParsedSource,
  range: lsp::Range,
) -> Vec<lsp::CodeAction> {
  let mut collector = TestCodeActionCollector::new(uri, parsed_source, range);
  parsed_source.program().visit_with(&mut collector);
  collector.take()
}

fn deno_test_call(
  node: &ast::CallExpr,
) -> Option<(bool, Option<TestToggleKind>)> {
  let ast::Callee::Expr(callee_expr) = &node.callee else {
    return None;
  };
  let mut prop_chain = ["", "", ""];
  let mut current_segment = callee_expr.as_ref();
  for (i, name) in prop_chain.iter_mut().enumerate().rev() {
    match current_segment {
      ast::Expr::Ident(ident) => {
        *name = ident.sym.as_str();
        break;
      }
      ast::Expr::Member(member_expr) => {
        if i == 0 {
          return None;
        }
        let ast::MemberProp::Ident(right) = &member_expr.prop else {
          return None;
        };
        *name = right.sym.as_str();
        current_segment = &member_expr.obj;
      }
      _ => return None,
    }
  }

  match prop_chain {
    ["", "Deno", "test"] => Some((true, None)),
    ["Deno", "test", "ignore"] => Some((true, Some(TestToggleKind::Ignore))),
    ["Deno", "test", "only"] => Some((true, Some(TestToggleKind::Only))),
    _ => None,
  }
}

fn prop_name_matches(prop_name: &ast::PropName, name: &str) -> bool {
  match prop_name {
    ast::PropName::Ident(ident) => ident.sym == name,
    ast::PropName::Str(str_) => str_.value == name,
    _ => false,
  }
}

fn ranges_intersect(a: lsp::Range, b: lsp::Range) -> bool {
  position_le(a.start, b.end) && position_le(b.start, a.end)
}

fn position_le(a: lsp::Position, b: lsp::Position) -> bool {
  a.line < b.line || (a.line == b.line && a.character <= b.character)
}

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_core::resolve_url;
  use pretty_assertions::assert_eq;

  use super::*;

  fn collect(source: &str, line: u32, character: u32) -> Vec<lsp::CodeAction> {
    let specifier = resolve_url("file:///a/example.ts").unwrap();
    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier,
      text: source.into(),
      media_type: MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    collect_test_code_actions(
      &"file:///a/example.ts".parse().unwrap(),
      &parsed_source,
      lsp::Range::new(
        lsp::Position::new(line, character),
        lsp::Position::new(line, character),
      ),
    )
  }

  fn action_texts(actions: &[lsp::CodeAction]) -> Vec<(&str, &str)> {
    actions
      .iter()
      .map(|action| {
        let edit = action
          .edit
          .as_ref()
          .unwrap()
          .changes
          .as_ref()
          .unwrap()
          .values()
          .next()
          .unwrap()
          .first()
          .unwrap();
        (action.title.as_str(), edit.new_text.as_str())
      })
      .collect()
  }

  #[test]
  fn test_basic_test_actions() {
    let actions = collect(r#"Deno.test("name", () => {});"#, 0, 5);
    assert_eq!(
      action_texts(&actions),
      vec![
        (
          "Ignore test",
          r#"{ name: "name", fn: () => {}, ignore: true }"#
        ),
        ("Only test", r#"{ name: "name", fn: () => {}, only: true }"#),
      ]
    );
  }

  #[test]
  fn test_object_test_actions() {
    let actions = collect(
      r#"Deno.test({ name: "name", fn: () => {}, only: false });"#,
      0,
      5,
    );
    assert_eq!(
      action_texts(&actions),
      vec![("Ignore test", r#", ignore: true"#), ("Only test", "true"),]
    );
  }

  #[test]
  fn test_object_test_disable_actions() {
    let actions = collect(
      r#"Deno.test({ name: "name", fn: () => {}, ignore: true, only: true });"#,
      0,
      5,
    );
    assert_eq!(
      action_texts(&actions),
      vec![("Unignore test", "false"), ("Unfocus test", "false")]
    );
  }

  #[test]
  fn test_modifier_test_disable_action() {
    let actions = collect(r#"Deno.test.ignore("name", () => {});"#, 0, 15);
    assert_eq!(action_texts(&actions), vec![("Unignore test", "Deno.test")]);
  }
}
