// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::BufVec;
use crate::OpState;
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

/// Collection for storing registered ops. The special 'get_op_catalog'
/// op with OpId `0` is automatically added when the OpTable is created.
pub struct OpTable(IndexMap<String, Rc<OpFn>>);

impl OpTable {
  pub fn route_op(
    &self,
    op_id: OpId,
    state: Rc<RefCell<OpState>>,
    bufs: BufVec,
  ) -> Op {
    if op_id == 0 {
      let ops = self.get_op_catalog();
      let buf = serde_json::to_vec(&ops).map(Into::into).unwrap();
      return Op::Sync(buf);
    }
    let op_fn = self
      .get_index(op_id)
      .map(|(_, op_fn)| op_fn.clone())
      .unwrap();
    (op_fn)(state, bufs)
  }

  pub fn get_op_catalog(&self) -> HashMap<String, OpId> {
    self.keys().cloned().zip(0..).collect()
  }

  pub fn register_op<F>(&mut self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<RefCell<OpState>>, BufVec) -> Op + 'static,
  {
    let (op_id, prev) = self.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }
}

impl Default for OpTable {
  fn default() -> Self {
    Self(once(("ops".to_owned(), Rc::new(dummy) as _)).collect())
  }
}

fn dummy(_state: Rc<RefCell<OpState>>, _v: BufVec) -> Op {
  todo!()
}

impl Deref for OpTable {
  type Target = IndexMap<String, Rc<OpFn>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for OpTable {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
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
