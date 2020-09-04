// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::Op;
use deno_core::OpManager;
use futures::future::FutureExt;
pub use serde_derive::Deserialize;
pub use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub type JsonResult = Result<Value, ErrBox>;

pub type AsyncJsonOp = Pin<Box<dyn Future<Output = JsonResult>>>;

pub enum JsonOp {
  Sync(Value),
  Async(AsyncJsonOp),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(AsyncJsonOp),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

pub fn json_op<D>(d: D) -> impl Fn(Rc<State>, BufVec) -> Op
where
  D: Fn(Rc<State>, Value, BufVec) -> Result<JsonOp, ErrBox>,
{
  move |state: Rc<State>, bufs: BufVec| {
    assert!(!bufs.is_empty(), "Expected JSON string at position 0");
    let async_args: AsyncArgs = match serde_json::from_slice(&bufs[0]) {
      Ok(args) => args,
      Err(e) => {
        let buf = state.json_serialize_op_result(None, Err(e.into()));
        return Op::Sync(buf);
      }
    };
    let promise_id = async_args.promise_id;
    let is_sync = promise_id.is_none();

    let state_ = state.clone();
    let result = serde_json::from_slice(&bufs[0])
      .map_err(ErrBox::from)
      .and_then(|args| d(state_, args, bufs[1..].into()));

    // Convert to Op
    match result {
      Ok(JsonOp::Sync(sync_value)) => {
        assert!(promise_id.is_none());
        Op::Sync(state.json_serialize_op_result(promise_id, Ok(sync_value)))
      }
      Ok(JsonOp::Async(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ready(
            state.json_serialize_op_result(promise_id, result),
          )
        });
        Op::Async(fut2.boxed_local())
      }
      Ok(JsonOp::AsyncUnref(fut)) => {
        assert!(promise_id.is_some());
        let fut2 = fut.then(move |result| {
          futures::future::ready(
            state.json_serialize_op_result(promise_id, result),
          )
        });
        Op::AsyncUnref(fut2.boxed_local())
      }
      Err(sync_err) => {
        let buf = state.json_serialize_op_result(promise_id, Err(sync_err));
        if is_sync {
          Op::Sync(buf)
        } else {
          Op::Async(futures::future::ready(buf).boxed_local())
        }
      }
    }
  }
}
