// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_ecma_ast;

use super::ts_type::ts_type_ann_to_def;
use super::ParamDef;
use super::ParamKind;
use crate::swc_ecma_ast::Pat;
use crate::swc_ecma_ast::TsFnParam;

pub fn ident_to_param_def(ident: &swc_ecma_ast::Ident) -> ParamDef {
  let ts_type = ident.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef {
    name: ident.sym.to_string(),
    kind: ParamKind::Identifier,
    optional: ident.optional,
    ts_type,
  }
}

fn rest_pat_to_param_def(rest_pat: &swc_ecma_ast::RestPat) -> ParamDef {
  let name = match &*rest_pat.arg {
    Pat::Ident(ident) => ident.sym.to_string(),
    _ => "<TODO>".to_string(),
  };
  let ts_type = rest_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef {
    name,
    kind: ParamKind::Rest,
    optional: false,
    ts_type,
  }
}

fn object_pat_to_param_def(object_pat: &swc_ecma_ast::ObjectPat) -> ParamDef {
  let ts_type = object_pat
    .type_ann
    .as_ref()
    .map(|rt| ts_type_ann_to_def(rt));

  ParamDef {
    name: "".to_string(),
    kind: ParamKind::Object,
    optional: object_pat.optional,
    ts_type,
  }
}

fn array_pat_to_param_def(array_pat: &swc_ecma_ast::ArrayPat) -> ParamDef {
  let ts_type = array_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef {
    name: "".to_string(),
    kind: ParamKind::Array,
    optional: array_pat.optional,
    ts_type,
  }
}

pub fn assign_pat_to_param_def(
  assign_pat: &swc_ecma_ast::AssignPat,
) -> ParamDef {
  pat_to_param_def(&*assign_pat.left)
}

pub fn pat_to_param_def(pat: &swc_ecma_ast::Pat) -> ParamDef {
  match pat {
    Pat::Ident(ident) => ident_to_param_def(ident),
    Pat::Array(array_pat) => array_pat_to_param_def(array_pat),
    Pat::Rest(rest_pat) => rest_pat_to_param_def(rest_pat),
    Pat::Object(object_pat) => object_pat_to_param_def(object_pat),
    Pat::Assign(assign_pat) => assign_pat_to_param_def(assign_pat),
    _ => unreachable!(),
  }
}

pub fn ts_fn_param_to_param_def(
  ts_fn_param: &swc_ecma_ast::TsFnParam,
) -> ParamDef {
  match ts_fn_param {
    TsFnParam::Ident(ident) => ident_to_param_def(ident),
    TsFnParam::Array(array_pat) => array_pat_to_param_def(array_pat),
    TsFnParam::Rest(rest_pat) => rest_pat_to_param_def(rest_pat),
    TsFnParam::Object(object_pat) => object_pat_to_param_def(object_pat),
  }
}
