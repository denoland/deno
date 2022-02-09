use super::function::create_function_template;
use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_define_class(
  env: napi_env,
  utf8name: *const c_char,
  // TODO
  _length: usize,
  constructor: napi_callback,
  callback_data: *mut c_void,
  property_count: usize,
  properties: *const napi_property_descriptor,
  result: *mut napi_value,
) -> Result {
  let mut env = &mut *(env as *mut Env);
  let name = std::ffi::CStr::from_ptr(utf8name).to_str().unwrap();
  let tpl: v8::Local<v8::FunctionTemplate> = std::mem::transmute(
    create_function_template(env, Some(name), constructor, callback_data),
  );
  let napi_properties = std::slice::from_raw_parts(properties, property_count);
  for p in napi_properties {
    let name = if !p.utf8name.is_null() {
      let name_str = CStr::from_ptr(p.utf8name).to_str().unwrap();
      v8::String::new(env.scope, name_str).unwrap()
    } else {
      std::mem::transmute(p.name)
    };

    if !(p.method as *const c_void).is_null() {
      let function: v8::Local<v8::FunctionTemplate> = std::mem::transmute(
        create_function_template(env, None, p.method, p.data),
      );
      let proto = tpl.prototype_template(env.scope);
      proto.set(name.into(), function.into());
    } else if !(p.getter as *const c_void).is_null()
      || !(p.setter as *const c_void).is_null()
    {
      let getter: Option<v8::Local<v8::FunctionTemplate>> =
        if !(p.getter as *const c_void).is_null() {
          Some(std::mem::transmute(create_function_template(
            env, None, p.getter, p.data,
          )))
        } else {
          None
        };
      let setter: Option<v8::Local<v8::FunctionTemplate>> =
        if !(p.setter as *const c_void).is_null() {
          Some(std::mem::transmute(create_function_template(
            env, None, p.setter, p.data,
          )))
        } else {
          None
        };

      let proto = tpl.prototype_template(env.scope);

      let base_name = CStr::from_ptr(p.utf8name).to_str().unwrap();
      let getter_name =
        v8::String::new(env.scope, format!("get_{}", base_name).as_str())
          .unwrap();
      let setter_name =
        v8::String::new(env.scope, format!("set_{}", base_name).as_str())
          .unwrap();

      // TODO: use set_accessor & set_accessor_with_setter
      match (getter, setter) {
        (Some(getter), None) => {
          proto.set(getter_name.into(), getter.into());
        }
        (Some(getter), Some(setter)) => {
          proto.set(getter_name.into(), getter.into());
          proto.set(setter_name.into(), setter.into());
        }
        (None, Some(setter)) => {
          proto.set(setter_name.into(), setter.into());
        }
        (None, None) => unreachable!(),
      }
    } else {
      let proto = tpl.prototype_template(env.scope);
      proto.set(name.into(), std::mem::transmute(p.value));
    }
  }

  let value: v8::Local<v8::Value> = tpl.get_function(env.scope).unwrap().into();
  *result = std::mem::transmute(value);
  Ok(())
}
