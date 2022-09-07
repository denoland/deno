// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_runtime::deno_napi::*;

#[repr(C)]
#[derive(Debug)]
pub struct CallbackInfo {
  pub env: napi_env,
  pub cb: napi_callback,
  pub cb_info: napi_callback_info,
  pub args: *const c_void,
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn create_function<'a>(
  env_ptr: *mut Env,
  name: Option<&str>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::Function> {
  let env: &mut Env = unsafe { &mut *env_ptr };
  let scope = &mut env.scope();
  let method_ptr = v8::External::new(scope, cb as *mut c_void);
  let cb_info_ext = v8::External::new(scope, cb_info);

  let env_ext = v8::External::new(scope, env_ptr as *mut c_void);

  let data_array = v8::Array::new_with_elements(
    scope,
    &[method_ptr.into(), cb_info_ext.into(), env_ext.into()],
  );

  let function = v8::Function::builder(
    |scope: &mut v8::HandleScope,
     args: v8::FunctionCallbackArguments,
     mut rv: v8::ReturnValue| {
      let data = args.data().unwrap();
      let data_array = v8::Local::<v8::Array>::try_from(data).unwrap();

      let method_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 0).unwrap(),
      )
      .unwrap();
      let cb: napi_callback =
        unsafe { std::mem::transmute(method_ptr.value()) };

      let cb_info_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 1).unwrap(),
      )
      .unwrap();
      let cb_info: napi_callback_info = cb_info_ptr.value();

      let env_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 2).unwrap(),
      )
      .unwrap();
      let env_ptr = env_ptr.value();
      let mut info = CallbackInfo {
        env: env_ptr,
        cb,
        cb_info,
        args: &args as *const _ as *const c_void,
      };

      let info_ptr = &mut info as *mut _ as *mut c_void;

      let value = unsafe { cb(env_ptr, info_ptr) };
      let value =
        unsafe { transmute::<napi_value, v8::Local<v8::Value>>(value) };
      rv.set(value);
    },
  )
  .data(data_array.into())
  .build(scope)
  .unwrap();

  if let Some(name) = name {
    let v8str = v8::String::new(scope, name).unwrap();
    function.set_name(v8str);
  }

  function
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn create_function_template<'a>(
  env_ptr: *mut Env,
  name: Option<&str>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::FunctionTemplate> {
  let env: &mut Env = unsafe { &mut *env_ptr };
  let scope = &mut env.scope();
  let method_ptr = v8::External::new(scope, cb as *mut c_void);
  let cb_info_ext = v8::External::new(scope, cb_info);

  let env_ext = v8::External::new(scope, env_ptr as *mut c_void);

  let data_array = v8::Array::new_with_elements(
    scope,
    &[method_ptr.into(), cb_info_ext.into(), env_ext.into()],
  );

  let function = v8::FunctionTemplate::builder(
    |scope: &mut v8::HandleScope,
     args: v8::FunctionCallbackArguments,
     mut rv: v8::ReturnValue| {
      let data = args.data().unwrap();
      let data_array = v8::Local::<v8::Array>::try_from(data).unwrap();

      let method_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 0).unwrap(),
      )
      .unwrap();
      let cb: napi_callback =
        unsafe { std::mem::transmute(method_ptr.value()) };

      let cb_info_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 1).unwrap(),
      )
      .unwrap();
      let cb_info: napi_callback_info = cb_info_ptr.value();

      let env_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 2).unwrap(),
      )
      .unwrap();
      let env_ptr = env_ptr.value();
      let mut info = CallbackInfo {
        env: env_ptr,
        cb,
        cb_info,
        args: &args as *const _ as *const c_void,
      };

      let info_ptr = &mut info as *mut _ as *mut c_void;

      let value = unsafe { cb(env_ptr, info_ptr) };
      let value =
        unsafe { transmute::<napi_value, v8::Local<v8::Value>>(value) };
      rv.set(value);
    },
  )
  .data(data_array.into())
  .build(scope);

  if let Some(name) = name {
    let v8str = v8::String::new(scope, name).unwrap();
    function.set_class_name(v8str);
  }

  function
}
