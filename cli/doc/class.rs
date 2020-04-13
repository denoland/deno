// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_common::SourceMap;
use crate::swc_common::Spanned;
use crate::swc_ecma_ast;
use serde::Serialize;

use super::function::function_to_function_def;
use super::function::FunctionDef;
use super::params::assign_pat_to_param_def;
use super::params::ident_to_param_def;
use super::params::pat_to_param_def;
use super::parser::DocParser;
use super::ts_type::ts_entity_name_to_name;
use super::ts_type::ts_type_ann_to_def;
use super::ts_type::TsTypeDef;
use super::ts_type_param::maybe_type_param_decl_to_type_param_defs;
use super::ts_type_param::TsTypeParamDef;
use super::Location;
use super::ParamDef;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassConstructorDef {
  pub js_doc: Option<String>,
  pub accessibility: Option<swc_ecma_ast::Accessibility>,
  pub name: String,
  pub params: Vec<ParamDef>,
  pub location: Location,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassPropertyDef {
  pub js_doc: Option<String>,
  pub ts_type: Option<TsTypeDef>,
  pub readonly: bool,
  pub accessibility: Option<swc_ecma_ast::Accessibility>,
  pub is_abstract: bool,
  pub is_static: bool,
  pub name: String,
  pub location: Location,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassMethodDef {
  pub js_doc: Option<String>,
  pub accessibility: Option<swc_ecma_ast::Accessibility>,
  pub is_abstract: bool,
  pub is_static: bool,
  pub name: String,
  pub kind: swc_ecma_ast::MethodKind,
  pub function_def: FunctionDef,
  pub location: Location,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassDef {
  // TODO(bartlomieju): decorators, super_type_params
  pub is_abstract: bool,
  pub constructors: Vec<ClassConstructorDef>,
  pub properties: Vec<ClassPropertyDef>,
  pub methods: Vec<ClassMethodDef>,
  pub super_class: Option<String>,
  pub implements: Vec<String>,
  pub type_params: Vec<TsTypeParamDef>,
}

fn prop_name_to_string(
  source_map: &SourceMap,
  prop_name: &swc_ecma_ast::PropName,
) -> String {
  use crate::swc_ecma_ast::PropName;
  match prop_name {
    PropName::Ident(ident) => ident.sym.to_string(),
    PropName::Str(str_) => str_.value.to_string(),
    PropName::Num(num) => num.value.to_string(),
    PropName::Computed(comp_prop_name) => {
      source_map.span_to_snippet(comp_prop_name.span).unwrap()
    }
  }
}

pub fn get_doc_for_class_decl(
  doc_parser: &DocParser,
  class_decl: &swc_ecma_ast::ClassDecl,
) -> (String, ClassDef) {
  let mut constructors = vec![];
  let mut methods = vec![];
  let mut properties = vec![];

  let super_class: Option<String> = match &class_decl.class.super_class {
    Some(boxed) => {
      use crate::swc_ecma_ast::Expr;
      let expr: &Expr = &**boxed;
      match expr {
        Expr::Ident(ident) => Some(ident.sym.to_string()),
        _ => None,
      }
    }
    None => None,
  };

  let implements: Vec<String> = class_decl
    .class
    .implements
    .iter()
    .map(|expr| ts_entity_name_to_name(&expr.expr))
    .collect();

  for member in &class_decl.class.body {
    use crate::swc_ecma_ast::ClassMember::*;

    match member {
      Constructor(ctor) => {
        let ctor_js_doc = doc_parser.js_doc_for_span(ctor.span());
        let constructor_name =
          prop_name_to_string(&doc_parser.source_map, &ctor.key);

        let mut params = vec![];

        for param in &ctor.params {
          use crate::swc_ecma_ast::PatOrTsParamProp::*;

          let param_def = match param {
            Pat(pat) => pat_to_param_def(pat),
            TsParamProp(ts_param_prop) => {
              use swc_ecma_ast::TsParamPropParam;

              match &ts_param_prop.param {
                TsParamPropParam::Ident(ident) => ident_to_param_def(ident),
                TsParamPropParam::Assign(assign_pat) => {
                  assign_pat_to_param_def(assign_pat)
                }
              }
            }
          };
          params.push(param_def);
        }

        let constructor_def = ClassConstructorDef {
          js_doc: ctor_js_doc,
          accessibility: ctor.accessibility,
          name: constructor_name,
          params,
          location: doc_parser
            .source_map
            .lookup_char_pos(ctor.span.lo())
            .into(),
        };
        constructors.push(constructor_def);
      }
      Method(class_method) => {
        let method_js_doc = doc_parser.js_doc_for_span(class_method.span());
        let method_name =
          prop_name_to_string(&doc_parser.source_map, &class_method.key);
        let fn_def = function_to_function_def(&class_method.function);
        let method_def = ClassMethodDef {
          js_doc: method_js_doc,
          accessibility: class_method.accessibility,
          is_abstract: class_method.is_abstract,
          is_static: class_method.is_static,
          name: method_name,
          kind: class_method.kind,
          function_def: fn_def,
          location: doc_parser
            .source_map
            .lookup_char_pos(class_method.span.lo())
            .into(),
        };
        methods.push(method_def);
      }
      ClassProp(class_prop) => {
        let prop_js_doc = doc_parser.js_doc_for_span(class_prop.span());

        let ts_type = class_prop
          .type_ann
          .as_ref()
          .map(|rt| ts_type_ann_to_def(rt));

        use crate::swc_ecma_ast::Expr;
        let prop_name = match &*class_prop.key {
          Expr::Ident(ident) => ident.sym.to_string(),
          _ => "<TODO>".to_string(),
        };

        let prop_def = ClassPropertyDef {
          js_doc: prop_js_doc,
          ts_type,
          readonly: class_prop.readonly,
          is_abstract: class_prop.is_abstract,
          is_static: class_prop.is_static,
          accessibility: class_prop.accessibility,
          name: prop_name,
          location: doc_parser
            .source_map
            .lookup_char_pos(class_prop.span.lo())
            .into(),
        };
        properties.push(prop_def);
      }
      // TODO(bartlomieju):
      TsIndexSignature(_) => {}
      PrivateMethod(_) => {}
      PrivateProp(_) => {}
    }
  }

  let type_params = maybe_type_param_decl_to_type_param_defs(
    class_decl.class.type_params.as_ref(),
  );
  let class_name = class_decl.ident.sym.to_string();
  let class_def = ClassDef {
    is_abstract: class_decl.class.is_abstract,
    super_class,
    implements,
    constructors,
    properties,
    methods,
    type_params,
  };

  (class_name, class_def)
}
