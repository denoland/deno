// Copyright 2018-2025 the Deno authors. MIT license.
use deno_core::op2;
use deno_core::v8;

deno_core::extension!(
  deno_console,
  ops = [op_preview_entries],
  esm = ["01_console.js"],
);

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
