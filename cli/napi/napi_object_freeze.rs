use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_object_freeze(env: &mut Env, object: v8::Local<v8::Value>) -> Result {
  let object = object.to_object(env.scope).unwrap();
  let maybe = object.set_integrity_level(env.scope, v8::IntegrityLevel::Frozen);

  match maybe {
    Some(_) => Ok(()),
    None => Err(Error::GenericFailure),
  }
}
