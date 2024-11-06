// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::lsp::analysis::source_range_to_lsp_range;

use super::definitions::TestModule;

use deno_ast::swc::ast;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use lsp::Range;
use std::collections::HashMap;
use std::collections::HashSet;
use tower_lsp::lsp_types as lsp;

/// Parse an arrow expression for any test steps and return them.
fn visit_arrow(
  arrow_expr: &ast::ArrowExpr,
  parent_id: &str,
  text_info: &SourceTextInfo,
  test_module: &mut TestModule,
) {
  if let Some((maybe_test_context, maybe_step_var)) =
    parse_test_context_param(arrow_expr.params.first())
  {
    let mut collector = TestStepCollector::new(
      maybe_test_context,
      maybe_step_var,
      parent_id,
      text_info,
      test_module,
    );
    arrow_expr.body.visit_with(&mut collector);
  }
}

/// Parse a function for any test steps and return them.
fn visit_fn(
  function: &ast::Function,
  parent_id: &str,
  text_info: &SourceTextInfo,
  test_module: &mut TestModule,
) {
  if let Some((maybe_test_context, maybe_step_var)) =
    parse_test_context_param(function.params.first().map(|p| &p.pat))
  {
    let mut collector = TestStepCollector::new(
      maybe_test_context,
      maybe_step_var,
      parent_id,
      text_info,
      test_module,
    );
    function.body.visit_with(&mut collector);
  }
}

/// Parse a param of a test function for the test context binding, or any
/// destructuring of a `steps` method from the test context.
fn parse_test_context_param(
  param: Option<&ast::Pat>,
) -> Option<(Option<String>, Option<String>)> {
  let mut maybe_test_context = None;
  let mut maybe_step_var = None;
  match param {
    // handles `(testContext)`
    Some(ast::Pat::Ident(binding_ident)) => {
      maybe_test_context = Some(binding_ident.id.sym.to_string());
    }
    Some(ast::Pat::Object(object_pattern)) => {
      for prop in &object_pattern.props {
        match prop {
          ast::ObjectPatProp::KeyValue(key_value_pat_prop) => {
            match &key_value_pat_prop.key {
              // handles `({ step: s })`
              ast::PropName::Ident(ident) => {
                if ident.sym.eq("step") {
                  if let ast::Pat::Ident(ident) =
                    key_value_pat_prop.value.as_ref()
                  {
                    maybe_step_var = Some(ident.id.sym.to_string());
                  }
                  break;
                }
              }
              // handles `({ "step": s })`
              ast::PropName::Str(string) => {
                if string.value.eq("step") {
                  if let ast::Pat::Ident(ident) =
                    key_value_pat_prop.value.as_ref()
                  {
                    maybe_step_var = Some(ident.id.sym.to_string());
                  }
                  break;
                }
              }
              _ => (),
            }
          }
          // handles `({ step = something })`
          ast::ObjectPatProp::Assign(assign_pat_prop)
            if assign_pat_prop.key.sym.eq("step") =>
          {
            maybe_step_var = Some("step".to_string());
            break;
          }
          // handles `({ ...ctx })`
          ast::ObjectPatProp::Rest(rest_pat) => {
            if let ast::Pat::Ident(ident) = rest_pat.arg.as_ref() {
              maybe_test_context = Some(ident.id.sym.to_string());
            }
            break;
          }
          _ => (),
        }
      }
    }
    _ => return None,
  }
  if maybe_test_context.is_none() && maybe_step_var.is_none() {
    None
  } else {
    Some((maybe_test_context, maybe_step_var))
  }
}

/// Check a call expression of a test or test step to determine the name of the
/// test or test step as well as any sub steps.
fn visit_call_expr(
  node: &ast::CallExpr,
  fns: Option<&HashMap<String, ast::Function>>,
  range: Range,
  parent_id: Option<&str>,
  text_info: &SourceTextInfo,
  test_module: &mut TestModule,
) {
  if let Some(expr) = node.args.first().map(|es| es.expr.as_ref()) {
    match expr {
      ast::Expr::Object(obj_lit) => {
        let mut maybe_name = None;
        for prop in &obj_lit.props {
          let ast::PropOrSpread::Prop(prop) = prop else {
            continue;
          };
          let ast::Prop::KeyValue(key_value_prop) = prop.as_ref() else {
            continue;
          };
          let ast::PropName::Ident(ast::IdentName { sym, .. }) =
            &key_value_prop.key
          else {
            continue;
          };
          if sym == "name" {
            match key_value_prop.value.as_ref() {
              // matches string literals (e.g. "test name" or
              // 'test name')
              ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
                maybe_name = Some(lit_str.value.to_string());
              }
              // matches template literals with only a single quasis
              // (e.g. `test name`)
              ast::Expr::Tpl(tpl) => {
                if tpl.quasis.len() == 1 {
                  maybe_name = Some(tpl.quasis[0].raw.to_string());
                }
              }
              _ => {}
            }
            break;
          }
        }
        let name = match maybe_name {
          Some(n) => n,
          None => return,
        };
        let (id, _) = test_module.register(
          name,
          Some(range),
          false,
          parent_id.map(str::to_owned),
        );
        for prop in &obj_lit.props {
          let ast::PropOrSpread::Prop(prop) = prop else {
            continue;
          };
          match prop.as_ref() {
            ast::Prop::KeyValue(key_value_prop) => {
              let ast::PropName::Ident(ast::IdentName { sym, .. }) =
                &key_value_prop.key
              else {
                continue;
              };
              if sym == "fn" {
                match key_value_prop.value.as_ref() {
                  ast::Expr::Arrow(arrow_expr) => {
                    visit_arrow(arrow_expr, &id, text_info, test_module);
                  }
                  ast::Expr::Fn(fn_expr) => {
                    visit_fn(&fn_expr.function, &id, text_info, test_module);
                  }
                  _ => {}
                }
                break;
              }
            }
            ast::Prop::Method(method_prop) => {
              let ast::PropName::Ident(ast::IdentName { sym, .. }) =
                &method_prop.key
              else {
                continue;
              };
              if sym == "fn" {
                visit_fn(&method_prop.function, &id, text_info, test_module);
                break;
              }
            }
            _ => {}
          }
        }
      }
      ast::Expr::Fn(fn_expr) => {
        if let Some(ast::Ident { sym, .. }) = fn_expr.ident.as_ref() {
          let name = sym.to_string();
          let (id, _) = test_module.register(
            name,
            Some(range),
            false,
            parent_id.map(str::to_owned),
          );
          visit_fn(&fn_expr.function, &id, text_info, test_module);
        }
      }
      ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
        let name = lit_str.value.to_string();
        let (id, _) = test_module.register(
          name,
          Some(range),
          false,
          parent_id.map(str::to_owned),
        );
        match node.args.get(1).map(|es| es.expr.as_ref()) {
          Some(ast::Expr::Fn(fn_expr)) => {
            visit_fn(&fn_expr.function, &id, text_info, test_module);
          }
          Some(ast::Expr::Arrow(arrow_expr)) => {
            visit_arrow(arrow_expr, &id, text_info, test_module);
          }
          _ => {}
        }
      }
      ast::Expr::Tpl(tpl) => {
        if tpl.quasis.len() == 1 {
          let name = tpl.quasis[0].raw.to_string();
          let (id, _) = test_module.register(
            name,
            Some(range),
            false,
            parent_id.map(str::to_owned),
          );
          match node.args.get(1).map(|es| es.expr.as_ref()) {
            Some(ast::Expr::Fn(fn_expr)) => {
              visit_fn(&fn_expr.function, &id, text_info, test_module);
            }
            Some(ast::Expr::Arrow(arrow_expr)) => {
              visit_arrow(arrow_expr, &id, text_info, test_module);
            }
            _ => {}
          }
        }
      }
      ast::Expr::Ident(ident) => {
        let name = ident.sym.to_string();
        if let Some(fn_expr) = fns.and_then(|fns| fns.get(&name)) {
          let (parent_id, _) = test_module.register(
            name,
            Some(range),
            false,
            parent_id.map(str::to_owned),
          );
          visit_fn(fn_expr, &parent_id, text_info, test_module);
        }
      }
      _ => {
        if parent_id.is_none() {
          let node_range = node.range();
          let indexes = text_info.line_and_column_display(node_range.start);
          test_module.register(
            format!("Test {}:{}", indexes.line_number, indexes.column_number),
            Some(range),
            false,
            None,
          );
        }
      }
    }
  }
}

/// A structure which can be used to walk a branch of AST determining if the
/// branch contains any testing steps.
struct TestStepCollector<'a> {
  maybe_test_context: Option<String>,
  vars: HashSet<String>,
  parent_id: &'a str,
  text_info: &'a SourceTextInfo,
  test_module: &'a mut TestModule,
}

impl<'a> TestStepCollector<'a> {
  fn new(
    maybe_test_context: Option<String>,
    maybe_step_var: Option<String>,
    parent_id: &'a str,
    text_info: &'a SourceTextInfo,
    test_module: &'a mut TestModule,
  ) -> Self {
    let mut vars = HashSet::new();
    if let Some(var) = maybe_step_var {
      vars.insert(var);
    }
    Self {
      maybe_test_context,
      vars,
      parent_id,
      text_info,
      test_module,
    }
  }
}

impl Visit for TestStepCollector<'_> {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    if let ast::Callee::Expr(callee_expr) = &node.callee {
      match callee_expr.as_ref() {
        // Identify calls to identified variables
        ast::Expr::Ident(ident) => {
          if self.vars.contains(&ident.sym.to_string()) {
            visit_call_expr(
              node,
              None,
              source_range_to_lsp_range(&ident.range(), self.text_info),
              Some(self.parent_id),
              self.text_info,
              self.test_module,
            );
          }
        }
        // Identify calls to `test.step()`
        ast::Expr::Member(member_expr) => {
          if let Some(test_context) = &self.maybe_test_context {
            if let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop {
              if ns_prop_ident.sym.eq("step") {
                if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                  if ident.sym == *test_context {
                    visit_call_expr(
                      node,
                      None,
                      source_range_to_lsp_range(
                        &ns_prop_ident.range(),
                        self.text_info,
                      ),
                      Some(self.parent_id),
                      self.text_info,
                      self.test_module,
                    );
                  }
                }
              }
            }
          }
        }
        _ => (),
      }
    }
  }

  fn visit_var_decl(&mut self, node: &ast::VarDecl) {
    if let Some(test_context) = &self.maybe_test_context {
      for decl in &node.decls {
        let Some(init) = &decl.init else {
          continue;
        };

        match init.as_ref() {
          // Identify destructured assignments of `step` from test context
          ast::Expr::Ident(ident) => {
            if ident.sym != *test_context {
              continue;
            }
            let ast::Pat::Object(object_pat) = &decl.name else {
              continue;
            };

            for prop in &object_pat.props {
              match prop {
                ast::ObjectPatProp::Assign(prop) => {
                  if prop.key.sym.eq("step") {
                    self.vars.insert(prop.key.sym.to_string());
                  }
                }
                ast::ObjectPatProp::KeyValue(prop) => {
                  if let ast::PropName::Ident(key_ident) = &prop.key {
                    if key_ident.sym.eq("step") {
                      if let ast::Pat::Ident(value_ident) = &prop.value.as_ref()
                      {
                        self.vars.insert(value_ident.id.sym.to_string());
                      }
                    }
                  }
                }
                _ => (),
              }
            }
          }
          // Identify variable assignments where the init is test context
          // `.step`
          ast::Expr::Member(member_expr) => {
            let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() else {
              continue;
            };

            if obj_ident.sym != *test_context {
              continue;
            }

            let ast::MemberProp::Ident(prop_ident) = &member_expr.prop else {
              continue;
            };

            if prop_ident.sym.eq("step") {
              if let ast::Pat::Ident(binding_ident) = &decl.name {
                self.vars.insert(binding_ident.id.sym.to_string());
              }
            }
          }
          _ => (),
        }
      }
    }
  }
}

/// Walk an AST and determine if it contains any `Deno.test` tests.
pub struct TestCollector {
  test_module: TestModule,
  vars: HashSet<String>,
  fns: HashMap<String, ast::Function>,
  text_info: SourceTextInfo,
}

impl TestCollector {
  pub fn new(specifier: ModuleSpecifier, text_info: SourceTextInfo) -> Self {
    Self {
      test_module: TestModule::new(specifier),
      vars: HashSet::new(),
      fns: HashMap::new(),
      text_info,
    }
  }

  /// Move out the test definitions
  pub fn take(self) -> TestModule {
    self.test_module
  }
}

impl Visit for TestCollector {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    fn visit_if_deno_test(
      collector: &mut TestCollector,
      node: &ast::CallExpr,
      range: &deno_ast::SourceRange,
      ns_prop_ident: &ast::IdentName,
      member_expr: &ast::MemberExpr,
    ) {
      if ns_prop_ident.sym == "test" {
        let ast::Expr::Ident(ident) = member_expr.obj.as_ref() else {
          return;
        };

        if ident.sym != "Deno" {
          return;
        }

        visit_call_expr(
          node,
          Some(&collector.fns),
          source_range_to_lsp_range(range, &collector.text_info),
          None,
          &collector.text_info,
          &mut collector.test_module,
        );
      }
    }

    let ast::Callee::Expr(callee_expr) = &node.callee else {
      return;
    };

    match callee_expr.as_ref() {
      ast::Expr::Ident(ident) => {
        if self.vars.contains(&ident.sym.to_string()) {
          visit_call_expr(
            node,
            Some(&self.fns),
            source_range_to_lsp_range(&ident.range(), &self.text_info),
            None,
            &self.text_info,
            &mut self.test_module,
          );
        }
      }
      ast::Expr::Member(member_expr) => {
        let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop else {
          return;
        };

        let ns_prop_ident_name = ns_prop_ident.sym.to_string();

        visit_if_deno_test(
          self,
          node,
          &ns_prop_ident.range(),
          ns_prop_ident,
          member_expr,
        );

        if ns_prop_ident_name == "ignore" || ns_prop_ident_name == "only" {
          let ast::Expr::Member(child_member_expr) = member_expr.obj.as_ref()
          else {
            return;
          };

          let ast::MemberProp::Ident(child_ns_prop_ident) =
            &child_member_expr.prop
          else {
            return;
          };

          visit_if_deno_test(
            self,
            node,
            &ns_prop_ident.range(),
            child_ns_prop_ident,
            child_member_expr,
          );
        }
      }
      _ => (),
    }
  }

  fn visit_var_decl(&mut self, node: &ast::VarDecl) {
    for decl in &node.decls {
      let Some(init) = &decl.init else { continue };

      match init.as_ref() {
        // Identify destructured assignments of `test` from `Deno`
        ast::Expr::Ident(ident) => {
          if ident.sym != "Deno" {
            continue;
          }

          let ast::Pat::Object(object_pat) = &decl.name else {
            continue;
          };

          for prop in &object_pat.props {
            match prop {
              ast::ObjectPatProp::Assign(prop) => {
                let name = prop.key.sym.to_string();
                if name == "test" {
                  self.vars.insert(name);
                }
              }
              ast::ObjectPatProp::KeyValue(prop) => {
                let ast::PropName::Ident(key_ident) = &prop.key else {
                  continue;
                };

                if key_ident.sym == "test" {
                  if let ast::Pat::Ident(value_ident) = &prop.value.as_ref() {
                    self.vars.insert(value_ident.id.sym.to_string());
                  }
                }
              }
              _ => (),
            }
          }
        }
        // Identify variable assignments where the init is `Deno.test`
        ast::Expr::Member(member_expr) => {
          let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() else {
            continue;
          };

          if obj_ident.sym != "Deno" {
            continue;
          };

          let ast::MemberProp::Ident(prop_ident) = &member_expr.prop else {
            continue;
          };

          if prop_ident.sym != "test" {
            continue;
          }

          if let ast::Pat::Ident(binding_ident) = &decl.name {
            self.vars.insert(binding_ident.id.sym.to_string());
          }
        }
        _ => (),
      }
    }
  }

  fn visit_fn_decl(&mut self, n: &ast::FnDecl) {
    self
      .fns
      .insert(n.ident.sym.to_string(), *n.function.clone());
  }
}

#[cfg(test)]
pub mod tests {
  use crate::lsp::testing::definitions::TestDefinition;

  use super::*;
  use deno_core::resolve_url;
  use lsp::Position;

  pub fn new_range(l1: u32, c1: u32, l2: u32, c2: u32) -> Range {
    Range::new(Position::new(l1, c1), Position::new(l2, c2))
  }

  fn collect(source: &str) -> TestModule {
    let specifier = resolve_url("file:///a/example.ts").unwrap();

    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source.into(),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let text_info = parsed_module.text_info_lazy().clone();
    let mut collector = TestCollector::new(specifier, text_info);
    parsed_module.program().visit_with(&mut collector);
    collector.take()
  }

  #[test]
  fn test_test_collector_test() {
    let test_module = collect(
      r#"
      Deno.test("test", () => {});
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
            .to_string(),
          TestDefinition {
            id:
              "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
                .to_string(),
            name: "test".to_string(),
            range: Some(new_range(1, 11, 1, 15)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_test_tpl() {
    let test_module = collect(
      r#"
      Deno.test(`test`, () => {});
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
            .to_string(),
          TestDefinition {
            id:
              "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
                .to_string(),
            name: "test".to_string(),
            range: Some(new_range(1, 11, 1, 15)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_a() {
    let test_module = collect(
      r#"
      Deno.test({
        name: "test",
        async fn(t) {
          await t.step("step", ({ step }) => {
            await step({
              name: "sub step",
              fn() {}
            })
          });
        }
      });
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![
          (
            "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string(),
            TestDefinition {
              id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string(),
              name: "test".to_string(),
              range: Some(new_range(1, 11, 1, 15)),
              is_dynamic: false,
              parent_id: None,
              step_ids: vec!["704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string()].into_iter().collect(),
            }
          ),
          (
            "704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string(),
            TestDefinition {
              id: "704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string(),
              name: "step".to_string(),
              range: Some(new_range(4, 18, 4, 22)),
              is_dynamic: false,
              parent_id: Some("4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string()),
              step_ids: vec!["0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string()].into_iter().collect(),
            }
          ),
          (
            "0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string(),
            TestDefinition {
              id: "0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string(),
              name: "sub step".to_string(),
              range: Some(new_range(5, 18, 5, 22)),
              is_dynamic: false,
              parent_id: Some("704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string()),
              step_ids: Default::default(),
            }
          ),
        ].into_iter().collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_a_tpl() {
    let test_module = collect(
      r#"
      Deno.test({
        name: `test`,
        async fn(t) {
          await t.step(`step`, ({ step }) => {
            await step({
              name: `sub step`,
              fn() {}
            })
          });
        }
      });
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![
          (
            "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string(),
            TestDefinition {
              id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string(),
              name: "test".to_string(),
              range: Some(new_range(1, 11, 1, 15)),
              is_dynamic: false,
              parent_id: None,
              step_ids: vec!["704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string()].into_iter().collect(),
            }
          ),
          (
            "704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string(),
            TestDefinition {
              id: "704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string(),
              name: "step".to_string(),
              range: Some(new_range(4, 18, 4, 22)),
              is_dynamic: false,
              parent_id: Some("4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9".to_string()),
              step_ids: vec!["0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string()].into_iter().collect(),
            }
          ),
          (
            "0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string(),
            TestDefinition {
              id: "0d006a4ec0abaa9cc1d18256b1ccd2677a4c882ff5cb807123890f7528ab1e8d".to_string(),
              name: "sub step".to_string(),
              range: Some(new_range(5, 18, 5, 22)),
              is_dynamic: false,
              parent_id: Some("704d24083fd4a3e1bd204faa20827dc594334812245e5d45dda222b3edc60a0c".to_string()),
              step_ids: Default::default(),
            }
          ),
        ].into_iter().collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_destructure() {
    let test_module = collect(
      r#"
      const { test } = Deno;
      test("test", () => {});
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
            .to_string(),
          TestDefinition {
            id:
              "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
                .to_string(),
            name: "test".to_string(),
            range: Some(new_range(2, 6, 2, 10)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_destructure_rebind_step() {
    let test_module = collect(
      r#"
      Deno.test(async function useFnName({ step: s }) {
        await s("step", () => {});
      });
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![
          (
            "86b4c821900e38fc89f24bceb0e45193608ab3f9d2a6019c7b6a5aceff5d7df2".to_string(),
            TestDefinition {
              id: "86b4c821900e38fc89f24bceb0e45193608ab3f9d2a6019c7b6a5aceff5d7df2".to_string(),
              name: "useFnName".to_string(),
              range: Some(new_range(1, 11, 1, 15)),
              is_dynamic: false,
              parent_id: None,
              step_ids: vec!["dac8a169b8f8c6babf11122557ea545de2733bfafed594d044b22bc6863a0856".to_string()].into_iter().collect(),
            }
          ),
          (
            "dac8a169b8f8c6babf11122557ea545de2733bfafed594d044b22bc6863a0856".to_string(),
            TestDefinition {
              id: "dac8a169b8f8c6babf11122557ea545de2733bfafed594d044b22bc6863a0856".to_string(),
              name: "step".to_string(),
              range: Some(new_range(2, 14, 2, 15)),
              is_dynamic: false,
              parent_id: Some("86b4c821900e38fc89f24bceb0e45193608ab3f9d2a6019c7b6a5aceff5d7df2".to_string()),
              step_ids: Default::default(),
            }
          ),
        ].into_iter().collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_rebind() {
    let test_module = collect(
      r#"
      const t = Deno.test;
      t("test", () => {});
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
            .to_string(),
          TestDefinition {
            id:
              "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
                .to_string(),
            name: "test".to_string(),
            range: Some(new_range(2, 6, 2, 7)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_separate_test_function_with_string_name() {
    let test_module = collect(
      r#"
      function someFunction() {}
      Deno.test("test", someFunction);
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
            .to_string(),
          TestDefinition {
            id:
              "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
                .to_string(),
            name: "test".to_string(),
            range: Some(new_range(2, 11, 2, 15)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_function_only() {
    let test_module = collect(
      r#"
      Deno.test(async function someFunction() {});
      Deno.test.ignore(function foo() {});
      Deno.test.only(function bar() {});
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![
          (
            "87f28e06f5ddadd90a74a93b84df2e31b9edced8301b0ad4c8fbab8d806ec99d".to_string(),
            TestDefinition {
              id: "87f28e06f5ddadd90a74a93b84df2e31b9edced8301b0ad4c8fbab8d806ec99d".to_string(),
              name: "foo".to_string(),
              range: Some(new_range(2, 16, 2, 22)),
              is_dynamic: false,
              parent_id: None,
              step_ids: Default::default(),
            },
          ),
          (
            "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568".to_string(),
            TestDefinition {
              id: "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568".to_string(),
              name: "someFunction".to_string(),
              range: Some(new_range(1, 11, 1, 15)),
              is_dynamic: false,
              parent_id: None,
              step_ids: Default::default(),
            }
          ),
          (
            "e1bd61cdaf5e64863d3d85baffe3e43bd57cdb8dc0b5d6a9e03ade18b7f68d47".to_string(),
            TestDefinition {
              id: "e1bd61cdaf5e64863d3d85baffe3e43bd57cdb8dc0b5d6a9e03ade18b7f68d47".to_string(),
              name: "bar".to_string(),
              range: Some(new_range(3, 16, 3, 20)),
                is_dynamic: false,
                parent_id: None,
                step_ids: Default::default(),
            }
          )
        ]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_separate_test_function() {
    let test_module = collect(
      r#"
      async function someFunction() {}
      Deno.test(someFunction);
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568"
            .to_string(),
          TestDefinition {
            id:
              "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568"
                .to_string(),
            name: "someFunction".to_string(),
            range: Some(new_range(2, 11, 2, 15)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  #[test]
  fn test_test_collector_unknown_test() {
    let test_module = collect(
      r#"
      const someFunction = () => ({ name: "test", fn: () => {} });
      Deno.test(someFunction());
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![(
          "6d05d6dc35548b86a1e70acaf24a5bc2dd35db686b35b685ad5931d201b4a918"
            .to_string(),
          TestDefinition {
            id:
              "6d05d6dc35548b86a1e70acaf24a5bc2dd35db686b35b685ad5931d201b4a918"
                .to_string(),
            name: "Test 3:7".to_string(),
            range: Some(new_range(2, 11, 2, 15)),
            is_dynamic: false,
            parent_id: None,
            step_ids: Default::default(),
          }
        ),]
        .into_iter()
        .collect(),
      }
    );
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/656.
  #[test]
  fn test_test_collector_nested_steps_same_name_and_level() {
    let test_module = collect(
      r#"
      Deno.test("1", async (t) => {
        await t.step("step 1", async (t) => {
          await t.step("nested step", () => {});
        });
        await t.step("step 2", async (t) => {
          await t.step("nested step", () => {});
        });
      });
    "#,
    );

    assert_eq!(
      &test_module,
      &TestModule {
        specifier: test_module.specifier.clone(),
        defs: vec![
          (
            "3799fc549a32532145ffc8532b0cd943e025bbc19a02e2cde9be94f87bceb829".to_string(),
            TestDefinition {
              id: "3799fc549a32532145ffc8532b0cd943e025bbc19a02e2cde9be94f87bceb829".to_string(),
              name: "1".to_string(),
              range: Some(new_range(1, 11, 1, 15)),
              is_dynamic: false,
              parent_id: None,
              step_ids: vec![
                "e714fc695c0895327bf7148a934c3303ad515af029a14906be46f80340c6d7e3".to_string(),
                "ec6b03d3dd3dde78d2d11ed981d3386083aeca701510cc049189d74bd79f8587".to_string(),
              ].into_iter().collect()
            }
          ),
          (
            "e714fc695c0895327bf7148a934c3303ad515af029a14906be46f80340c6d7e3".to_string(),
            TestDefinition {
              id: "e714fc695c0895327bf7148a934c3303ad515af029a14906be46f80340c6d7e3".to_string(),
              name: "step 1".to_string(),
              range: Some(new_range(2, 16, 2, 20)),
              is_dynamic: false,
              parent_id: Some("3799fc549a32532145ffc8532b0cd943e025bbc19a02e2cde9be94f87bceb829".to_string()),
              step_ids: vec!["d874949e18dfc297e15c52ff13f13b4e6ae911ec1818b2c761e3313bc018a3ab".to_string()].into_iter().collect()
            }
          ),
          (
            "d874949e18dfc297e15c52ff13f13b4e6ae911ec1818b2c761e3313bc018a3ab".to_string(),
            TestDefinition {
              id: "d874949e18dfc297e15c52ff13f13b4e6ae911ec1818b2c761e3313bc018a3ab".to_string(),
              name: "nested step".to_string(),
              range: Some(new_range(3, 18, 3, 22)),
              is_dynamic: false,
              parent_id: Some("e714fc695c0895327bf7148a934c3303ad515af029a14906be46f80340c6d7e3".to_string()),
              step_ids: Default::default(),
            }
          ),
          (
            "ec6b03d3dd3dde78d2d11ed981d3386083aeca701510cc049189d74bd79f8587".to_string(),
            TestDefinition {
              id: "ec6b03d3dd3dde78d2d11ed981d3386083aeca701510cc049189d74bd79f8587".to_string(),
              name: "step 2".to_string(),
              range: Some(new_range(5, 16, 5, 20)),
              is_dynamic: false,
              parent_id: Some("3799fc549a32532145ffc8532b0cd943e025bbc19a02e2cde9be94f87bceb829".to_string()),
              step_ids: vec!["96729f1f1608e50160b0bf11946719384b4021fd1d26b14eff7765034b3d2684".to_string()].into_iter().collect()
            }
          ),
          (
            "96729f1f1608e50160b0bf11946719384b4021fd1d26b14eff7765034b3d2684".to_string(),
            TestDefinition {
              id: "96729f1f1608e50160b0bf11946719384b4021fd1d26b14eff7765034b3d2684".to_string(),
              name: "nested step".to_string(),
              range: Some(new_range(6, 18, 6, 22)),
              is_dynamic: false,
              parent_id: Some("ec6b03d3dd3dde78d2d11ed981d3386083aeca701510cc049189d74bd79f8587".to_string()),
              step_ids: Default::default(),
            }
          ),
        ].into_iter().collect(),
      }
    );
  }
}
