// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::display::{display_optional, SliceDisplayer};
use super::ts_type::{ts_type_ann_to_def, TsTypeDef};
use serde::Serialize;
use std::fmt::{Display, Formatter, Result as FmtResult};
use swc_common::SourceMap;
use swc_ecmascript::ast::{ObjectPatProp, Pat, TsFnParam};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "kind")]
pub enum ParamDef {
  #[serde(rename_all = "camelCase")]
  Array {
    elements: Vec<Option<ParamDef>>,
    optional: bool,
    ts_type: Option<TsTypeDef>,
  },
  #[serde(rename_all = "camelCase")]
  Assign {
    left: Box<ParamDef>,
    right: String,
    ts_type: Option<TsTypeDef>,
  },
  #[serde(rename_all = "camelCase")]
  Identifier {
    name: String,
    optional: bool,
    ts_type: Option<TsTypeDef>,
  },
  #[serde(rename_all = "camelCase")]
  Object {
    props: Vec<ObjectPatPropDef>,
    optional: bool,
    ts_type: Option<TsTypeDef>,
  },
  #[serde(rename_all = "camelCase")]
  Rest {
    arg: Box<ParamDef>,
    ts_type: Option<TsTypeDef>,
  },
}

impl Display for ParamDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      Self::Array {
        elements,
        optional,
        ts_type,
      } => {
        write!(f, "[")?;
        if !elements.is_empty() {
          if let Some(v) = &elements[0] {
            write!(f, "{}", v)?;
          }
          for maybe_v in &elements[1..] {
            write!(f, ", ")?;
            if let Some(v) = maybe_v {
              write!(f, "{}", v)?;
            }
          }
        }
        write!(f, "]")?;
        write!(f, "{}", display_optional(*optional))?;
        if let Some(ts_type) = ts_type {
          write!(f, ": {}", ts_type)?;
        }
        Ok(())
      }
      Self::Assign { left, ts_type, .. } => {
        write!(f, "{}", left)?;
        if let Some(ts_type) = ts_type {
          write!(f, ": {}", ts_type)?;
        }
        // TODO(SyrupThinker) As we cannot display expressions the value is just omitted
        // write!(f, " = {}", right)?;
        Ok(())
      }
      Self::Identifier {
        name,
        optional,
        ts_type,
      } => {
        write!(f, "{}{}", name, display_optional(*optional))?;
        if let Some(ts_type) = ts_type {
          write!(f, ": {}", ts_type)?;
        }
        Ok(())
      }
      Self::Object {
        props,
        optional,
        ts_type,
      } => {
        write!(
          f,
          "{{{}}}{}",
          SliceDisplayer::new(&props, ", ", false),
          display_optional(*optional)
        )?;
        if let Some(ts_type) = ts_type {
          write!(f, ": {}", ts_type)?;
        }
        Ok(())
      }
      Self::Rest { arg, ts_type } => {
        write!(f, "...{}", arg)?;
        if let Some(ts_type) = ts_type {
          write!(f, ": {}", ts_type)?;
        }
        Ok(())
      }
    }
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "kind")]
pub enum ObjectPatPropDef {
  Assign { key: String, value: Option<String> },
  KeyValue { key: String, value: Box<ParamDef> },
  Rest { arg: Box<ParamDef> },
}

impl Display for ObjectPatPropDef {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      Self::KeyValue { key, .. } => {
        // The internal identifier does not need to be exposed
        write!(f, "{}", key)
      }
      Self::Assign { key, value } => {
        if let Some(_value) = value {
          // TODO(SyrupThinker) As we cannot display expressions the value is just omitted
          write!(f, "{}", key)
        } else {
          write!(f, "{}", key)
        }
      }
      Self::Rest { arg } => write!(f, "...{}", arg),
    }
  }
}

pub fn ident_to_param_def(
  ident: &swc_ecmascript::ast::Ident,
  _source_map: Option<&SourceMap>,
) -> ParamDef {
  let ts_type = ident.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef::Identifier {
    name: ident.sym.to_string(),
    optional: ident.optional,
    ts_type,
  }
}

fn rest_pat_to_param_def(
  rest_pat: &swc_ecmascript::ast::RestPat,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  let ts_type = rest_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef::Rest {
    arg: Box::new(pat_to_param_def(&*rest_pat.arg, source_map)),
    ts_type,
  }
}

fn object_pat_prop_to_def(
  object_pat_prop: &ObjectPatProp,
  source_map: Option<&SourceMap>,
) -> ObjectPatPropDef {
  match object_pat_prop {
    ObjectPatProp::Assign(assign) => ObjectPatPropDef::Assign {
      key: assign.key.sym.to_string(),
      value: assign.value.as_ref().map(|_| "<UNIMPLEMENTED>".to_string()),
    },
    ObjectPatProp::KeyValue(keyvalue) => ObjectPatPropDef::KeyValue {
      key: prop_name_to_string(&keyvalue.key, source_map),
      value: Box::new(pat_to_param_def(&*keyvalue.value, source_map)),
    },
    ObjectPatProp::Rest(rest) => ObjectPatPropDef::Rest {
      arg: Box::new(pat_to_param_def(&*rest.arg, source_map)),
    },
  }
}

fn object_pat_to_param_def(
  object_pat: &swc_ecmascript::ast::ObjectPat,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  let props = object_pat
    .props
    .iter()
    .map(|prop| object_pat_prop_to_def(prop, source_map))
    .collect::<Vec<_>>();
  let ts_type = object_pat
    .type_ann
    .as_ref()
    .map(|rt| ts_type_ann_to_def(rt));

  ParamDef::Object {
    props,
    optional: object_pat.optional,
    ts_type,
  }
}

fn array_pat_to_param_def(
  array_pat: &swc_ecmascript::ast::ArrayPat,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  let elements = array_pat
    .elems
    .iter()
    .map(|elem| elem.as_ref().map(|e| pat_to_param_def(e, source_map)))
    .collect::<Vec<Option<_>>>();
  let ts_type = array_pat.type_ann.as_ref().map(|rt| ts_type_ann_to_def(rt));

  ParamDef::Array {
    elements,
    optional: array_pat.optional,
    ts_type,
  }
}

pub fn assign_pat_to_param_def(
  assign_pat: &swc_ecmascript::ast::AssignPat,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  let ts_type = assign_pat
    .type_ann
    .as_ref()
    .map(|rt| ts_type_ann_to_def(rt));

  ParamDef::Assign {
    left: Box::new(pat_to_param_def(&*assign_pat.left, source_map)),
    right: "<UNIMPLEMENTED>".to_string(),
    ts_type,
  }
}

pub fn pat_to_param_def(
  pat: &swc_ecmascript::ast::Pat,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  match pat {
    Pat::Ident(ident) => ident_to_param_def(ident, source_map),
    Pat::Array(array_pat) => array_pat_to_param_def(array_pat, source_map),
    Pat::Rest(rest_pat) => rest_pat_to_param_def(rest_pat, source_map),
    Pat::Object(object_pat) => object_pat_to_param_def(object_pat, source_map),
    Pat::Assign(assign_pat) => assign_pat_to_param_def(assign_pat, source_map),
    _ => unreachable!(),
  }
}

pub fn ts_fn_param_to_param_def(
  ts_fn_param: &swc_ecmascript::ast::TsFnParam,
  source_map: Option<&SourceMap>,
) -> ParamDef {
  match ts_fn_param {
    TsFnParam::Ident(ident) => ident_to_param_def(ident, source_map),
    TsFnParam::Array(array_pat) => {
      array_pat_to_param_def(array_pat, source_map)
    }
    TsFnParam::Rest(rest_pat) => rest_pat_to_param_def(rest_pat, source_map),
    TsFnParam::Object(object_pat) => {
      object_pat_to_param_def(object_pat, source_map)
    }
  }
}

pub fn prop_name_to_string(
  prop_name: &swc_ecmascript::ast::PropName,
  source_map: Option<&SourceMap>,
) -> String {
  use swc_ecmascript::ast::PropName;
  match prop_name {
    PropName::Ident(ident) => ident.sym.to_string(),
    PropName::Str(str_) => str_.value.to_string(),
    PropName::Num(num) => num.value.to_string(),
    PropName::Computed(comp_prop_name) => source_map
      .map(|sm| sm.span_to_snippet(comp_prop_name.span).unwrap())
      .unwrap_or_else(|| "<UNAVAILABLE>".to_string()),
  }
}
