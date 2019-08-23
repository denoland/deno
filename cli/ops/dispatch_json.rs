// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use futures::Future;
use futures::Poll;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;

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

pub type Dispatcher = fn(
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
  let vec = serde_json::to_vec(&value).unwrap();
  vec.into_boxed_slice()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

pub fn dispatch(
  d: Dispatcher,
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
) -> CoreOp {
  let async_args: AsyncArgs = serde_json::from_slice(control).unwrap();
  let promise_id = async_args.promise_id;
  let is_sync = promise_id.is_none();

  let result = serde_json::from_slice(control)
    .map_err(ErrBox::from)
    .and_then(move |args| d(state, args, zero_copy));
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
