// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::bad_resource_id;
use crate::error::type_error;
use crate::error::AnyError;
use crate::gotham_state::GothamState;
use crate::resources::ResourceTable;
use crate::runtime::GetErrorBuilderFn;
use crate::BufVec;
use crate::ZeroCopyBuf;
use futures::Future;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::iter::once;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;

pub type OpAsyncFuture = Pin<Box<dyn Future<Output = Box<[u8]>>>>;
pub type OpFn = dyn Fn(Rc<RefCell<OpState>>, BufVec) -> Op + 'static;
pub type OpId = usize;

pub enum Op {
  Sync(Box<[u8]>),
  Async(OpAsyncFuture),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(OpAsyncFuture),
  NotFound,
}

/// Maintains the resources and ops inside a JS runtime.
pub struct OpState {
  pub resource_table: ResourceTable,
  pub op_table: OpTable,
  pub get_error_class_fn: GetErrorBuilderFn,
  gotham_state: GothamState,
}

impl OpState {
  pub(crate) fn new() -> OpState {
    OpState {
      resource_table: Default::default(),
      op_table: OpTable::default(),
      get_error_class_fn: &|_| "Error",
      gotham_state: Default::default(),
    }
  }
}

impl Deref for OpState {
  type Target = GothamState;

  fn deref(&self) -> &Self::Target {
    &self.gotham_state
  }
}

impl DerefMut for OpState {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.gotham_state
  }
}

/// Collection for storing registered ops. The special 'get_op_catalog'
/// op with OpId `0` is automatically added when the OpTable is created.
pub struct OpTable(IndexMap<String, Rc<OpFn>>);

impl OpTable {
  pub fn register_op<F>(&mut self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<RefCell<OpState>>, BufVec) -> Op + 'static,
  {
    let (op_id, prev) = self.0.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }

  pub fn route_op(
    op_id: OpId,
    state: Rc<RefCell<OpState>>,
    bufs: BufVec,
  ) -> Op {
    if op_id == 0 {
      let ops: HashMap<String, OpId> =
        state.borrow().op_table.0.keys().cloned().zip(0..).collect();
      let buf = serde_json::to_vec(&ops).map(Into::into).unwrap();
      Op::Sync(buf)
    } else {
      let op_fn = state
        .borrow()
        .op_table
        .0
        .get_index(op_id)
        .map(|(_, op_fn)| op_fn.clone());
      match op_fn {
        Some(f) => (f)(state, bufs),
        None => Op::NotFound,
      }
    }
  }
}

impl Default for OpTable {
  fn default() -> Self {
    fn dummy(_state: Rc<RefCell<OpState>>, _bufs: BufVec) -> Op {
      unreachable!()
    }
    Self(once(("ops".to_owned(), Rc::new(dummy) as _)).collect())
  }
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
      let promise_id = bufs[0]
        .get(0..8)
        .map(|b| u64::from_be_bytes(b.try_into().unwrap()))
        .ok_or_else(|| type_error("missing or invalid `promiseId`"))?;
      let args = serde_json::from_slice(&bufs[0][8..])?;
      let bufs = bufs[1..].into();
      use crate::futures::FutureExt;
      let fut = op_fn(state.clone(), args, bufs).map(move |result| {
        json_serialize_op_result(
          Some(promise_id),
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

fn json_serialize_op_result<R: Serialize>(
  promise_id: Option<u64>,
  result: Result<R, AnyError>,
  get_error_class_fn: crate::runtime::GetErrorBuilderFn,
) -> Box<[u8]> {
  let value = match result {
    Ok(v) => serde_json::json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => serde_json::json!({
      "promiseId": promise_id ,
      "err": {
        "className": (get_error_class_fn)(&err),
        "message": err.to_string(),
      }
    }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

/// Return map of resources with id as key
/// and string representation as value.
///
/// This op must be wrapped in `json_op_sync`.
pub fn op_resources(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let serialized_resources: HashMap<u32, String> = state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect();
  Ok(json!(serialized_resources))
}

/// Remove a resource from the resource table.
///
/// This op must be wrapped in `json_op_sync`.
pub fn op_close(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let rid = args
    .get("rid")
    .and_then(Value::as_u64)
    .ok_or_else(|| type_error("missing or invalid `rid`"))?;

  state
    .resource_table
    .close(rid as u32)
    .ok_or_else(bad_resource_id)?;

  Ok(json!({}))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn op_table() {
    let state = Rc::new(RefCell::new(OpState::new()));

    let foo_id;
    let bar_id;
    {
      let op_table = &mut state.borrow_mut().op_table;
      foo_id = op_table.register_op("foo", |_, _| Op::Sync(b"oof!"[..].into()));
      assert_eq!(foo_id, 1);
      bar_id = op_table.register_op("bar", |_, _| Op::Sync(b"rab!"[..].into()));
      assert_eq!(bar_id, 2);
    }

    let foo_res = OpTable::route_op(foo_id, state.clone(), Default::default());
    assert!(matches!(foo_res, Op::Sync(buf) if &*buf == b"oof!"));
    let bar_res = OpTable::route_op(bar_id, state.clone(), Default::default());
    assert!(matches!(bar_res, Op::Sync(buf) if &*buf == b"rab!"));

    let catalog_res = OpTable::route_op(0, state, Default::default());
    let mut catalog_entries = match catalog_res {
      Op::Sync(buf) => serde_json::from_slice::<HashMap<String, OpId>>(&buf)
        .map(|map| map.into_iter().collect::<Vec<_>>())
        .unwrap(),
      _ => panic!("unexpected `Op` variant"),
    };
    catalog_entries.sort_by(|(_, id1), (_, id2)| id1.partial_cmp(id2).unwrap());
    assert_eq!(
      catalog_entries,
      vec![
        ("ops".to_owned(), 0),
        ("foo".to_owned(), 1),
        ("bar".to_owned(), 2)
      ]
    );
  }
}
