// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::ts_type::TsTypeDef;
use crate::swc_ecma_ast::TsTypeParam;
use crate::swc_ecma_ast::TsTypeParamDecl;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TsTypeParamDef {
  pub name: String,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub constraint: Option<TsTypeDef>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub default: Option<TsTypeDef>,
}

impl Into<TsTypeParamDef> for &TsTypeParam {
  fn into(self) -> TsTypeParamDef {
    let name = self.name.sym.to_string();
    let constraint: Option<TsTypeDef> =
      if let Some(ts_type) = self.constraint.as_ref() {
        let type_def: TsTypeDef = (&**ts_type).into();
        Some(type_def)
      } else {
        None
      };
    let default: Option<TsTypeDef> =
      if let Some(ts_type) = self.default.as_ref() {
        let type_def: TsTypeDef = (&**ts_type).into();
        Some(type_def)
      } else {
        None
      };

    TsTypeParamDef {
      name,
      constraint,
      default,
    }
  }
}

pub fn maybe_type_param_decl_to_type_param_defs(
  maybe_type_param_decl: Option<&TsTypeParamDecl>,
) -> Vec<TsTypeParamDef> {
  if let Some(type_params_decl) = maybe_type_param_decl {
    type_params_decl
      .params
      .iter()
      .map(|type_param| type_param.into())
      .collect::<Vec<TsTypeParamDef>>()
  } else {
    vec![]
  }
}
