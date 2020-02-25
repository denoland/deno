// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::ZeroCopyBuf;
use futures::Future;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::RwLock;

pub type OpId = u32;

pub type Buf = Box<[u8]>;

pub type OpAsyncFuture<E> = Pin<Box<dyn Future<Output = Result<Buf, E>>>>;

pub(crate) type PendingOpFuture =
  Pin<Box<dyn Future<Output = Result<(OpId, Buf), CoreError>>>>;

pub type OpResult<E> = Result<Op<E>, E>;

pub enum Op<E> {
  Sync(Buf),
  Async(OpAsyncFuture<E>),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(OpAsyncFuture<E>),
}

pub type CoreError = ();

pub type CoreOp = Op<CoreError>;

/// Main type describing op
pub type OpDispatcher = dyn Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static;

#[derive(Default)]
pub struct OpRegistry {
  dispatchers: RwLock<Vec<Rc<OpDispatcher>>>,
  name_to_id: RwLock<HashMap<String, OpId>>,
}

impl OpRegistry {
  pub fn new() -> Self {
    let registry = Self::default();
    let op_id = registry.register("ops", |_, _| {
      // ops is a special op which is handled in call.
      unreachable!()
    });
    assert_eq!(op_id, 0);
    registry
  }

  pub fn register<F>(&self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static,
  {
    let mut lock = self.dispatchers.write().unwrap();
    let op_id = lock.len() as u32;

    let mut name_lock = self.name_to_id.write().unwrap();
    let existing = name_lock.insert(name.to_string(), op_id);
    assert!(
      existing.is_none(),
      format!("Op already registered: {}", name)
    );
    lock.push(Rc::new(op));
    drop(name_lock);
    drop(lock);
    op_id
  }

  fn json_map(&self) -> Buf {
    let lock = self.name_to_id.read().unwrap();
    let op_map_json = serde_json::to_string(&*lock).unwrap();
    op_map_json.as_bytes().to_owned().into_boxed_slice()
  }

  /// This function returns None only if op with given id doesn't exist in registry.
  pub fn call(
    &self,
    op_id: OpId,
    control: &[u8],
    zero_copy_buf: Option<ZeroCopyBuf>,
  ) -> Option<CoreOp> {
    // Op with id 0 has special meaning - it's a special op that is always
    // provided to retrieve op id map. The map consists of name to `OpId`
    // mappings.
    if op_id == 0 {
      return Some(Op::Sync(self.json_map()));
    }
    let lock = self.dispatchers.read().unwrap();
    if let Some(op) = lock.get(op_id as usize) {
      let op_ = Rc::clone(&op);
      // This should allow for changes to the dispatcher list during a call.
      drop(lock);
      Some(op_(control, zero_copy_buf))
    } else {
      None
    }
  }
}

#[test]
fn test_op_registry() {
  use std::sync::atomic;
  use std::sync::Arc;
  let op_registry = OpRegistry::new();

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let test_id = op_registry.register("test", move |_, _| {
    c_.fetch_add(1, atomic::Ordering::SeqCst);
    CoreOp::Sync(Box::new([]))
  });
  assert!(test_id != 0);

  let mut expected = HashMap::new();
  expected.insert("ops".to_string(), 0);
  expected.insert("test".to_string(), 1);
  let name_to_id = op_registry.name_to_id.read().unwrap();
  assert_eq!(*name_to_id, expected);

  let res = op_registry.call(test_id, &[], None).unwrap();
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);

  let res = op_registry.call(100, &[], None);
  assert!(res.is_none());
}

#[test]
fn register_op_during_call() {
  use std::sync::atomic;
  use std::sync::Arc;
  let op_registry = Arc::new(OpRegistry::new());

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let op_registry_ = op_registry.clone();
  let test_id = op_registry.register("dynamic_register_op", move |_, _| {
    let c__ = c_.clone();
    op_registry_.register("test", move |_, _| {
      c__.fetch_add(1, atomic::Ordering::SeqCst);
      CoreOp::Sync(Box::new([]))
    });
    CoreOp::Sync(Box::new([]))
  });
  assert!(test_id != 0);

  op_registry.call(test_id, &[], None);

  let mut expected = HashMap::new();
  expected.insert("ops".to_string(), 0);
  expected.insert("dynamic_register_op".to_string(), 1);
  expected.insert("test".to_string(), 2);
  let name_to_id = op_registry.name_to_id.read().unwrap();
  assert_eq!(*name_to_id, expected);

  let res = op_registry.call(2, &[], None).unwrap();
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);

  let res = op_registry.call(100, &[], None);
  assert!(res.is_none());
}
