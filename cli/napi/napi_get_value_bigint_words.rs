use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_value_bigint_words(
  env: napi_env,
  value: napi_value,
  sign_bit: *mut i32,
  size: *mut usize,
  out_words: *mut u64,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let value: v8::Local<v8::Value> = std::mem::transmute(value);
  let bigint = value.to_big_int(env.scope).unwrap();

  let out_words = std::slice::from_raw_parts_mut(out_words, *size);
  let mut words = Vec::with_capacity(bigint.word_count());
  let (sign, _) = bigint.to_words_array(words.as_mut_slice());
  *sign_bit = sign as i32;

  for (i, word) in out_words.iter_mut().enumerate() {
    *word = words[i];
  }

  Ok(())
}
