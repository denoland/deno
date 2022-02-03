use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_get_reference_value(
  env: napi_env,
  reference: napi_ref,
  result: *mut napi_value,
) -> Result {
  // TODO
  *result = transmute(reference);
  Ok(())
}
