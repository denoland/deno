pub fn op_blob_revoke_object_url(
  state: &mut deno_core::OpState,
  url: String,
) -> Result<(), AnyError> {
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobStore>();
  blob_store.remove_object_url(&url);
  Ok(())
}
