// Copyright 2018-2025 the Deno authors. MIT license.

#![deny(warnings)]
deno_ops_compile_test_runner::prelude!();

pub type ResourceId = i16;

#[op2]
#[smi]
pub async fn op_async(#[smi] rid: ResourceId) -> std::io::Result<ResourceId> {
  Ok(rid as _)
}
