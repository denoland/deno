// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;

use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_core::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use lsp_types::Uri;
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

use super::analysis::source_range_to_lsp_range;
use super::language_server;
use super::testing::collectors::parse_test_context_param;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum CodeLensSource {
  #[serde(rename = "implementations")]
  Implementations,
  #[serde(rename = "references")]
  References,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
  pub source: CodeLensSource,
  pub uri: Uri,
}

/// The body of a test or test step registration call, i.e. the function passed
/// to `Deno.test(...)`, `t.step(...)`, `describe(...)`, `it(...)`, etc.
enum TestFnBody<'a> {
  Arrow(&'a ast::ArrowExpr),
  Fn(&'a ast::Function),
}

/// Extract the name of a test or test step from its registration call, mirroring
/// the static collector in [`super::testing::collectors`]. Only string and
/// single-quasi template literal names (and named function expressions) are
/// resolvable statically.
fn find_test_name(node: &ast::CallExpr) -> Option<String> {
  match node.args.first().map(|es| es.expr.as_ref())? {
    ast::Expr::Object(obj_lit) => {
      for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop
          && let ast::Prop::KeyValue(key_value_prop) = prop.as_ref()
          && let ast::PropName::Ident(ast::IdentName { sym, .. }) =
            &key_value_prop.key
          && sym == "name"
        {
          match key_value_prop.value.as_ref() {
            ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
              return Some(lit_str.value.to_string_lossy().to_string());
            }
            ast::Expr::Tpl(tpl) if tpl.quasis.len() == 1 => {
              return Some(tpl.quasis.first().unwrap().raw.to_string());
            }
            _ => {}
          }
        }
      }
      None
    }
    ast::Expr::Fn(fn_expr) => {
      fn_expr.ident.as_ref().map(|ident| ident.sym.to_string())
    }
    ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
      Some(lit_str.value.to_string_lossy().to_string())
    }
    ast::Expr::Tpl(tpl) if tpl.quasis.len() == 1 => {
      Some(tpl.quasis.first().unwrap().raw.to_string())
    }
    _ => None,
  }
}

/// Locate the test function body of a test or test step registration call so it
/// can be searched for nested steps.
fn find_test_fn_body(node: &ast::CallExpr) -> Option<TestFnBody<'_>> {
  match node.args.first().map(|es| es.expr.as_ref()) {
    Some(ast::Expr::Object(obj_lit)) => {
      for prop in &obj_lit.props {
        let ast::PropOrSpread::Prop(prop) = prop else {
          continue;
        };
        match prop.as_ref() {
          ast::Prop::KeyValue(key_value_prop) => {
            if let ast::PropName::Ident(ast::IdentName { sym, .. }) =
              &key_value_prop.key
              && sym == "fn"
            {
              match key_value_prop.value.as_ref() {
                ast::Expr::Arrow(arrow_expr) => {
                  return Some(TestFnBody::Arrow(arrow_expr));
                }
                ast::Expr::Fn(fn_expr) => {
                  return Some(TestFnBody::Fn(&fn_expr.function));
                }
                _ => {}
              }
            }
          }
          ast::Prop::Method(method_prop) => {
            if let ast::PropName::Ident(ast::IdentName { sym, .. }) =
              &method_prop.key
              && sym == "fn"
            {
              return Some(TestFnBody::Fn(&method_prop.function));
            }
          }
          _ => {}
        }
      }
      None
    }
    Some(ast::Expr::Fn(fn_expr)) => Some(TestFnBody::Fn(&fn_expr.function)),
    Some(ast::Expr::Arrow(arrow_expr)) => Some(TestFnBody::Arrow(arrow_expr)),
    _ => match node.args.get(1).map(|es| es.expr.as_ref()) {
      Some(ast::Expr::Fn(fn_expr)) => Some(TestFnBody::Fn(&fn_expr.function)),
      Some(ast::Expr::Arrow(arrow_expr)) => Some(TestFnBody::Arrow(arrow_expr)),
      _ => None,
    },
  }
}

/// Push the "Run Test" and "Debug" code lenses for a test or test step. For a
/// test step, `name` is the name of the enclosing top-level test (the only
/// granularity `deno test --filter` supports), so debugging a step runs its
/// parent test under the inspector.
fn add_test_code_lenses(
  code_lenses: &mut Vec<lsp::CodeLens>,
  specifier: &ModuleSpecifier,
  parsed_source: &ParsedSource,
  name: &str,
  range: &SourceRange,
) {
  let range = source_range_to_lsp_range(range, parsed_source.text_info_lazy());
  for (title, inspect) in [("▶\u{fe0e} Run Test", false), ("Debug", true)] {
    code_lenses.push(lsp::CodeLens {
      range,
      command: Some(lsp::Command {
        title: title.to_string(),
        command: "deno.client.test".to_string(),
        arguments: Some(vec![
          json!(specifier),
          json!(name),
          json!({ "inspect": inspect }),
        ]),
      }),
      data: None,
    });
  }
}

/// Walk a test function body and emit a "Run Test"/"Debug" code lens for each
/// test step (`t.step(...)`, destructured `step(...)`, or BDD `it(...)`) found
/// in it. Every step's lens filters by `root_name` — the enclosing top-level
/// test — so it runs the same test the gutter "play" button does, but with the
/// option to attach the debugger.
fn collect_step_code_lenses(
  code_lenses: &mut Vec<lsp::CodeLens>,
  specifier: &ModuleSpecifier,
  parsed_source: &ParsedSource,
  root_name: &str,
  body: TestFnBody,
  is_describe: bool,
) {
  let param = match &body {
    TestFnBody::Arrow(arrow_expr) => arrow_expr.params.first(),
    TestFnBody::Fn(function) => function.params.first().map(|param| &param.pat),
  };
  let (maybe_test_context, maybe_step_var) = if is_describe {
    (None, None)
  } else {
    match parse_test_context_param(param) {
      Some(r) => r,
      None => return,
    }
  };
  let mut vars = HashSet::new();
  if let Some(var) = maybe_step_var {
    vars.insert(var);
  }
  let mut collector = DenoTestStepCollector {
    code_lenses,
    specifier,
    parsed_source,
    root_name,
    maybe_test_context,
    vars,
    is_describe,
  };
  match body {
    TestFnBody::Arrow(arrow_expr) => arrow_expr.body.visit_with(&mut collector),
    TestFnBody::Fn(function) => function.body.visit_with(&mut collector),
  }
}

struct DenoTestCollector {
  code_lenses: Vec<lsp::CodeLens>,
  parsed_source: ParsedSource,
  specifier: ModuleSpecifier,
  test_vars: HashSet<String>,
}

impl DenoTestCollector {
  pub fn new(specifier: ModuleSpecifier, parsed_source: ParsedSource) -> Self {
    Self {
      code_lenses: Vec::new(),
      parsed_source,
      specifier,
      test_vars: HashSet::new(),
    }
  }

  fn check_call_expr(
    &mut self,
    node: &ast::CallExpr,
    range: &SourceRange,
    is_describe: bool,
  ) {
    let Some(name) = find_test_name(node) else {
      return;
    };
    add_test_code_lenses(
      &mut self.code_lenses,
      &self.specifier,
      &self.parsed_source,
      &name,
      range,
    );
    if let Some(body) = find_test_fn_body(node) {
      collect_step_code_lenses(
        &mut self.code_lenses,
        &self.specifier,
        &self.parsed_source,
        &name,
        body,
        is_describe,
      );
    }
  }

  /// Move out the code lenses from the collector.
  fn take(self) -> Vec<lsp::CodeLens> {
    self.code_lenses
  }
}

impl Visit for DenoTestCollector {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    let ast::Callee::Expr(callee_expr) = &node.callee else {
      return;
    };
    let mut prop_chain = ["", "", ""];
    let mut current_segment = callee_expr.as_ref();
    let mut rightmost_symbol_range = None;
    for (i, name) in prop_chain.iter_mut().enumerate().rev() {
      match current_segment {
        ast::Expr::Ident(ident) => {
          *name = ident.sym.as_str();
          rightmost_symbol_range.get_or_insert_with(|| ident.range());
          break;
        }
        ast::Expr::Member(member_expr) => {
          if i == 0 {
            return;
          }
          let ast::MemberProp::Ident(right) = &member_expr.prop else {
            return;
          };
          *name = right.sym.as_str();
          rightmost_symbol_range.get_or_insert_with(|| right.range());
          current_segment = &member_expr.obj;
        }
        _ => return,
      }
    }
    let Some(rightmost_symbol_range) = rightmost_symbol_range else {
      debug_assert!(false, "rightmost symbol range should always be defined");
      return;
    };
    let is_describe = match prop_chain {
      ["", "Deno", "test"] | ["Deno", "test", "ignore" | "only"] => false,
      ["", "", "describe"] | ["", "describe", "ignore" | "only" | "skip"] => {
        true
      }
      ["", "", s] if self.test_vars.contains(s) => false,
      _ => return,
    };
    self.check_call_expr(node, &rightmost_symbol_range, is_describe);
  }

  fn visit_var_decl(&mut self, node: &ast::VarDecl) {
    for decl in &node.decls {
      if let Some(init) = &decl.init {
        match init.as_ref() {
          // Identify destructured assignments of `test` from `Deno`
          ast::Expr::Ident(ident) => {
            if ident.sym == "Deno"
              && let ast::Pat::Object(object_pat) = &decl.name
            {
              for prop in &object_pat.props {
                match prop {
                  ast::ObjectPatProp::Assign(prop) => {
                    let name = prop.key.sym.to_string();
                    if name == "test" {
                      self.test_vars.insert(name);
                    }
                  }
                  ast::ObjectPatProp::KeyValue(prop) => {
                    if let ast::PropName::Ident(key_ident) = &prop.key
                      && key_ident.sym == "test"
                      && let ast::Pat::Ident(value_ident) = &prop.value.as_ref()
                    {
                      self.test_vars.insert(value_ident.id.sym.to_string());
                    }
                  }
                  _ => (),
                }
              }
            }
          }
          // Identify variable assignments where the init is `Deno.test`
          ast::Expr::Member(member_expr) => {
            if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref()
              && obj_ident.sym == "Deno"
              && let ast::MemberProp::Ident(prop_ident) = &member_expr.prop
              && prop_ident.sym == "test"
              && let ast::Pat::Ident(binding_ident) = &decl.name
            {
              self.test_vars.insert(binding_ident.id.sym.to_string());
            }
          }
          _ => (),
        }
      }
    }
  }
}

/// Walks a test function body emitting code lenses for each test step. Mirrors
/// the step-detection logic of `super::testing::collectors`'s
/// `TestStepCollector`, but instead of building a test hierarchy it pushes a
/// "Run Test"/"Debug" code lens for each step (filtering by the enclosing
/// top-level test `root_name`).
struct DenoTestStepCollector<'a> {
  code_lenses: &'a mut Vec<lsp::CodeLens>,
  specifier: &'a ModuleSpecifier,
  parsed_source: &'a ParsedSource,
  root_name: &'a str,
  maybe_test_context: Option<String>,
  vars: HashSet<String>,
  is_describe: bool,
}

impl Visit for DenoTestStepCollector<'_> {
  fn visit_call_expr(&mut self, node: &ast::CallExpr) {
    let ast::Callee::Expr(callee_expr) = &node.callee else {
      return;
    };
    let mut prop_chain = ["", ""];
    let mut current_segment = callee_expr.as_ref();
    let mut rightmost_symbol_range = None;
    for (i, name) in prop_chain.iter_mut().enumerate().rev() {
      match current_segment {
        ast::Expr::Ident(ident) => {
          *name = ident.sym.as_str();
          rightmost_symbol_range.get_or_insert_with(|| ident.range());
          break;
        }
        ast::Expr::Member(member_expr) => {
          if i == 0 {
            return;
          }
          let ast::MemberProp::Ident(right) = &member_expr.prop else {
            return;
          };
          *name = right.sym.as_str();
          rightmost_symbol_range.get_or_insert_with(|| right.range());
          current_segment = &member_expr.obj;
        }
        _ => return,
      }
    }
    let Some(rightmost_symbol_range) = rightmost_symbol_range else {
      debug_assert!(false, "rightmost symbol range should always be defined");
      return;
    };
    // Only test steps are matched here: BDD `it(...)` inside a `describe`, a
    // `t.step(...)` call on the test context, or a destructured `step(...)`.
    match (
      self.is_describe,
      self.maybe_test_context.as_deref(),
      prop_chain,
    ) {
      (true, _, ["", "it"] | ["it", "ignore" | "only" | "skip"]) => {}
      (false, Some(c), [s, "step"]) if s == c => {}
      (false, _, ["", s]) if self.vars.contains(s) => {}
      _ => return,
    };
    add_test_code_lenses(
      self.code_lenses,
      self.specifier,
      self.parsed_source,
      self.root_name,
      &rightmost_symbol_range,
    );
    // A step body can itself contain nested steps (`t.step(...)` within a
    // `t.step(...)`), so descend into it as a `Deno.test`-style step body.
    if let Some(body) = find_test_fn_body(node) {
      collect_step_code_lenses(
        self.code_lenses,
        self.specifier,
        self.parsed_source,
        self.root_name,
        body,
        false,
      );
    }
  }

  fn visit_var_decl(&mut self, node: &ast::VarDecl) {
    let Some(test_context) = &self.maybe_test_context else {
      return;
    };
    for decl in &node.decls {
      let Some(init) = &decl.init else {
        continue;
      };
      match init.as_ref() {
        // Identify destructured assignments of `step` from the test context.
        ast::Expr::Ident(ident) => {
          if ident.sym != *test_context {
            continue;
          }
          let ast::Pat::Object(object_pat) = &decl.name else {
            continue;
          };
          for prop in &object_pat.props {
            match prop {
              ast::ObjectPatProp::Assign(prop) if prop.key.sym.eq("step") => {
                self.vars.insert(prop.key.sym.to_string());
              }
              ast::ObjectPatProp::KeyValue(prop) => {
                if let ast::PropName::Ident(key_ident) = &prop.key
                  && key_ident.sym.eq("step")
                  && let ast::Pat::Ident(value_ident) = &prop.value.as_ref()
                {
                  self.vars.insert(value_ident.id.sym.to_string());
                }
              }
              _ => (),
            }
          }
        }
        // Identify variable assignments where the init is test context `.step`.
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
          if prop_ident.sym.eq("step")
            && let ast::Pat::Ident(binding_ident) = &decl.name
          {
            self.vars.insert(binding_ident.id.sym.to_string());
          }
        }
        _ => (),
      }
    }
  }
}

async fn resolve_implementation_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &language_server::Inner,
  token: &CancellationToken,
) -> LspResult<lsp::CodeLens> {
  let locations = language_server
    .goto_implementation(
      lsp::request::GotoImplementationParams {
        text_document_position_params: lsp::TextDocumentPositionParams {
          text_document: lsp::TextDocumentIdentifier {
            uri: data.uri.clone(),
          },
          position: code_lens.range.start,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
      },
      token,
    )
    .await?
    .map(|r| match r {
      lsp::GotoDefinitionResponse::Scalar(location) => vec![location],
      lsp::GotoDefinitionResponse::Array(locations) => locations,
      lsp::GotoDefinitionResponse::Link(links) => links
        .into_iter()
        .map(|l| lsp::Location {
          uri: l.target_uri,
          range: l.target_selection_range,
        })
        .collect(),
    })
    .unwrap_or(Vec::new());
  let title = if locations.len() == 1 {
    "1 implementation".to_string()
  } else {
    format!("{} implementations", locations.len())
  };
  let command = if locations.is_empty() {
    lsp::Command {
      title,
      command: String::new(),
      arguments: None,
    }
  } else {
    lsp::Command {
      title,
      command: "deno.client.showReferences".to_string(),
      arguments: Some(vec![
        json!(data.uri),
        json!(code_lens.range.start),
        json!(locations),
      ]),
    }
  };
  Ok(lsp::CodeLens {
    range: code_lens.range,
    command: Some(command),
    data: None,
  })
}

async fn resolve_references_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &language_server::Inner,
  token: &CancellationToken,
) -> LspResult<lsp::CodeLens> {
  let locations = language_server
    .references(
      lsp::ReferenceParams {
        text_document_position: lsp::TextDocumentPositionParams {
          text_document: lsp::TextDocumentIdentifier {
            uri: data.uri.clone(),
          },
          position: code_lens.range.start,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: lsp::ReferenceContext {
          include_declaration: false,
        },
      },
      token,
    )
    .await?
    .unwrap_or_default();
  let title = if locations.len() == 1 {
    "1 reference".to_string()
  } else {
    format!("{} references", locations.len())
  };
  let command = if locations.is_empty() {
    lsp::Command {
      title,
      command: String::new(),
      arguments: None,
    }
  } else {
    lsp::Command {
      title,
      command: "deno.client.showReferences".to_string(),
      arguments: Some(vec![
        json!(data.uri),
        json!(code_lens.range.start),
        json!(locations),
      ]),
    }
  };
  Ok(lsp::CodeLens {
    range: code_lens.range,
    command: Some(command),
    data: None,
  })
}

pub async fn resolve_code_lens(
  code_lens: lsp::CodeLens,
  language_server: &language_server::Inner,
  token: &CancellationToken,
) -> LspResult<lsp::CodeLens> {
  let data: CodeLensData =
    serde_json::from_value(code_lens.data.clone().unwrap()).map_err(|err| {
      LspError::invalid_params(format!(
        "Unable to parse code lens data: {:#}",
        err
      ))
    })?;
  match data.source {
    CodeLensSource::Implementations => {
      resolve_implementation_code_lens(code_lens, data, language_server, token)
        .await
    }
    CodeLensSource::References => {
      resolve_references_code_lens(code_lens, data, language_server, token)
        .await
    }
  }
}

pub fn collect_test(
  specifier: &ModuleSpecifier,
  parsed_source: &ParsedSource,
  _token: &CancellationToken,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  // TODO(nayeemrmn): Do cancellation checks while collecting tests.
  let mut collector =
    DenoTestCollector::new(specifier.clone(), parsed_source.clone());
  parsed_source.program().visit_with(&mut collector);
  Ok(collector.take())
}

#[cfg(test)]
mod tests {
  use deno_ast::MediaType;
  use deno_core::resolve_url;

  use super::*;

  #[test]
  fn test_deno_test_collector() {
    let specifier = resolve_url("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
      Deno.test({
        name: "test a",
        fn() {}
      });

      Deno.test(function useFnName() {});

      Deno.test("test b", function anotherTest() {});

      Deno.test.ignore("test ignore", () => {});

      Deno.test.only("test only", () => {});

      Deno.test(`test template literal name`, () => {});

      describe("test describe", () => {});

      describe.ignore("test describe ignore", () => {});

      describe.only("test describe only", () => {});

      describe.skip("test describe skip", () => {});
    "#;
    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source.into(),
      media_type: MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let mut collector =
      DenoTestCollector::new(specifier, parsed_module.clone());
    parsed_module.program().visit_with(&mut collector);
    assert_eq!(
      collector.take(),
      vec![
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 1,
              character: 11
            },
            end: lsp::Position {
              line: 1,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test a"),
              json!({
                "inspect": false,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 1,
              character: 11
            },
            end: lsp::Position {
              line: 1,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test a"),
              json!({
                "inspect": true,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 6,
              character: 11
            },
            end: lsp::Position {
              line: 6,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("useFnName"),
              json!({
                "inspect": false,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 6,
              character: 11
            },
            end: lsp::Position {
              line: 6,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("useFnName"),
              json!({
                "inspect": true,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 8,
              character: 11
            },
            end: lsp::Position {
              line: 8,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test b"),
              json!({
                "inspect": false,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 8,
              character: 11
            },
            end: lsp::Position {
              line: 8,
              character: 15
            }
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test b"),
              json!({
                "inspect": true,
              }),
            ])
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 10,
              character: 16,
            },
            end: lsp::Position {
              line: 10,
              character: 22,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test ignore"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 10,
              character: 16,
            },
            end: lsp::Position {
              line: 10,
              character: 22,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test ignore"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 12,
              character: 16,
            },
            end: lsp::Position {
              line: 12,
              character: 20,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test only"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 12,
              character: 16,
            },
            end: lsp::Position {
              line: 12,
              character: 20,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test only"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 14,
              character: 11,
            },
            end: lsp::Position {
              line: 14,
              character: 15,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test template literal name"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 14,
              character: 11,
            },
            end: lsp::Position {
              line: 14,
              character: 15,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test template literal name"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 16,
              character: 6,
            },
            end: lsp::Position {
              line: 16,
              character: 14,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 16,
              character: 6,
            },
            end: lsp::Position {
              line: 16,
              character: 14,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 18,
              character: 15,
            },
            end: lsp::Position {
              line: 18,
              character: 21,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe ignore"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 18,
              character: 15,
            },
            end: lsp::Position {
              line: 18,
              character: 21,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe ignore"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 20,
              character: 15,
            },
            end: lsp::Position {
              line: 20,
              character: 19,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe only"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 20,
              character: 15,
            },
            end: lsp::Position {
              line: 20,
              character: 19,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe only"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 22,
              character: 15,
            },
            end: lsp::Position {
              line: 22,
              character: 19,
            },
          },
          command: Some(lsp::Command {
            title: "▶\u{fe0e} Run Test".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe skip"),
              json!({
                "inspect": false,
              }),
            ]),
          }),
          data: None,
        },
        lsp::CodeLens {
          range: lsp::Range {
            start: lsp::Position {
              line: 22,
              character: 15,
            },
            end: lsp::Position {
              line: 22,
              character: 19,
            },
          },
          command: Some(lsp::Command {
            title: "Debug".to_string(),
            command: "deno.client.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test describe skip"),
              json!({
                "inspect": true,
              }),
            ]),
          }),
          data: None,
        },
      ]
    );
  }

  /// Reduce a list of code lenses to `(line, title, filter_name, inspect)`
  /// tuples for compact assertions.
  fn summarize(
    code_lenses: &[lsp::CodeLens],
  ) -> Vec<(u32, String, String, bool)> {
    code_lenses
      .iter()
      .map(|cl| {
        let command = cl.command.as_ref().unwrap();
        let args = command.arguments.as_ref().unwrap();
        let name = args[1].as_str().unwrap().to_string();
        let inspect = args[2].get("inspect").unwrap().as_bool().unwrap();
        (cl.range.start.line, command.title.clone(), name, inspect)
      })
      .collect()
  }

  fn collect_code_lenses(source: &str) -> Vec<lsp::CodeLens> {
    let specifier = resolve_url("https://deno.land/x/mod.ts").unwrap();
    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: source.into(),
      media_type: MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let mut collector =
      DenoTestCollector::new(specifier, parsed_module.clone());
    parsed_module.program().visit_with(&mut collector);
    collector.take()
  }

  #[test]
  fn test_deno_test_collector_steps() {
    // Each `t.step(...)` gets "Run Test"/"Debug" code lenses that filter by the
    // enclosing top-level test name, so debugging a step runs its parent test
    // under the inspector. Regression test for
    // https://github.com/denoland/deno/issues/21664.
    let source = r#"
      Deno.test("test a", async (t) => {
        await t.step("step 1", () => {});
        await t.step("step 2", async (t) => {
          await t.step("nested step", () => {});
        });
      });
    "#;
    assert_eq!(
      summarize(&collect_code_lenses(source)),
      vec![
        (
          1,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (1, "Debug".to_string(), "test a".to_string(), true),
        (
          2,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (2, "Debug".to_string(), "test a".to_string(), true),
        (
          3,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (3, "Debug".to_string(), "test a".to_string(), true),
        (
          4,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (4, "Debug".to_string(), "test a".to_string(), true),
      ]
    );
  }

  #[test]
  fn test_deno_test_collector_steps_destructured() {
    // Destructured `step` from the test context is also detected.
    let source = r#"
      Deno.test("test a", async ({ step }) => {
        await step("step 1", () => {});
      });
    "#;
    assert_eq!(
      summarize(&collect_code_lenses(source)),
      vec![
        (
          1,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (1, "Debug".to_string(), "test a".to_string(), true),
        (
          2,
          "▶\u{fe0e} Run Test".to_string(),
          "test a".to_string(),
          false
        ),
        (2, "Debug".to_string(), "test a".to_string(), true),
      ]
    );
  }

  #[test]
  fn test_deno_test_collector_bdd_steps() {
    // BDD `it(...)` inside `describe(...)` gets step code lenses filtering by
    // the enclosing `describe` name.
    let source = r#"
      describe("suite", () => {
        it("does a thing", () => {});
        it.only("does another", () => {});
      });
    "#;
    assert_eq!(
      summarize(&collect_code_lenses(source)),
      vec![
        (
          1,
          "▶\u{fe0e} Run Test".to_string(),
          "suite".to_string(),
          false
        ),
        (1, "Debug".to_string(), "suite".to_string(), true),
        (
          2,
          "▶\u{fe0e} Run Test".to_string(),
          "suite".to_string(),
          false
        ),
        (2, "Debug".to_string(), "suite".to_string(), true),
        (
          3,
          "▶\u{fe0e} Run Test".to_string(),
          "suite".to_string(),
          false
        ),
        (3, "Debug".to_string(), "suite".to_string(), true),
      ]
    );
  }
}
