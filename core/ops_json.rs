// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::type_error;
use crate::error::AnyError;
use crate::BufVec;
use crate::Op;
use crate::OpFn;
use crate::OpState;
use crate::ZeroCopyBuf;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::convert::TryInto;
use std::future::Future;
use std::rc::Rc;

fn json_serialize_op_result<R: Serialize>(
  request_id: Option<u64>,
  result: Result<R, AnyError>,
  get_error_class_fn: crate::runtime::GetErrorClassFn,
) -> Box<[u8]> {
  let value = match result {
    Ok(v) => serde_json::json!({ "ok": v, "requestId": request_id }),
    Err(err) => serde_json::json!({
      "requestId": request_id,
      "err": {
        "className": (get_error_class_fn)(&err),
        "message": err.to_string(),
      }
    }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

/// Creates an op that passes data synchronously using JSON.
///
/// The provided function `op_fn` has the following parameters:
/// * `&mut OpState`: the op state, can be used to read/write resources in the runtime from an op.
/// * `V`: the deserializable value that is passed to the Rust function.
/// * `&mut [ZeroCopyBuf]`: raw bytes passed along, usually not needed if the JSON value is used.
///
/// `op_fn` returns a serializable value, which is directly returned to JavaScript.
///
/// When registering an op like this...
/// ```ignore
/// let mut runtime = JsRuntime::new(...);
/// runtime.register_op("hello", deno_core::json_op_sync(Self::hello_op));
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let result = Deno.core.jsonOpSync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn json_op_sync<F, V, R>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut OpState, V, &mut [ZeroCopyBuf]) -> Result<R, AnyError> + 'static,
  V: DeserializeOwned,
  R: Serialize,
{
  Box::new(move |state: Rc<RefCell<OpState>>, mut bufs: BufVec| -> Op {
    let result = serde_json::from_slice(&bufs[0])
      .map_err(AnyError::from)
      .and_then(|args| op_fn(&mut state.borrow_mut(), args, &mut bufs[1..]));
    let buf =
      json_serialize_op_result(None, result, state.borrow().get_error_class_fn);
    Op::Sync(buf)
  })
}

/// Creates an op that passes data asynchronously using JSON.
///
/// The provided function `op_fn` has the following parameters:
/// * `Rc<RefCell<OpState>`: the op state, can be used to read/write resources in the runtime from an op.
/// * `V`: the deserializable value that is passed to the Rust function.
/// * `BufVec`: raw bytes passed along, usually not needed if the JSON value is used.
///
/// `op_fn` returns a future, whose output is a serializable value. This value will be asynchronously
/// returned to JavaScript.
///
/// When registering an op like this...
/// ```ignore
/// let mut runtime = JsRuntime::new(...);
/// runtime.register_op("hello", deno_core::json_op_async(Self::hello_op));
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let future = Deno.core.jsonOpAsync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn json_op_async<F, V, R, RV>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, V, BufVec) -> R + 'static,
  V: DeserializeOwned,
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: Serialize,
{
  let try_dispatch_op =
    move |state: Rc<RefCell<OpState>>, bufs: BufVec| -> Result<Op, AnyError> {
      let request_id = bufs[0]
        .get(0..8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| type_error("missing or invalid `requestId`"))?;
      let args = serde_json::from_slice(&bufs[0][8..])?;
      let bufs = bufs[1..].into();
      use crate::futures::FutureExt;
      let fut = op_fn(state.clone(), args, bufs).map(move |result| {
        json_serialize_op_result(
          Some(request_id),
          result,
          state.borrow().get_error_class_fn,
        )
      });
      Ok(Op::Async(Box::pin(fut)))
    };

  Box::new(move |state: Rc<RefCell<OpState>>, bufs: BufVec| -> Op {
    match try_dispatch_op(state.clone(), bufs) {
      Ok(op) => op,
      Err(err) => Op::Sync(json_serialize_op_result(
        None,
        Err::<(), AnyError>(err),
        state.borrow().get_error_class_fn,
      )),
    }
  })
}
