// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostics;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_format_diagnostic::decl()])
    .build()
}

#[op]
fn op_format_diagnostic(args: Value) -> Result<Value, AnyError> {
  let diagnostic: Diagnostics = serde_json::from_value(args)?;
  Ok(json!(diagnostic.to_string()))
}
