use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_set_instance_data(
  env: napi_env,
  data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
) -> Result {
  let env = &mut *(env as *mut Env);
  let shared = env.shared_mut();
  shared.instance_data = data;
  shared.data_finalize = if !(finalize_cb as *const c_void).is_null() {
    Some(finalize_cb)
  } else {
    None
  };
  shared.data_finalize_hint = finalize_hint;
  Ok(())
}
