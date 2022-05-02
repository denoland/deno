// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::fmt_errors::format_file_name;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_format_file_name::decl()])
    .build()
}

#[op]
fn op_format_file_name(file_name: String) -> Result<String, AnyError> {
  Ok(format_file_name(&file_name))
}
