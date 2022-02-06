use deno_core::napi::*;
//

// TODO: properly implement ref counting stuff
#[napi_sym::napi_sym]
fn napi_create_reference(
  env: napi_env,
  value: napi_value,
  _initial_refcount: u32,
  result: *mut napi_ref,
) -> Result {
  let mut _env = &mut *(env as *mut Env);
  *result = value;
  Ok(())
}
