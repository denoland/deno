// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::BufVec;
use crate::Op;
use crate::OpId;
use crate::OpRegistry;
use crate::OpRouter;
use crate::OpTable;
use crate::ResourceTable;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A minimal state struct for use by tests, examples etc. It contains
/// an OpTable and ResourceTable, and implements the relevant traits
/// for working with ops in the most straightforward way possible.
#[derive(Default)]
pub struct BasicState {
  pub op_table: RefCell<OpTable<Self>>,
  pub resource_table: RefCell<ResourceTable>,
}

impl BasicState {
  pub fn new() -> Rc<Self> {
    Default::default()
  }
}

impl OpRegistry for BasicState {
  fn get_op_catalog(self: Rc<Self>) -> HashMap<String, OpId> {
    self.op_table.borrow().get_op_catalog()
  }

  fn register_op<F>(&self, name: &str, op_fn: F) -> OpId
  where
    F: Fn(Rc<Self>, BufVec) -> Op + 'static,
  {
    let mut op_table = self.op_table.borrow_mut();
    let (op_id, prev) = op_table.insert_full(name.to_owned(), Rc::new(op_fn));
    assert!(prev.is_none());
    op_id
  }
}

impl OpRouter for BasicState {
  fn route_op(self: Rc<Self>, op_id: OpId, bufs: BufVec) -> Op {
    let op_fn = self
      .op_table
      .borrow()
      .get_index(op_id)
      .map(|(_, op_fn)| op_fn.clone())
      .unwrap();
    (op_fn)(self, bufs)
  }
}

#[test]
fn test_basic_state_ops() {
  let state = BasicState::new();

  let foo_id = state.register_op("foo", |_, _| Op::Sync(b"oof!"[..].into()));
  assert_eq!(foo_id, 1);

  let bar_id = state.register_op("bar", |_, _| Op::Sync(b"rab!"[..].into()));
  assert_eq!(bar_id, 2);

  let state_ = state.clone();
  let foo_res = state_.route_op(foo_id, Default::default());
  assert!(matches!(foo_res, Op::Sync(buf) if &*buf == b"oof!"));

  let state_ = state.clone();
  let bar_res = state_.route_op(bar_id, Default::default());
  assert!(matches!(bar_res, Op::Sync(buf) if &*buf == b"rab!"));

  let catalog_res = state.route_op(0, Default::default());
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
