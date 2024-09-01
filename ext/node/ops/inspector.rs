use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::LocalInspectorSessionRaw;

#[op2(fast)]
pub fn op_inspector_disconnect(#[cppgc] session: &LocalInspectorSessionRaw) {
  session.disconnect();
}

#[op2]
pub fn op_inspector_post(
  #[cppgc] session: &LocalInspectorSessionRaw,
  #[smi] id: i32,
  #[string] method: String,
  #[serde] params: Option<serde_json::Value>,
) -> Result<(), AnyError> {
  session.post_message(id, &method, params);
  Ok(())
}

#[op2(async)]
#[string]
pub async fn op_inspector_get_message_from_v8(
  #[cppgc] session: &LocalInspectorSessionRaw,
) -> Option<String> {
  let maybe_inspector_message = session.receive_from_v8_session().await;
  maybe_inspector_message.map(|msg| msg.content)
}
