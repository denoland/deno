// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::client::Client;
use super::client::TestingNotification;
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
use indexmap::IndexMap;
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

fn span_to_range(span: &Span, source_text_info: &SourceTextInfo) -> lsp::Range {
  let start = source_text_info.line_and_column_index(span.lo);
  let end = source_text_info.line_and_column_index(span.hi);
  lsp::Range {
    start: lsp::Position {
      line: start.line_index as u32,
      character: start.column_index as u32,
    },
    end: lsp::Position {
      line: end.line_index as u32,
      character: end.column_index as u32,
    },
  }
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
      range: Some(span_to_range(&self.span, source_text_info)),
    }
  }
}

#[derive(Debug)]
struct TestDefinitions {
  definitions: Vec<TestDefinition>,
  script_version: String,
}

impl TestDefinitions {
  pub fn as_notification(
    &self,
    specifier: &ModuleSpecifier,
    maybe_root: Option<&ModuleSpecifier>,
    source_text_info: &SourceTextInfo,
  ) -> TestingNotification {
    let label = if let Some(root) = maybe_root {
      specifier.to_string().replace(root.as_str(), "")
    } else {
      specifier
        .path_segments()
        .map(|s| s.last().map(|s| s.to_string()))
        .flatten()
        .unwrap_or_else(|| "<unknown>".to_string())
    };
    let tests_map: HashMap<String, lsp_custom::TestData> = self
      .definitions
      .iter()
      .map(|td| {
        let test_data = td.as_test_data(source_text_info);
        (test_data.id.clone(), test_data)
      })
      .collect();
    TestingNotification::Module(lsp_custom::TestModuleNotificationParams {
      text_document: lsp::TextDocumentIdentifier {
        uri: specifier.clone(),
      },
      label,
      tests: tests_map.into_values().collect(),
    })
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
                    "name" => {
                      if let ast::Expr::Lit(ast::Lit::Str(lit_str)) =
                        key_value_prop.value.as_ref()
                      {
                        maybe_name = Some(lit_str.value.to_string());
                      }
                    }
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
        if let Some(name) = maybe_name {
          Some((name, steps))
        } else {
          None
        }
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
      span.clone(),
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
              if ns_prop_ident.sym.to_string() == "step" {
                if let ast::Expr::Ident(ident) = member_expr.obj.as_ref() {
                  if ident.sym.to_string() == *test_context {
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
              if ident.sym.to_string() == *test_context {
                if let ast::Pat::Object(object_pat) = &decl.name {
                  for prop in &object_pat.props {
                    match prop {
                      ast::ObjectPatProp::Assign(prop) => {
                        let name = prop.key.sym.to_string();
                        if name == "step" {
                          self.vars.insert(name);
                        }
                      }
                      ast::ObjectPatProp::KeyValue(prop) => {
                        if let ast::PropName::Ident(key_ident) = &prop.key {
                          if key_ident.sym.to_string() == "step" {
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
                if obj_ident.sym.to_string() == *test_context {
                  if let ast::MemberProp::Ident(prop_ident) = &member_expr.prop
                  {
                    if prop_ident.sym.to_string() == "step" {
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
      span.clone(),
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

// fn find_step<'a>(
//   id: &str,
//   steps: &'a [TestDefinition],
// ) -> Option<&'a TestDefinition> {
//   for step in steps {
//     if id == &step.id {
//       return Some(step);
//     }
//     if let Some(steps) = &step.steps {
//       if let Some(step) = find_step(id, steps) {
//         return Some(step);
//       }
//     }
//   }
//   None
// }

struct LspTestReporter {
  /// a channel for dispatching testing notification messages on its own thread
  channel: mpsc::UnboundedSender<TestingNotification>,
  /// contains the identifier of the current test if any
  current_test: Option<lsp_custom::TestIdentifier>,
  /// contains the identifier of the current test or test step, if any
  current_test_or_step: Option<lsp_custom::TestIdentifier>,
  id: u32,
  // tests: HashMap<ModuleSpecifier, HashMap<String, TestDefinition>>,
}

impl LspTestReporter {
  fn new(run: &TestRun, client: Client) -> Self {
    let (channel, mut rx) = mpsc::unbounded_channel::<TestingNotification>();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match rx.recv().await {
            None => break,
            Some(params) => {
              client.send_test_notification(params).await;
            }
          }
        }
      })
    });

    Self {
      channel,
      current_test: None,
      current_test_or_step: None,
      id: run.id,
      // tests: run
      //   .queue
      //   .iter()
      //   .map(|(s, t)| (s.clone(), t.clone().into_iter().collect()))
      //   .collect(),
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
}

impl test::TestReporter for LspTestReporter {
  fn report_plan(&mut self, _plan: &test::TestPlan) {
    // there is nothing to do on report_plan
  }

  fn report_wait(&mut self, description: &test::TestDescription) {
    let test: lsp_custom::TestIdentifier = description.into();
    self.current_test = Some(test.clone());
    self.current_test_or_step = Some(test.clone());
    self.progress(TestRunProgressMessage::Started { test });
  }

  fn report_output(&mut self, output: &test::TestOutput) {
    match output {
      test::TestOutput::Console(value) => {
        self.progress(TestRunProgressMessage::Output {
          value: format!("{}\n", value),
          test: self.current_test_or_step.clone(),
          location: None,
        })
      }
    }
  }

  fn report_result(
    &mut self,
    description: &test::TestDescription,
    result: &test::TestResult,
    elapsed: u64,
  ) {
    self.current_test = None;
    self.current_test_or_step = None;
    match result {
      test::TestResult::Ok => self.progress(TestRunProgressMessage::Passed {
        test: description.into(),
        duration: Some(elapsed as u32),
      }),
      test::TestResult::Ignored => {
        self.progress(TestRunProgressMessage::Skipped {
          test: description.into(),
        })
      }
      test::TestResult::Failed(message) => {
        self.progress(TestRunProgressMessage::Failed {
          test: description.into(),
          messages: as_test_messages(message, false),
          duration: Some(elapsed as u32),
        })
      }
    }
  }

  fn report_step_wait(&mut self, desc: &test::TestStepDescription) {
    // if let Ok(specifier) = ModuleSpecifier::parse(&desc.test.origin) {
    //   if let Some(tests) = self.tests.get(&specifier) {
    //     let id = checksum::gen(&[
    //       desc.test.origin.as_bytes(),
    //       desc.test.name.as_bytes(),
    //     ]);
    //     if let Some(test) = tests.get(&id) {
    //       if let Some(steps) = &test.steps {
    //         let id = checksum::gen(&[
    //           desc.test.origin.as_bytes(),
    //           &desc.level.to_be_bytes(),
    //           desc.name.as_bytes(),
    //         ]);
    //         if let Some(step) = find_step(&id, steps) {
    //           log::info!("found step: {:?}", step);
    //         }
    //       }
    //     }
    //   }
    // }
    let test: lsp_custom::TestIdentifier = desc.into();
    self.current_test_or_step = Some(test.clone());
    self.progress(TestRunProgressMessage::Started { test });
  }

  fn report_step_result(
    &mut self,
    description: &test::TestStepDescription,
    result: &test::TestStepResult,
    elapsed: u64,
  ) {
    self.current_test_or_step = self.current_test.clone();
    match result {
      test::TestStepResult::Ok => {
        self.progress(TestRunProgressMessage::Passed {
          test: description.into(),
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Ignored => {
        self.progress(TestRunProgressMessage::Skipped {
          test: description.into(),
        })
      }
      test::TestStepResult::Failed(message) => {
        let messages = if let Some(message) = message {
          as_test_messages(message, false)
        } else {
          vec![]
        };
        self.progress(TestRunProgressMessage::Failed {
          test: description.into(),
          messages,
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Pending(_) => {
        self.progress(TestRunProgressMessage::Enqueued {
          test: description.into(),
        })
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
  channel: std::sync::mpsc::Sender<test::TestEvent>,
  filter: &[String],
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

  let test_result = worker.js_runtime.execute_script(
    &located_script_name!(),
    &format!(
      r#"Deno[Deno.internal].runTests({})"#,
      json!({
        "filter": filter,
      })
    ),
  )?;

  worker.js_runtime.resolve_value(test_result).await?;

  worker.dispatch_unload_event(&located_script_name!())?;

  Ok(())
}

#[derive(Debug, Clone)]
struct TestRun {
  id: u32,
  kind: lsp_custom::TestRunKind,
  queue: Vec<(ModuleSpecifier, IndexMap<String, TestDefinition>)>,
}

impl TestRun {
  fn new(
    params: &lsp_custom::TestRunRequestParams,
    tests: &HashMap<ModuleSpecifier, TestDefinitions>,
  ) -> Self {
    let mut queue =
      Vec::<(ModuleSpecifier, IndexMap<String, TestDefinition>)>::new();
    if let Some(include) = &params.include {
      let mut included =
        IndexMap::<ModuleSpecifier, IndexMap<String, TestDefinition>>::new();
      for item in include {
        if let Some(definitions) = tests.get(&item.text_document.uri) {
          let included_module =
            included.entry(item.text_document.uri.clone()).or_default();
          if let Some(id) = &item.id {
            if let Some(def) =
              definitions.definitions.iter().find(|def| def.id == *id)
            {
              included_module.insert(def.id.clone(), def.clone());
            }
          } else {
            included_module.extend(
              definitions
                .definitions
                .iter()
                .map(|def| (def.id.clone(), def.clone())),
            );
          }
        }
      }
      queue.extend(included.iter().map(|(s, td)| (s.clone(), td.clone())));
    } else {
      queue.extend(tests.iter().map(|(s, td)| {
        (
          s.clone(),
          td.definitions
            .iter()
            .map(|def| (def.id.clone(), def.clone()))
            .collect(),
        )
      }));
    }

    let mut excluded = HashMap::<ModuleSpecifier, HashSet<String>>::new();
    if let Some(exclude) = &params.exclude {
      for item in exclude {
        if let Some(definitions) = tests.get(&item.text_document.uri) {
          let excluded_module =
            excluded.entry(item.text_document.uri.clone()).or_default();
          if let Some(id) = &item.id {
            // we can't exclude individual steps
            if item.step_id.is_none() {
              if let Some(def) =
                definitions.definitions.iter().find(|def| def.id == *id)
              {
                excluded_module.insert(def.id.clone());
              }
            }
          } else {
            excluded_module
              .extend(definitions.definitions.iter().map(|def| def.id.clone()));
          }
        }
      }
    }
    if !excluded.is_empty() {
      let mut filtered_queue =
        Vec::<(ModuleSpecifier, IndexMap<String, TestDefinition>)>::new();
      for (item, tests) in &queue {
        if let Some(exclude_ids) = excluded.get(item) {
          let filtered: IndexMap<String, TestDefinition> = tests
            .iter()
            .filter_map(|(id, def)| {
              if exclude_ids.contains(id) {
                None
              } else {
                Some((id.clone(), def.clone()))
              }
            })
            .collect();
          if !filtered.is_empty() {
            filtered_queue.push((item.clone(), filtered));
          }
        } else {
          filtered_queue.push((item.clone(), tests.clone()));
        }
      }

      queue = filtered_queue;
    }

    Self {
      id: params.id,
      kind: params.kind.clone(),
      queue,
    }
  }

  fn as_enqueued(&self) -> Vec<lsp_custom::EnqueuedTestModule> {
    self
      .queue
      .iter()
      .map(|(specifier, tests)| lsp_custom::EnqueuedTestModule {
        text_document: lsp::TextDocumentIdentifier {
          uri: specifier.clone(),
        },
        ids: tests.keys().cloned().collect(),
      })
      .collect()
  }

  /// Execute the tests, dispatching progress notifications to the client.
  async fn exec(
    &self,
    client: &Client,
    token: CancellationToken,
  ) -> Result<(), AnyError> {
    let mut args = vec!["deno", "test", "--allow-all"];
    if self.kind == lsp_custom::TestRunKind::Debug {
      args.push("--inspect");
    }
    let flags = flags::flags_from_vec(args)?;
    let ps = proc_state::ProcState::build(Arc::new(flags)).await?;
    let permissions =
      Permissions::from_options(&ps.flags.permissions_options());
    let test_modules_with_mode: Vec<(
      ModuleSpecifier,
      Vec<String>,
      test::TestMode,
    )> = self
      .queue
      .iter()
      .map(|(s, t)| {
        (
          s.clone(),
          t.values().map(|td| td.name.clone()).collect(),
          test::TestMode::Executable,
        )
      })
      .collect();
    test::check_specifiers(
      &ps,
      permissions.clone(),
      test_modules_with_mode
        .iter()
        .map(|(s, _, m)| (s.clone(), m.clone()))
        .collect(),
      emit::TypeLib::DenoWindow,
    )
    .await?;

    let (sender, receiver) = std::sync::mpsc::channel::<test::TestEvent>();
    // TODO(@kitsonk) - experiment with allowing users to adjust this
    let concurrent_jobs = 1;
    // TODO(@kitsonk) - experiment with allowing users to set this
    let fail_fast = None;

    let join_handles = test_modules_with_mode.into_iter().map(
      move |(specifier, filter, mode)| {
        let ps = ps.clone();
        let permissions = permissions.clone();
        let sender = sender.clone();

        tokio::task::spawn_blocking(move || {
          let join_handle = std::thread::spawn(move || {
            let future =
              test_specifier(ps, permissions, specifier, mode, sender, &filter);
            run_basic(future)
          });

          join_handle.join().unwrap()
        })
      },
    );

    let join_stream = stream::iter(join_handles)
      .buffer_unordered(concurrent_jobs)
      .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

    let mut reporter: Box<dyn test::TestReporter + Send> =
      Box::new(LspTestReporter::new(self, client.clone()));

    let handler = {
      tokio::task::spawn_blocking(move || {
        let earlier = Instant::now();
        let mut summary = test::TestSummary::new();
        let mut used_only = false;

        for event in receiver.iter() {
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

#[derive(Debug)]
pub struct TestServer {
  client: Client,
  performance: Arc<Performance>,
  run_channel: mpsc::UnboundedSender<RunRequest>,
  runs: Arc<Mutex<HashMap<u32, TestRun>>>,
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
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
                      definitions: collector.take(),
                      script_version,
                    };
                    if !test_definitions.definitions.is_empty() {
                      client
                        .send_test_notification(
                          test_definitions.as_notification(
                            specifier,
                            maybe_root_uri.as_ref(),
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

    // let tests = server.tests.clone();
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
                match run.exec(&client, token).await {
                  Ok(_) => (),
                  Err(err) => {
                    log::info!("exec err: {}", err);
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
  ) -> LspResult<Option<Value>> {
    let test_run = {
      let tests = self.tests.lock();
      TestRun::new(&params, &tests)
    };
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
