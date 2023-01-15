// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::*;

#[repr(C)]
#[derive(Debug)]
pub struct CallbackInfo {
  pub env: napi_env,
  pub cb: napi_callback,
  pub cb_info: napi_callback_info,
  pub args: *const c_void,
}

impl CallbackInfo {
  #[inline]
  pub fn new_raw(
    env: napi_env,
    cb: napi_callback,
    cb_info: napi_callback_info,
  ) -> *mut Self {
    Box::into_raw(Box::new(Self {
      env,
      cb,
      cb_info,
      args: std::ptr::null(),
    }))
  }
}

extern "C" fn call_fn(info: *const v8::FunctionCallbackInfo) {
  let info = unsafe { &*info };
  let args = v8::FunctionCallbackArguments::from_function_callback_info(info);
  let mut rv = v8::ReturnValue::from_function_callback_info(info);
  // SAFETY: create_function guarantees that the data is a CallbackInfo external.
  let info_ptr: *mut CallbackInfo = unsafe {
    let external_value = v8::Local::<v8::External>::cast(args.data());
    external_value.value() as _
  };

  // SAFETY: pointer from Box::into_raw.
  let mut info = unsafe { &mut *info_ptr };
  info.args = &args as *const _ as *const c_void;

  if let Some(f) = info.cb {
    // SAFETY: calling user provided function pointer.
    let value = unsafe { f(info.env, info_ptr as *mut _) };
    // SAFETY: napi_value is reprsented as v8::Local<v8::Value> internally.
    rv.set(unsafe { transmute::<napi_value, v8::Local<v8::Value>>(value) });
  }
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

  let external = v8::External::new(
    scope,
    CallbackInfo::new_raw(env_ptr as _, cb, cb_info) as *mut _,
  );
  let function = v8::Function::builder_raw(call_fn)
    .data(external.into())
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

  let external = v8::External::new(
    scope,
    CallbackInfo::new_raw(env_ptr as _, cb, cb_info) as *mut _,
  );
  let function = v8::FunctionTemplate::builder_raw(call_fn)
    .data(external.into())
    .build(scope);

  if let Some(name) = name {
    let v8str = v8::String::new(scope, name).unwrap();
    function.set_class_name(v8str);
  }

  function
}
