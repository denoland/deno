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
type OpDispatcher = dyn Fn(&[u8], Option<PinnedBuf>) -> CoreOp;

#[derive(Default)]
pub struct OpRegistry {
  dispatchers: Vec<Box<OpDispatcher>>,
  name_to_id: HashMap<String, OpId>,
}

impl OpRegistry {
  pub fn new() -> Self {
    let mut registry = Self::default();
    let op_id = registry.register("ops", |_, _| {
      // ops is a special op which is handled in call.
      unreachable!()
    });
    assert_eq!(op_id, 0);
    registry
  }

  pub fn register<F>(&mut self, name: &str, op: F) -> OpId
  where
    F: Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static,
  {
    let op_id = self.dispatchers.len() as u32;

    let existing = self.name_to_id.insert(name.to_string(), op_id);
    assert!(
      existing.is_none(),
      format!("Op already registered: {}", name)
    );

    self.dispatchers.push(Box::new(op));
    op_id
  }

  fn json_map(&self) -> Buf {
    let op_map_json = serde_json::to_string(&self.name_to_id).unwrap();
    op_map_json.as_bytes().to_owned().into_boxed_slice()
  }

  /// This function returns None only if op with given id doesn't exist in registry.
  pub fn call(
    &self,
    op_id: OpId,
    control: &[u8],
    zero_copy_buf: Option<PinnedBuf>,
  ) -> Option<CoreOp> {
    // Op with id 0 has special meaning - it's a special op that is always
    // provided to retrieve op id map. The map consists of name to `OpId`
    // mappings.
    if op_id == 0 {
      return Some(Op::Sync(self.json_map()));
    }

    let d = match self.dispatchers.get(op_id as usize) {
      Some(handler) => &*handler,
      None => return None,
    };

    Some(d(control, zero_copy_buf))
  }
}

#[test]
fn test_op_registry() {
  use std::sync::atomic;
  use std::sync::Arc;
  let mut op_registry = OpRegistry::new();

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
  assert_eq!(op_registry.name_to_id, expected);

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
