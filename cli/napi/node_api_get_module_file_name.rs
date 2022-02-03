use deno_core::napi::*;

#[napi_sym::napi_sym]
fn node_api_get_module_file_name(
  env: napi_env,
  result: *mut *const c_char,
) -> Result {
  let env = &mut *(env as *mut Env);
  let shared = env.shared();
  *result = shared.filename;
  Ok(())
}
