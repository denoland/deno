fn op_read_sync(
  state: &mut OpState,
  rid: ResourceId,
  mut buf: ZeroCopyBuf,
) -> Result<u32, AnyError> {
  // Should not warn about unused `mut buf` binding.
  //
  // This was caused due to incorrect codegen by fast_call.rs
  // on an incompatible op function.
  Ok(23)
}
