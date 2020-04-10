// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::*;
use futures::future::FutureExt;
use serde_derive::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub trait JsonError {
  fn kind(&self) -> i32;
  fn message(&self) -> String;
}

pub type JsonResult = Result<Value, Box<dyn JsonError>>;

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
    Ok(v) => json!({ 
      "ok": v, 
      "promiseId": promise_id 
    }),
    Err(err) => json!({ 
      "err": { 
        "kind": err.kind(), 
        "message": err.message() 
      }, 
      "promiseId": promise_id 
    }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

impl JsonError for serde_json::Error {
  fn kind(&self) -> i32 {
    -1
  }

  fn message(&self) -> String {
    self.to_string()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

pub fn json_op<D>(d: D) -> impl Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp
where
  D: Fn(Value, Option<ZeroCopyBuf>) -> Result<JsonOp, Box<dyn JsonError>>,
{
  move |control: &[u8], zero_copy: Option<ZeroCopyBuf>| {
    let async_args: AsyncArgs = match serde_json::from_slice(control) {
      Ok(args) => args,
      Err(e) => {
        let buf = serialize_result(None, Err(Box::new(e)));
        return CoreOp::Sync(buf);
      }
    };

    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    // Already deserialized once - can't fail if we're this far
    let args = serde_json::from_slice(control).unwrap();
    let result = d(args, zero_copy);

    // Convert to CoreOp
    match result {
      Ok(JsonOp::Sync(sync_value)) => {
        assert!(promise_id.is_none());
        CoreOp::Sync(serialize_result(promise_id, Ok(sync_value)))
      }
      Ok(JsonOp::Async(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ok(serialize_result(promise_id, result))
        });
        CoreOp::Async(fut2.boxed_local())
      }
      Ok(JsonOp::AsyncUnref(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ok(serialize_result(promise_id, result))
        });
        CoreOp::AsyncUnref(fut2.boxed_local())
      }
      Err(sync_err) => {
        let buf = serialize_result(promise_id, Err(sync_err));
        if is_sync {
          CoreOp::Sync(buf)
        } else {
          CoreOp::Async(futures::future::ok(buf).boxed_local())
        }
      }
    }
  }
}
