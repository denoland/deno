use crate::state::ThreadSafeState;
use crate::worker::Worker;
use deno::Buf;
use deno::CoreOp;
use deno::Named;
use deno::Op;
use deno::OpDispatcher;
use deno::PinnedBuf;
use futures::future::Future;
use futures::stream::Stream;
use std::convert::TryInto;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc as async_mpsc;

pub type RequestSender = async_mpsc::Sender<Buf>;
pub type RequestReceiver = async_mpsc::Receiver<Buf>;

#[allow(dead_code)]
/// State container for state worker specific state
pub struct StateWorkerState {
  request_sender: RequestSender,
  request_receiver: Mutex<RequestReceiver>,
}

impl StateWorkerState {
  pub fn new() -> Self {
    let (request_sender, request_receiver) = async_mpsc::channel::<Buf>(10);

    Self {
      request_sender,
      request_receiver: Mutex::new(request_receiver),
    }
  }
}

#[allow(dead_code)]
struct OpGetRequest {
  state: ThreadSafeState,
  state_worker_state: Arc<StateWorkerState>,
}

impl OpGetRequest {
  pub fn new(
    state: ThreadSafeState,
    state_worker_state: Arc<StateWorkerState>,
  ) -> Self {
    Self {
      state,
      state_worker_state,
    }
  }
}

impl OpDispatcher for OpGetRequest {
  fn dispatch(&self, _args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
    let state_worker_state = Arc::clone(&self.state_worker_state);
    Op::Async(Box::new(
      futures::future::poll_fn(move || {
        let mut receiver = state_worker_state.request_receiver.lock().unwrap();
        receiver.poll()
      })
      .map_err(|e| panic!("{}", e))
      .and_then(move |maybe_buf| Ok(maybe_buf.unwrap())),
    ))
  }
}

impl Named for OpGetRequest {
  const NAME: &'static str = "getRequest";
}

#[allow(dead_code)]
struct OpRespond {
  state: ThreadSafeState,
  state_worker_state: Arc<StateWorkerState>,
}

impl OpRespond {
  pub fn new(
    state: ThreadSafeState,
    state_worker_state: Arc<StateWorkerState>,
  ) -> Self {
    Self {
      state,
      state_worker_state,
    }
  }
}

impl OpDispatcher for OpRespond {
  fn dispatch(&self, args: &[u8], _buf: Option<PinnedBuf>) -> CoreOp {
    let (respond_to_rid_bytes, data) =
      args.split_at(std::mem::size_of::<u32>());
    let respond_to_rid =
      u32::from_ne_bytes(respond_to_rid_bytes.try_into().unwrap());
    let lock = self.state.stateless_workers.read().unwrap();
    match &lock[respond_to_rid as usize] {
      Some((state, _worker)) => state.send_message(data.into()),
      _ => panic!("bad rid"),
    };
    Op::Sync(Box::new([]))
  }
}

impl Named for OpRespond {
  const NAME: &'static str = "respond";
}

static STATE_WORKER_NAMESPACE: &'static str = "stateWorker";

pub fn register_op_dispatchers(worker: Arc<Worker>) -> Arc<StateWorkerState> {
  let state_worker_state = Arc::new(StateWorkerState::new());

  let state = worker.state.clone();

  worker.register_op(
    STATE_WORKER_NAMESPACE,
    OpGetRequest::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  worker.register_op(
    STATE_WORKER_NAMESPACE,
    OpRespond::new(state.clone(), Arc::clone(&state_worker_state)),
  );

  state_worker_state
}
