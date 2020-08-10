// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::Buf;
use crate::CoreIsolateState;
use crate::Op;
use crate::OpDispatcher;
use crate::ZeroCopyBuf;
use futures::future::FutureExt;
pub use serde_derive::Deserialize;
pub use serde_derive::Serialize;
use serde_json::json;
pub use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

#[derive(Serialize)]
pub struct JsonError {
  message: String,
  kind: String,
}

impl From<serde_json::error::Error> for JsonError {
  fn from(error: serde_json::error::Error) -> Self {
    JsonError::from(&error)
  }
}

impl From<&serde_json::error::Error> for JsonError {
  fn from(error: &serde_json::error::Error) -> Self {
    use serde_json::error::*;
    let kind = match error.classify() {
      Category::Io => "TypeError",
      Category::Syntax => "TypeError",
      Category::Data => "InvalidData",
      Category::Eof => "UnexpectedEof",
    }.to_string();

    Self {
      kind, 
      message: error.to_string()
    }
  }
}

pub type JsonResult = Result<Value, JsonError>;

pub type AsyncJsonOp = Pin<Box<dyn Future<Output = JsonResult>>>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(AsyncJsonOp),
}

fn serialize_result(promise_id: Option<u64>, result: JsonResult) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": err, "promiseId": promise_id }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

/// Like OpDispatcher but with additional json `Value` parameter
/// and return a result of `JsonOp` instead of `Op`.
pub trait JsonOpDispatcher {
  fn dispatch(
    &self,
    isolate_state: &mut CoreIsolateState,
    json: Value,
    zero_copy: &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, JsonError>;
}

impl<F> JsonOpDispatcher for F
where
  F: Fn(
    &mut CoreIsolateState,
    Value,
    &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, JsonError>,
{
  fn dispatch(
    &self,
    isolate_state: &mut CoreIsolateState,
    json: Value,
    zero_copy: &mut [ZeroCopyBuf],
  ) -> Result<JsonOp, JsonError> {
    self(isolate_state, json, zero_copy)
  }
}

pub fn json_op(d: impl JsonOpDispatcher) -> impl OpDispatcher {
  move |isolate_state: &mut CoreIsolateState, zero_copy: &mut [ZeroCopyBuf]| {
    assert!(!zero_copy.is_empty(), "Expected JSON string at position 0");
    let async_args: AsyncArgs = match serde_json::from_slice(&zero_copy[0]) {
      Ok(args) => args,
      Err(e) => {
        let buf = serialize_result(None, Err(JsonError::from(e)));
        return Op::Sync(buf);
      }
    };
    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    let result = serde_json::from_slice(&zero_copy[0])
      .map_err(JsonError::from)
      .and_then(|args| d.dispatch(isolate_state, args, &mut zero_copy[1..]));

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
