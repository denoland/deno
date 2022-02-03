 
use deno_core::napi::*;
use super::function::create_function;

#[napi_sym::napi_sym]
fn napi_define_properties(
  env: napi_env,
  obj: napi_value,
  property_count: usize,
  properties: *const napi_property_descriptor,
) -> Result {
  let mut env = &mut *(env as *mut Env);

  let object: v8::Local<v8::Object> = std::mem::transmute(obj);
  let properties = std::slice::from_raw_parts(properties, property_count);

  for property in properties {
    let name_str = CStr::from_ptr(property.utf8name).to_str().unwrap();
    let name = v8::String::new(env.scope, name_str).unwrap();

    let method_ptr: *mut c_void = std::mem::transmute(property.method);

    if !method_ptr.is_null() {
      let function: v8::Local<v8::Value> = {
        let function =
          create_function(env, Some(name_str), property.method, property.data);
        let value: v8::Local<v8::Value> = function.into();
        std::mem::transmute(value)
      };
      object.set(env.scope, name.into(), function).unwrap();
    }
  }

  Ok(())
}
