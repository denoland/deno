use crate::libdeno::OpId;
use crate::libdeno::PinnedBuf;
use futures::future::Future;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::RwLock;

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

type NotifyOpFn = dyn Fn(OpId, String, String) + Send + Sync + 'static;

struct NotifyerReg {
  notifiers: Vec<Option<Box<NotifyOpFn>>>,
  free_slots: VecDeque<usize>,
}

impl NotifyerReg {
  pub fn new() -> Self {
    Self {
      notifiers: Vec::new(),
      free_slots: VecDeque::new(),
    }
  }

  pub fn add_notifier(&mut self, n: Box<NotifyOpFn>) -> usize {
    match self.free_slots.pop_front() {
      Some(slot) => {
        assert!(self.notifiers[slot].is_none());
        self.notifiers[slot] = Some(n);
        slot
      }
      None => {
        let slot = self.notifiers.len();
        self.notifiers.push(Some(n));
        assert!(self.notifiers.len() == (slot + 1));
        slot
      }
    }
  }

  pub fn remove_notifier(&mut self, slot: usize) {
    // This assert isn't really needed, but it might help us locate bugs
    // before they become a problem.
    assert!(self.notifiers[slot].is_some());
    self.notifiers[slot] = None;
    self.free_slots.push_back(slot);
  }

  pub fn notify(&self, op_id: OpId, namespace: String, name: String) {
    for maybe_notifier in &self.notifiers {
      if let Some(notifier) = maybe_notifier {
        notifier(op_id, namespace.clone(), name.clone())
      }
    }
  }
}

type OpDispatcherRegistry = Vec<Option<Arc<Box<dyn OpDispatcher>>>>;

/// Op dispatcher registry. Used to keep track of dynamicly registered dispatchers
/// and make them addressable by id.
pub struct OpDisReg {
  // Quick lookups by unique "op id"/"resource id"
  // The main goal of op_dis_registry is to perform lookups as fast
  // as possible at all times.
  op_dis_registry: RwLock<OpDispatcherRegistry>,
  next_op_dis_id: AtomicU32,
  // Serves as "phone book" for op_dis_registry
  // This should only be referenced for initial lookups. It isn't
  // possible to achieve the level of perfromance we want if we
  // have to query this for every dispatch, but it may be needed
  // to build caches for subseqent lookups.
  op_dis_id_registry: RwLock<HashMap<String, HashMap<&'static str, OpId>>>,
  notifier_reg: RwLock<NotifyerReg>,
}

impl OpDisReg {
  pub fn new() -> Self {
    Self {
      op_dis_registry: RwLock::new(Vec::new()),
      next_op_dis_id: AtomicU32::new(0),
      op_dis_id_registry: RwLock::new(HashMap::new()),
      notifier_reg: RwLock::new(NotifyerReg::new()),
    }
  }

  fn add_op_dis<D: Named + OpDispatcher + 'static>(&self, op_id: OpId, d: D) {
    let mut holder = self.op_dis_registry.write().unwrap();
    let new_len = holder.len().max(op_id as usize) + 1;
    holder.resize(new_len, None);
    holder.insert(op_id as usize, Some(Arc::new(Box::new(d))));
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
      .write()
      .unwrap()
      .entry(namespace_string.clone())
      .or_default()
      .entry(D::NAME)
      .and_modify(|_| panic!("Op already registered {}:{}", namespace, D::NAME))
      .or_insert(op_id);
    // If we can successfully add the rid to the "phone book" then add this
    // op to the primary registry.
    self.add_op_dis(op_id, d);
    self.notifier_reg.read().unwrap().notify(
      op_id,
      namespace_string.clone(),
      D::NAME.to_string(),
    );
    (op_id, namespace_string, D::NAME.to_string())
  }

  pub fn dispatch_op(
    &self,
    op_id: OpId,
    args: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    let lock = self.op_dis_registry.read().unwrap();
    if let Some(op) = &lock[op_id as usize] {
      let op_ = Arc::clone(&op);
      drop(lock);
      op_.dispatch(args, buf)
    } else {
      unimplemented!("Bad op id");
    }
  }

  pub fn lookup_op_id(&self, namespace: &str, name: &str) -> Option<OpId> {
    match self.op_dis_id_registry.read().unwrap().get(namespace) {
      Some(ns) => ns.get(&name).copied(),
      None => None,
    }
  }

  pub fn sync_ops_and_add_notify<S, N>(&self, sync_fn: S, notifiy_fn: N)
  where
    S: FnOnce(Vec<(OpId, String, String)>),
    N: Fn(OpId, String, String),
    N: Send + Sync + 'static,
  {
    // Add notifier first so no ops get missed.
    let mut notifier_reg = self.notifier_reg.write().unwrap();
    notifier_reg.add_notifier(Box::new(notifiy_fn));
    // Drop the lock so we don't hold onto this longer then needed.
    drop(notifier_reg);
    let op_id_reg = self.op_dis_id_registry.read().unwrap();
    let mut ops: Vec<(OpId, String, String)> = Vec::new();
    for (namespace_str, namespace) in op_id_reg.iter() {
      for (name, op_id) in namespace.iter() {
        ops.push((*op_id, namespace_str.clone(), name.to_string()));
      }
    }
    sync_fn(ops);
  }

  pub fn remove_notify(&self, slot: usize) {
    let mut notifier_reg = self.notifier_reg.write().unwrap();
    notifier_reg.remove_notifier(slot);
  }
}

impl Default for OpDisReg {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate::js_check;
  use crate::isolate::Isolate;
  use crate::isolate::StartupData;
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

  #[test]
  fn isolate_shared_dynamic_register_multithread() {
    // This is intended to represent the most complicated use case,
    // synced state and registries in different threads and isolates.
    let op_dis_reg = Arc::new(OpDisReg::new());

    let state = ThreadSafeMockState::new(Arc::clone(&op_dis_reg));

    let namespace = "MockNamespace";

    // After isolate 1 is setup and dispatcher registry is set.
    let (sync_1_tx, sync_1_rx) = std::sync::mpsc::channel::<()>();
    // After isolate 2 is setup, dispatcher registry is set, register op
    // dispatcher is registed, and the op id notifyer for
    // MockStatefulDispatcherCounter is set.
    let (sync_2_tx, sync_2_rx) = std::sync::mpsc::channel::<()>();
    // After isolate 1 disptaches MockStatefulDispatcherRegisterOp.
    // MockStatefulDispatcherCounter should be registed for both isolates.
    let (sync_3_tx, sync_3_rx) = std::sync::mpsc::channel::<()>();
    // After isolate 2 calls counter op sucessfully.
    let (sync_4_tx, sync_4_rx) = std::sync::mpsc::channel::<()>();
    let op_dis_reg_ = Arc::clone(&op_dis_reg);
    let t1 = std::thread::spawn(move || {
      let mut isolate = Isolate::new(StartupData::None, false);

      isolate.set_dispatcher_registry(op_dis_reg_);
      sync_1_tx.send(()).ok();
      sync_2_rx.recv().unwrap();
      js_check(isolate.execute(
        "register_op.js",
        r#"
          function assert(cond) {
            if (!cond) {
              throw Error("assert");
            }
          }

          let registerOpId;
          Deno.ops.MockNamespace.MockStatefulDispatcherRegisterOp = (id) => {
            registerOpId = id;
          };

          // "MockNamespace" as Uint8Array;
          const namespaceStrBuffer = new Uint8Array([77, 111, 99, 107, 78, 97, 109, 101, 115, 112, 97, 99, 101]);

          function registerOp() {
            assert(registerOpId !== undefined);
            Deno.core.dispatch(registerOpId, namespaceStrBuffer);
          }
        "#,
      ));
      js_check(isolate.execute("<anonymous>", "registerOp();"));
      sync_3_tx.send(()).ok();
    });

    let op_dis_reg_ = Arc::clone(&op_dis_reg);
    let state_ = state.clone();
    let t2 = std::thread::spawn(move || {
      sync_1_rx.recv().unwrap();
      let mut isolate = Isolate::new(StartupData::None, false);

      isolate.set_dispatcher_registry(op_dis_reg_);
      isolate.register_op(
        namespace,
        Arc::new(MockStatefulDispatcherRegisterOp::new(state.clone())),
      );
      js_check(isolate.execute(
        "count_op.js",
        r#"
          function assert(cond) {
            if (!cond) {
              throw Error("assert");
            }
          }

          let counterOpId;
          Deno.ops.MockNamespace.MockStatefulDispatcherCounter = (id) => {
            counterOpId = id;
          };

          function countOp(number) {
            assert(counterOpId !== undefined);
            return Deno.core.dispatch(counterOpId, new Uint32Array([number]));
          }
        "#,
      ));
      sync_2_tx.send(()).ok();
      sync_3_rx.recv().unwrap();
      let state = state_.clone();
      let intial_counter_value = state.get_count();
      let ammount = 25u32;
      js_check(isolate.execute(
        "<anonymous>",
        &format!(
          r#"
            const response = countOp({});
            assert(response instanceof Uint8Array);
            assert(response.length == 4);
            assert(new DataView(response.buffer).getUint32(0, true) == 0);
          "#,
          ammount,
        ),
      ));
      let expected_final_counter_value = ammount + intial_counter_value;
      let final_counter_value = state.get_count();
      assert_eq!(final_counter_value, expected_final_counter_value);
      sync_4_tx.send(()).ok();
    });

    sync_4_rx.recv().unwrap();

    t1.join().unwrap();
    t2.join().unwrap();
  }
}
