use crate::state_worker;
use crate::stateless_worker;
use crate::worker::Worker;
use deno::js_check;
use deno::StartupData;
use futures::future::Future;
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use tokio;

static STATELESS_WORKER_SOURCE: &'static str =
  include_str!("stateless_worker.js");

pub type ResourceId = u32;

pub struct State {
  pub state_workers:
    RwLock<Vec<Option<(Arc<state_worker::StateWorkerState>, Arc<Worker>)>>>,
  pub state_workers_ids: RwLock<HashMap<String, ResourceId>>,
  pub stateless_workers: RwLock<
    Vec<Option<(Arc<stateless_worker::StatelessWorkerState>, Arc<Worker>)>>,
  >,
  pub listeners: Mutex<Vec<Option<tokio::net::TcpListener>>>,
}

pub struct ThreadSafeState(Arc<State>);

impl Clone for ThreadSafeState {
  fn clone(&self) -> Self {
    ThreadSafeState(self.0.clone())
  }
}

impl Deref for ThreadSafeState {
  type Target = Arc<State>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ThreadSafeState {
  pub fn new() -> Self {
    Self(Arc::new(State {
      state_workers: RwLock::new(Vec::new()),
      state_workers_ids: RwLock::new(HashMap::new()),
      stateless_workers: RwLock::new(Vec::new()),
      listeners: Mutex::new(Vec::new()),
    }))
  }

  pub fn add_stateless_worker(
    &self,
    listener_rid: ResourceId,
    script: &str,
  ) -> (
    ResourceId,
    Box<dyn Future<Item = (), Error = ()> + Send + 'static>,
  ) {
    let mut lock = self.stateless_workers.write().unwrap();
    let rid: ResourceId = lock.len() as ResourceId;
    let worker = Arc::new(Worker::new(StartupData::None, self.clone()));
    let stateless_worker_state =
      crate::stateless_worker::register_op_dispatchers(
        Arc::clone(&worker),
        listener_rid,
      );
    lock.push(Some((stateless_worker_state, Arc::clone(&worker))));
    js_check(worker.execute("stateless_worker.js", STATELESS_WORKER_SOURCE));
    js_check(worker.execute("main.js", script));
    (rid, Box::new(worker.run_in_thread()))
  }

  #[allow(dead_code)]
  // State workers are for stateful services in the form of a javascript worker.
  // This allows us to share state information between multiple stateless workers.
  pub fn add_state_worker(&self, _name: &str, _script: &str) -> ResourceId {
    let mut lock = self.state_workers.write().unwrap();
    let rid: ResourceId = lock.len() as ResourceId;
    let worker = Arc::new(Worker::new(StartupData::None, self.clone()));
    let state_worker_state =
      crate::state_worker::register_op_dispatchers(Arc::clone(&worker));
    lock.push(Some((state_worker_state, Arc::clone(&worker))));
    // TODO(afinch7) execute state_worker.js + script here.
    rid
  }

  pub fn listen(
    &self,
    addr: String,
    worker_script: &str,
    worker_count: u32,
  ) -> FuturesUnordered<Box<dyn Future<Item = (), Error = ()> + Send + 'static>>
  {
    let addr = addr.parse::<SocketAddr>().unwrap();
    let listener = tokio::net::TcpListener::bind(&addr).unwrap();
    let mut listeners = self.listeners.lock().unwrap();
    let listener_rid: ResourceId = listeners.len() as ResourceId;
    listeners.push(Some(listener));
    drop(listeners);
    let mut worker_futures: FuturesUnordered<
      Box<dyn Future<Item = (), Error = ()> + Send + 'static>,
    > = FuturesUnordered::new();
    for _ in 0..worker_count {
      worker_futures
        .push(self.add_stateless_worker(listener_rid, worker_script).1);
    }
    worker_futures
  }
}
