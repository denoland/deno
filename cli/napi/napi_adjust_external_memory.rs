use deno_core::napi::*;

#[napi_sym::napi_sym]
fn napi_adjust_external_memory(
  env: &mut Env,
  change_in_bytes: i64,
  adjusted_value: &mut i64,
) -> Result {
  let isolate = unsafe { &mut *env.isolate_ptr };
  *adjusted_value =
    isolate.adjust_amount_of_external_allocated_memory(change_in_bytes);
  Ok(())
}
