// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::doc::display::{display_optional, display_readonly, SliceDisplayer};
use serde::Serialize;

use super::params::ts_fn_param_to_param_def;
use super::parser::DocParser;
use super::ts_type::ts_type_ann_to_def;
use super::ts_type::TsTypeDef;
use super::ts_type_param::maybe_type_param_decl_to_type_param_defs;
use super::ts_type_param::TsTypeParamDef;
use super::Location;
use super::ParamDef;

use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceMethodDef {
  pub name: String,
  pub location: Location,
  pub js_doc: Option<String>,
  pub optional: bool,
  pub params: Vec<ParamDef>,
  pub return_type: Option<TsTypeDef>,
  pub type_params: Vec<TsTypeParamDef>,
}

impl Display for InterfaceMethodDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}{}({})",
      colors::bold(&self.name),
      display_optional(self.optional),
      SliceDisplayer::new(&self.params, ", ", false),
    )?;
    if let Some(return_type) = &self.return_type {
      write!(f, ": {}", return_type)?;
    }
    Ok(())
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfacePropertyDef {
  pub name: String,
  pub location: Location,
  pub js_doc: Option<String>,
  pub params: Vec<ParamDef>,
  pub computed: bool,
  pub optional: bool,
  pub ts_type: Option<TsTypeDef>,
  pub type_params: Vec<TsTypeParamDef>,
}

impl Display for InterfacePropertyDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}{}",
      colors::bold(&self.name),
      display_optional(self.optional),
    )?;
    if let Some(ts_type) = &self.ts_type {
      write!(f, ": {}", ts_type)?;
    }
    Ok(())
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceIndexSignatureDef {
  pub readonly: bool,
  pub params: Vec<ParamDef>,
  pub ts_type: Option<TsTypeDef>,
}

impl Display for InterfaceIndexSignatureDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}[{}]",
      display_readonly(self.readonly),
      SliceDisplayer::new(&self.params, ", ", false)
    )?;
    if let Some(ts_type) = &self.ts_type {
      write!(f, ": {}", ts_type)?;
    }
    Ok(())
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceCallSignatureDef {
  pub location: Location,
  pub js_doc: Option<String>,
  pub params: Vec<ParamDef>,
  pub ts_type: Option<TsTypeDef>,
  pub type_params: Vec<TsTypeParamDef>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceDef {
  pub extends: Vec<TsTypeDef>,
  pub methods: Vec<InterfaceMethodDef>,
  pub properties: Vec<InterfacePropertyDef>,
  pub call_signatures: Vec<InterfaceCallSignatureDef>,
  pub index_signatures: Vec<InterfaceIndexSignatureDef>,
  pub type_params: Vec<TsTypeParamDef>,
}

pub fn expr_to_name(expr: &swc_ecmascript::ast::Expr) -> String {
  use swc_ecmascript::ast::Expr::*;
  use swc_ecmascript::ast::ExprOrSuper::*;

  match expr {
    Ident(ident) => ident.sym.to_string(),
    Member(member_expr) => {
      let left = match &member_expr.obj {
        Super(_) => "super".to_string(),
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
  interface_decl: &swc_ecmascript::ast::TsInterfaceDecl,
) -> (String, InterfaceDef) {
  let interface_name = interface_decl.id.sym.to_string();

  let mut methods = vec![];
  let mut properties = vec![];
  let mut call_signatures = vec![];
  let mut index_signatures = vec![];

  for type_element in &interface_decl.body.body {
    use swc_ecmascript::ast::TsTypeElement::*;

    match &type_element {
      TsMethodSignature(ts_method_sig) => {
        let method_js_doc = doc_parser.js_doc_for_span(ts_method_sig.span);

        let mut params = vec![];

        for param in &ts_method_sig.params {
          let param_def = ts_fn_param_to_param_def(
            param,
            Some(&doc_parser.ast_parser.source_map),
          );
          params.push(param_def);
        }

        let name = expr_to_name(&*ts_method_sig.key);

        let maybe_return_type = ts_method_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(rt));

        let type_params = maybe_type_param_decl_to_type_param_defs(
          ts_method_sig.type_params.as_ref(),
        );

        let method_def = InterfaceMethodDef {
          name,
          js_doc: method_js_doc,
          location: doc_parser
            .ast_parser
            .get_span_location(ts_method_sig.span)
            .into(),
          optional: ts_method_sig.optional,
          params,
          return_type: maybe_return_type,
          type_params,
        };
        methods.push(method_def);
      }
      TsPropertySignature(ts_prop_sig) => {
        let prop_js_doc = doc_parser.js_doc_for_span(ts_prop_sig.span);
        let name = expr_to_name(&*ts_prop_sig.key);

        let mut params = vec![];

        for param in &ts_prop_sig.params {
          let param_def = ts_fn_param_to_param_def(
            param,
            Some(&doc_parser.ast_parser.source_map),
          );
          params.push(param_def);
        }

        let ts_type = ts_prop_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(rt));

        let type_params = maybe_type_param_decl_to_type_param_defs(
          ts_prop_sig.type_params.as_ref(),
        );

        let prop_def = InterfacePropertyDef {
          name,
          js_doc: prop_js_doc,
          location: doc_parser
            .ast_parser
            .get_span_location(ts_prop_sig.span)
            .into(),
          params,
          ts_type,
          computed: ts_prop_sig.computed,
          optional: ts_prop_sig.optional,
          type_params,
        };
        properties.push(prop_def);
      }
      TsCallSignatureDecl(ts_call_sig) => {
        let call_sig_js_doc = doc_parser.js_doc_for_span(ts_call_sig.span);

        let mut params = vec![];
        for param in &ts_call_sig.params {
          let param_def = ts_fn_param_to_param_def(
            param,
            Some(&doc_parser.ast_parser.source_map),
          );
          params.push(param_def);
        }

        let ts_type = ts_call_sig
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(rt));

        let type_params = maybe_type_param_decl_to_type_param_defs(
          ts_call_sig.type_params.as_ref(),
        );

        let call_sig_def = InterfaceCallSignatureDef {
          js_doc: call_sig_js_doc,
          location: doc_parser
            .ast_parser
            .get_span_location(ts_call_sig.span)
            .into(),
          params,
          ts_type,
          type_params,
        };
        call_signatures.push(call_sig_def);
      }
      TsIndexSignature(ts_index_sig) => {
        let mut params = vec![];
        for param in &ts_index_sig.params {
          let param_def = ts_fn_param_to_param_def(param, None);
          params.push(param_def);
        }

        let ts_type = ts_index_sig
          .type_ann
          .as_ref()
          .map(|rt| (&*rt.type_ann).into());

        let index_sig_def = InterfaceIndexSignatureDef {
          readonly: ts_index_sig.readonly,
          params,
          ts_type,
        };
        index_signatures.push(index_sig_def);
      }
      // TODO:
      TsConstructSignatureDecl(_) => {}
    }
  }

  let type_params = maybe_type_param_decl_to_type_param_defs(
    interface_decl.type_params.as_ref(),
  );

  let extends = interface_decl
    .extends
    .iter()
    .map(|expr| expr.into())
    .collect::<Vec<TsTypeDef>>();

  let interface_def = InterfaceDef {
    extends,
    methods,
    properties,
    call_signatures,
    index_signatures,
    type_params,
  };

  (interface_name, interface_def)
}
