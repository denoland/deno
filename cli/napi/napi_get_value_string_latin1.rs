use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_value_string_latin1(
  env: napi_env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let value: v8::Local<v8::Value> = std::mem::transmute(value);

  if !value.is_string() && !value.is_string_object() {
    return Err(Error::StringExpected);
  }

  let v8str = value.to_string(env.scope).unwrap();
  let string_len = v8str.utf8_length(env.scope);

  if buf.is_null() {
    *result = string_len;
  } else if bufsize != 0 {
    let buffer = std::slice::from_raw_parts_mut(buf, bufsize - 1);
    let copied = v8str.write_one_byte(
      env.scope,
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    buf.offset(copied as isize).write(0);
    if !result.is_null() {
      *result = copied;
    }
  } else if !result.is_null() {
    *result = string_len;
  }

  Ok(())
}
