// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::config::Config;
use super::config::WorkspaceSettings;
use super::language_server;
use super::text::LineIndex;
use super::tsc;
use super::tsc::NavigationTree;

use deno_ast::swc::ast;
use deno_ast::swc::common::Span;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

static ABSTRACT_MODIFIER: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\babstract\b").unwrap());

static EXPORT_MODIFIER: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\bexport\b").unwrap());

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

fn span_to_range(span: &Span, parsed_source: &ParsedSource) -> lsp::Range {
  let start = parsed_source.source().line_and_column_index(span.lo);
  let end = parsed_source.source().line_and_column_index(span.hi);
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

  fn add_code_lenses<N: AsRef<str>>(&mut self, name: N, span: &Span) {
    let range = span_to_range(span, &self.parsed_source);
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
        command: "deno.test".to_string(),
        arguments: Some(vec![
          json!(self.specifier),
          json!(name.as_ref()),
          options,
        ]),
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
                if let ast::PropName::Ident(ast::Ident { sym, .. }) =
                  &key_value_prop.key
                {
                  if sym == "name" {
                    if let ast::Expr::Lit(ast::Lit::Str(lit_str)) =
                      key_value_prop.value.as_ref()
                    {
                      let name = lit_str.value.to_string();
                      self.add_code_lenses(name, span);
                    }
                  }
                }
              }
            }
          }
        }
        ast::Expr::Fn(fn_expr) => {
          if let Some(ast::Ident { sym, .. }) = fn_expr.ident.as_ref() {
            let name = sym.to_string();
            self.add_code_lenses(name, span);
          }
        }
        ast::Expr::Lit(ast::Lit::Str(lit_str)) => {
          let name = lit_str.value.to_string();
          self.add_code_lenses(name, span);
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
    if let ast::Callee::Expr(callee_expr) = &node.callee {
      match callee_expr.as_ref() {
        ast::Expr::Ident(ident) => {
          if self.test_vars.contains(&ident.sym.to_string()) {
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
            if let ast::Expr::Ident(obj_ident) = member_expr.obj.as_ref() {
              if obj_ident.sym.to_string() == "Deno" {
                if let ast::MemberProp::Ident(prop_ident) = &member_expr.prop {
                  if prop_ident.sym.to_string() == "test" {
                    if let ast::Pat::Ident(binding_ident) = &decl.name {
                      self.test_vars.insert(binding_ident.id.sym.to_string());
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
  language_server: &language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let asset_or_doc =
    language_server.get_cached_asset_or_document(&data.specifier)?;
  let line_index = asset_or_doc.line_index();
  let req = tsc::RequestMethod::GetImplementation((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot();
  let maybe_implementations: Option<Vec<tsc::ImplementationLocation>> =
    language_server.ts_server.request(snapshot, req).await?;
  if let Some(implementations) = maybe_implementations {
    let mut locations = Vec::new();
    for implementation in implementations {
      let implementation_specifier =
        resolve_url(&implementation.document_span.file_name)?;
      let implementation_location =
        implementation.to_location(line_index.clone(), language_server);
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
  language_server: &language_server::Inner,
) -> Result<lsp::CodeLens, AnyError> {
  let asset_or_document =
    language_server.get_cached_asset_or_document(&data.specifier)?;
  let line_index = asset_or_document.line_index();
  let req = tsc::RequestMethod::GetReferences((
    data.specifier.clone(),
    line_index.offset_tsc(code_lens.range.start)?,
  ));
  let snapshot = language_server.snapshot();
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
      let asset_or_doc = language_server
        .get_asset_or_document(&reference_specifier)
        .await?;
      locations.push(
        reference
          .to_location(asset_or_doc.line_index(), &language_server.url_map),
      );
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
  language_server: &language_server::Inner,
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
  parsed_source: Option<ParsedSource>,
  config: &Config,
  line_index: Arc<LineIndex>,
  navigation_tree: &NavigationTree,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let mut code_lenses = collect_test(specifier, parsed_source, config)?;
  code_lenses.extend(
    collect_tsc(
      specifier,
      &config.get_workspace_settings(),
      line_index,
      navigation_tree,
    )
    .await?,
  );

  Ok(code_lenses)
}

fn collect_test(
  specifier: &ModuleSpecifier,
  parsed_source: Option<ParsedSource>,
  config: &Config,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  if config.specifier_code_lens_test(specifier) {
    if let Some(parsed_source) = parsed_source {
      let mut collector =
        DenoTestCollector::new(specifier.clone(), parsed_source.clone());
      parsed_source.module().visit_with(&mut collector);
      return Ok(collector.take());
    }
  }
  Ok(Vec::new())
}

/// Return tsc navigation tree code lenses.
async fn collect_tsc(
  specifier: &ModuleSpecifier,
  workspace_settings: &WorkspaceSettings,
  line_index: Arc<LineIndex>,
  navigation_tree: &NavigationTree,
) -> Result<Vec<lsp::CodeLens>, AnyError> {
  let code_lenses = Rc::new(RefCell::new(Vec::new()));
  navigation_tree.walk(&|i, mp| {
    let mut code_lenses = code_lenses.borrow_mut();

    // TSC Implementations Code Lens
    if workspace_settings.code_lens.implementations {
      let source = CodeLensSource::Implementations;
      match i.kind {
        tsc::ScriptElementKind::InterfaceElement => {
          code_lenses.push(i.to_code_lens(
            line_index.clone(),
            specifier,
            &source,
          ));
        }
        tsc::ScriptElementKind::ClassElement
        | tsc::ScriptElementKind::MemberFunctionElement
        | tsc::ScriptElementKind::MemberVariableElement
        | tsc::ScriptElementKind::MemberGetAccessorElement
        | tsc::ScriptElementKind::MemberSetAccessorElement => {
          if ABSTRACT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(
              line_index.clone(),
              specifier,
              &source,
            ));
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
          code_lenses.push(i.to_code_lens(
            line_index.clone(),
            specifier,
            &source,
          ));
        }
      }
      match i.kind {
        tsc::ScriptElementKind::FunctionElement => {
          if workspace_settings.code_lens.references_all_functions {
            code_lenses.push(i.to_code_lens(
              line_index.clone(),
              specifier,
              &source,
            ));
          }
        }
        tsc::ScriptElementKind::ConstElement
        | tsc::ScriptElementKind::LetElement
        | tsc::ScriptElementKind::VariableElement => {
          if EXPORT_MODIFIER.is_match(&i.kind_modifiers) {
            code_lenses.push(i.to_code_lens(
              line_index.clone(),
              specifier,
              &source,
            ));
          }
        }
        tsc::ScriptElementKind::ClassElement => {
          if i.text != "<class>" {
            code_lenses.push(i.to_code_lens(
              line_index.clone(),
              specifier,
              &source,
            ));
          }
        }
        tsc::ScriptElementKind::InterfaceElement
        | tsc::ScriptElementKind::TypeElement
        | tsc::ScriptElementKind::EnumElement => {
          code_lenses.push(i.to_code_lens(
            line_index.clone(),
            specifier,
            &source,
          ));
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
                    line_index.clone(),
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
  use deno_ast::MediaType;
  use deno_ast::SourceTextInfo;

  use super::*;

  #[test]
  fn test_deno_test_collector() {
    let specifier = resolve_url("https://deno.land/x/mod.ts").unwrap();
    let source = Arc::new(
      r#"
      Deno.test({
        name: "test a",
        fn() {}
      });

      Deno.test(function useFnName() {});

      Deno.test("test b", function anotherTest() {});
    "#
      .to_string(),
    );
    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      source: SourceTextInfo::new(source),
      media_type: MediaType::TypeScript,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
    .unwrap();
    let mut collector =
      DenoTestCollector::new(specifier, parsed_module.clone());
    parsed_module.module().visit_with(&mut collector);
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
            command: "deno.test".to_string(),
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
            command: "deno.test".to_string(),
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
            command: "deno.test".to_string(),
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
            command: "deno.test".to_string(),
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
            command: "deno.test".to_string(),
            arguments: Some(vec![
              json!("https://deno.land/x/mod.ts"),
              json!("test b"),
              json!({
                "inspect": true,
              }),
            ])
          }),
          data: None,
        }
      ]
    );
  }
}
