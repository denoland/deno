// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
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
  let callback_info = unsafe { &*info };
  let args =
    v8::FunctionCallbackArguments::from_function_callback_info(callback_info);
  let mut rv = v8::ReturnValue::from_function_callback_info(callback_info);
  // SAFETY: create_function guarantees that the data is a CallbackInfo external.
  let info_ptr: *mut CallbackInfo = unsafe {
    let external_value = v8::Local::<v8::External>::cast(args.data());
    external_value.value() as _
  };

  // SAFETY: pointer from Box::into_raw.
  let info = unsafe { &mut *info_ptr };
  info.args = &args as *const _ as *const c_void;

  if let Some(f) = info.cb {
    // SAFETY: calling user provided function pointer.
    let value = unsafe { f(info.env, info_ptr as *mut _) };
    if let Some(exc) = unsafe { &mut *(info.env as *mut Env) }
      .last_exception
      .take()
    {
      let scope = unsafe { &mut v8::CallbackScope::new(callback_info) };
      let exc = v8::Local::new(scope, exc);
      scope.throw_exception(exc);
    }
    if let Some(value) = *value {
      rv.set(value);
    }
  }
}

/// # Safety
/// env_ptr must be valid
pub unsafe fn create_function<'a>(
  env_ptr: *mut Env,
  name: Option<v8::Local<v8::String>>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::Function> {
  let env = unsafe { &mut *env_ptr };
  let scope = &mut env.scope();

  let external = v8::External::new(
    scope,
    CallbackInfo::new_raw(env_ptr as _, cb, cb_info) as *mut _,
  );
  let function = v8::Function::builder_raw(call_fn)
    .data(external.into())
    .build(scope)
    .unwrap();

  if let Some(v8str) = name {
    function.set_name(v8str);
  }

  function
}

/// # Safety
/// env_ptr must be valid
pub unsafe fn create_function_template<'a>(
  env_ptr: *mut Env,
  name: Option<v8::Local<v8::String>>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::FunctionTemplate> {
  let env = unsafe { &mut *env_ptr };
  let scope = &mut env.scope();

  let external = v8::External::new(
    scope,
    CallbackInfo::new_raw(env_ptr as _, cb, cb_info) as *mut _,
  );
  let function = v8::FunctionTemplate::builder_raw(call_fn)
    .data(external.into())
    .build(scope);

  if let Some(v8str) = name {
    function.set_class_name(v8str);
  }

  function
}
