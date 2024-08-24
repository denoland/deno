use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::StreamExt as _;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::LocalInspectorSession;
use deno_core::LocalInspectorSessionRaw;
use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;

type InspectorMessageReceiver =
  Arc<Mutex<UnboundedReceiver<deno_core::InspectorMsg>>>;
type NotificationReceiver = Arc<Mutex<UnboundedReceiver<serde_json::Value>>>;

#[op2(fast)]
pub fn op_inspector_connect(_state: &mut OpState) {

  // let inspector = state.borrow::<Arc::Mutex<LocalInspectorSession>>();
}

#[op2(fast)]
pub fn op_inspector_disconnect() {}

#[op2(async)]
#[serde]
pub async fn op_inspector_post(
  state: Rc<RefCell<OpState>>,
  #[smi] id: i32,
  #[string] method: String,
  #[serde] params: Option<serde_json::Value>,
) -> Result<(), AnyError> {
  let session = {
    let s = state.borrow();
    s.borrow::<Rc<LocalInspectorSessionRaw>>().clone()
  };
  session.post_message(id, &method, params);
  Ok(())
}

#[op2(async)]
#[string]
pub async fn op_inspector_get_message_from_v8(
  state: Rc<RefCell<OpState>>,
) -> Option<String> {
  let session = {
    let s = state.borrow();
    s.borrow::<Rc<LocalInspectorSessionRaw>>().clone()
  };
  eprintln!("waiting for message");
  let maybe_inspector_message = session.receive_from_v8_session().await;
  eprintln!("waited for message");
  maybe_inspector_message.map(|msg| msg.content)
}
