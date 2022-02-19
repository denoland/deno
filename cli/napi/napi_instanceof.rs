use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_instanceof(
  env: &mut Env,
  value: v8::Local<v8::Value>,
  constructor: v8::Local<v8::Value>,
  result: *mut bool,
) -> Result {
  let ctor = constructor.to_object(env.scope).unwrap();
  if !ctor.is_function() {
    return Err(Error::FunctionExpected);
  }
  let maybe = value.instance_of(env.scope, ctor);
  match maybe {
    Some(res) => {
      *result = res;
      Ok(())
    }
    None => Err(Error::GenericFailure),
  }
}
