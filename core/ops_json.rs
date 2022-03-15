// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::ops::OpCall;
use crate::serialize_op_result;
use crate::Op;
use crate::OpFn;
use crate::OpState;
use anyhow::Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

/// A helper function that returns a sync NOP OpFn
///
/// It's mainly intended for embedders who want to disable ops, see ./examples/disable_ops.rs
pub fn void_op_sync() -> Box<OpFn> {
  op_sync(|_, _: (), _: ()| Ok(()))
}

/// A helper function that returns an async NOP OpFn
///
/// It's mainly intended for embedders who want to disable ops, see ./examples/disable_ops.rs
pub fn void_op_async() -> Box<OpFn> {
  op_async(|_, _: (), _: ()| futures::future::ok(()))
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
/// runtime.register_op("hello", deno_core::op_sync(Self::hello_op));
/// runtime.sync_ops_cache();
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// let result = Deno.core.opSync("hello", args);
/// ```
///
/// `runtime.sync_ops_cache()` must be called after registering new ops
/// A more complete example is available in the examples directory.
pub fn op_sync<F, A, B, R>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut OpState, A, B) -> Result<R, Error> + 'static,
  A: DeserializeOwned,
  B: DeserializeOwned,
  R: Serialize + 'static,
{
  Box::new(move |state, payload| -> Op {
    let result = payload
      .deserialize()
      .and_then(|(a, b)| op_fn(&mut state.borrow_mut(), a, b));
    Op::Sync(serialize_op_result(result, state))
  })
}

/// Creates an op that passes data asynchronously using JSON.
///
/// When this op is dispatched, the runtime doesn't exit while processing it.
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
/// runtime.register_op("hello", deno_core::op_async(Self::hello_op));
/// runtime.sync_ops_cache();
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// let future = Deno.core.opAsync("hello", args);
/// ```
///
/// `runtime.sync_ops_cache()` must be called after registering new ops
/// A more complete example is available in the examples directory.
pub fn op_async<F, A, B, R, RV>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, A, B) -> R + 'static,
  A: DeserializeOwned,
  B: DeserializeOwned,
  R: Future<Output = Result<RV, Error>> + 'static,
  RV: Serialize + 'static,
{
  Box::new(move |state, payload| -> Op {
    let op_id = payload.op_id;
    let pid = payload.promise_id;
    // Deserialize args, sync error on failure
    let args = match payload.deserialize() {
      Ok(args) => args,
      Err(err) => {
        return Op::Sync(serialize_op_result(Err::<(), Error>(err), state))
      }
    };
    let (a, b) = args;

    use crate::futures::FutureExt;
    let fut = op_fn(state.clone(), a, b)
      .map(move |result| (pid, op_id, serialize_op_result(result, state)));
    Op::Async(OpCall::eager(fut))
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn op_async_stack_trace() {
    async fn op_throw(
      _state: Rc<RefCell<OpState>>,
      msg: Option<String>,
      _: (),
    ) -> Result<(), Error> {
      assert_eq!(msg.unwrap(), "hello");
      Err(crate::error::generic_error("foo"))
    }

    let ext = crate::Extension::builder()
      .ops(vec![("op_throw", op_async(op_throw))])
      .build();

    let mut runtime = crate::JsRuntime::new(crate::RuntimeOptions {
      extensions: vec![ext],
      ..Default::default()
    });

    runtime
      .execute_script(
        "<init>",
        r#"
    async function f1() {
      await Deno.core.opAsync('op_throw', 'hello');
    }

    async function f2() {
      await f1();
    }

    f2();
    "#,
      )
      .unwrap();
    let e = runtime.run_event_loop(false).await.unwrap_err().to_string();
    println!("{}", e);
    assert!(e.contains("Error: foo"));
    assert!(e.contains("at async f1 (<init>:"));
    assert!(e.contains("at async f2 (<init>:"));
  }
}
