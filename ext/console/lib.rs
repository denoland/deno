// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::op2;
use deno_core::v8;
use std::path::PathBuf;

deno_core::extension!(
  deno_console,
  ops = [op_is_any_arraybuffer, op_preview_entries,],
  esm = ["01_console.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_console.d.ts")
}

#[op2(fast)]
fn op_is_any_arraybuffer(value: &v8::Value) -> bool {
  value.is_array_buffer() || value.is_shared_array_buffer()
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
