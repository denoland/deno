// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::core_isolate::CoreIsolateState;
use crate::ZeroCopyBuf;
use futures::Future;
use std::collections::HashMap;
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
}

/// Main type describing op
pub type OpDispatcher =
  dyn Fn(&mut CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op + 'static;

#[derive(Default)]
pub struct OpRegistry {
  dispatchers: Vec<Rc<OpDispatcher>>,
  name_to_id: HashMap<String, OpId>,
}

impl OpRegistry {
  pub fn new() -> Self {
    let mut registry = Self::default();
    let op_id = registry.register("ops", |state, _, _| {
      let buf = state.op_registry.json_map();
      Op::Sync(buf)
    });
    assert_eq!(op_id, 0);
    registry
  }

  pub fn register<F>(&mut self, name: &str, op: F) -> OpId
  where
    F: Fn(&mut CoreIsolateState, &[u8], &mut [ZeroCopyBuf]) -> Op + 'static,
  {
    let op_id = self.dispatchers.len() as u32;

    let existing = self.name_to_id.insert(name.to_string(), op_id);
    assert!(
      existing.is_none(),
      format!("Op already registered: {}", name)
    );
    self.dispatchers.push(Rc::new(op));
    op_id
  }

  fn json_map(&self) -> Buf {
    let op_map_json = serde_json::to_string(&self.name_to_id).unwrap();
    op_map_json.as_bytes().to_owned().into_boxed_slice()
  }

  pub fn get(&self, op_id: OpId) -> Option<Rc<OpDispatcher>> {
    self.dispatchers.get(op_id as usize).map(Rc::clone)
  }
}

#[test]
fn test_op_registry() {
  use crate::CoreIsolate;
  use std::sync::atomic;
  use std::sync::Arc;
  let mut op_registry = OpRegistry::new();

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let test_id = op_registry.register("test", move |_, _, _| {
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
  let res = dispatch(&mut state, &[], &mut []);
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);

  assert!(op_registry.get(100).is_none());
}

#[test]
fn register_op_during_call() {
  use crate::CoreIsolate;
  use std::sync::atomic;
  use std::sync::Arc;
  use std::sync::Mutex;
  let op_registry = Arc::new(Mutex::new(OpRegistry::new()));

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let op_registry_ = op_registry.clone();

  let test_id = {
    let mut g = op_registry.lock().unwrap();
    g.register("dynamic_register_op", move |_, _, _| {
      let c__ = c_.clone();
      let mut g = op_registry_.lock().unwrap();
      g.register("test", move |_, _, _| {
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
    dispatcher1(&mut state, &[], &mut []);
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
  let res = dispatcher2(&mut state, &[], &mut []);
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);

  let g = op_registry.lock().unwrap();
  assert!(g.get(100).is_none());
}
