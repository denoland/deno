// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::BufVec;
use crate::ErrBox;
use crate::ZeroCopyBuf;
use futures::Future;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::iter::once;
use std::ops::Deref;
use std::ops::DerefMut;
use std::pin::Pin;
use std::rc::Rc;

pub type OpAsyncFuture = Pin<Box<dyn Future<Output = Box<[u8]>>>>;
pub type OpFn<S> = dyn Fn(Rc<S>, BufVec) -> Op + 'static;
pub type OpId = usize;

pub enum Op {
  Sync(Box<[u8]>),
  Async(OpAsyncFuture),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(OpAsyncFuture),
  NotFound,
}
pub trait OpRouter {
  fn route_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op;
}

#[derive(Default)]
pub struct MockOpRouter {
  _private: usize,
}

impl MockOpRouter {
  pub fn new() -> Rc<Self> {
    Default::default()
  }
}

impl OpRouter for MockOpRouter {
  fn route_op(self: Rc<Self>, _op_id: OpId, _bufs: BufVec) -> Op {
    unimplemented!()
  }
}

pub trait OpRegistry: OpRouter + 'static {
  fn get_op_catalog(self: Rc<Self>) -> HashMap<String, OpId>;

  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static;

  fn register_op_json_sync<F>(self: &Rc<Self>, name: &str, op_fn: F) -> OpId
  where
    F: Fn(&Self, Value, &mut [ZeroCopyBuf]) -> Result<Value, ErrBox> + 'static,
  {
    let base_op_fn = move |state: Rc<Self>, mut bufs: BufVec| -> Op {
      let result = serde_json::from_slice(&bufs[0])
        .map_err(ErrBox::from)
        .and_then(|args| op_fn(&state, args, &mut bufs[1..]));
      let buf = state.json_serialize_op_result(None, result);
      Op::Sync(buf)
    };

    self.register_op(name, base_op_fn)
  }

  fn register_op_json_async<F, R>(self: &Rc<Self>, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, Value, BufVec) -> R + 'static,
    R: Future<Output = Result<Value, ErrBox>> + 'static,
  {
    let try_dispatch_op = move |state: Rc<Self>,
                                bufs: BufVec|
          -> Result<Op, ErrBox> {
      let args: Value = serde_json::from_slice(&bufs[0])?;
      let promise_id = args
        .get("promiseId")
        .and_then(Value::as_u64)
        .ok_or_else(|| ErrBox::type_error("`promiseId` missing or invalid"))?;
      let bufs = bufs[1..].into();
      let fut = op_fn(state.clone(), args, bufs).map(move |result| {
        state.json_serialize_op_result(Some(promise_id), result)
      });
      Ok(Op::Async(Box::pin(fut)))
    };

    let base_op_fn = move |state: Rc<Self>, bufs: BufVec| -> Op {
      match try_dispatch_op(state.clone(), bufs) {
        Ok(op) => op,
        Err(err) => Op::Sync(state.json_serialize_op_result(None, Err(err))),
      }
    };

    self.register_op(name, base_op_fn)
  }

  fn json_serialize_op_result(
    &self,
    promise_id: Option<u64>,
    result: Result<Value, ErrBox>,
  ) -> Box<[u8]> {
    let value = match result {
      Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
      Err(err) => json!({
        "promiseId": promise_id ,
        "err": {
          "className": self.get_error_class_name(&err),
          "message": err.to_string(),
        }
      }),
    };
    serde_json::to_vec(&value).unwrap().into_boxed_slice()
  }

  fn get_error_class_name(&self, _err: &ErrBox) -> &'static str {
    "Error"
  }
}

/// Collection for storing registered ops. The special 'get_op_catalog'
/// op with OpId `0` is automatically added when the OpTable is created.
pub struct OpTable<S>(IndexMap<String, Rc<OpFn<S>>>);

impl<S: OpRegistry> OpTable<S> {
  pub fn get_op_catalog(&self) -> HashMap<String, OpId> {
    self.keys().cloned().zip(0..).collect()
  }

  fn op_get_op_catalog(state: Rc<S>, _bufs: BufVec) -> Op {
    let ops = state.get_op_catalog();
    let buf = serde_json::to_vec(&ops).map(Into::into).unwrap();
    Op::Sync(buf)
  }
}

impl<S: OpRegistry> Default for OpTable<S> {
  fn default() -> Self {
    Self(
      once(("ops".to_owned(), Rc::new(Self::op_get_op_catalog) as _)).collect(),
    )
  }
}

impl<S> Deref for OpTable<S> {
  type Target = IndexMap<String, Rc<OpFn<S>>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<S> DerefMut for OpTable<S> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[cfg(test_off)]
#[test]
fn test_op_registry() {
  use crate::CoreIsolate;
  use std::sync::atomic;
  use std::sync::Arc;
  let mut op_registry = SimpleOpRegistry::default();

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let test_id = op_registry.register("test", move |_, _| {
    c_.fetch_add(1, atomic::Ordering::SeqCst);
    Op::Sync(Box::new([]))
  });
  assert!(test_id != 0);

  let mut expected = HashMap::new();
  expected.insert("ops".to_string(), 0);
  expected.insert("test".to_string(), 1);
  assert_eq!(op_registry.name_to_id, expected);

  let isolate = CoreIsolate::new(crate::StartupData::None, false);

  let dispatch = op_registry.get(test_id).unwrap();
  let state_rc = CoreIsolate::state(&isolate);
  let mut state = state_rc.borrow_mut();
  let res = dispatch(&mut state, &mut []);
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);
}

#[cfg(test_off)]
#[test]
fn register_op_during_call() {
  use crate::CoreIsolate;
  use std::sync::atomic;
  use std::sync::Arc;
  use std::sync::Mutex;
  let op_registry = Arc::new(Mutex::new(SimpleOpRegistry::default()));

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let op_registry_ = op_registry.clone();

  let test_id = {
    let mut g = op_registry.lock().unwrap();
    g.register("dynamic_register_op", move |_, _| {
      let c__ = c_.clone();
      let mut g = op_registry_.lock().unwrap();
      g.register("test", move |_, _| {
        c__.fetch_add(1, atomic::Ordering::SeqCst);
        Op::Sync(Box::new([]))
      });
      Op::Sync(Box::new([]))
    })
  };
  assert!(test_id != 0);

  let isolate = CoreIsolate::new(crate::StartupData::None, false);

  let dispatcher1 = {
    let g = op_registry.lock().unwrap();
    g.get(test_id).unwrap()
  };
  {
    let state_rc = CoreIsolate::state(&isolate);
    let mut state = state_rc.borrow_mut();
    dispatcher1(&mut state, &mut []);
  }

  let mut expected = HashMap::new();
  expected.insert("ops".to_string(), 0);
  expected.insert("dynamic_register_op".to_string(), 1);
  expected.insert("test".to_string(), 2);
  {
    let g = op_registry.lock().unwrap();
    assert_eq!(g.name_to_id, expected);
  }

  let dispatcher2 = {
    let g = op_registry.lock().unwrap();
    g.get(2).unwrap()
  };
  let state_rc = CoreIsolate::state(&isolate);
  let mut state = state_rc.borrow_mut();
  let res = dispatcher2(&mut state, &mut []);
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);

  let g = op_registry.lock().unwrap();
  assert!(g.get(100).is_none());
}
