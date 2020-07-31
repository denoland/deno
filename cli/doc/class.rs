// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::colors;
use crate::doc::display::{
  display_abstract, display_accessibility, display_async, display_generator,
  display_method, display_optional, display_readonly, display_static,
  SliceDisplayer,
};
use serde::Serialize;
use swc_common::Spanned;

use super::function::function_to_function_def;
use super::function::FunctionDef;
use super::interface::expr_to_name;
use super::params::{
  assign_pat_to_param_def, ident_to_param_def, pat_to_param_def,
  prop_name_to_string, ts_fn_param_to_param_def,
};
use super::parser::DocParser;
use super::ts_type::{
  maybe_type_param_instantiation_to_type_defs, ts_type_ann_to_def, TsTypeDef,
};
use super::ts_type_param::maybe_type_param_decl_to_type_param_defs;
use super::ts_type_param::TsTypeParamDef;
use super::Location;
use super::ParamDef;

use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassConstructorDef {
  pub js_doc: Option<String>,
  pub accessibility: Option<swc_ecmascript::ast::Accessibility>,
  pub name: String,
  pub params: Vec<ParamDef>,
  pub location: Location,
}

impl Display for ClassConstructorDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}{}({})",
      display_accessibility(self.accessibility),
      colors::magenta("constructor"),
      SliceDisplayer::new(&self.params, ", ", false),
    )
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassPropertyDef {
  pub js_doc: Option<String>,
  pub ts_type: Option<TsTypeDef>,
  pub readonly: bool,
  pub accessibility: Option<swc_ecmascript::ast::Accessibility>,
  pub optional: bool,
  pub is_abstract: bool,
  pub is_static: bool,
  pub name: String,
  pub location: Location,
}

impl Display for ClassPropertyDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}{}{}{}{}{}",
      display_abstract(self.is_abstract),
      display_accessibility(self.accessibility),
      display_static(self.is_static),
      display_readonly(self.readonly),
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
pub struct ClassIndexSignatureDef {
  pub readonly: bool,
  pub params: Vec<ParamDef>,
  pub ts_type: Option<TsTypeDef>,
}

impl Display for ClassIndexSignatureDef {
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
pub struct ClassMethodDef {
  pub js_doc: Option<String>,
  pub accessibility: Option<swc_ecmascript::ast::Accessibility>,
  pub optional: bool,
  pub is_abstract: bool,
  pub is_static: bool,
  pub name: String,
  pub kind: swc_ecmascript::ast::MethodKind,
  pub function_def: FunctionDef,
  pub location: Location,
}

impl Display for ClassMethodDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(
      f,
      "{}{}{}{}{}{}{}{}({})",
      display_abstract(self.is_abstract),
      display_accessibility(self.accessibility),
      display_static(self.is_static),
      display_async(self.function_def.is_async),
      display_method(self.kind),
      display_generator(self.function_def.is_generator),
      colors::bold(&self.name),
      display_optional(self.optional),
      SliceDisplayer::new(&self.function_def.params, ", ", false),
    )?;
    if let Some(return_type) = &self.function_def.return_type {
      write!(f, ": {}", return_type)?;
    }
    Ok(())
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClassDef {
  // TODO(bartlomieju): decorators
  pub is_abstract: bool,
  pub constructors: Vec<ClassConstructorDef>,
  pub properties: Vec<ClassPropertyDef>,
  pub index_signatures: Vec<ClassIndexSignatureDef>,
  pub methods: Vec<ClassMethodDef>,
  pub extends: Option<String>,
  pub implements: Vec<TsTypeDef>,
  pub type_params: Vec<TsTypeParamDef>,
  pub super_type_params: Vec<TsTypeDef>,
}

pub fn class_to_class_def(
  doc_parser: &DocParser,
  class: &swc_ecmascript::ast::Class,
) -> ClassDef {
  let mut constructors = vec![];
  let mut methods = vec![];
  let mut properties = vec![];
  let mut index_signatures = vec![];

  let extends: Option<String> = match &class.super_class {
    Some(boxed) => {
      use swc_ecmascript::ast::Expr;
      let expr: &Expr = &**boxed;
      match expr {
        Expr::Ident(ident) => Some(ident.sym.to_string()),
        _ => None,
      }
    }
    None => None,
  };

  let implements = class
    .implements
    .iter()
    .map(|expr| expr.into())
    .collect::<Vec<TsTypeDef>>();

  for member in &class.body {
    use swc_ecmascript::ast::ClassMember::*;

    match member {
      Constructor(ctor) => {
        let ctor_js_doc = doc_parser.js_doc_for_span(ctor.span());
        let constructor_name = prop_name_to_string(
          &ctor.key,
          Some(&doc_parser.ast_parser.source_map),
        );

        let mut params = vec![];

        for param in &ctor.params {
          use swc_ecmascript::ast::ParamOrTsParamProp::*;

          let param_def = match param {
            Param(param) => pat_to_param_def(
              &param.pat,
              Some(&doc_parser.ast_parser.source_map),
            ),
            TsParamProp(ts_param_prop) => {
              use swc_ecmascript::ast::TsParamPropParam;

              match &ts_param_prop.param {
                TsParamPropParam::Ident(ident) => ident_to_param_def(
                  ident,
                  Some(&doc_parser.ast_parser.source_map),
                ),
                TsParamPropParam::Assign(assign_pat) => {
                  assign_pat_to_param_def(
                    assign_pat,
                    Some(&doc_parser.ast_parser.source_map),
                  )
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
          location: doc_parser.ast_parser.get_span_location(ctor.span).into(),
        };
        constructors.push(constructor_def);
      }
      Method(class_method) => {
        let method_js_doc = doc_parser.js_doc_for_span(class_method.span());
        let method_name = prop_name_to_string(
          &class_method.key,
          Some(&doc_parser.ast_parser.source_map),
        );
        let fn_def =
          function_to_function_def(&doc_parser, &class_method.function);
        let method_def = ClassMethodDef {
          js_doc: method_js_doc,
          accessibility: class_method.accessibility,
          optional: class_method.is_optional,
          is_abstract: class_method.is_abstract,
          is_static: class_method.is_static,
          name: method_name,
          kind: class_method.kind,
          function_def: fn_def,
          location: doc_parser
            .ast_parser
            .get_span_location(class_method.span)
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

        let prop_name = expr_to_name(&*class_prop.key);

        let prop_def = ClassPropertyDef {
          js_doc: prop_js_doc,
          ts_type,
          readonly: class_prop.readonly,
          optional: class_prop.is_optional,
          is_abstract: class_prop.is_abstract,
          is_static: class_prop.is_static,
          accessibility: class_prop.accessibility,
          name: prop_name,
          location: doc_parser
            .ast_parser
            .get_span_location(class_prop.span)
            .into(),
        };
        properties.push(prop_def);
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

        let index_sig_def = ClassIndexSignatureDef {
          readonly: ts_index_sig.readonly,
          params,
          ts_type,
        };
        index_signatures.push(index_sig_def);
      }
      // TODO(bartlomieju):
      PrivateMethod(_) => {}
      PrivateProp(_) => {}
      _ => {}
    }
  }

  let type_params =
    maybe_type_param_decl_to_type_param_defs(class.type_params.as_ref());

  let super_type_params = maybe_type_param_instantiation_to_type_defs(
    class.super_type_params.as_ref(),
  );

  ClassDef {
    is_abstract: class.is_abstract,
    extends,
    implements,
    constructors,
    properties,
    index_signatures,
    methods,
    type_params,
    super_type_params,
  }
}

pub fn get_doc_for_class_decl(
  doc_parser: &DocParser,
  class_decl: &swc_ecmascript::ast::ClassDecl,
) -> (String, ClassDef) {
  let class_name = class_decl.ident.sym.to_string();
  let class_def = class_to_class_def(doc_parser, &class_decl.class);

  (class_name, class_def)
}
