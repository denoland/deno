// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::serialize_op_result;
use crate::Op;
use crate::OpFn;
use crate::OpState;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

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
  F: Fn(&mut OpState, A, B) -> Result<R, AnyError> + 'static,
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
/// Use op_async_unref instead if you want to make the runtime exit while processing it.
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
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: Serialize + 'static,
{
  Box::new(move |state, payload| -> Op {
    let pid = payload.promise_id;
    // Deserialize args, sync error on failure
    let args = match payload.deserialize() {
      Ok(args) => args,
      Err(err) => {
        return Op::Sync(serialize_op_result(Err::<(), AnyError>(err), state))
      }
    };
    let (a, b) = args;

    use crate::futures::FutureExt;
    let fut = op_fn(state.clone(), a, b)
      .map(move |result| (pid, serialize_op_result(result, state)));
    Op::Async(Box::pin(fut))
  })
}

/// Creates an op that passes data asynchronously using JSON.
///
/// When this op is dispatched, the runtime still can exit while processing it.
///
/// The other usages are the same as `op_async`.
pub fn op_async_unref<F, A, B, R, RV>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, A, B) -> R + 'static,
  A: DeserializeOwned,
  B: DeserializeOwned,
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: Serialize + 'static,
{
  Box::new(move |state, payload| -> Op {
    let pid = payload.promise_id;
    // Deserialize args, sync error on failure
    let args = match payload.deserialize() {
      Ok(args) => args,
      Err(err) => {
        return Op::Sync(serialize_op_result(Err::<(), AnyError>(err), state))
      }
    };
    let (a, b) = args;

    use crate::futures::FutureExt;
    let fut = op_fn(state.clone(), a, b)
      .map(move |result| (pid, serialize_op_result(result, state)));
    Op::AsyncUnref(Box::pin(fut))
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn op_async_stack_trace() {
    let mut runtime = crate::JsRuntime::new(Default::default());

    async fn op_throw(
      _state: Rc<RefCell<OpState>>,
      msg: Option<String>,
      _: (),
    ) -> Result<(), AnyError> {
      assert_eq!(msg.unwrap(), "hello");
      Err(crate::error::generic_error("foo"))
    }

    runtime.register_op("op_throw", op_async(op_throw));
    runtime.sync_ops_cache();
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
