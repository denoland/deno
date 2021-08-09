// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::analysis;
use super::language_server;
use super::tsc;
use crate::ast::ParsedModule;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use swc_common::Span;
use swc_ecmascript::ast;
use swc_ecmascript::visit::Node;
use swc_ecmascript::visit::Visit;
use swc_ecmascript::visit::VisitWith;

lazy_static::lazy_static! {
  static ref ABSTRACT_MODIFIER: Regex = Regex::new(r"\babstract\b").unwrap();
  static ref EXPORT_MODIFIER: Regex = Regex::new(r"\bexport\b").unwrap();
}

#[derive(Debug, Deserialize, Serialize)]
pub enum CodeLensSource {
  #[serde(rename = "implementations")]
  Implementations,
  #[serde(rename = "references")]
  References,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
  pub source: CodeLensSource,
  pub specifier: ModuleSpecifier,
}

fn span_to_range(span: &Span, parsed_module: &ParsedModule) -> lsp::Range {
  let start = parsed_module.get_location(span.lo);
  let end = parsed_module.get_location(span.hi);
  lsp::Range {
    start: lsp::Position {
      line: (start.line - 1) as u32,
      character: start.col as u32,
    },
    end: lsp::Position {
      line: (end.line - 1) as u32,
      character: end.col as u32,
    },
  }
}

struct DenoTestCollector<'a> {
  code_lenses: Vec<lsp::CodeLens>,
  parsed_module: &'a ParsedModule,
  specifier: ModuleSpecifier,
  test_vars: HashSet<String>,
}

impl<'a> DenoTestCollector<'a> {
  pub fn new(
    specifier: ModuleSpecifier,
    parsed_module: &'a ParsedModule,
  ) -> Self {
    Self {
      code_lenses: Vec::new(),
      parsed_module,
      specifier,
      test_vars: HashSet::new(),
    }
  }

  fn add_code_lens<N: AsRef<str>>(&mut self, name: N, span: &Span) {
    let range = span_to_range(span, self.parsed_module);
    self.code_lenses.push(lsp::CodeLens {
      range,
      command: Some(lsp::Command {
        title: "▶\u{fe0e} Run Test".to_string(),
        command: "deno.test".to_string(),
        arguments: Some(vec![json!(self.specifier), json!(name.as_ref())]),
      }),
      data: None,
    });
  }

  fn check_call_expr(&mut self, node: &ast::CallExpr, span: &Span) {
    if let Some(expr) = node.args.get(0).map(|es| es.expr.as_ref()) {
      match expr {
        ast::Expr::Object(obj_lit) => {
          for prop in &obj_lit.props {
            if let ast::PropOrSpread::Prop(prop) = prop {
              if let ast::Prop::KeyValue(key_value_prop) = prop.as_ref() {
                if let ast::PropName::Ident(ident) = &key_value_prop.key {
                  if ident.sym.to_string() == "name" {
                    if let ast::Expr::Lit(ast::Lit::Str(lit_str)) =
                      key_value_prop.value.as_ref()
                    {
                      let name = lit_str.value.to_string();
                      self.add_code_lens(name, span);
                    }
                  }
                }
              }
            }
          }
        }
        ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
          let name = lit_str.value.to_string();
          self.add_code_lens(name, span);
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

impl<'a> Visit for DenoTestCollector<'a> {
  fn visit_call_expr(&mut self, node: &ast::CallExpr, _parent: &dyn Node) {
    if let ast::ExprOrSuper::Expr(callee_expr) = &node.callee {
      match callee_expr.as_ref() {
        ast::Expr::Ident(ident) => {
          if self.test_vars.contains(&ident.sym.to_string()) {
            self.check_call_expr(node, &ident.span);
          }
        }
        ast::Expr::Member(member_expr) => {
          if let ast::Expr::Ident(ns_prop_ident) = member_expr.prop.as_ref() {
            if ns_prop_ident.sym.to_string() == "test" {
              if let ast::ExprOrSuper::Expr(obj_expr) = &member_expr.obj {
                if let ast::Expr::Ident(ident) = obj_expr.as_ref() {
                  if ident.sym.to_string() == "Deno" {
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

  fn visit_var_decl(&mut self, node: &ast::VarDecl, _parent: &dyn Node) {
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
                        self.test_vars.insert(name);
                      }
                    }
                    ast::ObjectPatProp::KeyValue(prop) => {
                      if let ast::PropName::Ident(key_ident) = &prop.key {
                        if key_ident.sym.to_string() == "test" {
                          if let ast::Pat::Ident(value_ident) =
                            &prop.value.as_ref()
                          {
                            self
                              .test_vars
                              .insert(value_ident.id.sym.to_string());
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
            if let ast::ExprOrSuper::Expr(expr) = &member_expr.obj {
              if let ast::Expr::Ident(obj_ident) = expr.as_ref() {
                if obj_ident.sym.to_string() == "Deno" {
                  if let ast::Expr::Ident(prop_ident) =
                    &member_expr.prop.as_ref()
                  {
                    if prop_ident.sym.to_string() == "test" {
                      if let ast::Pat::Ident(binding_ident) = &decl.name {
                        self.test_vars.insert(binding_ident.id.sym.to_string());
                      }
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

async fn resolve_implementation_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let line_index = language_server
    .get_line_index_sync(&data.specifier)
    .unwrap();
  let req = tsc::RequestMethod::GetImplementation((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot()?;
  let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> =
    language_server.ts_server.request(snapshot, req).await?;
  if let Some(implementations) = maybe_implementations {
    let mut locations = Vec::new();
    for implementation in implementations {
      let implementation_specifier =
        resolve_url(&implementation.document_span.file_name)?;
      let implementation_location =
        implementation.to_location(&line_index, language_server);
      if !(implementation_specifier == data.specifier
        && implementation_location.range.start == code_lens.range.start)
      {
        locations.push(implementation_location);
      }
    }
    let command = if !locations.is_empty() {
      let title = if locations.len() > 1 {
        format!("{} implementations", locations.len())
      } else {
        "1 implementation".to_string()
      };
      lsp::Command {
        title,
        command: "deno.showReferences".to_string(),
        arguments: Some(vec![
          json!(data.specifier),
          json!(code_lens.range.start),
          json!(locations),
        ]),
      }
    } else {
      lsp::Command {
        title: "0 implementations".to_string(),
        command: "".to_string(),
        arguments: None,
      }
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  } else {
    let command = Some(lsp::Command {
      title: "0 implementations".to_string(),
      command: "".to_string(),
      arguments: None,
    });
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command,
      data: None,
    })
  }
}

async fn resolve_references_code_lens(
  code_lens: lsp::CodeLens,
  data: CodeLensData,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let line_index = language_server
    .get_line_index_sync(&data.specifier)
    .unwrap();
  let req = tsc::RequestMethod::GetReferences((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot()?;
  let maybe_references: Option<Vec<tsc::ReferenceEntry>> =
    language_server.ts_server.request(snapshot, req).await?;
  if let Some(references) = maybe_references {
    let mut locations = Vec::new();
    for reference in references {
      if reference.is_definition {
        continue;
      }
      let reference_specifier =
        resolve_url(&reference.document_span.file_name)?;
      let line_index =
        language_server.get_line_index(reference_specifier).await?;
      locations.push(reference.to_location(&line_index, language_server));
    }
    let command = if !locations.is_empty() {
      let title = if locations.len() > 1 {
        format!("{} references", locations.len())
      } else {
        "1 reference".to_string()
      };
      lsp::Command {
        title,
        command: "deno.showReferences".to_string(),
        arguments: Some(vec![
          json!(data.specifier),
          json!(code_lens.range.start),
          json!(locations),
        ]),
      }
    } else {
      lsp::Command {
        title: "0 references".to_string(),
        command: "".to_string(),
        arguments: None,
      }
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  } else {
    let command = lsp::Command {
      title: "0 references".to_string(),
      command: "".to_string(),
      arguments: None,
    };
    Ok(lsp::CodeLens {
      range: code_lens.range,
      command: Some(command),
      data: None,
    })
  }
}

pub(crate) async fn resolve_code_lens(
  code_lens: lsp::CodeLens,
  language_server: &mut language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let data: CodeLensData =
    serde_json::from_value(code_lens.data.clone().unwrap())?;
  match data.source {
    CodeLensSource::Implementations => {
      resolve_implementation_code_lens(code_lens, data, language_server).await
    }
    CodeLensSource::References => {
      resolve_references_code_lens(code_lens, data, language_server).await
    }
  }
}

pub(crate) async fn collect(
  specifier: &ModuleSpecifier,
  language_server: &mut language_server::Inner,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let mut code_lenses = collect_test(specifier, language_server)?;
  code_lenses.extend(collect_tsc(specifier, language_server).await?);

  Ok(code_lenses)
}

fn collect_test(
  specifier: &ModuleSpecifier,
  language_server: &mut language_server::Inner,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  if language_server.config.specifier_code_lens_test(specifier) {
    let source = language_server
      .get_text_content(specifier)
      .ok_or_else(|| anyhow!("Missing text content: {}", specifier))?;
    let media_type = language_server
      .get_media_type(specifier)
      .ok_or_else(|| anyhow!("Missing media type: {}", specifier))?;
    // we swallow parsed errors, as they are meaningless here.
    // TODO(@kitsonk) consider caching previous code_lens results to return if
    // there is a parse error to avoid issues of lenses popping in and out
    if let Ok(parsed_module) =
      analysis::parse_module(specifier, &source, &media_type)
    {
      let mut collector =
        DenoTestCollector::new(specifier.clone(), &parsed_module);
      parsed_module.module.visit_with(
        &ast::Invalid {
          span: swc_common::DUMMY_SP,
        },
        &mut collector,
      );
      return Ok(collector.take());
    }
  }
  Ok(Vec::new())
}

/// Return tsc navigation tree code lenses.
async fn collect_tsc(
  specifier: &ModuleSpecifier,
  language_server: &mut language_server::Inner,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let workspace_settings = language_server.config.get_workspace_settings();
  let line_index = language_server
    .get_line_index_sync(specifier)
    .ok_or_else(|| anyhow!("Missing line index."))?;
  let navigation_tree = language_server.get_navigation_tree(specifier).await?;
  let code_lenses = Rc::new(RefCell::new(Vec::new()));
  navigation_tree.walk(&|i, mp| {
    let mut code_lenses = code_lenses.borrow_mut();

    // TSC Implementations Code Lens
    if workspace_settings.code_lens.implementations {
      let source = CodeLensSource::Implementations;
      match i.kind {
        tsc::ScriptElementKind::InterfaceElement => {
          code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
        }
        tsc::ScriptElementKind::ClassElement
        | tsc::ScriptElementKind::MemberFunctionElement
        | tsc::ScriptElementKind::MemberVariableElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement => {
          if ABSTRACT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
          }
        }
        _ => (),
      }
    }

    // TSC References Code Lens
    if workspace_settings.code_lens.references {
      let source = CodeLensSource::References;
      if let Some(parent) = &mp {
        if parent.kind == tsc::ScriptElementKind::EnumElement {
          code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
        }
      }
      match i.kind {
        tsc::ScriptElementKind::FunctionElement => {
          if workspace_settings.code_lens.references_all_functions {
            code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
          }
        }
        tsc::ScriptElementKind::ConstElement
        | tsc::ScriptElementKind::LetElement
        | tsc::ScriptElementKind::VariableElement => {
          if EXPORT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
          }
        }
        tsc::ScriptElementKind::ClassElement => {
          if i.text != "<class>" {
            code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
          }
        }
        tsc::ScriptElementKind::InterfaceElement
        | tsc::ScriptElementKind::TypeElement
        | tsc::ScriptElementKind::EnumElement => {
          code_lenses.push(i.to_code_lens(&line_index, specifier, &source));
        }
        tsc::ScriptElementKind::LocalFunctionElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement
        | tsc::ScriptElementKind::ConstructorImplementationElement
        | tsc::ScriptElementKind::MemberVariableElement => {
          if let Some(parent) = &mp {
            if parent.spans[0].start != i.spans[0].start {
              match parent.kind {
                tsc::ScriptElementKind::ClassElement
                | tsc::ScriptElementKind::InterfaceElement
                | tsc::ScriptElementKind::TypeElement => {
                  code_lenses.push(i.to_code_lens(
                    &line_index,
                    specifier,
                    &source,
                  ));
                }
                _ => (),
              }
            }
          }
        }
        _ => (),
      }
    }
  });
  Ok(Rc::try_unwrap(code_lenses).unwrap().into_inner())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::media_type::MediaType;

  #[test]
  fn test_deno_test_collector() {
    let specifier = resolve_url("https://deno.land/x/mod.ts").unwrap();
    let source = r#"
      Deno.test({
        name: "test a",
        fn() {}
      });

      Deno.test("test b", function anotherTest() {});
    "#;
    let parsed_module =
      analysis::parse_module(&specifier, source, &MediaType::TypeScript)
        .unwrap();
    let mut collector = DenoTestCollector::new(specifier, &parsed_module);
    parsed_module.module.visit_with(
      &ast::Invalid {
        span: swc_common::DUMMY_SP,
      },
      &mut collector,
    );
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
            command: "deno.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test a"),
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
            command: "deno.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test b"),
            ])
          }),
          data: None,
        }
      ]
    );
  }
}
