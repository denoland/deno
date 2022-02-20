use deno_core::napi::*;

pub struct TsFn {
  pub maybe_func: Option<v8::Global<v8::Function>>,
  pub maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  pub context: *const c_void,
}

#[napi_sym::napi_sym]
fn napi_create_threadsafe_function(
  env: &mut Env,
  func: napi_value,
  _async_resource: napi_value,
  _async_resource_name: napi_value,
  _max_queue_size: usize,
  initial_thread_count: usize,
  _thread_finialize_data: *mut c_void,
  _thread_finalize_cb: napi_finalize,
  context: *const c_void,
  maybe_call_js_cb: Option<napi_threadsafe_function_call_js>,
  result: *mut napi_threadsafe_function,
) -> Result {
  if initial_thread_count <= 0 {
    return Err(Error::InvalidArg);
  }

  let maybe_func = func
    .as_mut()
    .map(|func| {
      let value =
        unsafe { transmute::<napi_value, v8::Local<v8::Value>>(func) };
      let func = v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::FunctionExpected)?;
      Ok(v8::Global::new(env.scope, func))
    })
    .transpose()?;
  let tsfn = TsFn {
    maybe_func,
    maybe_call_js_cb,
    context,
  };

  *result = transmute::<Box<TsFn>, _>(Box::new(tsfn));

  Ok(())
}
