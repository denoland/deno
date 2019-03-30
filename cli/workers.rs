// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate::{DenoBehavior, Isolate};
use crate::isolate_state::WorkerChannels;
use crate::js_errors::JSErrorColor;
use crate::resources;
use crate::tokio_util;
use deno::Buf;
use deno::JSError;
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
}

impl<B: WorkerBehavior> Future for Worker<B> {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    self.isolate.poll()
  }
}

pub fn spawn<B: WorkerBehavior + 'static>(
  behavior: B,
  js_source: String,
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
        let (mut worker, external_channels) = Worker::new(behavior);
        let resource = resources::add_worker(external_channels);
        p.send(resource.clone()).unwrap();

        worker
          .execute("denoMain()")
          .expect("worker denoMain failed");
        worker
          .execute("workerMain()")
          .expect("worker workerMain failed");
        worker.execute(&js_source).expect("worker js_source failed");

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
      r#"
      onmessage = function(e) {
        let s = new TextDecoder().decode(e.data);;
        console.log("msg from main script", s);
        if (s == "exit") {
          close();
          return;
        } else {
          console.assert(s === "hi");
        }
        postMessage(new Uint8Array([1, 2, 3]));
        console.log("after postMessage");
      }
      "#.into(),
    );
    let msg = String::from("hi").into_boxed_str().into_boxed_bytes();

    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());

    let maybe_msg =
      resources::worker_recv_message(resource.rid).wait().unwrap();
    assert!(maybe_msg.is_some());
    assert_eq!(*maybe_msg.unwrap(), [1, 2, 3]);

    let msg = String::from("exit").into_boxed_str().into_boxed_bytes();
    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());
  }

  #[test]
  fn removed_from_resource_table_on_close() {
    let resource = spawn(
      CompilerBehavior::new(Arc::new(IsolateState::mock())),
      "onmessage = () => close();".into(),
    );

    assert_eq!(
      resources::get_type(resource.rid),
      Some("worker".to_string())
    );

    let msg = String::from("hi").into_boxed_str().into_boxed_bytes();
    let r = resources::worker_post_message(resource.rid, msg).wait();
    assert!(r.is_ok());
    println!("rid {:?}", resource.rid);

    // TODO Need a way to get a future for when a resource closes.
    // For now, just sleep for a bit.
    // resource.close();
    thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(resources::get_type(resource.rid), None);
  }
}
