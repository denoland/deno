use crate::state::ThreadSafeState;
use deno::CoreOp;
use deno::OpId;
use deno::PinnedBuf;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

#[derive(Default)]
pub struct OpDispatcher<H>
where
  H: Copy + 'static,
{
  pub op_registry: RwLock<BTreeMap<OpId, H>>,
  pub name_registry: RwLock<BTreeMap<String, OpId>>,
  pub next_op_id: AtomicU32,
}

impl<H> OpDispatcher<H>
where
  H: Copy + 'static,
{
  pub fn register_op(&self, name: &str, handler: H) -> OpId {
    let op_id = self.next_op_id.fetch_add(1, Ordering::SeqCst);
    // TODO: verify that we didn't overflow 1000 ops

    // Ensure the op isn't a duplicate, and can be registered.
    self
      .op_registry
      .write()
      .unwrap()
      .entry(op_id)
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(handler);

    self
      .name_registry
      .write()
      .unwrap()
      .entry(name.to_string())
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(op_id);

    op_id
  }

  pub fn select_op(&self, op_id: OpId) -> H {
    *self
      .op_registry
      .read()
      .unwrap()
      .get(&op_id)
      .expect("Op not found!")
  }

  pub fn get_map(&self) -> BTreeMap<String, OpId> {
    self.name_registry.read().unwrap().clone()
  }
}

pub trait Dispatch {
  fn dispatch(
    &self,
    op_id: OpId,
    _state: &ThreadSafeState,
    control: &[u8],
    zero_copy: Option<PinnedBuf>,
  ) -> CoreOp;
}
