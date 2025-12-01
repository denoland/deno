// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

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
use lazy_regex::lazy_regex;
use lsp_types::Uri;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

use super::analysis::source_range_to_lsp_range;
use super::config::CodeLensSettings;
use super::language_server;
use super::text::LineIndex;
use super::tsc;
use super::tsc::NavigationTree;

static ABSTRACT_MODIFIER: Lazy<Regex> = lazy_regex!(r"\babstract\b");

static EXPORT_MODIFIER: Lazy<Regex> = lazy_regex!(r"\bexport\b");

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

  fn add_code_lenses<N: AsRef<str>>(&mut self, name: N, range: &SourceRange) {
    let range =
      source_range_to_lsp_range(range, self.parsed_source.text_info_lazy());
    self.add_code_lens(&name, range, "▶\u{fe0e} Run Test", false);
    self.add_code_lens(&name, range, "Debug", true);
  }

  fn add_code_lens<N: AsRef<str>>(
    &mut self,
    name: &N,
    range: lsp::Range,
    title: &str,
    inspect: bool,
  ) {
    let options = json!({
      "inspect": inspect,
    });
    self.code_lenses.push(lsp::CodeLens {
      range,
      command: Some(lsp::Command {
        title: title.to_string(),
        command: "deno.client.test".to_string(),
        arguments: Some(vec![
          json!(self.specifier),
          json!(name.as_ref()),
          options,
        ]),
      }),
      data: None,
    });
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, range: &SourceRange) {
    if let Some(expr) = node.args.first().map(|es| es.expr.as_ref()) {
      match expr {
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
                  let name = lit_str.value.to_string_lossy().to_string();
                  self.add_code_lenses(name, range);
                }
                ast::Expr::Tpl(tpl) if tpl.quasis.len() == 1 => {
                  let name = tpl.quasis.first().unwrap().raw.to_string();
                  self.add_code_lenses(name, range);
                }
                _ => {}
              }
            }
          }
        }
        ast::Expr::Fn(fn_expr) => {
          if let Some(ast::Ident { sym, .. }) = fn_expr.ident.as_ref() {
            let name = sym.to_string();
            self.add_code_lenses(name, range);
          }
        }
        ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
          let name = lit_str.value.to_string_lossy().to_string();
          self.add_code_lenses(name, range);
        }
        ast::Expr::Tpl(tpl) if tpl.quasis.len() == 1 => {
          let name = tpl.quasis.first().unwrap().raw.to_string();
          self.add_code_lenses(name, range);
        }
        _ => (),
      }
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
    match prop_chain {
      ["", "Deno", "test"]
      | ["Deno", "test", "ignore" | "only"]
      | ["", "", "describe"]
      | ["", "describe", "ignore" | "only" | "skip"] => {}
      ["", "", s] if self.test_vars.contains(s) => {}
      _ => return,
    };
    self.check_call_expr(node, &rightmost_symbol_range);
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

/// Return tsc navigation tree code lenses.
pub fn collect_tsc(
  uri: &Uri,
  code_lens_settings: &CodeLensSettings,
  line_index: Arc<LineIndex>,
  navigation_tree: &NavigationTree,
  token: &CancellationToken,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let code_lenses = Rc::new(RefCell::new(Vec::new()));
  navigation_tree.walk(token, &|i, mp| {
    let mut code_lenses = code_lenses.borrow_mut();

    // TSC Implementations Code Lens
    if code_lens_settings.implementations {
      let source = CodeLensSource::Implementations;
      match i.kind {
        tsc::ScriptElementKind::InterfaceElement => {
          code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
        }
        tsc::ScriptElementKind::ClassElement
        | tsc::ScriptElementKind::MemberFunctionElement
        | tsc::ScriptElementKind::MemberVariableElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement => {
          if ABSTRACT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
          }
        }
        _ => (),
      }
    }

    // TSC References Code Lens
    if code_lens_settings.references {
      let source = CodeLensSource::References;
      if let Some(parent) = &mp
        && parent.kind == tsc::ScriptElementKind::EnumElement
      {
        code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
      }
      match i.kind {
        tsc::ScriptElementKind::FunctionElement => {
          if code_lens_settings.references_all_functions {
            code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
          }
        }
        tsc::ScriptElementKind::ConstElement
        | tsc::ScriptElementKind::LetElement
        | tsc::ScriptElementKind::VariableElement => {
          if EXPORT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
          }
        }
        tsc::ScriptElementKind::ClassElement => {
          if i.text != "<class>" {
            code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
          }
        }
        tsc::ScriptElementKind::InterfaceElement
        | tsc::ScriptElementKind::TypeElement
        | tsc::ScriptElementKind::EnumElement => {
          code_lenses.push(i.to_code_lens(line_index.clone(), uri, source));
        }
        tsc::ScriptElementKind::LocalFunctionElement
        | tsc::ScriptElementKind::MemberFunctionElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement
        | tsc::ScriptElementKind::ConstructorImplementationElement
        | tsc::ScriptElementKind::MemberVariableElement => {
          if let Some(parent) = &mp
            && parent.spans[0].start != i.spans[0].start
          {
            match parent.kind {
              tsc::ScriptElementKind::ClassElement
              | tsc::ScriptElementKind::InterfaceElement
              | tsc::ScriptElementKind::TypeElement => {
                code_lenses.push(i.to_code_lens(
                  line_index.clone(),
                  uri,
                  source,
                ));
              }
              _ => (),
            }
          }
        }
        _ => (),
      }
    }
  })?;
  Ok(Rc::try_unwrap(code_lenses).unwrap().into_inner())
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
}
