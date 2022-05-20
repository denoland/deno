// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::definitions::TestDefinition;

use deno_ast::swc::ast;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_core::ModuleSpecifier;
use std::collections::HashSet;

/// Parse an arrow expression for any test steps and return them.
fn arrow_to_steps(
  parent: &str,
  level: usize,
  arrow_expr: &ast::ArrowExpr,
) -> Option<Vec<TestDefinition>> {
  if let Some((maybe_test_context, maybe_step_var)) =
    parse_test_context_param(arrow_expr.params.get(0))
  {
    let mut collector = TestStepCollector::new(
      parent.to_string(),
      level,
      maybe_test_context,
      maybe_step_var,
    );
    arrow_expr.body.visit_with(&mut collector);
    let steps = collector.take();
    if !steps.is_empty() {
      Some(steps)
    } else {
      None
    }
  } else {
    None
  }
}

/// Parse a function for any test steps and return them.
fn fn_to_steps(
  parent: &str,
  level: usize,
  function: &ast::Function,
) -> Option<Vec<TestDefinition>> {
  if let Some((maybe_test_context, maybe_step_var)) =
    parse_test_context_param(function.params.get(0).map(|p| &p.pat))
  {
    let mut collector = TestStepCollector::new(
      parent.to_string(),
      level,
      maybe_test_context,
      maybe_step_var,
    );
    function.body.visit_with(&mut collector);
    let steps = collector.take();
    if !steps.is_empty() {
      Some(steps)
    } else {
      None
    }
  } else {
    None
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
fn check_call_expr(
  parent: &str,
  node: &ast::CallExpr,
  level: usize,
) -> Option<(String, Option<Vec<TestDefinition>>)> {
  if let Some(expr) = node.args.get(0).map(|es| es.expr.as_ref()) {
    match expr {
      ast::Expr::Object(obj_lit) => {
        let mut maybe_name = None;
        let mut steps = None;
        for prop in &obj_lit.props {
          if let ast::PropOrSpread::Prop(prop) = prop {
            match prop.as_ref() {
              ast::Prop::KeyValue(key_value_prop) => {
                if let ast::PropName::Ident(ast::Ident { sym, .. }) =
                  &key_value_prop.key
                {
                  match sym.to_string().as_str() {
                    "name" => match key_value_prop.value.as_ref() {
                      // matches string literals (e.g. "test name" or
                      // 'test name')
                      ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
                        maybe_name = Some(lit_str.value.to_string());
                      }
                      // matches template literals with only a single quasis
                      // (e.g. `test name`)
                      ast::Expr::Tpl(tpl) => {
                        if tpl.quasis.len() == 1 {
                          if let Some(tpl_element) = tpl.quasis.get(0) {
                            maybe_name = Some(tpl_element.raw.to_string());
                          }
                        }
                      }
                      _ => (),
                    },
                    "fn" => match key_value_prop.value.as_ref() {
                      ast::Expr::Arrow(arrow_expr) => {
                        steps = arrow_to_steps(parent, level, arrow_expr);
                      }
                      ast::Expr::Fn(fn_expr) => {
                        steps = fn_to_steps(parent, level, &fn_expr.function);
                      }
                      _ => (),
                    },
                    _ => (),
                  }
                }
              }
              ast::Prop::Method(method_prop) => {
                steps = fn_to_steps(parent, level, &method_prop.function);
              }
              _ => (),
            }
          }
        }
        maybe_name.map(|name| (name, steps))
      }
      ast::Expr::Fn(fn_expr) => {
        if let Some(ast::Ident { sym, .. }) = fn_expr.ident.as_ref() {
          let name = sym.to_string();
          let steps = fn_to_steps(parent, level, &fn_expr.function);
          Some((name, steps))
        } else {
          None
        }
      }
      ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
        let name = lit_str.value.to_string();
        let mut steps = None;
        match node.args.get(1).map(|es| es.expr.as_ref()) {
          Some(ast::Expr::Fn(fn_expr)) => {
            steps = fn_to_steps(parent, level, &fn_expr.function);
          }
          Some(ast::Expr::Arrow(arrow_expr)) => {
            steps = arrow_to_steps(parent, level, arrow_expr);
          }
          _ => (),
        }
        Some((name, steps))
      }
      _ => None,
    }
  } else {
    None
  }
}

/// A structure which can be used to walk a branch of AST determining if the
/// branch contains any testing steps.
struct TestStepCollector {
  steps: Vec<TestDefinition>,
  level: usize,
  parent: String,
  maybe_test_context: Option<String>,
  vars: HashSet<String>,
}

impl TestStepCollector {
  fn new(
    parent: String,
    level: usize,
    maybe_test_context: Option<String>,
    maybe_step_var: Option<String>,
  ) -> Self {
    let mut vars = HashSet::new();
    if let Some(var) = maybe_step_var {
      vars.insert(var);
    }
    Self {
      steps: Vec::default(),
      level,
      parent,
      maybe_test_context,
      vars,
    }
  }

  fn add_step<N: AsRef<str>>(
    &mut self,
    name: N,
    range: SourceRange,
    steps: Option<Vec<TestDefinition>>,
  ) {
    let step = TestDefinition::new_step(
      name.as_ref().to_string(),
      range,
      self.parent.clone(),
      self.level,
      steps,
    );
    self.steps.push(step);
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, range: SourceRange) {
    if let Some((name, steps)) =
      check_call_expr(&self.parent, node, self.level + 1)
    {
      self.add_step(name, range, steps);
    }
  }

  /// Move out the test definitions
  pub fn take(self) -> Vec<TestDefinition> {
    self.steps
  }
}

impl Visit for TestStepCollector {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    if let ast::Callee::Expr(callee_expr) = &node.callee {
      match callee_expr.as_ref() {
        // Identify calls to identified variables
        ast::Expr::Ident(ident) => {
          if self.vars.contains(&ident.sym.to_string()) {
            self.check_call_expr(node, ident.range());
          }
        }
        // Identify calls to `test.step()`
        ast::Expr::Member(member_expr) => {
          if let Some(test_context) = &self.maybe_test_context {
            if let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop {
              if ns_prop_ident.sym.eq("step") {
                if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                  if ident.sym == *test_context {
                    self.check_call_expr(node, ns_prop_ident.range());
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
        if let Some(init) = &decl.init {
          match init.as_ref() {
            // Identify destructured assignments of `step` from test context
            ast::Expr::Ident(ident) => {
              if ident.sym == *test_context {
                if let ast::Pat::Object(object_pat) = &decl.name {
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
                            if let ast::Pat::Ident(value_ident) =
                              &prop.value.as_ref()
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
              }
            }
            // Identify variable assignments where the init is test context
            // `.step`
            ast::Expr::Member(member_expr) => {
              if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() {
                if obj_ident.sym == *test_context {
                  if let ast::MemberProp::Ident(prop_ident) = &member_expr.prop
                  {
                    if prop_ident.sym.eq("step") {
                      if let ast::Pat::Ident(binding_ident) = &decl.name {
                        self.vars.insert(binding_ident.id.sym.to_string());
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
    }
  }
}

/// Walk an AST and determine if it contains any `Deno.test` tests.
pub struct TestCollector {
  definitions: Vec<TestDefinition>,
  specifier: ModuleSpecifier,
  vars: HashSet<String>,
}

impl TestCollector {
  pub fn new(specifier: ModuleSpecifier) -> Self {
    Self {
      definitions: Vec::new(),
      specifier,
      vars: HashSet::new(),
    }
  }

  fn add_definition<N: AsRef<str>>(
    &mut self,
    name: N,
    range: SourceRange,
    steps: Option<Vec<TestDefinition>>,
  ) {
    let definition = TestDefinition::new(
      &self.specifier,
      name.as_ref().to_string(),
      range,
      steps,
    );
    self.definitions.push(definition);
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, range: SourceRange) {
    if let Some((name, steps)) =
      check_call_expr(self.specifier.as_str(), node, 1)
    {
      self.add_definition(name, range, steps);
    }
  }

  /// Move out the test definitions
  pub fn take(self) -> Vec<TestDefinition> {
    self.definitions
  }
}

impl Visit for TestCollector {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    if let ast::Callee::Expr(callee_expr) = &node.callee {
      match callee_expr.as_ref() {
        ast::Expr::Ident(ident) => {
          if self.vars.contains(&ident.sym.to_string()) {
            self.check_call_expr(node, ident.range());
          }
        }
        ast::Expr::Member(member_expr) => {
          if let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop {
            if ns_prop_ident.sym.to_string() == "test" {
              if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                if ident.sym.to_string() == "Deno" {
                  self.check_call_expr(node, ns_prop_ident.range());
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
    for decl in &node.decls {
      if let Some(init) = &decl.init {
        match init.as_ref() {
          // Identify destructured assignments of `test` from `Deno`
          ast::Expr::Ident(ident) => {
            if ident.sym.to_string() == "Deno" {
              if let ast::Pat::Object(object_pat) = &decl.name {
                for prop in &object_pat.props {
                  match prop {
                    ast::ObjectPatProp::Assign(prop) => {
                      let name = prop.key.sym.to_string();
                      if name == "test" {
                        self.vars.insert(name);
                      }
                    }
                    ast::ObjectPatProp::KeyValue(prop) => {
                      if let ast::PropName::Ident(key_ident) = &prop.key {
                        if key_ident.sym.to_string() == "test" {
                          if let ast::Pat::Ident(value_ident) =
                            &prop.value.as_ref()
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
            }
          }
          // Identify variable assignments where the init is `Deno.test`
          ast::Expr::Member(member_expr) => {
            if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() {
              if obj_ident.sym.to_string() == "Deno" {
                if let ast::MemberProp::Ident(prop_ident) = &member_expr.prop {
                  if prop_ident.sym.to_string() == "test" {
                    if let ast::Pat::Ident(binding_ident) = &decl.name {
                      self.vars.insert(binding_ident.id.sym.to_string());
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
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use deno_ast::StartSourcePos;
  use deno_core::resolve_url;

  pub fn new_range(start: usize, end: usize) -> SourceRange {
    SourceRange::new(
      StartSourcePos::START_SOURCE_POS + start,
      StartSourcePos::START_SOURCE_POS + end,
    )
  }

  #[test]
  fn test_test_collector() {
    let specifier = resolve_url("file:///a/example.ts").unwrap();
    let source = r#"
      Deno.test({
        name: "test a",
        async fn(t) {
          await t.step("a step", ({ step }) => {
            await step({
              name: "sub step",
              fn() {}
            })
          });
        }
      });

      Deno.test(async function useFnName({ step: s }) {
        await s("step c", () => {});
      });

      Deno.test("test b", () => {});

      const { test } = Deno;
      test("test c", () => {});

      const t = Deno.test;
      t("test d", () => {});
    "#;

    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: deno_ast::SourceTextInfo::new(source.into()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let mut collector = TestCollector::new(specifier);
    parsed_module.module().visit_with(&mut collector);
    assert_eq!(
      collector.take(),
      vec![
        TestDefinition {
          id: "cf31850c831233526df427cdfd25b6b84b2af0d6ce5f8ee1d22c465234b46348".to_string(),
          level: 0,
          name: "test a".to_string(),
          range: new_range(12, 16),
          steps: Some(vec![
            TestDefinition {
              id: "4c7333a1e47721631224408c467f32751fe34b876cab5ec1f6ac71980ff15ad3".to_string(),
              level: 1,
              name: "a step".to_string(),
              range: new_range(83, 87),
              steps: Some(vec![
                TestDefinition {
                  id: "abf356f59139b77574089615f896a6f501c010985d95b8a93abeb0069ccb2201".to_string(),
                  level: 2,
                  name: "sub step".to_string(),
                  range: new_range(132, 136),
                  steps: None,
                }
              ])
            }
          ]),
        },
        TestDefinition {
          id: "86b4c821900e38fc89f24bceb0e45193608ab3f9d2a6019c7b6a5aceff5d7df2".to_string(),
          level: 0,
          name: "useFnName".to_string(),
          range: new_range(254, 258),
          steps: Some(vec![
            TestDefinition {
              id: "67a390d0084ae5fb88f3510c470a72a553581f1d0d5ba5fa89aee7a754f3953a".to_string(),
              level: 1,
              name: "step c".to_string(),
              range: new_range(313, 314),
              steps: None,
            }
          ])
        },
        TestDefinition {
          id: "580eda89d7f5e619774c20e13b7d07a8e77c39cba101d60565144d48faa837cb".to_string(),
          level: 0,
          name: "test b".to_string(),
          range: new_range(358, 362),
          steps: None,
        },
        TestDefinition {
          id: "0b7c6bf3cd617018d33a1bf982a08fe088c5bb54fcd5eb9e802e7c137ec1af94".to_string(),
          level: 0,
          name: "test c".to_string(),
          range: new_range(420, 424),
          steps: None,
        },
        TestDefinition {
          id: "69d9fe87f64f5b66cb8b631d4fd2064e8224b8715a049be54276c42189ff8f9f".to_string(),
          level: 0,
          name: "test d".to_string(),
          range: new_range(480, 481),
          steps: None,
        }
      ]
    );
  }
}
