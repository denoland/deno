use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc::UnboundedReceiver;
use deno_core::futures::StreamExt as _;
use deno_core::op2;
use deno_core::serde_json;
use deno_core::LocalInspectorSession;
use deno_core::OpState;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

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
  #[string] method: String,
  #[serde] params: Option<serde_json::Value>,
) -> Result<serde_json::Value, AnyError> {
  let session = {
    let s = state.borrow();
    s.borrow::<Arc<Mutex<LocalInspectorSession>>>().clone()
  };

  let mut lock = session.lock().unwrap();
  lock.post_message(&method, params).await
}

#[op2(async)]
#[serde]
pub async fn op_inspector_get_notification(
  state: Rc<RefCell<OpState>>,
) -> Option<serde_json::Value> {
  let receiver = {
    let s = state.borrow();
    s.borrow::<NotificationReceiver>().clone()
  };

  let mut receiver = receiver.lock().unwrap();
  let maybe_msg = receiver.next().await;
  maybe_msg
}
