// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::gotham_state::GothamState;
use crate::BufVec;
use futures::Future;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
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

pub struct OpState {
  pub resource_table: crate::ResourceTable,
  pub get_error_class_fn: crate::runtime::GetErrorClassFn,
  pub op_table: OpTable,
  gotham_state: GothamState,
}

impl Default for OpState {
  fn default() -> OpState {
    OpState {
      resource_table: crate::ResourceTable::default(),
      get_error_class_fn: &|_| "Error",
      op_table: OpTable::default(),
      gotham_state: GothamState::default(),
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
  pub fn get_op_catalog(&self) -> HashMap<String, OpId> {
    self.0.keys().cloned().zip(0..).collect()
  }

  fn op_get_op_catalog(state: Rc<RefCell<OpState>>, _bufs: BufVec) -> Op {
    let ops = state.borrow().op_table.get_op_catalog();
    let buf = serde_json::to_vec(&ops).map(Into::into).unwrap();
    Op::Sync(buf)
  }

  pub fn register_op<F>(&mut self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<RefCell<OpState>>, BufVec) -> Op + 'static,
  {
    let (op_id, prev) = self.0.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }

  pub fn route_op(
    &self,
    op_id: OpId,
    state: Rc<RefCell<OpState>>,
    bufs: BufVec,
  ) -> Op {
    if let Some(op_fn) = self.0.get_index(op_id).map(|(_, op_fn)| op_fn.clone())
    {
      (op_fn)(state, bufs)
    } else {
      Op::NotFound
    }
  }
}

impl Default for OpTable {
  fn default() -> Self {
    Self(
      once(("ops".to_owned(), Rc::new(Self::op_get_op_catalog) as _)).collect(),
    )
  }
}

/*
#[test]
fn test_optable() {
  let op_table = OpTable::new();

  let foo_id = op_table.register_op("foo", |_, _| Op::Sync(b"oof!"[..].into()));
  assert_eq!(foo_id, 1);

  let bar_id = op_table.register_op("bar", |_, _| Op::Sync(b"rab!"[..].into()));
  assert_eq!(bar_id, 2);

  let state_ = op_table.clone();
  let foo_res = state_.route_op(foo_id, Default::default());
  assert!(matches!(foo_res, Op::Sync(buf) if &*buf == b"oof!"));

  let state_ = op_table.clone();
  let bar_res = state_.route_op(bar_id, Default::default());
  assert!(matches!(bar_res, Op::Sync(buf) if &*buf == b"rab!"));

  let catalog_res = op_table.route_op(0, Default::default());
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
  )
}
*/
