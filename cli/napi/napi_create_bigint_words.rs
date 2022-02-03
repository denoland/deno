 
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_bigint_words(
  env: napi_env,
  sign_bit: bool,
  words: *const u64,
  word_count: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = v8::BigInt::new_from_words(
    env.scope,
    sign_bit,
    std::slice::from_raw_parts(words, word_count),
  )
  .unwrap()
  .into();
  *result = std::mem::transmute(value);
  Ok(())
}
