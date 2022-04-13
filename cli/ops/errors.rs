// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostics;
use crate::fmt_errors::format_file_name;
use crate::proc_state::ProcState;
use crate::source_maps::get_orig_position;
use crate::source_maps::CachedMaps;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::Extension;
use deno_core::OpState;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      op_apply_source_map::decl(),
      op_format_diagnostic::decl(),
      op_format_file_name::decl(),
    ])
    .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplySourceMap {
  file_name: String,
  line_number: i32,
  column_number: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppliedSourceMap {
  file_name: String,
  line_number: u32,
  column_number: u32,
}

#[op]
fn op_apply_source_map(
  state: &mut OpState,
  args: ApplySourceMap,
) -> Result<AppliedSourceMap, AnyError> {
  let mut mappings_map: CachedMaps = HashMap::new();
  let ps = state.borrow::<ProcState>().clone();

  let (orig_file_name, orig_line_number, orig_column_number, _) =
    get_orig_position(
      args.file_name,
      args.line_number.into(),
      args.column_number.into(),
      &mut mappings_map,
      ps,
    );

  Ok(AppliedSourceMap {
    file_name: orig_file_name,
    line_number: orig_line_number as u32,
    column_number: orig_column_number as u32,
  })
}

#[op]
fn op_format_diagnostic(args: Value) -> Result<Value, AnyError> {
  let diagnostic: Diagnostics = serde_json::from_value(args)?;
  Ok(json!(diagnostic.to_string()))
}

#[op]
fn op_format_file_name(file_name: String) -> Result<String, AnyError> {
  Ok(format_file_name(&file_name))
}
