mod json_ops;
mod main_worker;
// mod ops;
mod state;
mod state_worker;
mod stateless_worker;
mod worker;

use deno::js_check;
use deno::StartupData;
use std::sync::Arc;

static MAIN_WORKER_SOURCE: &'static str = include_str!("main_worker.js");

fn main() {
  let state = state::ThreadSafeState::new();

  let main_worker =
    Arc::new(worker::Worker::new(StartupData::None, state.clone()));

  main_worker::register_op_dispatchers(Arc::clone(&main_worker));

  js_check(main_worker.execute("main_worker.js", MAIN_WORKER_SOURCE));

  // TODO(afinch7) replace this with a future for the main worker.
  std::thread::sleep(std::time::Duration::from_secs(30));
}
