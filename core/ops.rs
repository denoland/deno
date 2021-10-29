// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::error::type_error;
use crate::error::AnyError;
use crate::gotham_state::GothamState;
use crate::ops_metrics::OpsTracker;
use crate::resources::ResourceTable;
use crate::runtime::GetErrorClassFn;
use futures::future::maybe_done;
use futures::future::FusedFuture;
use futures::future::MaybeDone;
use futures::ready;
use futures::task::noop_waker;
use futures::Future;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::iter::once;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

/// Wrapper around a Future, which causes that Future to be polled immediately.
/// (Background: ops are stored in a `FuturesUnordered` structure which polls
/// them, but without the `OpCall` wrapper this doesn't happen until the next
/// turn of the event loop, which is too late for certain ops.)
pub struct OpCall<T>(MaybeDone<Pin<Box<dyn Future<Output = T>>>>);

impl<T> OpCall<T> {
  /// Wraps a future, and polls the inner future immediately.
  /// This should be the default choice for ops.
  pub fn eager(fut: impl Future<Output = T> + 'static) -> Self {
    let boxed = Box::pin(fut) as Pin<Box<dyn Future<Output = T>>>;
    let mut inner = maybe_done(boxed);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = Pin::new(&mut inner);
    let _ = pinned.as_mut().poll(&mut cx);
    Self(inner)
  }

  /// Wraps a future; the inner future is polled the usual way (lazily).
  pub fn lazy(fut: impl Future<Output = T> + 'static) -> Self {
    let boxed = Box::pin(fut) as Pin<Box<dyn Future<Output = T>>>;
    let inner = maybe_done(boxed);
    Self(inner)
  }

  /// Create a future by specifying its output. This is basically the same as
  /// `async { value }` or `futures::future::ready(value)`.
  pub fn ready(value: T) -> Self {
    Self(MaybeDone::Done(value))
  }
}

impl<T> Future for OpCall<T> {
  type Output = T;

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let inner = unsafe { &mut self.get_unchecked_mut().0 };
    let mut pinned = Pin::new(inner);
    ready!(pinned.as_mut().poll(cx));
    Poll::Ready(pinned.as_mut().take_output().unwrap())
  }
}

impl<F> FusedFuture for OpCall<F>
where
  F: Future,
{
  fn is_terminated(&self) -> bool {
    self.0.is_terminated()
  }
}

pub type PromiseId = u64;
pub type OpAsyncFuture = OpCall<(PromiseId, OpId, OpResult)>;
pub type OpFn = dyn Fn(Rc<RefCell<OpState>>, OpPayload) -> Op + 'static;
pub type OpId = usize;

pub struct OpPayload<'a, 'b, 'c> {
  pub(crate) scope: &'a mut v8::HandleScope<'b>,
  pub(crate) a: v8::Local<'c, v8::Value>,
  pub(crate) b: v8::Local<'c, v8::Value>,
  pub(crate) op_id: OpId,
  pub(crate) promise_id: PromiseId,
}

impl<'a, 'b, 'c> OpPayload<'a, 'b, 'c> {
  pub fn deserialize<T: DeserializeOwned, U: DeserializeOwned>(
    self,
  ) -> Result<(T, U), AnyError> {
    let a: T = serde_v8::from_v8(self.scope, self.a)
      .map_err(AnyError::from)
      .map_err(|e| type_error(format!("Error parsing args: {}", e)))?;

    let b: U = serde_v8::from_v8(self.scope, self.b)
      .map_err(AnyError::from)
      .map_err(|e| type_error(format!("Error parsing args: {}", e)))?;
    Ok((a, b))
  }
}

pub enum Op {
  Sync(OpResult),
  Async(OpAsyncFuture),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(OpAsyncFuture),
  NotFound,
}

pub enum OpResult {
  Ok(serde_v8::SerializablePkg),
  Err(OpError),
}

impl OpResult {
  pub fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    match self {
      Self::Ok(x) => x.to_v8(scope),
      Self::Err(err) => serde_v8::to_v8(scope, err),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpError {
  #[serde(rename = "$err_class_name")]
  class_name: &'static str,
  message: String,
}

pub fn serialize_op_result<R: Serialize + 'static>(
  result: Result<R, AnyError>,
  state: Rc<RefCell<OpState>>,
) -> OpResult {
  match result {
    Ok(v) => OpResult::Ok(v.into()),
    Err(err) => OpResult::Err(OpError {
      class_name: (state.borrow().get_error_class_fn)(&err),
      message: err.to_string(),
    }),
  }
}

/// Maintains the resources and ops inside a JS runtime.
pub struct OpState {
  pub resource_table: ResourceTable,
  pub op_table: OpTable,
  pub get_error_class_fn: GetErrorClassFn,
  pub(crate) tracker: OpsTracker,
  gotham_state: GothamState,
}

impl OpState {
  pub(crate) fn new() -> OpState {
    OpState {
      resource_table: Default::default(),
      op_table: OpTable::default(),
      get_error_class_fn: &|_| "Error",
      tracker: OpsTracker {
        ops: RefCell::new(Vec::with_capacity(256)),
      },
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
    F: Fn(Rc<RefCell<OpState>>, OpPayload) -> Op + 'static,
  {
    let (op_id, prev) = self.0.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }

  pub fn op_entries(state: Rc<RefCell<OpState>>) -> Vec<(String, OpId)> {
    state.borrow().op_table.0.keys().cloned().zip(0..).collect()
  }

  pub fn route_op(
    op_id: OpId,
    state: Rc<RefCell<OpState>>,
    payload: OpPayload,
  ) -> Op {
    let op_fn = state
      .borrow()
      .op_table
      .0
      .get_index(op_id)
      .map(|(_, op_fn)| op_fn.clone());
    match op_fn {
      Some(f) => (f)(state, payload),
      None => Op::NotFound,
    }
  }
}

impl Default for OpTable {
  fn default() -> Self {
    fn dummy(_state: Rc<RefCell<OpState>>, _p: OpPayload) -> Op {
      unreachable!()
    }
    Self(once(("ops".to_owned(), Rc::new(dummy) as _)).collect())
  }
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
      foo_id =
        op_table.register_op("foo", |_, _| Op::Sync(OpResult::Ok(321.into())));
      assert_eq!(foo_id, 1);
      bar_id =
        op_table.register_op("bar", |_, _| Op::Sync(OpResult::Ok(123.into())));
      assert_eq!(bar_id, 2);
    }

    let mut catalog_entries = OpTable::op_entries(state);
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
