// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::op_error::OpError;
use deno_core::Buf;
use deno_core::CoreIsolateState;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub type JsonResult = Result<Value, OpError>;

pub type AsyncJsonOp = Pin<Box<dyn Future<Output = JsonResult>>>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(AsyncJsonOp),
}

fn json_err(err: OpError) -> Value {
  json!({
    "message": err.msg,
    "kind": err.kind_str,
  })
}

pub fn serialize_result(promise_id: Option<u64>, result: JsonResult) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": json_err(err), "promiseId": promise_id }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

pub fn json_op<D>(
  d: D,
) -> impl Fn(&mut CoreIsolateState, &mut [ZeroCopyBuf]) -> Op
where
  D: Fn(
    &mut CoreIsolateState,
    Value,
    &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, OpError>,
{
  move |isolate_state: &mut CoreIsolateState, zero_copy: &mut [ZeroCopyBuf]| {
    assert!(!zero_copy.is_empty(), "Expected JSON string at position 0");
    let async_args: AsyncArgs = match serde_json::from_slice(&zero_copy[0]) {
      Ok(args) => args,
      Err(e) => {
        let buf = serialize_result(None, Err(OpError::from(e)));
        return Op::Sync(buf);
      }
    };
    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    let result = serde_json::from_slice(&zero_copy[0])
      .map_err(OpError::from)
      .and_then(|args| d(isolate_state, args, &mut zero_copy[1..]));

    // Convert to Op
    match result {
      Ok(JsonOp::Sync(sync_value)) => {
        assert!(promise_id.is_none());
        Op::Sync(serialize_result(promise_id, Ok(sync_value)))
      }
      Ok(JsonOp::Async(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ready(serialize_result(promise_id, result))
        });
        Op::Async(fut2.boxed_local())
      }
      Ok(JsonOp::AsyncUnref(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ready(serialize_result(promise_id, result))
        });
        Op::AsyncUnref(fut2.boxed_local())
      }
      Err(sync_err) => {
        let buf = serialize_result(promise_id, Err(sync_err));
        if is_sync {
          Op::Sync(buf)
        } else {
          Op::Async(futures::future::ready(buf).boxed_local())
        }
      }
    }
  }
}

pub fn blocking_json<F>(is_sync: bool, f: F) -> Result<JsonOp, OpError>
where
  F: 'static + Send + FnOnce() -> JsonResult,
{
  if is_sync {
    Ok(JsonOp::Sync(f()?))
  } else {
    let fut = async move { tokio::task::spawn_blocking(f).await.unwrap() };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}
