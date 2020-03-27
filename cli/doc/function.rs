// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;
use swc_ecma_ast;

use super::parser::DocParser;
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
  doc_parser: &DocParser,
  function: &swc_ecma_ast::Function,
) -> FunctionDef {
  let mut params = vec![];

  for param in &function.params {
    use swc_ecma_ast::Pat;

    let param_def = match param {
      Pat::Ident(ident) => {
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

  let maybe_return_type = function
    .return_type
    .as_ref()
    .map(|rt| ts_type_ann_to_def(&doc_parser.source_map, rt));

  FunctionDef {
    params,
    return_type: maybe_return_type,
    is_async: function.is_async,
    is_generator: function.is_generator,
  }
}

pub fn get_doc_for_fn_decl(
  doc_parser: &DocParser,
  fn_decl: &swc_ecma_ast::FnDecl,
) -> (String, FunctionDef) {
  let name = fn_decl.ident.sym.to_string();
  let fn_def = function_to_function_def(doc_parser, &fn_decl.function);
  (name, fn_def)
}
