// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::cli::Buf;
use crate::cli::Cli;
use crate::cli::Isolate;
use crate::isolate_init::IsolateInit;
use crate::isolate_state::IsolateState;
use crate::isolate_state::WorkerChannels;
use crate::js_errors::JSErrorColor;
use crate::permissions::DenoPermissions;
use crate::resources;
use deno_core::JSError;
use futures::lazy;
use futures::sync::mpsc;
use futures::sync::oneshot;
use futures::Future;
use futures::Poll;
use std::sync::Arc;
use std::thread;

/// Rust interface for WebWorkers.
pub struct Worker {
  isolate: Isolate,
}

impl Worker {
  pub fn new(
    init: IsolateInit,
    parent_state: &Arc<IsolateState>,
    permissions: DenoPermissions,
  ) -> (Self, WorkerChannels) {
    let (worker_in_tx, worker_in_rx) = mpsc::channel::<Buf>(1);
    let (worker_out_tx, worker_out_rx) = mpsc::channel::<Buf>(1);

    let internal_channels = (worker_out_tx, worker_in_rx);
    let external_channels = (worker_in_tx, worker_out_rx);

    let state = Arc::new(IsolateState::new(
      parent_state.flags.clone(),
      parent_state.argv.clone(),
      Some(internal_channels),
    ));

    let cli = Cli::new(init, state, permissions);
    let isolate = Isolate::new(cli);

    let worker = Worker { isolate };
    (worker, external_channels)
  }

  pub fn execute(&self, js_source: &str) -> Result<(), JSError> {
    self.isolate.execute("<anonymous>", js_source)
  }
}

impl Future for Worker {
  type Item = ();
  type Error = JSError;

  fn poll(&mut self) -> Poll<(), JSError> {
    self.isolate.poll()
  }
}

pub fn spawn(
  init: IsolateInit,
  state: Arc<IsolateState>,
  js_source: String,
  permissions: DenoPermissions,
) -> resources::Resource {
  // TODO This function should return a Future, so that the caller can retrieve
  // the JSError if one is thrown. Currently it just prints to stderr and calls
  // exit(1).
  // let (js_error_tx, js_error_rx) = oneshot::channel::<JSError>();
  let (p, c) = oneshot::channel::<resources::Resource>();
  let builder = thread::Builder::new().name("worker".to_string());
  let _tid = builder
    .spawn(move || {
      let (worker, external_channels) = Worker::new(init, &state, permissions);
      let resource = resources::add_worker(external_channels);
      let resource_ = resource.clone();

      let worker_future = lazy(move || {
        p.send(resource_).unwrap();

        worker.execute("denoMain()")?;
        worker.execute("workerMain()")?;
        worker.execute(&js_source)?;
        Ok(worker)
      }).and_then(|_| Ok(()))
      .or_else(|err| -> Result<(), ()> {
        eprintln!("{}", JSErrorColor(&err).to_string());
        std::process::exit(1)
      });

      tokio::spawn(worker_future);

      resource.close();
    }).unwrap();

  c.wait().unwrap()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate_init;

  #[test]
  fn test_spawn() {
    let isolate_init = isolate_init::compiler_isolate_init();
    let resource = spawn(
      isolate_init,
      IsolateState::mock(),
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
      DenoPermissions::default(),
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
    let isolate_init = isolate_init::compiler_isolate_init();
    let resource = spawn(
      isolate_init,
      IsolateState::mock(),
      "onmessage = () => close();".into(),
      DenoPermissions::default(),
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
