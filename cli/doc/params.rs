// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::swc_ecma_ast;

use super::ts_type::ts_type_ann_to_def;
use super::ParamDef;
use super::ParamKind;
use crate::swc_ecma_ast::Pat;
use crate::swc_ecma_ast::TsFnParam;

pub fn pat_to_param_def(pat: &swc_ecma_ast::Pat) -> ParamDef {
  match pat {
    Pat::Ident(ident) => {
      let ts_type = ident.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

      ParamDef {
        name: ident.sym.to_string(),
        kind: ParamKind::Identifier,
        ts_type,
      }
    }
    Pat::Array(array_pat) => {
      dbg!(array_pat);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
    Pat::Rest(rest_pat) => {
      dbg!(rest_pat);

      let name = match &*rest_pat.arg {
        Pat::Ident(ident) => ident.sym.to_string(),
        _ => "<TODO>".to_string(),
      };
      let ts_type = rest_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

      ParamDef {
        name,
        kind: ParamKind::Rest,
        ts_type,
      }
    }
    Pat::Object(object_pat) => {
      dbg!(object_pat);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
    Pat::Assign(assign_pat) => {
      dbg!(assign_pat);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
    Pat::Expr(boxed_expr) => {
      dbg!(&*boxed_expr);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
    Pat::Invalid(invalid) => {
      dbg!(invalid);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
  }
}

pub fn ts_fn_param_to_param_def(
  ts_fn_param: &swc_ecma_ast::TsFnParam,
) -> ParamDef {
  match ts_fn_param {
    TsFnParam::Ident(ident) => {
      let ts_type = ident.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

      ParamDef {
        name: ident.sym.to_string(),
        kind: ParamKind::Identifier,
        ts_type,
      }
    }
    TsFnParam::Array(array_pat) => {
      dbg!(array_pat);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
    TsFnParam::Rest(rest_pat) => {
      dbg!(rest_pat);

      let name = match &*rest_pat.arg {
        Pat::Ident(ident) => ident.sym.to_string(),
        _ => "<TODO>".to_string(),
      };
      let ts_type = rest_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

      ParamDef {
        name,
        kind: ParamKind::Rest,
        ts_type,
      }
    }
    TsFnParam::Object(object_pat) => {
      dbg!(object_pat);
      ParamDef {
        name: "<TODO>".to_string(),
        kind: ParamKind::Identifier,
        ts_type: None,
      }
    }
  }
}
