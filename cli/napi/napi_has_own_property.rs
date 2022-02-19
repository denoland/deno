use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_has_own_property(
  env: napi_env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let value: v8::Local<v8::Value> = transmute::<napi_value, _>(object);
  let object = value.to_object(env.scope).unwrap();

  let key: v8::Local<v8::Value> = transmute::<napi_value, _>(key);
  if !key.is_name() {
    return Err(Error::NameExpected);
  }

  let maybe = object
    .has_own_property(env.scope, v8::Local::<v8::Name>::try_from(key).unwrap())
    .unwrap_or(false);

  *result = maybe;
  if !maybe {
    return Err(Error::GenericFailure);
  }

  Ok(())
}
