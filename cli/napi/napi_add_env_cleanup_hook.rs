use deno_core::napi::*;

// TODO: properly implement
#[napi_sym::napi_sym]
fn napi_add_env_cleanup_hook(
  env: napi_env,
  hook: extern "C" fn(*const c_void),
  data: *const c_void,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  Ok(())
}
