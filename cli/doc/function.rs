// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_ecma_ast;
use serde::Serialize;

use super::params::pat_to_param_def;
use super::ts_type::ts_type_ann_to_def;
use super::ts_type::TsTypeDef;
use super::ParamDef;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDef {
  pub params: Vec<ParamDef>,
  pub return_type: Option<TsTypeDef>,
  pub is_async: bool,
  pub is_generator: bool,
  // TODO: type_params, decorators
}

pub fn function_to_function_def(
  function: &swc_ecma_ast::Function,
) -> FunctionDef {
  let mut params = vec![];

  for param in &function.params {
    let param_def = pat_to_param_def(param);
    params.push(param_def);
  }

  let maybe_return_type = function
    .return_type
    .as_ref()
    .map(|rt| ts_type_ann_to_def(rt));

  FunctionDef {
    params,
    return_type: maybe_return_type,
    is_async: function.is_async,
    is_generator: function.is_generator,
  }
}

pub fn get_doc_for_fn_decl(
  fn_decl: &swc_ecma_ast::FnDecl,
) -> (String, FunctionDef) {
  let name = fn_decl.ident.sym.to_string();
  let fn_def = function_to_function_def(&fn_decl.function);
  (name, fn_def)
}
