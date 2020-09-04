// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::BufVec;
use crate::ErrBox;
use crate::ZeroCopyBuf;
use futures::Future;
use futures::FutureExt;
use serde_json::json;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::pin::Pin;
use std::rc::Rc;

pub type OpId = u32;

pub type Buf = Box<[u8]>;

pub type OpAsyncFuture = Pin<Box<dyn Future<Output = Buf>>>;

pub enum Op {
  Sync(Buf),
  Async(OpAsyncFuture),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(OpAsyncFuture),
  NoSuchOp,
}
pub trait OpRouter {
  fn dispatch_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op;
}

pub trait OpManager: OpRouter + 'static {
  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static;

  fn register_op_json_sync<F>(self: &Rc<Self>, name: &str, op_fn: F) -> OpId
  where
    F: Fn(
        &Self,
        serde_json::Value,
        &mut [ZeroCopyBuf],
      ) -> Result<serde_json::Value, ErrBox>
      + 'static,
  {
    let base_op_fn = move |mgr: Rc<Self>, mut bufs: BufVec| -> Op {
      let value = serde_json::from_slice(&bufs[0]).unwrap();
      let result = op_fn(&mgr, value, &mut bufs[1..]);
      let buf = serialize_result(None, result, |err| mgr.get_error_class(err));
      Op::Sync(buf)
    };

    self.register_op(name, base_op_fn)
  }

  fn register_op_json_async<F, R>(self: &Rc<Self>, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, serde_json::Value, BufVec) -> R + 'static,
    R: Future<Output = Result<serde_json::Value, ErrBox>> + 'static,
  {
    let base_op_fn = move |mgr: Rc<Self>, bufs: BufVec| -> Op {
      let value: serde_json::Value = serde_json::from_slice(&bufs[0]).unwrap();
      let promise_id = value.get("promiseId").unwrap().as_u64().unwrap();
      let bufs = bufs[1..].into();
      let fut = op_fn(mgr.clone(), value, bufs).map(move |result| {
        serialize_result(Some(promise_id), result, move |err| {
          mgr.get_error_class(err)
        })
      });
      Op::Async(Box::pin(fut))
    };

    self.register_op(name, base_op_fn)
  }

  fn register_op_json_catalog<F>(self: &Rc<Self>, op_fn: F) -> OpId
  where
    for<'a> F: Fn(&'a Self, &'a mut dyn FnMut((String, OpId))) + 'static,
  {
    let base_op_fn = move |mgr: Rc<Self>, _: BufVec| -> Op {
      let mut index = HashMap::<String, OpId>::new();
      let mut iter_fn = |(k, v)| {
        assert!(index.insert(k, v).is_none());
      };
      op_fn(&mgr, &mut iter_fn);
      let buf = serde_json::to_vec(&index).unwrap().into();
      Op::Sync(buf)
    };

    let op_id = self.register_op("ops", base_op_fn);
    assert_eq!(
      op_id, 0,
      "the 'meta_catalog' op should be the first one registered"
    );
    op_id
  }

  fn get_error_class(&self, _err: &ErrBox) -> &'static str {
    "Error"
  }
}

pub struct OpRegistry(RefCell<OpRegistryInner>)
where
  Self: OpManager;

#[derive(Default)]
pub struct OpRegistryInner {
  dispatchers: Vec<Rc<dyn Fn(Rc<OpRegistry>, BufVec) -> Op + 'static>>,
  name_to_id: HashMap<String, OpId>,
}

impl Deref for OpRegistry {
  type Target = RefCell<OpRegistryInner>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl OpRegistry {
  pub fn new() -> Rc<Self> {
    let self_rc = Rc::new(Self(Default::default()));
    self_rc.register_op_json_catalog(|s, it| {
      s.borrow()
        .name_to_id
        .iter()
        .map(|(k, &v)| (k.clone(), v))
        .for_each(it);
    });
    self_rc
  }
}

impl OpRouter for OpRegistry {
  fn dispatch_op<'s>(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op {
    let op_fn = self.borrow_mut().dispatchers.get(op_id as usize).cloned();
    match op_fn {
      Some(op_fn) => (op_fn)(self, bufs),
      None => Op::NoSuchOp,
    }
  }
}

impl OpManager for OpRegistry {
  /// Defines the how Deno.core.dispatch() acts.
  /// Called whenever Deno.core.dispatch() is called in JavaScript. zero_copy_buf
  /// corresponds to the second argument of Deno.core.dispatch().
  ///
  /// Requires runtime to explicitly ask for op ids before using any of the ops.
  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static,
  {
    let mut inner = self.borrow_mut();
    let op_id = inner.dispatchers.len() as u32;
    let removed = inner.name_to_id.insert(name.to_string(), op_id);
    assert!(removed.is_none(), "op already registered: {}", name);
    inner.dispatchers.push(Rc::new(op_fn));
    op_id
  }
}

pub fn serialize_result(
  promise_id: Option<u64>,
  result: Result<Value, ErrBox>,
  get_error_class_fn: impl Fn(&ErrBox) -> &'static str,
) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({
      "promiseId": promise_id ,
      "err": {
        "className": (get_error_class_fn)(&err),
        "message": err.to_string(),
      }
    }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

#[cfg(test_off)]
#[test]
fn test_op_registry() {
  use crate::CoreIsolate;
  use std::sync::atomic;
  use std::sync::Arc;
  let mut op_registry = OpRegistry::default();

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
  let op_registry = Arc::new(Mutex::new(OpRegistry::default()));

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
