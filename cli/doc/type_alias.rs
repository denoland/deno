// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::parser::DocParser;
use super::ts_type::TsTypeDef;
use super::ts_type_param::maybe_type_param_decl_to_type_param_defs;
use super::ts_type_param::TsTypeParamDef;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeAliasDef {
  pub ts_type: TsTypeDef,
  pub type_params: Vec<TsTypeParamDef>,
}

pub fn get_doc_for_ts_type_alias_decl(
  _doc_parser: &DocParser,
  type_alias_decl: &swc_ecmascript::ast::TsTypeAliasDecl,
) -> (String, TypeAliasDef) {
  let alias_name = type_alias_decl.id.sym.to_string();
  let ts_type = type_alias_decl.type_ann.as_ref().into();
  let type_params = maybe_type_param_decl_to_type_param_defs(
    type_alias_decl.type_params.as_ref(),
  );
  let type_alias_def = TypeAliasDef {
    ts_type,
    type_params,
  };

  (alias_name, type_alias_def)
}
