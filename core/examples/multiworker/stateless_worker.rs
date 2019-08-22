use crate::state::ResourceId;
use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::Buf;
use deno::CoreOp;
use deno::Named;
use deno::Op;
use deno::OpDispatcher;
use deno::PinnedBuf;
use futures::future::Future;
use futures::sink::Sink;
use std::convert::TryInto;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc as async_mpsc;

pub type StateResponseSender = async_mpsc::Sender<Buf>;
pub type StateResponseReceiver = async_mpsc::Receiver<Buf>;

#[allow(dead_code)]
/// State container for stateless worker specific state
pub struct StatelessWorkerState {
  state_response_sender: StateResponseSender,
  state_response_receiver: Mutex<StateResponseReceiver>,
  connections: Mutex<Vec<Option<tokio::net::TcpStream>>>,
  listener_rid: ResourceId,
}

impl StatelessWorkerState {
  pub fn new(listener_rid: ResourceId) -> Self {
    let (state_response_sender, state_response_receiver) =
      async_mpsc::channel::<Buf>(1);

    Self {
      state_response_sender,
      state_response_receiver: Mutex::new(state_response_receiver),
      connections: Mutex::new(Vec::new()),
      listener_rid,
    }
  }

  pub fn send_message(&self, msg: Buf) {
    self.state_response_sender.clone().send(msg).wait().unwrap();
  }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Response {
  pub promise_id: i32,
  pub result: i32,
}

impl Into<Buf> for Response {
  fn into(self) -> Buf {
    let vec = vec![self.promise_id, self.result];
    let buf32 = vec.into_boxed_slice();
    let ptr = Box::into_raw(buf32) as *mut [u8; 2 * 4];
    unsafe { Box::from_raw(ptr) }
  }
}

struct OpAccept {
  state: ThreadSafeState,
  stateless_worker_state: Arc<StatelessWorkerState>,
}

impl OpAccept {
  pub fn new(
    state: ThreadSafeState,
    stateless_worker_state: Arc<StatelessWorkerState>,
  ) -> Self {
    Self {
      state,
      stateless_worker_state,
    }
  }
}

impl OpDispatcher for OpAccept {
  fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
    let (promise_id_bytes, _rest) = args.split_at(std::mem::size_of::<i32>());
    let promise_id = i32::from_ne_bytes(promise_id_bytes.try_into().unwrap());

    let state = self.state.clone();
    let stateless_worker_state = Arc::clone(&self.stateless_worker_state);
    let stateless_worker_state_ = Arc::clone(&self.stateless_worker_state);
    Op::Async(Box::new(
      futures::future::poll_fn(move || {
        let mut table = state.listeners.lock().unwrap();
        match &mut table[stateless_worker_state.listener_rid as usize] {
          Some(listener) => listener.poll_accept(),
          _ => panic!("bad rid {}", stateless_worker_state.listener_rid),
        }
      })
      .map_err(|e| panic!(e))
      .and_then(move |(stream, _addr)| {
        let mut guard = stateless_worker_state_.connections.lock().unwrap();
        let rid = guard.len();
        guard.push(Some(stream));

        Ok(
          Response {
            promise_id,
            result: rid as i32,
          }
          .into(),
        )
      }),
    ))
  }
}

impl Named for OpAccept {
  const NAME: &'static str = "accept";
}

#[allow(dead_code)]
struct OpClose {
  state: ThreadSafeState,
  stateless_worker_state: Arc<StatelessWorkerState>,
}

impl OpClose {
  pub fn new(
    state: ThreadSafeState,
    stateless_worker_state: Arc<StatelessWorkerState>,
  ) -> Self {
    Self {
      state,
      stateless_worker_state,
    }
  }
}

impl OpDispatcher for OpClose {
  fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
    let (connection_rid_bytes, _) = args.split_at(std::mem::size_of::<i32>());
    let connection_rid =
      i32::from_ne_bytes(connection_rid_bytes.try_into().unwrap());

    let stateless_worker_state = Arc::clone(&self.stateless_worker_state);
    let mut table = stateless_worker_state.connections.lock().unwrap();
    let r = table[connection_rid as usize].take();
    let result: i32 = if r.is_some() { 0i32 } else { -1i32 };
    Op::Sync(result.to_ne_bytes()[..].into())
  }
}

impl Named for OpClose {
  const NAME: &'static str = "close";
}

#[allow(dead_code)]
struct OpRead {
  state: ThreadSafeState,
  stateless_worker_state: Arc<StatelessWorkerState>,
}

impl OpRead {
  pub fn new(
    state: ThreadSafeState,
    stateless_worker_state: Arc<StatelessWorkerState>,
  ) -> Self {
    Self {
      state,
      stateless_worker_state,
    }
  }
}

impl OpDispatcher for OpRead {
  fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {
    let (promise_id_bytes, rest) = args.split_at(std::mem::size_of::<i32>());
    let promise_id = i32::from_ne_bytes(promise_id_bytes.try_into().unwrap());
    let (connection_rid_bytes, _) = rest.split_at(std::mem::size_of::<i32>());
    let connection_rid =
      i32::from_ne_bytes(connection_rid_bytes.try_into().unwrap());

    let stateless_worker_state = Arc::clone(&self.stateless_worker_state);
    let mut buf = buf.unwrap();
    Op::Async(Box::new(
      futures::future::poll_fn(move || {
        let mut table = stateless_worker_state.connections.lock().unwrap();
        match table[connection_rid as usize] {
          Some(ref mut stream) => stream.poll_read(&mut buf),
          _ => panic!("bad rid"),
        }
      })
      .map_err(|e| panic!("{}", e))
      .and_then(move |nread| {
        Ok(
          Response {
            promise_id,
            result: nread as i32,
          }
          .into(),
        )
      }),
    ))
  }
}

impl Named for OpRead {
  const NAME: &'static str = "read";
}

#[allow(dead_code)]
struct OpWrite {
  state: ThreadSafeState,
  stateless_worker_state: Arc<StatelessWorkerState>,
}

impl OpWrite {
  pub fn new(
    state: ThreadSafeState,
    stateless_worker_state: Arc<StatelessWorkerState>,
  ) -> Self {
    Self {
      state,
      stateless_worker_state,
    }
  }
}

impl OpDispatcher for OpWrite {
  fn dispatch(&self, args: &[u8], buf: Option<PinnedBuf>) -> CoreOp {
    let (promise_id_bytes, rest) = args.split_at(std::mem::size_of::<i32>());
    let promise_id = i32::from_ne_bytes(promise_id_bytes.try_into().unwrap());
    let (connection_rid_bytes, _) = rest.split_at(std::mem::size_of::<i32>());
    let connection_rid =
      i32::from_ne_bytes(connection_rid_bytes.try_into().unwrap());

    let stateless_worker_state = Arc::clone(&self.stateless_worker_state);
    let buf = buf.unwrap();
    Op::Async(Box::new(
      futures::future::poll_fn(move || {
        let mut table = stateless_worker_state.connections.lock().unwrap();
        match table[connection_rid as usize] {
          Some(ref mut stream) => stream.poll_write(&buf),
          _ => panic!("bad rid"),
        }
      })
      .map_err(|e| panic!(e))
      .and_then(move |nwritten| {
        Ok(
          Response {
            promise_id,
            result: nwritten as i32,
          }
          .into(),
        )
      }),
    ))
  }
}

impl Named for OpWrite {
  const NAME: &'static str = "write";
}

#[allow(dead_code)]
struct OpGetStateWorkeRid {
  state: ThreadSafeState,
  stateless_worker_state: Arc<StatelessWorkerState>,
}

impl OpGetStateWorkeRid {
  pub fn new(
    state: ThreadSafeState,
    stateless_worker_state: Arc<StatelessWorkerState>,
  ) -> Self {
    Self {
      state,
      stateless_worker_state,
    }
  }
}

impl OpDispatcher for OpGetStateWorkeRid {
  fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
    let name = std::str::from_utf8(&args[..]).unwrap();

    let lock = self.state.state_workers_ids.read().unwrap();
    let rid = lock.get(name).unwrap();

    Op::Sync((*rid as u32).to_ne_bytes()[..].into())
  }
}

impl Named for OpGetStateWorkeRid {
  const NAME: &'static str = "getStateWorkerRid";
}

// TODO(afinch7) add state request ops

static STATELESS_WORKER_NAMESPACE: &'static str = "statelessWorker";

pub fn register_op_dispatchers(
  worker: Arc<Worker>,
  listener_rid: ResourceId,
) -> Arc<StatelessWorkerState> {
  let state_worker_state = Arc::new(StatelessWorkerState::new(listener_rid));

  let state = worker.state.clone();

  worker.register_op(
    STATELESS_WORKER_NAMESPACE,
    OpAccept::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  worker.register_op(
    STATELESS_WORKER_NAMESPACE,
    OpClose::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  worker.register_op(
    STATELESS_WORKER_NAMESPACE,
    OpRead::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  worker.register_op(
    STATELESS_WORKER_NAMESPACE,
    OpWrite::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  worker.register_op(
    STATELESS_WORKER_NAMESPACE,
    OpGetStateWorkeRid::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  state_worker_state
}
