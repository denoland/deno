fn op_sync_serialize_object_with_numbers_as_keys(
  value: serde_json::Value,
) -> Result<(), Error> {
  assert_eq!(
    value.to_string(),
    r#"{"lines":{"100":{"unit":"m"},"200":{"unit":"cm"}}}"#
  );
  Ok(())
}
