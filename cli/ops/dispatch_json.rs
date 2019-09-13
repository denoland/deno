// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::ops::*;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use futures::Poll;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

pub type AsyncJsonOp = Box<dyn Future<Item = Value, Error = ErrBox> + Send>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
}

fn json_err(err: ErrBox) -> Value {
  use crate::deno_error::GetErrorKind;
  json!({
    "message": err.to_string(),
    "kind": err.kind() as u32,
  })
}

#[allow(dead_code)]
pub type JsonOpHandler = fn(
  state: &ThreadSafeState,
  args: Value,
  zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox>;

fn serialize_result(
  promise_id: Option<u64>,
  result: Result<Value, ErrBox>,
) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": json_err(err), "promiseId": promise_id }),
  };
  let mut vec = serde_json::to_vec(&value).unwrap();
  debug!("JSON response pre-align, len={}", vec.len());
  // Align to 32bit word, padding with the space character.
  vec.resize((vec.len() + 3usize) & !3usize, b' ');
  vec.into_boxed_slice()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

pub struct JsonDispatcher {
  op_registry: RwLock<HashMap<OpId, JsonOpHandler>>,
  next_op_id: AtomicU32,
}

impl JsonDispatcher {
  pub fn new() -> Self {
    Self {
      next_op_id: AtomicU32::new(2001),
      op_registry: RwLock::new(HashMap::new()),
    }
  }

  pub fn register_op(&self, handler: JsonOpHandler) -> OpId {
    let op_id = self.next_op_id.fetch_add(1, Ordering::SeqCst);
    // TODO: verify that we didn't overflow 1000 ops

    // Ensure the op isn't a duplicate, and can be registered.
    self
      .op_registry
      .write()
      .unwrap()
      .entry(op_id)
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(handler);

    op_id
  }

  fn select_op(&self, op_id: OpId) -> JsonOpHandler {
    *self
      .op_registry
      .read()
      .unwrap()
      .get(&op_id)
      .expect("Op not found!")
  }

  pub fn dispatch(
    &self,
    op_id: OpId,
    state: &ThreadSafeState,
    control: &[u8],
    zero_copy: Option<PinnedBuf>,
  ) -> CoreOp {
    let async_args: AsyncArgs = serde_json::from_slice(control).unwrap();
    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    // Select and run JsonOpHandler
    let handler = self.select_op(op_id);
    let result = serde_json::from_slice(control)
      .map_err(ErrBox::from)
      .and_then(move |args| handler(state, args, zero_copy));

    // Convert to CoreOp
    match result {
      Ok(JsonOp::Sync(sync_value)) => {
        assert!(promise_id.is_none());
        CoreOp::Sync(serialize_result(promise_id, Ok(sync_value)))
      }
      Ok(JsonOp::Async(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = Box::new(fut.then(move |result| -> Result<Buf, ()> {
          Ok(serialize_result(promise_id, result))
        }));
        CoreOp::Async(fut2)
      }
      Err(sync_err) => {
        let buf = serialize_result(promise_id, Err(sync_err));
        if is_sync {
          CoreOp::Sync(buf)
        } else {
          CoreOp::Async(Box::new(futures::future::ok(buf)))
        }
      }
    }
  }
}

// This is just type conversion. Implement From trait?
// See https://github.com/tokio-rs/tokio/blob/ffd73a64e7ec497622b7f939e38017afe7124dc4/tokio-fs/src/lib.rs#L76-L85
fn convert_blocking_json<F>(f: F) -> Poll<Value, ErrBox>
where
  F: FnOnce() -> Result<Value, ErrBox>,
{
  use futures::Async::*;
  match tokio_threadpool::blocking(f) {
    Ok(Ready(Ok(v))) => Ok(Ready(v)),
    Ok(Ready(Err(err))) => Err(err),
    Ok(NotReady) => Ok(NotReady),
    Err(err) => panic!("blocking error {}", err),
  }
}

pub fn blocking_json<F>(is_sync: bool, f: F) -> Result<JsonOp, ErrBox>
where
  F: 'static + Send + FnOnce() -> Result<Value, ErrBox>,
{
  if is_sync {
    Ok(JsonOp::Sync(f()?))
  } else {
    Ok(JsonOp::Async(Box::new(futures::sync::oneshot::spawn(
      tokio_util::poll_fn(move || convert_blocking_json(f)),
      &tokio_executor::DefaultExecutor::current(),
    ))))
  }
}
