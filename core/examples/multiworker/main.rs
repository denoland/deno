mod json_ops;
mod main_worker;
mod minimal_ops;
// mod ops;
mod state;
mod state_worker;
mod stateless_worker;
mod worker;

use deno::js_check;
use deno::StartupData;
use futures::future::Future;

static MAIN_WORKER_SOURCE: &'static str = include_str!("main_worker.js");

fn main() {
  let main_future = futures::future::lazy(|| {
    let state = state::ThreadSafeState::new();

    let main_worker = worker::Worker::new(StartupData::None, state.clone());

    main_worker::register_op_dispatchers(&main_worker);

    js_check(main_worker.execute("main_worker.js", MAIN_WORKER_SOURCE));

    main_worker.then(|r| {
      js_check(r);
      Ok(())
    })
  });

  tokio::run(main_future);
}
