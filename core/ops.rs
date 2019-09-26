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

impl OpRegistry {
  pub fn new() -> Self {
    let mut registry = Self::default();
    let op_id = registry.register_op("get_op_map", |_, _| {
      // get_op_map is a special op which is handled in call_op.
      unreachable!()
    });
    assert_eq!(op_id, 0);
    registry
  }

  pub fn register_op<F>(&mut self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static,
  {
    let op_id = self.ops.len() as u32;

    let existing = self.op_map.insert(name.to_string(), op_id);
    assert!(
      existing.is_none(),
      format!("Op already registered: {}", name)
    );

    self.ops.push(Box::new(op));
    op_id
  }

  fn json_map(&self) -> Buf {
    let op_map_json = serde_json::to_string(&self.op_map).unwrap();
    op_map_json.as_bytes().to_owned().into_boxed_slice()
  }

  pub fn call_op(
    &self,
    op_id: OpId,
    control: &[u8],
    zero_copy_buf: Option<PinnedBuf>,
  ) -> CoreOp {
    // Op with id 0 has special meaning - it's a special op that is always
    // provided to retrieve op id map. The map consists of name to `OpId`
    // mappings.
    if op_id == 0 {
      return Op::Sync(self.json_map());
    }

    let op_handler = &*self.ops.get(op_id as usize).expect("Op not found!");
    op_handler(control, zero_copy_buf)
  }
}

#[test]
fn test_op_registry() {
  use std::sync::atomic;
  use std::sync::Arc;
  let mut op_registry = OpRegistry::new();

  let c = Arc::new(atomic::AtomicUsize::new(0));
  let c_ = c.clone();

  let test_id = op_registry.register_op("test", move |_, _| {
    c_.fetch_add(1, atomic::Ordering::SeqCst);
    CoreOp::Sync(Box::new([]))
  });
  assert!(test_id != 0);

  let mut expected_map = HashMap::new();
  expected_map.insert("get_op_map".to_string(), 0);
  expected_map.insert("test".to_string(), 1);
  assert_eq!(op_registry.op_map, expected_map);

  let res = op_registry.call_op(test_id, &[], None);
  if let Op::Sync(buf) = res {
    assert_eq!(buf.len(), 0);
  } else {
    unreachable!();
  }
  assert_eq!(c.load(atomic::Ordering::SeqCst), 1);
}
