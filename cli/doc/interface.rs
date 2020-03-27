// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;
use swc_ecma_ast;

use super::parser::DocParser;
use super::ts_type::ts_type_ann_to_def;
use super::ts_type::TsTypeDef;
use super::Location;
use super::ParamDef;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceMethodDef {
  // TODO: type_params
  pub name: String,
  pub location: Location,
  pub js_doc: Option<String>,
  pub params: Vec<ParamDef>,
  pub return_type: Option<TsTypeDef>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfacePropertyDef {
  // TODO: type_params
  pub name: String,
  pub location: Location,
  pub js_doc: Option<String>,
  pub params: Vec<ParamDef>,
  pub computed: bool,
  pub optional: bool,
  pub ts_type: Option<TsTypeDef>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceCallSignatureDef {
  // TODO: type_params
  pub location: Location,
  pub js_doc: Option<String>,
  pub params: Vec<ParamDef>,
  pub ts_type: Option<TsTypeDef>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceDef {
  // TODO: extends, type params
  pub methods: Vec<InterfaceMethodDef>,
  pub properties: Vec<InterfacePropertyDef>,
  pub call_signatures: Vec<InterfaceCallSignatureDef>,
}

fn expr_to_name(expr: &swc_ecma_ast::Expr) -> String {
  use swc_ecma_ast::Expr::*;
  use swc_ecma_ast::ExprOrSuper::*;

  match expr {
    Ident(ident) => ident.sym.to_string(),
    Member(member_expr) => {
      let left = match &member_expr.obj {
        Super(_) => "TODO".to_string(),
        Expr(boxed_expr) => expr_to_name(&*boxed_expr),
      };
      let right = expr_to_name(&*member_expr.prop);
      format!("[{}.{}]", left, right)
    }
    _ => "<TODO>".to_string(),
  }
}

pub fn get_doc_for_ts_interface_decl(
  doc_parser: &DocParser,
  interface_decl: &swc_ecma_ast::TsInterfaceDecl,
) -> (String, InterfaceDef) {
  let interface_name = interface_decl.id.sym.to_string();

  let mut methods = vec![];
  let mut properties = vec![];
  let mut call_signatures = vec![];

  for type_element in &interface_decl.body.body {
    use swc_ecma_ast::TsTypeElement::*;

    match &type_element {
      TsMethodSignature(ts_method_sig) => {
        let method_js_doc = doc_parser.js_doc_for_span(ts_method_sig.span);

        let mut params = vec![];

        for param in &ts_method_sig.params {
          use swc_ecma_ast::TsFnParam::*;

          let param_def = match param {
            Ident(ident) => {
              let ts_type = ident
                .type_ann
                .as_ref()
                .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

              ParamDef {
                name: ident.sym.to_string(),
                ts_type,
              }
            }
            _ => ParamDef {
              name: "<TODO>".to_string(),
              ts_type: None,
            },
          };

          params.push(param_def);
        }

        let name = expr_to_name(&*ts_method_sig.key);

        let maybe_return_type = ts_method_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

        let method_def = InterfaceMethodDef {
          name,
          js_doc: method_js_doc,
          location: doc_parser
            .source_map
            .lookup_char_pos(ts_method_sig.span.lo())
            .into(),
          params,
          return_type: maybe_return_type,
        };
        methods.push(method_def);
      }
      TsPropertySignature(ts_prop_sig) => {
        let prop_js_doc = doc_parser.js_doc_for_span(ts_prop_sig.span);
        let name = match &*ts_prop_sig.key {
          swc_ecma_ast::Expr::Ident(ident) => ident.sym.to_string(),
          _ => "TODO".to_string(),
        };

        let mut params = vec![];

        for param in &ts_prop_sig.params {
          use swc_ecma_ast::TsFnParam::*;

          let param_def = match param {
            Ident(ident) => {
              let ts_type = ident
                .type_ann
                .as_ref()
                .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

              ParamDef {
                name: ident.sym.to_string(),
                ts_type,
              }
            }
            _ => ParamDef {
              name: "<TODO>".to_string(),
              ts_type: None,
            },
          };

          params.push(param_def);
        }

        let ts_type = ts_prop_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

        let prop_def = InterfacePropertyDef {
          name,
          js_doc: prop_js_doc,
          location: doc_parser
            .source_map
            .lookup_char_pos(ts_prop_sig.span.lo())
            .into(),
          params,
          ts_type,
          computed: ts_prop_sig.computed,
          optional: ts_prop_sig.optional,
        };
        properties.push(prop_def);
      }
      TsCallSignatureDecl(ts_call_sig) => {
        let call_sig_js_doc = doc_parser.js_doc_for_span(ts_call_sig.span);

        let mut params = vec![];
        for param in &ts_call_sig.params {
          use swc_ecma_ast::TsFnParam::*;

          let param_def = match param {
            Ident(ident) => {
              let ts_type = ident
                .type_ann
                .as_ref()
                .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

              ParamDef {
                name: ident.sym.to_string(),
                ts_type,
              }
            }
            _ => ParamDef {
              name: "<TODO>".to_string(),
              ts_type: None,
            },
          };

          params.push(param_def);
        }

        let ts_type = ts_call_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

        let call_sig_def = InterfaceCallSignatureDef {
          js_doc: call_sig_js_doc,
          location: doc_parser
            .source_map
            .lookup_char_pos(ts_call_sig.span.lo())
            .into(),
          params,
          ts_type,
        };
        call_signatures.push(call_sig_def);
      }
      // TODO:
      TsConstructSignatureDecl(_) => {}
      TsIndexSignature(_) => {}
    }
  }

  let interface_def = InterfaceDef {
    methods,
    properties,
    call_signatures,
  };

  (interface_name, interface_def)
}
