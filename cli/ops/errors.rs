// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostic;
use crate::source_maps::get_orig_position;
use crate::source_maps::CachedMaps;
use crate::state::State;
use deno_core::ErrBox;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use serde_derive::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_apply_source_map", op_apply_source_map);
  s.register_op_json_sync("op_format_diagnostic", op_format_diagnostic);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplySourceMap {
  file_name: String,
  line_number: i32,
  column_number: i32,
}

fn op_apply_source_map(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let args: ApplySourceMap = serde_json::from_value(args)?;

  let mut mappings_map: CachedMaps = HashMap::new();
  let (orig_file_name, orig_line_number, orig_column_number) =
    get_orig_position(
      args.file_name,
      args.line_number.into(),
      args.column_number.into(),
      &mut mappings_map,
      &state.global_state.ts_compiler,
    );

  Ok(json!({
    "fileName": orig_file_name,
    "lineNumber": orig_line_number as u32,
    "columnNumber": orig_column_number as u32,
  }))
}

fn op_format_diagnostic(
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  let diagnostic = serde_json::from_value::<Diagnostic>(args)?;
  Ok(json!(diagnostic.to_string()))
}
