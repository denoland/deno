pub fn op_blob_revoke_object_url(
  state: &mut OpState,
  url: String,
) -> Result<(), AnyError> {
  // TODO(@littledivy): fast compatible https://github.com/denoland/deno/issues/17159
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobStore>();
  blob_store.remove_object_url(&url);
  Ok(())
}
