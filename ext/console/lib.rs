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
    op_is_generator_object,
    op_is_map_iterator,
    op_is_module_namespace_object,
    op_is_native_error,
    op_is_promise,
    op_is_reg_exp,
    op_is_set_iterator,
    op_preview_entries,
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
pub fn op_is_native_error(value: &v8::Value) -> bool {
  value.is_native_error()
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

#[op2]
pub fn op_preview_entries<'s>(
  scope: &mut v8::HandleScope<'s>,
  object: &v8::Object,
  slow_path: bool,
) -> v8::Local<'s, v8::Value> {
  let (entries, is_key_value) = object.preview_entries(scope);
  match entries {
    None => v8::undefined(scope).into(),
    Some(entries) => {
      if !slow_path {
        return entries.into();
      }

      let ret: [v8::Local<v8::Value>; 2] =
        [entries.into(), v8::Boolean::new(scope, is_key_value).into()];
      v8::Array::new_with_elements(scope, &ret).into()
    }
  }
}
