use crate::test_dispatcher::TestMessage;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::sync::mpsc::Sender;

pub fn init(rt: &mut JsRuntime) {
  super::reg_json_sync(rt, "op_send_test_message", op_send_test_message);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendTestMessageArgs {
  message: TestMessage,
}

fn op_send_test_message(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: SendTestMessageArgs = serde_json::from_value(args)?;
  let sender = state.borrow::<Sender<TestMessage>>().clone();

  if sender.send(args.message).is_err() {
    Ok(json!(false))
  } else {
    Ok(json!(true))
  }
}
