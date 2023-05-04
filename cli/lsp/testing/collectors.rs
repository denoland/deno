// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::definitions::TestDefinition;

use deno_ast::swc::ast;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::collections::HashSet;

/// Parse an arrow expression for any test steps and return them.
fn arrow_to_steps(
  parent: &str,
  level: usize,
  arrow_expr: &ast::ArrowExpr,
) -> Vec<TestDefinition> {
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
    collector.take()
  } else {
    vec![]
  }
}

/// Parse a function for any test steps and return them.
fn fn_to_steps(
  parent: &str,
  level: usize,
  function: &ast::Function,
) -> Vec<TestDefinition> {
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
    collector.take()
  } else {
    vec![]
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
  fns: Option<&HashMap<String, ast::Function>>,
  text_info: Option<&SourceTextInfo>,
) -> Option<(String, Vec<TestDefinition>)> {
  if let Some(expr) = node.args.get(0).map(|es| es.expr.as_ref()) {
    match expr {
      ast::Expr::Object(obj_lit) => {
        let mut maybe_name = None;
        let mut steps = vec![];
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
                          maybe_name = Some(tpl.quasis[0].raw.to_string());
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
        let mut steps = vec![];
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
      ast::Expr::Tpl(tpl) => {
        if tpl.quasis.len() == 1 {
          let mut steps = vec![];
          match node.args.get(1).map(|es| es.expr.as_ref()) {
            Some(ast::Expr::Fn(fn_expr)) => {
              steps = fn_to_steps(parent, level, &fn_expr.function);
            }
            Some(ast::Expr::Arrow(arrow_expr)) => {
              steps = arrow_to_steps(parent, level, arrow_expr);
            }
            _ => (),
          }

          Some((tpl.quasis[0].raw.to_string(), steps))
        } else {
          None
        }
      }
      ast::Expr::Ident(ident) => {
        let name = ident.sym.to_string();
        fns.and_then(|fns| {
          fns
            .get(&name)
            .map(|fn_expr| (name, fn_to_steps(parent, level, fn_expr)))
        })
      }
      _ => {
        if let Some(text_info) = text_info {
          let range = node.range();
          let indexes = text_info.line_and_column_display(range.start);
          Some((
            format!("Test {}:{}", indexes.line_number, indexes.column_number),
            vec![],
          ))
        } else {
          None
        }
      }
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
    steps: Vec<TestDefinition>,
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
      check_call_expr(&self.parent, node, self.level + 1, None, None)
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
  fns: HashMap<String, ast::Function>,
  text_info: SourceTextInfo,
}

impl TestCollector {
  pub fn new(specifier: ModuleSpecifier, text_info: SourceTextInfo) -> Self {
    Self {
      definitions: Vec::new(),
      specifier,
      vars: HashSet::new(),
      fns: HashMap::new(),
      text_info,
    }
  }

  fn add_definition<N: AsRef<str>>(
    &mut self,
    name: N,
    range: SourceRange,
    steps: Vec<TestDefinition>,
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
    if let Some((name, steps)) = check_call_expr(
      self.specifier.as_str(),
      node,
      1,
      Some(&self.fns),
      Some(&self.text_info),
    ) {
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

  fn visit_fn_decl(&mut self, n: &ast::FnDecl) {
    self
      .fns
      .insert(n.ident.sym.to_string(), *n.function.clone());
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

  fn collect(source: &str) -> Vec<TestDefinition> {
    let specifier = resolve_url("file:///a/example.ts").unwrap();

    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: deno_ast::SourceTextInfo::new(source.into()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let text_info = parsed_module.text_info().clone();
    let mut collector = TestCollector::new(specifier, text_info);
    parsed_module.module().visit_with(&mut collector);
    collector.take()
  }

  #[test]
  fn test_test_collector_test() {
    let res = collect(
      r#"
      Deno.test("test", () => {});
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(12, 16),
        steps: vec![],
      },]
    );
  }

  #[test]
  fn test_test_collector_test_tpl() {
    let res = collect(
      r#"
      Deno.test(`test`, () => {});
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(12, 16),
        steps: vec![],
      },]
    );
  }

  #[test]
  fn test_test_collector_a() {
    let res = collect(
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
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(12, 16),
        steps: vec![TestDefinition {
          id:
            "b3b2daad49e5c3095fe26aba0a840131f3d8f32e105e95507f5fc5118642b059"
              .to_string(),
          level: 1,
          name: "step".to_string(),
          range: new_range(81, 85),
          steps: vec![TestDefinition {
            id:
              "abf356f59139b77574089615f896a6f501c010985d95b8a93abeb0069ccb2201"
                .to_string(),
            level: 2,
            name: "sub step".to_string(),
            range: new_range(128, 132),
            steps: vec![],
          }]
        }],
      },]
    );
  }

  #[test]
  fn test_test_collector_a_tpl() {
    let res = collect(
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
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(12, 16),
        steps: vec![TestDefinition {
          id:
            "b3b2daad49e5c3095fe26aba0a840131f3d8f32e105e95507f5fc5118642b059"
              .to_string(),
          level: 1,
          name: "step".to_string(),
          range: new_range(81, 85),
          steps: vec![TestDefinition {
            id:
              "abf356f59139b77574089615f896a6f501c010985d95b8a93abeb0069ccb2201"
                .to_string(),
            level: 2,
            name: "sub step".to_string(),
            range: new_range(128, 132),
            steps: vec![],
          }]
        }],
      },]
    );
  }

  #[test]
  fn test_test_collector_destructure() {
    let res = collect(
      r#"
      const { test } = Deno;
      test("test", () => {});
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(36, 40),
        steps: vec![],
      }]
    );
  }

  #[test]
  fn test_test_collector_destructure_rebind_step() {
    let res = collect(
      r#"
      Deno.test(async function useFnName({ step: s }) {
        await s("step", () => {});
      });
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "86b4c821900e38fc89f24bceb0e45193608ab3f9d2a6019c7b6a5aceff5d7df2"
          .to_string(),
        level: 0,
        name: "useFnName".to_string(),
        range: new_range(12, 16),
        steps: vec![TestDefinition {
          id:
            "b3b2daad49e5c3095fe26aba0a840131f3d8f32e105e95507f5fc5118642b059"
              .to_string(),
          level: 1,
          name: "step".to_string(),
          range: new_range(71, 72),
          steps: vec![],
        }],
      }]
    );
  }

  #[test]
  fn test_test_collector_rebind() {
    let res = collect(
      r#"
      const t = Deno.test;
      t("test", () => {});
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(34, 35),
        steps: vec![],
      }]
    );
  }

  #[test]
  fn test_test_collector_separate_test_function_with_string_name() {
    let res = collect(
      r#"
      function someFunction() {}
      Deno.test("test", someFunction);
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "4ebb361c93f76a0f1bac300638675609f1cf481e6f3b9006c3c98604b3a184e9"
          .to_string(),
        level: 0,
        name: "test".to_string(),
        range: new_range(45, 49),
        steps: vec![],
      }]
    );
  }

  #[test]
  fn test_test_collector_function_only() {
    let res = collect(
      r#"
      Deno.test(async function someFunction() {});
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568"
          .to_string(),
        level: 0,
        name: "someFunction".to_string(),
        range: new_range(12, 16),
        steps: vec![]
      }]
    );
  }

  #[test]
  fn test_test_collector_separate_test_function() {
    let res = collect(
      r#"
      async function someFunction() {}
      Deno.test(someFunction);
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "e0f6a73647b763f82176c98a019e54200b799a32007f9859fb782aaa9e308568"
          .to_string(),
        level: 0,
        name: "someFunction".to_string(),
        range: new_range(51, 55),
        steps: vec![]
      }]
    );
  }

  #[test]
  fn test_test_collector_unknown_test() {
    let res = collect(
      r#"
      const someFunction = () => ({ name: "test", fn: () => {} });
      Deno.test(someFunction());
    "#,
    );

    assert_eq!(
      res,
      vec![TestDefinition {
        id: "6d05d6dc35548b86a1e70acaf24a5bc2dd35db686b35b685ad5931d201b4a918"
          .to_string(),
        level: 0,
        name: "Test 3:7".to_string(),
        range: new_range(79, 83),
        steps: vec![]
      }]
    );
  }
}
