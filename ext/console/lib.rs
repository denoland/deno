// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use deno_core::op2;
use deno_core::v8;
use std::path::PathBuf;

deno_core::extension!(
  deno_console,
  ops = [
    op_is_any_arraybuffer,
    op_is_arguments_object,
    op_is_async_function,
    op_is_generator_function,
    op_is_map_iterator,
    op_is_module_namespace_object,
    op_is_promise,
    op_is_reg_exp,
    op_is_set_iterator,
  ],
  esm = ["01_console.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}

#[op2(fast)]
fn op_is_any_arraybuffer(value: &v8::Value) -> bool {
  value.is_array_buffer() || value.is_shared_array_buffer()
}

#[op2(fast)]
pub fn op_is_arguments_object(value: &v8::Value) -> bool {
  value.is_arguments_object()
}

#[op2(fast)]
pub fn op_is_async_function(value: &v8::Value) -> bool {
  value.is_async_function()
}

#[op2(fast)]
pub fn op_is_generator_function(value: &v8::Value) -> bool {
  value.is_generator_function()
}

#[op2(fast)]
pub fn op_is_generator_object(value: &v8::Value) -> bool {
  value.is_generator_object()
}

#[op2(fast)]
pub fn op_is_map_iterator(value: &v8::Value) -> bool {
  value.is_map_iterator()
}

#[op2(fast)]
pub fn op_is_module_namespace_object(value: &v8::Value) -> bool {
  value.is_module_namespace_object()
}

#[op2(fast)]
pub fn op_is_promise(value: &v8::Value) -> bool {
  value.is_promise()
}

#[op2(fast)]
pub fn op_is_reg_exp(value: &v8::Value) -> bool {
  value.is_reg_exp()
}

#[op2(fast)]
pub fn op_is_set_iterator(value: &v8::Value) -> bool {
  value.is_set_iterator()
}
