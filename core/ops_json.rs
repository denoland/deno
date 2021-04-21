// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::AnyError;
use crate::serialize_op_result;
use crate::Op;
use crate::OpFn;
use crate::OpState;
use crate::ZeroCopyBuf;
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
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let result = Deno.core.opSync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn op_sync<F, V, R>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut OpState, V, Option<ZeroCopyBuf>) -> Result<R, AnyError> + 'static,
  V: DeserializeOwned,
  R: Serialize + 'static,
{
  Box::new(move |state, payload, buf| -> Op {
    let result = payload
      .deserialize()
      .and_then(|args| op_fn(&mut state.borrow_mut(), args, buf));
    Op::Sync(serialize_op_result(result, state))
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
/// runtime.register_op("hello", deno_core::op_async(Self::hello_op));
/// ```
///
/// ...it can be invoked from JS using the provided name, for example:
/// ```js
/// Deno.core.ops();
/// let future = Deno.core.opAsync("function_name", args);
/// ```
///
/// The `Deno.core.ops()` statement is needed once before any op calls, for initialization.
/// A more complete example is available in the examples directory.
pub fn op_async<F, V, R, RV>(op_fn: F) -> Box<OpFn>
where
  F: Fn(Rc<RefCell<OpState>>, V, Option<ZeroCopyBuf>) -> R + 'static,
  V: DeserializeOwned,
  R: Future<Output = Result<RV, AnyError>> + 'static,
  RV: Serialize + 'static,
{
  Box::new(move |state, payload, buf| -> Op {
    let pid = payload.promise_id;
    // Deserialize args, sync error on failure
    let args = match payload.deserialize() {
      Ok(args) => args,
      Err(err) => {
        return Op::Sync(serialize_op_result(Err::<(), AnyError>(err), state))
      }
    };

    use crate::futures::FutureExt;
    let fut = op_fn(state.clone(), args, buf)
      .map(move |result| (pid, serialize_op_result(result, state)));
    Op::Async(Box::pin(fut))
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
      zero_copy: Option<ZeroCopyBuf>,
    ) -> Result<(), AnyError> {
      assert_eq!(msg.unwrap(), "hello");
      assert!(zero_copy.is_none());
      Err(crate::error::generic_error("foo"))
    }

    runtime.register_op("op_throw", op_async(op_throw));
    runtime
      .execute(
        "<init>",
        r#"
    // First we initialize the ops cache. This maps op names to their id's.
    Deno.core.ops();
    // Register the error class.
    Deno.core.registerErrorClass('Error', Error);

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
    let e = runtime.run_event_loop().await.unwrap_err().to_string();
    println!("{}", e);
    assert!(e.contains("Error: foo"));
    assert!(e.contains("at async f1 (<init>:"));
    assert!(e.contains("at async f2 (<init>:"));
  }
}
