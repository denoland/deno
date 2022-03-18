// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::client::Client;
use super::client::TestingNotification;
use super::config;
use super::language_server::StateSnapshot;
use super::logging::lsp_log;
use super::lsp_custom;
use super::lsp_custom::TestRunProgressMessage;
use super::performance::Performance;

use crate::checksum;
use crate::create_main_worker;
use crate::emit;
use crate::flags;
use crate::located_script_name;
use crate::ops;
use crate::proc_state;
use crate::tools::test;

use deno_ast::swc::ast;
use deno_ast::swc::common::Span;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::create_basic_runtime;
use deno_runtime::tokio_util::run_basic;
use lspower::jsonrpc::Error as LspError;
use lspower::jsonrpc::Result as LspResult;
use lspower::lsp;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn span_to_range(
  span: &Span,
  source_text_info: &SourceTextInfo,
) -> Option<lsp::Range> {
  let start = source_text_info.line_and_column_index(span.lo);
  let end = source_text_info.line_and_column_index(span.hi);
  Some(lsp::Range {
    start: lsp::Position {
      line: start.line_index as u32,
      character: start.column_index as u32,
    },
    end: lsp::Position {
      line: end.line_index as u32,
      character: end.column_index as u32,
    },
  })
}

fn as_delete_notification(uri: ModuleSpecifier) -> TestingNotification {
  TestingNotification::DeleteModule(
    lsp_custom::TestModuleDeleteNotificationParams {
      text_document: lsp::TextDocumentIdentifier { uri },
    },
  )
}

#[derive(Debug, Clone)]
struct TestDefinition {
  id: String,
  level: usize,
  name: String,
  span: Span,
  steps: Option<Vec<TestDefinition>>,
}

impl TestDefinition {
  fn new(
    specifier: &ModuleSpecifier,
    name: String,
    span: Span,
    steps: Option<Vec<TestDefinition>>,
  ) -> Self {
    let id = checksum::gen(&[specifier.as_str().as_bytes(), name.as_bytes()]);
    Self {
      id,
      level: 0,
      name,
      span,
      steps,
    }
  }

  fn new_step(
    name: String,
    span: Span,
    parent: String,
    level: usize,
    steps: Option<Vec<TestDefinition>>,
  ) -> Self {
    let id = checksum::gen(&[
      parent.as_bytes(),
      &level.to_be_bytes(),
      name.as_bytes(),
    ]);
    Self {
      id,
      level,
      name,
      span,
      steps,
    }
  }

  fn as_test_data(
    &self,
    source_text_info: &SourceTextInfo,
  ) -> lsp_custom::TestData {
    lsp_custom::TestData {
      id: self.id.clone(),
      label: self.name.clone(),
      steps: self.steps.as_ref().map(|steps| {
        steps
          .iter()
          .map(|step| step.as_test_data(source_text_info))
          .collect()
      }),
      range: span_to_range(&self.span, source_text_info),
    }
  }

  fn find_step(&self, name: &str, level: usize) -> Option<&TestDefinition> {
    if let Some(steps) = &self.steps {
      for step in steps {
        if step.name == name && step.level == level {
          return Some(step);
        } else if let Some(step) = step.find_step(name, level) {
          return Some(step);
        }
      }
    }
    None
  }
}

#[derive(Debug)]
struct TestDefinitions {
  /// definitions of tests and their steps which were statically discovered from
  /// the source document.
  discovered: Vec<TestDefinition>,
  /// Tests and steps which the test runner notified us of, which were
  /// dynamically added
  injected: Vec<lsp_custom::TestData>,
  /// The version of the document that the discovered tests relate to.
  script_version: String,
}

impl TestDefinitions {
  /// Return the test definitions as a testing module notification.
  pub fn as_notification(
    &self,
    specifier: &ModuleSpecifier,
    maybe_root: Option<&ModuleSpecifier>,
    source_text_info: &SourceTextInfo,
  ) -> TestingNotification {
    let label = if let Some(root) = maybe_root {
      specifier.as_str().replace(root.as_str(), "")
    } else {
      specifier
        .path_segments()
        .and_then(|s| s.last().map(|s| s.to_string()))
        .unwrap_or_else(|| "<unknown>".to_string())
    };
    let mut tests_map: HashMap<String, lsp_custom::TestData> = self
      .injected
      .iter()
      .map(|td| (td.id.clone(), td.clone()))
      .collect();
    tests_map.extend(self.discovered.iter().map(|td| {
      let test_data = td.as_test_data(source_text_info);
      (test_data.id.clone(), test_data)
    }));
    TestingNotification::Module(lsp_custom::TestModuleNotificationParams {
      text_document: lsp::TextDocumentIdentifier {
        uri: specifier.clone(),
      },
      kind: lsp_custom::TestModuleNotificationKind::Replace,
      label,
      tests: tests_map.into_values().collect(),
    })
  }

  /// Return a test definition identified by the test ID.
  fn get_by_id<S: AsRef<str>>(&self, id: S) -> Option<&TestDefinition> {
    self
      .discovered
      .iter()
      .find(|td| td.id.as_str() == id.as_ref())
  }

  /// Return a test definition by the test name.
  fn get_by_name(&self, name: &str) -> Option<&TestDefinition> {
    self.discovered.iter().find(|td| td.name.as_str() == name)
  }

  fn get_step_by_name(
    &self,
    test_name: &str,
    level: usize,
    name: &str,
  ) -> Option<&TestDefinition> {
    self
      .get_by_name(test_name)
      .and_then(|td| td.find_step(name, level))
  }
}

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
                            maybe_name =
                              Some(tpl_element.raw.value.to_string());
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
    span: &Span,
    steps: Option<Vec<TestDefinition>>,
  ) {
    let step = TestDefinition::new_step(
      name.as_ref().to_string(),
      *span,
      self.parent.clone(),
      self.level,
      steps,
    );
    self.steps.push(step);
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, span: &Span) {
    if let Some((name, steps)) =
      check_call_expr(&self.parent, node, self.level + 1)
    {
      self.add_step(name, span, steps);
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
            self.check_call_expr(node, &ident.span);
          }
        }
        // Identify calls to `test.step()`
        ast::Expr::Member(member_expr) => {
          if let Some(test_context) = &self.maybe_test_context {
            if let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop {
              if ns_prop_ident.sym.eq("step") {
                if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                  if ident.sym == *test_context {
                    self.check_call_expr(node, &ns_prop_ident.span);
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
struct TestCollector {
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
    span: &Span,
    steps: Option<Vec<TestDefinition>>,
  ) {
    let definition = TestDefinition::new(
      &self.specifier,
      name.as_ref().to_string(),
      *span,
      steps,
    );
    self.definitions.push(definition);
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, span: &Span) {
    if let Some((name, steps)) =
      check_call_expr(self.specifier.as_str(), node, 1)
    {
      self.add_definition(name, span, steps);
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
            self.check_call_expr(node, &ident.span);
          }
        }
        ast::Expr::Member(member_expr) => {
          if let ast::MemberProp::Ident(ns_prop_ident) = &member_expr.prop {
            if ns_prop_ident.sym.to_string() == "test" {
              if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                if ident.sym.to_string() == "Deno" {
                  self.check_call_expr(node, &ns_prop_ident.span);
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

impl From<&test::TestDescription> for lsp_custom::TestData {
  fn from(desc: &test::TestDescription) -> Self {
    let id = checksum::gen(&[desc.origin.as_bytes(), desc.name.as_bytes()]);

    Self {
      id,
      label: desc.name.clone(),
      steps: Default::default(),
      range: None,
    }
  }
}

impl From<&test::TestDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &test::TestDescription) -> Self {
    let uri = ModuleSpecifier::parse(&desc.origin).unwrap();
    let id = Some(checksum::gen(&[
      desc.origin.as_bytes(),
      desc.name.as_bytes(),
    ]));

    Self {
      text_document: lsp::TextDocumentIdentifier { uri },
      id,
      step_id: None,
    }
  }
}

impl From<&test::TestStepDescription> for lsp_custom::TestData {
  fn from(desc: &test::TestStepDescription) -> Self {
    let id = checksum::gen(&[
      desc.test.origin.as_bytes(),
      &desc.level.to_be_bytes(),
      desc.name.as_bytes(),
    ]);

    Self {
      id,
      label: desc.name.clone(),
      steps: Default::default(),
      range: None,
    }
  }
}

impl From<&test::TestStepDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &test::TestStepDescription) -> Self {
    let uri = ModuleSpecifier::parse(&desc.test.origin).unwrap();
    let id = Some(checksum::gen(&[
      desc.test.origin.as_bytes(),
      desc.test.name.as_bytes(),
    ]));
    let step_id = Some(checksum::gen(&[
      desc.test.origin.as_bytes(),
      &desc.level.to_be_bytes(),
      desc.name.as_bytes(),
    ]));

    Self {
      text_document: lsp::TextDocumentIdentifier { uri },
      id,
      step_id,
    }
  }
}

#[derive(Debug, PartialEq)]
enum TestOrTestStepDescription {
  TestDescription(test::TestDescription),
  TestStepDescription(test::TestStepDescription),
}

impl From<&test::TestDescription> for TestOrTestStepDescription {
  fn from(desc: &test::TestDescription) -> Self {
    Self::TestDescription(desc.clone())
  }
}

impl From<&test::TestStepDescription> for TestOrTestStepDescription {
  fn from(desc: &test::TestStepDescription) -> Self {
    Self::TestStepDescription(desc.clone())
  }
}

impl From<&TestOrTestStepDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &TestOrTestStepDescription) -> lsp_custom::TestIdentifier {
    match desc {
      TestOrTestStepDescription::TestDescription(test_desc) => test_desc.into(),
      TestOrTestStepDescription::TestStepDescription(test_step_desc) => {
        test_step_desc.into()
      }
    }
  }
}

impl From<&TestOrTestStepDescription> for lsp_custom::TestData {
  fn from(desc: &TestOrTestStepDescription) -> Self {
    match desc {
      TestOrTestStepDescription::TestDescription(desc) => desc.into(),
      TestOrTestStepDescription::TestStepDescription(desc) => desc.into(),
    }
  }
}

fn as_test_messages<S: AsRef<str>>(
  message: S,
  is_markdown: bool,
) -> Vec<lsp_custom::TestMessage> {
  let message = lsp::MarkupContent {
    kind: if is_markdown {
      lsp::MarkupKind::Markdown
    } else {
      lsp::MarkupKind::PlainText
    },
    value: message.as_ref().to_string(),
  };
  vec![lsp_custom::TestMessage {
    message,
    expected_output: None,
    actual_output: None,
    location: None,
  }]
}

struct LspTestReporter {
  /// a channel for dispatching testing notification messages on its own thread
  channel: mpsc::UnboundedSender<TestingNotification>,
  current_origin: Option<String>,
  maybe_root_uri: Option<ModuleSpecifier>,
  id: u32,
  stack: HashMap<String, Vec<TestOrTestStepDescription>>,
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
}

impl LspTestReporter {
  fn new(
    run: &TestRun,
    client: Client,
    maybe_root_uri: Option<&ModuleSpecifier>,
    tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
  ) -> Self {
    let (channel, mut rx) = mpsc::unbounded_channel::<TestingNotification>();

    let _join_handle = tokio::task::spawn(async move {
      loop {
        match rx.recv().await {
          None => break,
          Some(params) => {
            client.send_test_notification(params).await;
          }
        }
      }
    });

    Self {
      channel,
      current_origin: None,
      maybe_root_uri: maybe_root_uri.cloned(),
      id: run.id,
      stack: HashMap::new(),
      tests,
    }
  }

  fn add_step(&self, desc: &test::TestStepDescription) {
    if let Ok(specifier) = ModuleSpecifier::parse(&desc.test.origin) {
      let mut tests = self.tests.lock();
      let entry =
        tests
          .entry(specifier.clone())
          .or_insert_with(|| TestDefinitions {
            discovered: Default::default(),
            injected: Default::default(),
            script_version: "1".to_string(),
          });
      let mut prev: lsp_custom::TestData = desc.into();
      if let Some(stack) = self.stack.get(&desc.test.origin) {
        for item in stack.iter().rev() {
          let mut data: lsp_custom::TestData = item.into();
          data.steps = Some(vec![prev]);
          prev = data;
        }
        entry.injected.push(prev.clone());
        let label = if let Some(root) = &self.maybe_root_uri {
          specifier.as_str().replace(root.as_str(), "")
        } else {
          specifier
            .path_segments()
            .and_then(|s| s.last().map(|s| s.to_string()))
            .unwrap_or_else(|| "<unknown>".to_string())
        };
        self
          .channel
          .send(TestingNotification::Module(
            lsp_custom::TestModuleNotificationParams {
              text_document: lsp::TextDocumentIdentifier { uri: specifier },
              kind: lsp_custom::TestModuleNotificationKind::Insert,
              label,
              tests: vec![prev],
            },
          ))
          .unwrap_or_else(|err| {
            lsp_log!("{}", err);
          });
      }
    }
  }

  /// Add a test which is being reported from the test runner but was not
  /// statically identified
  fn add_test(&self, desc: &test::TestDescription) {
    if let Ok(specifier) = ModuleSpecifier::parse(&desc.origin) {
      let mut tests = self.tests.lock();
      let entry =
        tests
          .entry(specifier.clone())
          .or_insert_with(|| TestDefinitions {
            discovered: Default::default(),
            injected: Default::default(),
            script_version: "1".to_string(),
          });
      entry.injected.push(desc.into());
      let label = if let Some(root) = &self.maybe_root_uri {
        specifier.as_str().replace(root.as_str(), "")
      } else {
        specifier
          .path_segments()
          .and_then(|s| s.last().map(|s| s.to_string()))
          .unwrap_or_else(|| "<unknown>".to_string())
      };
      self
        .channel
        .send(TestingNotification::Module(
          lsp_custom::TestModuleNotificationParams {
            text_document: lsp::TextDocumentIdentifier { uri: specifier },
            kind: lsp_custom::TestModuleNotificationKind::Insert,
            label,
            tests: vec![desc.into()],
          },
        ))
        .unwrap_or_else(|err| {
          lsp_log!("{}", err);
        });
    }
  }

  fn progress(&self, message: TestRunProgressMessage) {
    self
      .channel
      .send(TestingNotification::Progress(
        lsp_custom::TestRunProgressParams {
          id: self.id,
          message,
        },
      ))
      .unwrap_or_else(|err| {
        lsp_log!("{}", err);
      });
  }

  fn includes_step(&self, desc: &test::TestStepDescription) -> bool {
    if let Ok(specifier) = ModuleSpecifier::parse(&desc.test.origin) {
      let tests = self.tests.lock();
      if let Some(test_definitions) = tests.get(&specifier) {
        return test_definitions
          .get_step_by_name(&desc.test.name, desc.level, &desc.name)
          .is_some();
      }
    }
    false
  }

  fn includes_test(&self, desc: &test::TestDescription) -> bool {
    if let Ok(specifier) = ModuleSpecifier::parse(&desc.origin) {
      let tests = self.tests.lock();
      if let Some(test_definitions) = tests.get(&specifier) {
        return test_definitions.get_by_name(&desc.name).is_some();
      }
    }
    false
  }
}

impl test::TestReporter for LspTestReporter {
  fn report_plan(&mut self, _plan: &test::TestPlan) {
    // there is nothing to do on report_plan
  }

  fn report_wait(&mut self, desc: &test::TestDescription) {
    if !self.includes_test(desc) {
      self.add_test(desc);
    }
    self.current_origin = Some(desc.origin.clone());
    let test: lsp_custom::TestIdentifier = desc.into();
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    assert!(stack.is_empty());
    stack.push(desc.into());
    self.progress(TestRunProgressMessage::Started { test });
  }

  fn report_output(&mut self, output: &test::TestOutput) {
    let test = self.current_origin.as_ref().and_then(|origin| {
      self
        .stack
        .get(origin)
        .and_then(|v| v.last().map(|td| td.into()))
    });
    match output {
      test::TestOutput::Console(value) => {
        self.progress(TestRunProgressMessage::Output {
          value: value.replace('\n', "\r\n"),
          test,
          // TODO(@kitsonk) test output should include a location
          location: None,
        })
      }
    }
  }

  fn report_result(
    &mut self,
    desc: &test::TestDescription,
    result: &test::TestResult,
    elapsed: u64,
  ) {
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.pop(), Some(desc.into()));
    self.current_origin = None;
    match result {
      test::TestResult::Ok => self.progress(TestRunProgressMessage::Passed {
        test: desc.into(),
        duration: Some(elapsed as u32),
      }),
      test::TestResult::Ignored => {
        self.progress(TestRunProgressMessage::Skipped { test: desc.into() })
      }
      test::TestResult::Failed(message) => {
        self.progress(TestRunProgressMessage::Failed {
          test: desc.into(),
          messages: as_test_messages(message, false),
          duration: Some(elapsed as u32),
        })
      }
    }
  }

  fn report_step_wait(&mut self, desc: &test::TestStepDescription) {
    if !self.includes_step(desc) {
      self.add_step(desc);
    }
    let test: lsp_custom::TestIdentifier = desc.into();
    let stack = self.stack.entry(desc.test.origin.clone()).or_default();
    self.current_origin = Some(desc.test.origin.clone());
    assert!(!stack.is_empty());
    stack.push(desc.into());
    self.progress(TestRunProgressMessage::Started { test });
  }

  fn report_step_result(
    &mut self,
    desc: &test::TestStepDescription,
    result: &test::TestStepResult,
    elapsed: u64,
  ) {
    let stack = self.stack.entry(desc.test.origin.clone()).or_default();
    assert_eq!(stack.pop(), Some(desc.into()));
    match result {
      test::TestStepResult::Ok => {
        self.progress(TestRunProgressMessage::Passed {
          test: desc.into(),
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Ignored => {
        self.progress(TestRunProgressMessage::Skipped { test: desc.into() })
      }
      test::TestStepResult::Failed(message) => {
        let messages = if let Some(message) = message {
          as_test_messages(message, false)
        } else {
          vec![]
        };
        self.progress(TestRunProgressMessage::Failed {
          test: desc.into(),
          messages,
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Pending(_) => {
        self.progress(TestRunProgressMessage::Enqueued { test: desc.into() })
      }
    }
  }

  fn report_summary(
    &mut self,
    _summary: &test::TestSummary,
    _elapsed: &Duration,
  ) {
    // there is nothing to do on report_summary
  }
}

async fn test_specifier(
  ps: proc_state::ProcState,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  mode: test::TestMode,
  channel: mpsc::UnboundedSender<test::TestEvent>,
  options: Option<Value>,
) -> Result<(), AnyError> {
  let mut worker = create_main_worker(
    &ps,
    specifier.clone(),
    permissions,
    vec![ops::testing::init(channel.clone())],
  );

  worker
    .execute_script(&located_script_name!(), "Deno.core.enableOpCallTracing();")
    .unwrap();

  if mode != test::TestMode::Documentation {
    worker.execute_side_module(&specifier).await?;
  }

  worker.dispatch_load_event(&located_script_name!())?;

  let options = options.unwrap_or_else(|| json!({}));
  let test_result = worker.js_runtime.execute_script(
    &located_script_name!(),
    &format!(r#"Deno[Deno.internal].runTests({})"#, json!(options)),
  )?;

  worker.js_runtime.resolve_value(test_result).await?;

  worker.dispatch_unload_event(&located_script_name!())?;

  Ok(())
}

#[derive(Debug, Clone, Default)]
struct TestFilter {
  maybe_include: Option<HashMap<String, TestDefinition>>,
  maybe_exclude: Option<HashMap<String, TestDefinition>>,
}

impl TestFilter {
  fn as_ids(&self, test_definitions: &TestDefinitions) -> Vec<String> {
    let ids: Vec<String> = if let Some(include) = &self.maybe_include {
      include.keys().cloned().collect()
    } else {
      test_definitions
        .discovered
        .iter()
        .map(|td| td.id.clone())
        .collect()
    };
    if let Some(exclude) = &self.maybe_exclude {
      ids
        .into_iter()
        .filter(|id| !exclude.contains_key(id))
        .collect()
    } else {
      ids
    }
  }

  /// return the filter as a JSON value, suitable for sending as a filter to the
  /// test runner.
  fn as_test_options(&self) -> Value {
    let maybe_include: Option<Vec<String>> = self
      .maybe_include
      .as_ref()
      .map(|inc| inc.iter().map(|(_, td)| td.name.clone()).collect());
    let maybe_exclude: Option<Vec<String>> = self
      .maybe_exclude
      .as_ref()
      .map(|ex| ex.iter().map(|(_, td)| td.name.clone()).collect());
    json!({
      "filter": {
        "include": maybe_include,
        "exclude": maybe_exclude,
      }
    })
  }
}

/// Logic to convert a test request into a set of test modules to be tested and
/// any filters to be applied to those tests
fn as_queue_and_filters(
  params: &lsp_custom::TestRunRequestParams,
  tests: &HashMap<ModuleSpecifier, TestDefinitions>,
) -> (
  HashSet<ModuleSpecifier>,
  HashMap<ModuleSpecifier, TestFilter>,
) {
  let mut queue: HashSet<ModuleSpecifier> = HashSet::new();
  let mut filters: HashMap<ModuleSpecifier, TestFilter> = HashMap::new();

  if let Some(include) = &params.include {
    for item in include {
      if let Some(test_definitions) = tests.get(&item.text_document.uri) {
        queue.insert(item.text_document.uri.clone());
        if let Some(id) = &item.id {
          if let Some(test) = test_definitions.get_by_id(id) {
            let filter =
              filters.entry(item.text_document.uri.clone()).or_default();
            if let Some(include) = filter.maybe_include.as_mut() {
              include.insert(test.id.clone(), test.clone());
            } else {
              let mut include = HashMap::new();
              include.insert(test.id.clone(), test.clone());
              filter.maybe_include = Some(include);
            }
          }
        }
      }
    }
  }

  // if we didn't have any specific include filters, we assume that all modules
  // will be tested
  if queue.is_empty() {
    queue.extend(tests.keys().cloned());
  }

  if let Some(exclude) = &params.exclude {
    for item in exclude {
      if let Some(test_definitions) = tests.get(&item.text_document.uri) {
        if let Some(id) = &item.id {
          // there is currently no way to filter out a specific test, so we have
          // to ignore the exclusion
          if item.step_id.is_none() {
            if let Some(test) = test_definitions.get_by_id(id) {
              let filter =
                filters.entry(item.text_document.uri.clone()).or_default();
              if let Some(exclude) = filter.maybe_exclude.as_mut() {
                exclude.insert(test.id.clone(), test.clone());
              } else {
                let mut exclude = HashMap::new();
                exclude.insert(test.id.clone(), test.clone());
                filter.maybe_exclude = Some(exclude);
              }
            }
          }
        } else {
          // the entire test module is excluded
          queue.remove(&item.text_document.uri);
        }
      }
    }
  }

  (queue, filters)
}

#[derive(Debug, Clone)]
struct TestRun {
  id: u32,
  kind: lsp_custom::TestRunKind,
  filters: HashMap<ModuleSpecifier, TestFilter>,
  queue: HashSet<ModuleSpecifier>,
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
  workspace_settings: config::WorkspaceSettings,
}

impl TestRun {
  fn new(
    params: &lsp_custom::TestRunRequestParams,
    tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
    workspace_settings: config::WorkspaceSettings,
  ) -> Self {
    let (queue, filters) = {
      let tests = tests.lock();
      as_queue_and_filters(params, &tests)
    };

    Self {
      id: params.id,
      kind: params.kind.clone(),
      filters,
      queue,
      tests,
      workspace_settings,
    }
  }

  /// Provide the tests of a test run as an enqueued module which can be sent
  /// to the client to indicate tests are enqueued for testing.
  fn as_enqueued(&self) -> Vec<lsp_custom::EnqueuedTestModule> {
    let tests = self.tests.lock();
    self
      .queue
      .iter()
      .map(|s| {
        let ids = if let Some(test_definitions) = tests.get(s) {
          if let Some(filter) = self.filters.get(s) {
            filter.as_ids(test_definitions)
          } else {
            test_definitions
              .discovered
              .iter()
              .map(|test| test.id.clone())
              .collect()
          }
        } else {
          Vec::new()
        };
        lsp_custom::EnqueuedTestModule {
          text_document: lsp::TextDocumentIdentifier { uri: s.clone() },
          ids,
        }
      })
      .collect()
  }

  fn get_args(&self) -> Vec<&str> {
    let mut args = vec!["deno", "test"];
    args.extend(
      self
        .workspace_settings
        .testing
        .args
        .iter()
        .map(|s| s.as_str()),
    );
    if self.workspace_settings.unstable && !args.contains(&"--unstable") {
      args.push("--unstable");
    }
    if let Some(config) = &self.workspace_settings.config {
      if !args.contains(&"--config") && !args.contains(&"-c") {
        args.push("--config");
        args.push(config.as_str());
      }
    }
    if let Some(import_map) = &self.workspace_settings.import_map {
      if !args.contains(&"--import-map") {
        args.push("--import-map");
        args.push(import_map.as_str());
      }
    }
    if self.kind == lsp_custom::TestRunKind::Debug
      && !args.contains(&"--inspect")
      && !args.contains(&"--inspect-brk")
    {
      args.push("--inspect");
    }
    args
  }

  /// Execute the tests, dispatching progress notifications to the client.
  async fn exec(
    &self,
    client: &Client,
    token: CancellationToken,
    maybe_root_uri: Option<&ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    let args = self.get_args();
    lsp_log!("Executing test run with arguments: {}", args.join(" "));
    let flags = flags::flags_from_vec(args)?;
    let ps = proc_state::ProcState::build(Arc::new(flags)).await?;
    let permissions =
      Permissions::from_options(&ps.flags.permissions_options());
    test::check_specifiers(
      &ps,
      permissions.clone(),
      self
        .queue
        .iter()
        .map(|s| (s.clone(), test::TestMode::Executable))
        .collect(),
      emit::TypeLib::DenoWindow,
    )
    .await?;

    let (sender, mut receiver) = mpsc::unbounded_channel::<test::TestEvent>();

    let (concurrent_jobs, fail_fast) =
      if let flags::DenoSubcommand::Test(test_flags) = &ps.flags.subcommand {
        (
          test_flags.concurrent_jobs.into(),
          test_flags.fail_fast.map(|count| count.into()),
        )
      } else {
        unreachable!("Should always be Test subcommand.");
      };

    let mut queue = self.queue.iter().collect::<Vec<&ModuleSpecifier>>();
    queue.sort();

    let join_handles = queue.into_iter().map(move |specifier| {
      let specifier = specifier.clone();
      let ps = ps.clone();
      let permissions = permissions.clone();
      let sender = sender.clone();
      let options = self.filters.get(&specifier).map(|f| f.as_test_options());

      tokio::task::spawn_blocking(move || {
        let future = test_specifier(
          ps,
          permissions,
          specifier,
          test::TestMode::Executable,
          sender,
          options,
        );

        run_basic(future)
      })
    });

    let join_stream = stream::iter(join_handles)
      .buffer_unordered(concurrent_jobs)
      .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

    let mut reporter: Box<dyn test::TestReporter + Send> =
      Box::new(LspTestReporter::new(
        self,
        client.clone(),
        maybe_root_uri,
        self.tests.clone(),
      ));

    let handler = {
      tokio::task::spawn(async move {
        let earlier = Instant::now();
        let mut summary = test::TestSummary::new();
        let mut used_only = false;

        while let Some(event) = receiver.recv().await {
          match event {
            test::TestEvent::Plan(plan) => {
              summary.total += plan.total;
              summary.filtered_out += plan.filtered_out;

              if plan.used_only {
                used_only = true;
              }

              reporter.report_plan(&plan);
            }
            test::TestEvent::Wait(description) => {
              reporter.report_wait(&description);
            }
            test::TestEvent::Output(output) => {
              reporter.report_output(&output);
            }
            test::TestEvent::Result(description, result, elapsed) => {
              match &result {
                test::TestResult::Ok => summary.passed += 1,
                test::TestResult::Ignored => summary.ignored += 1,
                test::TestResult::Failed(error) => {
                  summary.failed += 1;
                  summary.failures.push((description.clone(), error.clone()));
                }
              }

              reporter.report_result(&description, &result, elapsed);
            }
            test::TestEvent::StepWait(description) => {
              reporter.report_step_wait(&description);
            }
            test::TestEvent::StepResult(description, result, duration) => {
              match &result {
                test::TestStepResult::Ok => {
                  summary.passed_steps += 1;
                }
                test::TestStepResult::Ignored => {
                  summary.ignored_steps += 1;
                }
                test::TestStepResult::Failed(_) => {
                  summary.failed_steps += 1;
                }
                test::TestStepResult::Pending(_) => {
                  summary.pending_steps += 1;
                }
              }
              reporter.report_step_result(&description, &result, duration);
            }
          }

          if token.is_cancelled() {
            break;
          }

          if let Some(count) = fail_fast {
            if summary.failed >= count {
              break;
            }
          }
        }

        let elapsed = Instant::now().duration_since(earlier);
        reporter.report_summary(&summary, &elapsed);

        if used_only {
          return Err(anyhow!(
            "Test failed because the \"only\" option was used"
          ));
        }

        if summary.failed > 0 {
          return Err(anyhow!("Test failed"));
        }

        Ok(())
      })
    };

    let (join_results, result) = future::join(join_stream, handler).await;

    // propagate any errors
    for join_result in join_results {
      join_result??;
    }

    result??;

    Ok(())
  }
}

#[derive(Debug)]
enum RunRequest {
  Start(u32),
  Cancel(u32),
}

/// The main structure which handles requests and sends notifications related
/// to the Testing API.
#[derive(Debug)]
pub struct TestServer {
  client: Client,
  performance: Arc<Performance>,
  /// A channel for handling run requests from the client
  run_channel: mpsc::UnboundedSender<RunRequest>,
  /// A map of run ids to test runs
  runs: Arc<Mutex<HashMap<u32, TestRun>>>,
  /// Tests that are discovered from a versioned document
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
  /// A channel for requesting that changes to documents be statically analyzed
  /// for tests
  update_channel: mpsc::UnboundedSender<Arc<StateSnapshot>>,
}

impl TestServer {
  pub fn new(
    client: Client,
    performance: Arc<Performance>,
    maybe_root_uri: Option<ModuleSpecifier>,
  ) -> Self {
    let tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>> =
      Arc::new(Mutex::new(HashMap::new()));

    let (update_channel, mut update_rx) =
      mpsc::unbounded_channel::<Arc<StateSnapshot>>();
    let (run_channel, mut run_rx) = mpsc::unbounded_channel::<RunRequest>();

    let server = Self {
      client,
      performance,
      run_channel,
      runs: Default::default(),
      tests,
      update_channel,
    };

    let tests = server.tests.clone();
    let client = server.client.clone();
    let performance = server.performance.clone();
    let mru = maybe_root_uri.clone();
    let _update_join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match update_rx.recv().await {
            None => break,
            Some(snapshot) => {
              let mark = performance.mark("testing_update", None::<()>);
              let mut tests = tests.lock();
              // we create a list of test modules we currently are tracking
              // eliminating any we go over when iterating over the document
              let mut keys: HashSet<ModuleSpecifier> =
                tests.keys().cloned().collect();
              for document in snapshot.documents.documents(false, true) {
                let specifier = document.specifier();
                keys.remove(specifier);
                let script_version = document.script_version();
                let valid = if let Some(test) = tests.get(specifier) {
                  test.script_version == script_version
                } else {
                  false
                };
                if !valid {
                  if let Some(Ok(parsed_source)) =
                    document.maybe_parsed_source()
                  {
                    let mut collector = TestCollector::new(specifier.clone());
                    parsed_source.module().visit_with(&mut collector);
                    let test_definitions = TestDefinitions {
                      discovered: collector.take(),
                      injected: Default::default(),
                      script_version,
                    };
                    if !test_definitions.discovered.is_empty() {
                      client
                        .send_test_notification(
                          test_definitions.as_notification(
                            specifier,
                            mru.as_ref(),
                            parsed_source.source(),
                          ),
                        )
                        .await;
                    }
                    tests.insert(specifier.clone(), test_definitions);
                  }
                }
              }
              for key in keys {
                client
                  .send_test_notification(as_delete_notification(key))
                  .await;
              }
              performance.measure(mark);
            }
          }
        }
      })
    });

    let client = server.client.clone();
    let runs = server.runs.clone();
    let mut tokens = HashMap::<u32, CancellationToken>::new();
    let _run_join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match run_rx.recv().await {
            None => break,
            Some(RunRequest::Start(id)) => {
              let maybe_run = {
                let runs = runs.lock();
                runs.get(&id).cloned()
              };
              if let Some(run) = maybe_run {
                let token = {
                  let token = CancellationToken::new();
                  tokens.insert(id, token.clone());
                  token
                };
                match run.exec(&client, token, maybe_root_uri.as_ref()).await {
                  Ok(_) => (),
                  Err(err) => {
                    client.show_message(lsp::MessageType::ERROR, err).await;
                  }
                }
                tokens.remove(&id);
                client
                  .send_test_notification(TestingNotification::Progress(
                    lsp_custom::TestRunProgressParams {
                      id,
                      message: TestRunProgressMessage::End,
                    },
                  ))
                  .await;
              }
            }
            Some(RunRequest::Cancel(id)) => {
              if let Some(token) = tokens.get(&id) {
                token.cancel();
              }
            }
          }
        }
      })
    });

    server
  }

  fn cancel_run(&self, id: u32) -> Result<(), AnyError> {
    self
      .run_channel
      .send(RunRequest::Cancel(id))
      .map_err(|err| err.into())
  }

  fn enqueue_run(&self, id: u32) -> Result<(), AnyError> {
    self
      .run_channel
      .send(RunRequest::Start(id))
      .map_err(|err| err.into())
  }

  /// A request from the client to cancel a test run.
  pub fn run_cancel_request(
    &self,
    params: lsp_custom::TestRunCancelParams,
  ) -> LspResult<Option<Value>> {
    if self.runs.lock().contains_key(&params.id) {
      self.cancel_run(params.id).map_err(|err| {
        log::error!("cannot cancel run: {}", err);
        LspError::internal_error()
      })?;
      Ok(Some(json!(true)))
    } else {
      Ok(Some(json!(false)))
    }
  }

  /// A request from the client to start a test run.
  pub fn run_request(
    &self,
    params: lsp_custom::TestRunRequestParams,
    workspace_settings: config::WorkspaceSettings,
  ) -> LspResult<Option<Value>> {
    let test_run =
      { TestRun::new(&params, self.tests.clone(), workspace_settings) };
    let enqueued = test_run.as_enqueued();
    {
      let mut runs = self.runs.lock();
      runs.insert(params.id, test_run);
    }
    self.enqueue_run(params.id).map_err(|err| {
      log::error!("cannot enqueue run: {}", err);
      LspError::internal_error()
    })?;
    Ok(Some(json!({ "enqueued": enqueued })))
  }

  pub(crate) fn update(
    &self,
    snapshot: Arc<StateSnapshot>,
  ) -> Result<(), AnyError> {
    self.update_channel.send(snapshot).map_err(|err| err.into())
  }
}
