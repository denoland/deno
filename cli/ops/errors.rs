// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::fmt_errors::JSError;
use crate::source_maps::get_orig_position;
use crate::source_maps::CachedMaps;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;
use std::collections::HashMap;

// Format Error

pub struct OpFormatError;

#[derive(Deserialize)]
struct FormatErrorArgs {
  error: String,
}

impl DenoOpDispatcher for OpFormatError {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: FormatErrorArgs = serde_json::from_value(args)?;
        let error = JSError::from_json(&args.error, &state.ts_compiler);

        Ok(JsonOp::Sync(json!({
          "error": error.to_string(),
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "formatError";
}

// Apply Source Map

pub struct OpApplySourceMap;

#[derive(Deserialize)]
struct ApplySourceMapArgs {
  filename: String,
  line: i32,
  column: i32,
}

impl DenoOpDispatcher for OpApplySourceMap {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ApplySourceMapArgs = serde_json::from_value(args)?;

        let mut mappings_map: CachedMaps = HashMap::new();
        let (orig_filename, orig_line, orig_column) = get_orig_position(
          args.filename,
          args.line.into(),
          args.column.into(),
          &mut mappings_map,
          &state.ts_compiler,
        );

        Ok(JsonOp::Sync(json!({
          "filename": orig_filename.to_string(),
          "line": orig_line as u32,
          "column": orig_column as u32,
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "applySourceMap";
}
