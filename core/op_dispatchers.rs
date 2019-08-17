use crate::libdeno::OpId;
use crate::libdeno::PinnedBuf;
use futures::future::Future;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

pub type Buf = Box<[u8]>;

pub type OpAsyncFuture<E> = Box<dyn Future<Item = Buf, Error = E> + Send>;

pub enum Op<E> {
  Sync(Buf),
  Async(OpAsyncFuture<E>),
}

impl<E> Op<E> {
  /// Unwrap op as sync result. This will panic if op is async.
  pub fn unwrap_sync(self) -> Buf {
    match self {
      Self::Sync(buf) => buf,
      _ => panic!("Async ops can't be unwraped as sync"),
    }
  }
}

pub type CoreError = ();

pub type CoreOp = Op<CoreError>;

pub type OpResult<E> = Result<Op<E>, E>;

pub trait OpDispatcher: Send + Sync {
  fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp;
}

trait OpDispatcherAlt: Send + Sync {
  fn dispatch_alt(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp;
}

impl<D: OpDispatcher + 'static> OpDispatcher for Arc<D> {
  fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {
    D::dispatch(self, args, buf)
  }
}

pub trait Named {
  const NAME: &'static str;
}

impl<D: Named + 'static> Named for Arc<D> {
  const NAME: &'static str = D::NAME;
}

/// Op dispatcher registry. Used to keep track of dynamicly registered dispatchers
/// and make them addressable by id.
pub struct OpDisReg {
  // Quick lookups by unique "op id"/"resource id"
  // The main goal of op_dis_registry is to perform lookups as fast
  // as possible at all times.
  op_dis_registry: Mutex<BTreeMap<OpId, Arc<Box<dyn OpDispatcher>>>>,
  next_op_dis_id: AtomicU32,
  // Serves as "phone book" for op_dis_registry
  // This should only be referenced for initial lookups. It isn't
  // possible to achieve the level of perfromance we want if we
  // have to query this for every dispatch, but it may be needed
  // to build caches for subseqent lookups.
  op_dis_id_registry: Mutex<HashMap<String, HashMap<&'static str, OpId>>>,
}

impl OpDisReg {
  pub fn new() -> Self {
    Self {
      op_dis_registry: Mutex::new(BTreeMap::new()),
      next_op_dis_id: AtomicU32::new(0),
      op_dis_id_registry: Mutex::new(HashMap::new()),
    }
  }

  pub fn register_op<D: Named + OpDispatcher + 'static>(
    &self,
    namespace: &str,
    d: D,
  ) -> (OpId, String, String) {
    let op_id = self.next_op_dis_id.fetch_add(1, Ordering::SeqCst);
    let namespace_string = namespace.to_string();
    // Ensure the op isn't a duplicate, and can be registed.
    self
      .op_dis_id_registry
      .lock()
      .unwrap()
      .entry(namespace_string.clone())
      .or_default()
      .entry(D::NAME)
      .and_modify(|_| panic!("Op already registered {}:{}", namespace, D::NAME))
      .or_insert(op_id);
    // If we can successfully add the rid to the "phone book" then add this
    // op to the primary registry.
    self
      .op_dis_registry
      .lock()
      .unwrap()
      .entry(op_id)
      .and_modify(|_| unreachable!("Op id already registered"))
      .or_insert(Arc::new(Box::new(d)));
    (op_id, namespace_string, D::NAME.to_string())
  }

  pub fn dispatch_op(
    &self,
    op_id: OpId,
    args: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    let lock = self.op_dis_registry.lock().unwrap();
    if let Some(op) = lock.get(&op_id) {
      let op_ = Arc::clone(op);
      drop(lock);
      op_.dispatch(args, buf)
    } else {
      unimplemented!("Bad op id");
    }
  }

  pub fn lookup_op_id(&self, namespace: &str, name: &str) -> Option<OpId> {
    match self.op_dis_id_registry.lock().unwrap().get(namespace) {
      Some(ns) => ns.get(&name).copied(),
      None => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::convert::TryInto;
  use std::ops::Deref;

  struct MockSimpleDispatcher;

  impl OpDispatcher for MockSimpleDispatcher {
    fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {
      let args_str = std::str::from_utf8(&args[..]).unwrap();

      let buf_str =
        buf.map(|buf| std::str::from_utf8(&buf[..]).unwrap().to_string());

      let result_str = format!("ARGS: {} BUF: {:?}", args_str, buf_str);

      Op::Sync(result_str.as_bytes().into())
    }
  }

  impl Named for MockSimpleDispatcher {
    const NAME: &'static str = "MockSimpleDispatcher";
  }

  #[test]
  fn simple_register_and_dispatch() {
    let op_dis_reg = Arc::new(OpDisReg::new());

    let dispatcher = MockSimpleDispatcher;

    let namespace = "MockNamespace";

    let (op_id, register_namespace, register_name) =
      op_dis_reg.register_op(namespace, dispatcher);
    let lookup_op_id = op_dis_reg
      .lookup_op_id(namespace, MockSimpleDispatcher::NAME)
      .unwrap();
    assert_eq!(op_id, lookup_op_id);
    let lookup_op_id = op_dis_reg
      .lookup_op_id(&register_namespace, &register_name)
      .unwrap();
    assert_eq!(op_id, lookup_op_id);

    assert_eq!(
      None,
      op_dis_reg.lookup_op_id(namespace, "UnrecognizedOpName")
    );
    assert_eq!(
      None,
      op_dis_reg.lookup_op_id("UnkownNamespace", "UnrecognizedOpName")
    );

    if let Op::Sync(buf) = op_dis_reg.dispatch_op(op_id, b"test", None) {
      assert_eq!(buf[..], b"ARGS: test BUF: None"[..]);
    } else {
      panic!("Dispatch returned async, expected sync");
    }
    // TODO(afinch7) add zero_copy test condition.
  }

  struct MockState {
    op_dis_reg: Arc<OpDisReg>,
    counter: AtomicU32,
  }

  struct ThreadSafeMockState(Arc<MockState>);

  impl Clone for ThreadSafeMockState {
    fn clone(&self) -> Self {
      ThreadSafeMockState(self.0.clone())
    }
  }

  impl Deref for ThreadSafeMockState {
    type Target = Arc<MockState>;
    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl ThreadSafeMockState {
    pub fn new(op_dis_reg: Arc<OpDisReg>) -> Self {
      Self(Arc::new(MockState {
        op_dis_reg,
        counter: AtomicU32::new(0),
      }))
    }

    pub fn fetch_add(&self, ammount: u32) -> u32 {
      self.counter.fetch_add(ammount, Ordering::SeqCst)
    }

    pub fn get_count(&self) -> u32 {
      self.counter.load(Ordering::SeqCst)
    }

    pub fn register_new_op<D: Named + OpDispatcher + 'static>(
      &self,
      namespace: &str,
      d: D,
    ) -> OpId {
      self.op_dis_reg.register_op(namespace, d).0
    }
  }

  struct MockStatefulDispatcherCounter {
    state: ThreadSafeMockState,
  }

  impl MockStatefulDispatcherCounter {
    pub fn new(state: ThreadSafeMockState) -> Self {
      Self { state }
    }
  }

  impl OpDispatcher for MockStatefulDispatcherCounter {
    fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
      let (int_bytes, _) = args.split_at(std::mem::size_of::<u32>());
      let ammount = u32::from_ne_bytes(int_bytes.try_into().unwrap());

      let result = self.state.fetch_add(ammount);

      let result_buf = result.to_ne_bytes();
      Op::Sync(result_buf[..].into())
    }
  }

  impl Named for MockStatefulDispatcherCounter {
    const NAME: &'static str = "MockStatefulDispatcherCounter";
  }

  struct MockStatefulDispatcherRegisterOp {
    state: ThreadSafeMockState,
  }

  impl MockStatefulDispatcherRegisterOp {
    pub fn new(state: ThreadSafeMockState) -> Self {
      Self { state }
    }
  }

  impl OpDispatcher for MockStatefulDispatcherRegisterOp {
    fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
      let namespace = std::str::from_utf8(&args[..]).unwrap();

      let dispatcher = MockStatefulDispatcherCounter::new(self.state.clone());

      let result = self.state.register_new_op(namespace, dispatcher);

      let result_buf = result.to_ne_bytes();
      Op::Sync(result_buf[..].into())
    }
  }

  impl Named for MockStatefulDispatcherRegisterOp {
    const NAME: &'static str = "MockStatefulDispatcherRegisterOp";
  }

  #[test]
  fn dynamic_register() {
    let op_dis_reg = Arc::new(OpDisReg::new());

    let state = ThreadSafeMockState::new(Arc::clone(&op_dis_reg));

    let register_op_dispatcher =
      Arc::new(MockStatefulDispatcherRegisterOp::new(state.clone()));

    let namespace = "MockNamespace";

    // Register MockStatefulDispatcherRegisterOp manually
    // We want to hold onto the cc namespace and name returned, so
    // we can check it later.
    let (register_op_id, register_namespace, register_name) =
      op_dis_reg.register_op(namespace, register_op_dispatcher);

    // Dispatch MockStatefulDispatcherRegisterOp op
    // this should register MockStatefulDispatcherCounter under the namespace
    // provided to args
    let register_op_result = op_dis_reg
      .dispatch_op(register_op_id, namespace.as_bytes(), None)
      .unwrap_sync();

    // Get op id for MockStatefulDispatcherCounter from the return of the last op
    let (count_op_id_bytes, _) =
      register_op_result.split_at(std::mem::size_of::<u32>());
    let count_op_id = u32::from_ne_bytes(count_op_id_bytes.try_into().unwrap());

    let intial_counter_value = state.get_count();
    let ammount = 25u32;
    let counter_op_result = op_dis_reg
      .dispatch_op(count_op_id, &ammount.to_ne_bytes()[..], None)
      .unwrap_sync();

    let (counter_value_bytes, _) =
      counter_op_result.split_at(std::mem::size_of::<u32>());
    let counter_value =
      u32::from_ne_bytes(counter_value_bytes.try_into().unwrap());
    assert_eq!(intial_counter_value, counter_value);

    let expected_final_counter_value = ammount + intial_counter_value;
    let final_counter_value = state.get_count();
    assert_eq!(final_counter_value, expected_final_counter_value);

    let lookup_op_id = op_dis_reg
      .lookup_op_id(&register_namespace, &register_name)
      .unwrap();
    assert_eq!(register_op_id, lookup_op_id);
  }
}
