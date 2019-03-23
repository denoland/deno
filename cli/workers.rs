// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::errors::RustOrJsError;
use crate::isolate::{DenoBehavior, Isolate};
use crate::isolate_state::WorkerChannels;
use crate::js_errors::JSErrorColor;
use crate::resources;
use crate::tokio_util;
use deno_core::Buf;
use deno_core::JSError;
use futures::future::lazy;
use futures::sync::mpsc;
use futures::sync::oneshot;
use futures::Future;
use futures::Poll;
use std::thread;

/// Behavior trait specific to workers
pub trait WorkerBehavior: DenoBehavior {
  /// Used to setup internal channels at worker creation.
  /// This is intended to be temporary fix.
  /// TODO(afinch7) come up with a better solution to set worker channels
  fn set_internal_channels(&mut self, worker_channels: WorkerChannels);
}

/// Rust interface for WebWorkers.
pub struct Worker<B: WorkerBehavior> {
  isolate: Isolate<B>,
}

impl<B: WorkerBehavior> Worker<B> {
  pub fn new(mut behavior: B) -> (Self, WorkerChannels) {
    let (worker_in_tx, worker_in_rx) = mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = mpsc::channel::<Buf>(1);

    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);

    behavior.set_internal_channels(internal_channels);

    let isolate = Isolate::new(behavior);

    let worker = Worker { isolate };
    (worker, external_channels)
  }

  pub fn execute(&mut self, js_source: &str) -> Result<(), JSError> {
    self.isolate.execute(js_source)
  }

  pub fn execute_mod(
    &mut self,
    js_filename: &str,
    is_prefetch: bool,
  ) -> Result<(), RustOrJsError> {
    self.isolate.execute_mod(js_filename, is_prefetch)
  }
}

impl<B: WorkerBehavior> Future for Worker<B> {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    self.isolate.poll()
  }
}

pub enum WorkerInit {
  Script(String),
  MainModule(),
}

pub fn spawn<B: WorkerBehavior + 'static>(
  behavior: B,
  init: WorkerInit,
) -> resources::Resource {
  // TODO This function should return a Future, so that the caller can retrieve
  // the JSError if one is thrown. Currently it just prints to stderr and calls
  // exit(1).
  // let (js_error_tx, js_error_rx) = oneshot::channel::<JSError>();
  let (p, c) = oneshot::channel::<resources::Resource>();
  let builder = thread::Builder::new().name("worker".to_string());

  let _tid = builder
    .spawn(move || {
      tokio_util::run(lazy(move || {
        let state = behavior.state().clone();
        let (mut worker, external_channels) = Worker::new(behavior);
        let resource = resources::add_worker(external_channels);
        p.send(resource.clone()).unwrap();

        worker
          .execute("denoMain()")
          .expect("worker denoMain failed");
        worker
          .execute("workerMain()")
          .expect("worker workerMain failed");
        match init {
          WorkerInit::Script(script) => {
            worker.execute(&script).expect("worker init script failed")
          }
          WorkerInit::MainModule() => {
            let should_prefetch = state.flags.prefetch || state.flags.info;
            let main_module_option = state.main_module();
            assert!(main_module_option.is_some());
            let main_module = main_module_option.unwrap();
            worker
              .execute_mod(&main_module, should_prefetch)
              .expect("worker init main module failed");
          }
        };

        worker.then(move |r| -> Result<(), ()> {
          resource.close();
          debug!("workers.rs after resource close");
          if let Err(err) = r {
            eprintln!("{}", JSErrorColor(&err).to_string());
            std::process::exit(1);
          }
          Ok(())
        })
      }));

      debug!("workers.rs after spawn");
    }).unwrap();

  c.wait().unwrap()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::compiler::CompilerBehavior;
  use crate::isolate_state::IsolateState;
  use std::sync::Arc;

  #[test]
  fn test_spawn() {
    let resource = spawn(
      CompilerBehavior::new(Arc::new(IsolateState::mock())),
      WorkerInit::Script(
        r#"
      onmessage = function(e) {
        console.log("msg from main script", e.data);
        if (e.data == "exit") {
          close();
          return;
        } else {
          console.assert(e.data === "hi");
        }
        postMessage([1, 2, 3]);
        console.log("after postMessage");
      }
      "#.into(),
      ),
    );
    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();

    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());

    let maybe_msg = resources::get_message_from_worker(resource.rid)
      .wait()
      .unwrap();
    assert!(maybe_msg.is_some());
    // Check if message received is [1, 2, 3] as json encoded
    assert_eq!(*maybe_msg.unwrap(), [91, 49, 44, 50, 44, 51, 93]);

    let msg = json!("exit")
      .to_string()
      .into_boxed_str()
      .into_boxed_bytes();
    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let resource = spawn(
      CompilerBehavior::new(Arc::new(IsolateState::mock())),
      WorkerInit::Script("onmessage = () => close();".into()),
    );

    assert_eq!(
      resources::get_type(resource.rid),
      Some("worker".to_string())
    );

    let msg = json!("hi").to_string().into_boxed_str().into_boxed_bytes();
    let r = resources::post_message_to_worker(resource.rid, msg).wait();
    assert!(r.is_ok());
    println!("rid {:?}", resource.rid);

    // TODO Need a way to get a future for when a resource closes.
    // For now, just sleep for a bit.
    // resource.close();
    thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(resources::get_type(resource.rid), None);
  }
}
