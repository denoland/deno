// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Error;
use deno_core::op;
use deno_core::Extension;
use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::RuntimeOptions;
use futures::channel::mpsc;
use futures::stream::StreamExt;
use std::task::Poll;

// This is a hack to make the `#[op]` macro work with
// deno_core examples.
// You can remove this:
use deno_core::*;

type Task = Box<dyn FnOnce()>;

fn main() {
  let my_ext = Extension::builder()
    .ops(vec![op_schedule_task::decl()])
    .event_loop_middleware(|state_rc, cx| {
      let mut state = state_rc.borrow_mut();
      let recv = state.borrow_mut::<mpsc::UnboundedReceiver<Task>>();
      let mut ref_loop = false;
      while let Poll::Ready(Some(call)) = recv.poll_next_unpin(cx) {
        call();
        ref_loop = true; // `call` can callback into runtime and schedule new callbacks :-)
      }
      ref_loop
    })
    .state(move |state| {
      let (tx, rx) = mpsc::unbounded::<Task>();
      state.put(tx);
      state.put(rx);

      Ok(())
    })
    .build();

  // Initialize a runtime instance
  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![my_ext],
    ..Default::default()
  });
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  let future = async move {
    // Schedule 10 tasks.
    js_runtime
      .execute_script(
        "<usage>",
        r#"for (let i = 1; i <= 10; i++) Deno.core.ops.op_schedule_task(i);"#,
      )
      .unwrap();
    js_runtime.run_event_loop(false).await
  };
  runtime.block_on(future).unwrap();
}

#[op]
fn op_schedule_task(state: &mut OpState, i: u8) -> Result<(), Error> {
  let tx = state.borrow_mut::<mpsc::UnboundedSender<Task>>();
  tx.unbounded_send(Box::new(move || println!("Hello, world! x{}", i)))
    .expect("unbounded_send failed");
  Ok(())
}
