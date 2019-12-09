// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#[macro_use]
extern crate log;

use deno_core::*;
use futures::future::FutureExt;
pub use serde_derive::Deserialize;
use serde_json::json;
pub use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub type AsyncJsonOp =
  Pin<Box<dyn Future<Output = Result<Value, ErrBox>> + Send>>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
}

fn json_err(err: ErrBox) -> Value {
  json!({
      "message": err.to_string(),
  })
}

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

pub fn json_op(
  d: Box<
    dyn Fn(Value, Option<PinnedBuf>) -> Result<JsonOp, ErrBox>
      + Send
      + Sync
      + 'static,
  >,
) -> Box<dyn Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static> {
  Box::new(move |control: &[u8], zero_copy: Option<PinnedBuf>| {
    let async_args: AsyncArgs = match serde_json::from_slice(control) {
      Ok(args) => args,
      Err(e) => {
        let buf = serialize_result(None, Err(ErrBox::from(e)));
        return CoreOp::Sync(buf);
      }
    };
    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    let result = serde_json::from_slice(control)
      .map_err(ErrBox::from)
      .and_then(|args| d(args, zero_copy));

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
        CoreOp::Async(fut2.boxed())
      }
      Err(sync_err) => {
        let buf = serialize_result(promise_id, Err(sync_err));
        if is_sync {
          CoreOp::Sync(buf)
        } else {
          CoreOp::Async(futures::future::ok(buf).boxed())
        }
      }
    }
  })
}
