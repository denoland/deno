use crate::state::ThreadSafeState;
use deno::js_check;
use deno::ErrBox;
use deno::Named;
use deno::OpDisReg;
use deno::OpDispatcher;
use deno::StartupData;
use futures::future::Future;
use futures::sync::oneshot;
use futures::Async;
use std::sync::Arc;
use std::sync::Mutex;

pub struct Worker {
  isolate: Arc<Mutex<deno::Isolate>>,
  pub state: ThreadSafeState,
}

impl Worker {
  pub fn new(startup_data: StartupData, state: ThreadSafeState) -> Worker {
    let isolate = Arc::new(Mutex::new(deno::Isolate::new(startup_data, false)));
    {
      let mut i = isolate.lock().unwrap();

      let registry = Arc::new(OpDisReg::new());
      i.set_dispatcher_registry(registry);
    }
    Self { isolate, state }
  }

  pub fn register_op<D: Named + OpDispatcher + 'static>(
    &self,
    namespace: &str,
    d: D,
  ) {
    let i = self.isolate.lock().unwrap();
    i.register_op(namespace, d);
  }

  pub fn execute(
    &self,
    js_filename: &str,
    js_source: &str,
  ) -> Result<(), ErrBox> {
    let isolate = self.isolate.lock().unwrap();
    isolate.execute(js_filename, js_source)
  }

  pub fn run_in_thread(&self) -> impl Future<Item = (), Error = ()> {
    let isolate = Arc::clone(&self.isolate);
    let (tx, rx) = oneshot::channel();
    let poll_fut = futures::future::poll_fn(move || {
      let mut i = isolate.lock().unwrap();
      i.poll()
    })
    .then(|r| {
      js_check(r);
      tx.send(()).unwrap();
      Ok(())
    });
    tokio::spawn(poll_fut);
    rx.map_err(|e| panic!("channel closed {}", e))
  }
}

impl Future for Worker {
  type Item = ();
  type Error = ErrBox;

  fn poll(&mut self) -> Result<Async<()>, ErrBox> {
    let mut isolate = self.isolate.lock().unwrap();
    isolate.poll()
  }
}
