// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
pub use crate::libdeno::OpId;
use crate::PinnedBuf;
use futures::Future;
use std::collections::HashMap;

pub type Buf = Box<[u8]>;

pub type OpAsyncFuture<E> = Box<dyn Future<Item = Buf, Error = E> + Send>;

pub(crate) type PendingOpFuture =
  Box<dyn Future<Item = (OpId, Buf), Error = CoreError> + Send>;

pub type OpResult<E> = Result<Op<E>, E>;

pub enum Op<E> {
  Sync(Buf),
  Async(OpAsyncFuture<E>),
}

pub type CoreError = ();

pub type CoreOp = Op<CoreError>;

/// Main type describing op
pub type CoreOpHandler = dyn Fn(&[u8], Option<PinnedBuf>) -> CoreOp;

#[derive(Default)]
pub struct OpRegistry {
  pub ops: Vec<Box<CoreOpHandler>>,
  pub op_map: HashMap<String, OpId>,
}

fn op_noop(_control: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  Op::Sync(Box::new([]))
}

impl OpRegistry {
  pub fn new() -> Self {
    let mut registry = Self::default();
    // TODO: We should register actual "get_op_map" op here, but I couldn't
    // get past borrow checker when I wanted to do:
    //    registry.register_op("get_op_map", Box::new(registry.op_noop));

    // Add single noop symbolizing "get_op_map" function. The actual
    // handling is done in `isolate.rs`.
    let op_id = registry.register_op("get_op_map", Box::new(op_noop));
    assert_eq!(op_id, 0);
    registry
  }

  pub fn get_op_map(&self) -> HashMap<String, OpId> {
    self.op_map.clone()
  }

  pub fn register_op(
    &mut self,
    name: &str,
    serialized_op: Box<CoreOpHandler>,
  ) -> OpId {
    let op_id = self.ops.len() as u32;

    self
      .op_map
      .entry(name.to_string())
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(op_id);

    self.ops.push(serialized_op);
    op_id
  }
}

#[test]
fn test_op_registry() {
  let mut op_registry = OpRegistry::new();
  let op_id = op_registry.register_op("test", Box::new(op_noop));
  assert!(op_id != 0);

  let mut expected_map = HashMap::new();
  expected_map.insert("get_op_map", 0);
  expected_map.insert("test", 1);
  let op_map = op_registry.get_op_map();
  assert_eq!(op_map, expected_map);
}
