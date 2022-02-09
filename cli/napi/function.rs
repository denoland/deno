use deno_core::napi::*;

#[repr(C)]
#[derive(Debug)]
pub struct CallbackInfo {
  pub env: napi_env,
  pub cb: napi_callback,
  pub cb_info: napi_callback_info,
  pub args: *const c_void,
}

pub fn create_function<'a>(
  env: &'a mut Env,
  name: Option<&str>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::Function> {
  let method_ptr = v8::External::new(env.scope, cb as *mut c_void);
  let cb_info_ext = v8::External::new(env.scope, unsafe { transmute(cb_info) });
  let env_ptr = env as *mut _ as *mut c_void;
  let env_ext = v8::External::new(env.scope, env_ptr);

  let data_array = v8::Array::new_with_elements(
    env.scope,
    &[method_ptr.into(), cb_info_ext.into(), env_ext.into()],
  );

  let function = v8::Function::builder(
    |handle_scope: &mut v8::HandleScope,
     args: v8::FunctionCallbackArguments,
     mut rv: v8::ReturnValue| {
      let context = v8::Context::new(handle_scope);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

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
      let cb_info: napi_callback_info =
        unsafe { std::mem::transmute(cb_info_ptr.value()) };

      let env_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 2).unwrap(),
      )
      .unwrap();
      let env_ptr = env_ptr.value() as *mut Env;
      let sender = unsafe { (*(env_ptr)).async_work_sender.clone() };

      let mut env = unsafe { (*(env_ptr)).with_new_scope(scope, sender) };
      let env_ptr = &mut env as *mut _ as *mut c_void;

      let mut info = CallbackInfo {
        env: env_ptr,
        cb,
        cb_info,
        args: &args as *const _ as *const c_void,
      };

      let info_ptr = &mut info as *mut _ as *mut c_void;

      let value = unsafe { cb(env_ptr, info_ptr) };
      let value = unsafe { std::mem::transmute(value) };
      rv.set(value);
    },
  )
  .data(data_array.into())
  .build(env.scope)
  .unwrap();

  if let Some(name) = name {
    let v8str = v8::String::new(env.scope, name).unwrap();
    function.set_name(v8str);
  }

  function
}

pub fn create_function_template<'a>(
  env: &'a mut Env,
  name: Option<&str>,
  cb: napi_callback,
  cb_info: napi_callback_info,
) -> v8::Local<'a, v8::FunctionTemplate> {
  let method_ptr = v8::External::new(env.scope, cb as *mut c_void);
  let cb_info_ext = v8::External::new(env.scope, unsafe { transmute(cb_info) });
  let env_ptr = env as *mut _ as *mut c_void;
  let env_ext = v8::External::new(env.scope, env_ptr);

  let data_array = v8::Array::new_with_elements(
    env.scope,
    &[method_ptr.into(), cb_info_ext.into(), env_ext.into()],
  );

  let function = v8::FunctionTemplate::builder(
    |handle_scope: &mut v8::HandleScope,
     args: v8::FunctionCallbackArguments,
     mut rv: v8::ReturnValue| {
      let context = v8::Context::new(handle_scope);
      let scope = &mut v8::ContextScope::new(handle_scope, context);

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
      let cb_info: napi_callback_info =
        unsafe { std::mem::transmute(cb_info_ptr.value()) };

      let env_ptr = v8::Local::<v8::External>::try_from(
        data_array.get_index(scope, 2).unwrap(),
      )
      .unwrap();
      let env_ptr = env_ptr.value() as *mut Env;
      let sender = unsafe { (*(env_ptr)).async_work_sender.clone() };

      let mut env = unsafe { (*(env_ptr)).with_new_scope(scope, sender) };
      let env_ptr = &mut env as *mut _ as *mut c_void;

      let mut info = CallbackInfo {
        env: env_ptr,
        cb,
        cb_info,
        args: &args as *const _ as *const c_void,
      };

      let info_ptr = &mut info as *mut _ as *mut c_void;

      let value = unsafe { cb(env_ptr, info_ptr) };
      let value = unsafe { transmute(value) };
      rv.set(value);
    },
  )
  .data(data_array.into())
  .build(env.scope);

  if let Some(name) = name {
    let v8str = v8::String::new(env.scope, name).unwrap();
    function.set_class_name(v8str);
  }

  function
}
