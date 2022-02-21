use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_string_latin1(
  env: napi_env,
  string: *const u8,
  length: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let string = std::slice::from_raw_parts(string, length);
  
  match v8::String::new_from_one_byte(env.scope, string, v8::NewStringType::Normal) {
    Some(v8str) => {
      let value: v8::Local<v8::Value> = v8str.into();
      *result = std::mem::transmute(value);
    },
    None => return Err(Error::GenericFailure),
  }

  Ok(())
}
