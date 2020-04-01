// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use serde::Serialize;
use swc_ecma_ast;

use super::parser::DocParser;
use super::ts_type::TsTypeDef;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeAliasDef {
  pub ts_type: TsTypeDef,
  // TODO: type_params
}

pub fn get_doc_for_ts_type_alias_decl(
  _doc_parser: &DocParser,
  type_alias_decl: &swc_ecma_ast::TsTypeAliasDecl,
) -> (String, TypeAliasDef) {
  let alias_name = type_alias_decl.id.sym.to_string();
  let ts_type = type_alias_decl.type_ann.as_ref().into();

  let type_alias_def = TypeAliasDef { ts_type };

  (alias_name, type_alias_def)
}
