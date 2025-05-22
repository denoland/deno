use deno_core::v8;

use super::SqliteError;

pub(super) fn sql_str(
  scope: &mut v8::HandleScope,
  value: v8::Local<v8::Value>,
) -> Result<(), SqliteError> {
  Ok(())
}
