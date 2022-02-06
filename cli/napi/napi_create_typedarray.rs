use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_create_typedarray(
  env: napi_env,
  ty: napi_typedarray_type,
  length: usize,
  arraybuffer: napi_value,
  byte_offset: usize,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let ab: v8::Local<v8::Value> = std::mem::transmute(arraybuffer);
  let ab = v8::Local::<v8::ArrayBuffer>::try_from(ab).unwrap();
  let typedarray: v8::Local<v8::Value> = match ty {
    napi_uint8_array => v8::Uint8Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint8_clamped_array => {
      v8::Uint8ClampedArray::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int8_array => v8::Int8Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint16_array => {
      v8::Uint16Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int16_array => v8::Int16Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_uint32_array => {
      v8::Uint32Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_int32_array => v8::Int32Array::new(env.scope, ab, byte_offset, length)
      .unwrap()
      .into(),
    napi_float32_array => {
      v8::Float32Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_float64_array => {
      v8::Float64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_bigint64_array => {
      v8::BigInt64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    napi_biguint64_array => {
      v8::BigUint64Array::new(env.scope, ab, byte_offset, length)
        .unwrap()
        .into()
    }
    _ => {
      return Err(Error::InvalidArg);
    }
  };
  *result = std::mem::transmute(typedarray);
  Ok(())
}
