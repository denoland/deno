fn op_ffi_ptr_of<FP>(state: &mut OpState, buf: *const u8, out: &mut [u32])
where
  FP: FfiPermissions + 'static,
{
  // ..
}
